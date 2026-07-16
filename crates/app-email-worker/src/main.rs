//! Durable, privacy-enforced email dispatch worker.
//!
//! PostgreSQL leases make claims restart-safe. The worker resolves verified
//! contact data only at the dispatch boundary and never logs or persists raw
//! destinations or message content.

use std::path::{Path, PathBuf};
use std::time::Duration;

use adapter_provider_email::{EmailDispatch, EmailProvider, HttpEmailProvider};
use platform_auth::{authenticate_request, require_service_scope, AuthConfig, AuthUser};
use platform_db::{
    CommunicationActor, CommunicationChannel, CommunicationEvidenceKey, CommunicationJob,
    CommunicationState, Database, COMMUNICATION_SCOPE_DISPATCH, COMMUNICATION_SCOPE_GLOBAL,
};
use tokio::signal;
use zeroize::Zeroizing;

const SERVICE_NAME: &str = "app-email-worker";

#[derive(Debug, Clone)]
struct WorkerConfig {
    universe_id: i64,
    worker_id: String,
    claim_limit: i64,
    lease_seconds: i64,
    poll_interval: Duration,
    retry_base_seconds: i64,
    token_file: PathBuf,
}

impl WorkerConfig {
    fn from_env() -> Result<Self, &'static str> {
        let universe_id = required_i64("EMAIL_WORKER_UNIVERSE_ID")?;
        let worker_id = std::env::var("EMAIL_WORKER_ID")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "email-worker-1".to_string());
        let claim_limit = optional_i64("EMAIL_WORKER_CLAIM_LIMIT", 20)?;
        let lease_seconds = optional_i64("EMAIL_WORKER_LEASE_SECONDS", 90)?;
        let poll_millis = optional_u64("EMAIL_WORKER_POLL_MILLIS", 1_000)?;
        let retry_base_seconds = optional_i64("EMAIL_WORKER_RETRY_BASE_SECONDS", 15)?;
        let token_file = std::env::var("COMMUNICATION_SERVICE_TOKEN_FILE")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .ok_or("COMMUNICATION_SERVICE_TOKEN_FILE is required")?;
        if universe_id <= 0
            || !(1..=100).contains(&claim_limit)
            || !(5..=900).contains(&lease_seconds)
            || !(50..=60_000).contains(&poll_millis)
            || !(1..=86_400).contains(&retry_base_seconds)
        {
            return Err("email worker numeric configuration is invalid");
        }
        Ok(Self {
            universe_id,
            worker_id,
            claim_limit,
            lease_seconds,
            poll_interval: Duration::from_millis(poll_millis),
            retry_base_seconds,
            token_file,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DispatchOutcome {
    Sent,
    Suppressed,
    Retry,
    Dead,
    LeaseDeferred,
}

fn required_i64(name: &str) -> Result<i64, &'static str> {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .ok_or("required integer configuration is missing or invalid")
}

fn optional_i64(name: &str, default: i64) -> Result<i64, &'static str> {
    match std::env::var(name) {
        Ok(value) => value
            .parse::<i64>()
            .map_err(|_| "integer configuration is invalid"),
        Err(_) => Ok(default),
    }
}

fn optional_u64(name: &str, default: u64) -> Result<u64, &'static str> {
    match std::env::var(name) {
        Ok(value) => value
            .parse::<u64>()
            .map_err(|_| "integer configuration is invalid"),
        Err(_) => Ok(default),
    }
}

fn communication_actor(user: AuthUser) -> Result<CommunicationActor, &'static str> {
    if let Some(universe_id) = user.universe_id {
        CommunicationActor::authenticated_service(user.user_id, universe_id, user.scopes)
            .map_err(|_| "service token tenant authority is invalid")
    } else {
        if !user
            .scopes
            .iter()
            .any(|scope| scope == COMMUNICATION_SCOPE_GLOBAL)
        {
            return Err("global communication scope is required");
        }
        CommunicationActor::authenticated_global_service(user.user_id, user.scopes)
            .map_err(|_| "global service token authority is invalid")
    }
}

fn authenticate_dispatch_actor(
    token_file: &Path,
    universe_id: i64,
) -> Result<CommunicationActor, &'static str> {
    // Reload both verifier configuration and the rotatable token for every
    // claim/finalization operation. Stale credentials therefore fail closed.
    let auth = AuthConfig::from_env();
    auth.validate_runtime()
        .map_err(|_| "authentication configuration is invalid")?;
    let token = Zeroizing::new(
        std::fs::read_to_string(token_file)
            .map_err(|_| "communication service token cannot be read")?,
    );
    let authorization = Zeroizing::new(format!("Bearer {}", token.trim()));
    let user = authenticate_request(&auth, authorization.as_str())
        .map_err(|_| "communication service token is invalid")?;
    require_service_scope(&user, COMMUNICATION_SCOPE_DISPATCH)
        .map_err(|_| "communication dispatch scope is required")?;
    let actor = communication_actor(user)?;
    actor
        .require_universe(universe_id)
        .map_err(|_| "communication tenant authority is required")?;
    Ok(actor)
}

fn retry_delay_seconds(base: i64, attempts: i32, retryable: bool) -> i64 {
    let exponent = attempts.saturating_sub(1).clamp(0, 10) as u32;
    let delay = base.saturating_mul(2_i64.saturating_pow(exponent));
    if retryable {
        delay.min(3_600)
    } else {
        delay.clamp(300, 86_400)
    }
}

async fn suppress(
    database: &Database,
    job: &CommunicationJob,
    worker_id: &str,
    reason: &'static str,
    actor: &CommunicationActor,
    evidence_key: &CommunicationEvidenceKey,
) -> DispatchOutcome {
    match database
        .suppress_communication(job, worker_id, reason, actor, evidence_key)
        .await
    {
        Ok(_) => DispatchOutcome::Suppressed,
        Err(_) => DispatchOutcome::LeaseDeferred,
    }
}

async fn dispatch_one(
    database: &Database,
    provider: &HttpEmailProvider,
    job: CommunicationJob,
    config: &WorkerConfig,
    evidence_key: &CommunicationEvidenceKey,
) -> DispatchOutcome {
    let Some(worker_id) = job.lease_owner.as_deref() else {
        return DispatchOutcome::LeaseDeferred;
    };
    let Ok(actor) = authenticate_dispatch_actor(&config.token_file, job.universe_id) else {
        return DispatchOutcome::LeaseDeferred;
    };
    let renewed = match database
        .renew_communication_lease(&job, worker_id, config.lease_seconds, &actor)
        .await
    {
        Ok(job) => job,
        Err(_) => return DispatchOutcome::LeaseDeferred,
    };
    let policy = match database
        .communication_delivery_policy(&renewed, &actor)
        .await
    {
        Ok(Some(policy)) => policy,
        Ok(None) => {
            return suppress(
                database,
                &renewed,
                worker_id,
                "channel_policy_disabled",
                &actor,
                evidence_key,
            )
            .await;
        }
        Err(_) => return DispatchOutcome::LeaseDeferred,
    };
    if policy.provider_key != provider.provider_key() {
        return suppress(
            database,
            &renewed,
            worker_id,
            "provider_policy_mismatch",
            &actor,
            evidence_key,
        )
        .await;
    }
    let contact = match database
        .resolve_verified_communication_contact(&renewed, &actor, evidence_key)
        .await
    {
        Ok(Some(contact)) => contact,
        Ok(None) => {
            return suppress(
                database,
                &renewed,
                worker_id,
                "verified_contact_unavailable",
                &actor,
                evidence_key,
            )
            .await;
        }
        Err(_) => return DispatchOutcome::LeaseDeferred,
    };

    // This is deliberately the final database decision before the provider
    // call. Essential categories bypass opt-out only inside the canonical
    // privacy policy; channel/provider policy and verified evidence still ran.
    match database
        .communication_allowed(
            renewed.universe_id,
            renewed.user_id,
            renewed.channel.as_str(),
            renewed.category.as_str(),
        )
        .await
    {
        Ok(true) => {}
        Ok(false) => {
            return suppress(
                database,
                &renewed,
                worker_id,
                "privacy_policy_denied",
                &actor,
                evidence_key,
            )
            .await;
        }
        Err(_) => {
            return suppress(
                database,
                &renewed,
                worker_id,
                "privacy_policy_unavailable",
                &actor,
                evidence_key,
            )
            .await;
        }
    }

    let provider = provider.clone();
    let provider_key = provider.provider_key().to_string();
    let provider_template_key = policy.provider_template_key;
    let payload_identity = renewed.payload_identity.clone();
    let idempotency_key = renewed.idempotency_key.clone();
    let job_id = renewed.id;
    let destination = contact.destination;
    let destination_hmac = contact.destination_hmac;
    let destination_masked = contact.destination_masked;
    let provider_result = tokio::task::spawn_blocking(move || {
        provider.dispatch(EmailDispatch {
            job_id,
            destination: destination.as_str(),
            provider_template_key: &provider_template_key,
            payload_identity: &payload_identity,
            idempotency_key: &idempotency_key,
        })
    })
    .await;

    let Ok(final_actor) = authenticate_dispatch_actor(&config.token_file, renewed.universe_id)
    else {
        return DispatchOutcome::LeaseDeferred;
    };
    match provider_result {
        Ok(Ok(result)) => match database
            .mark_communication_sent(
                &renewed,
                worker_id,
                &result.provider_key,
                &result.provider_message_id,
                destination_hmac,
                &destination_masked,
                &final_actor,
                evidence_key,
            )
            .await
        {
            Ok(_) => DispatchOutcome::Sent,
            Err(_) => DispatchOutcome::LeaseDeferred,
        },
        Ok(Err(error)) => {
            let retryable = error.retryable();
            let reason = error.reason_code();
            let delay = retry_delay_seconds(config.retry_base_seconds, renewed.attempts, retryable);
            match database
                .fail_communication_attempt(
                    &renewed,
                    worker_id,
                    &provider_key,
                    reason,
                    delay,
                    &final_actor,
                    evidence_key,
                )
                .await
            {
                Ok(job) if job.state == CommunicationState::Dead => DispatchOutcome::Dead,
                Ok(_) => DispatchOutcome::Retry,
                Err(_) => DispatchOutcome::LeaseDeferred,
            }
        }
        Err(_) => match database
            .fail_communication_attempt(
                &renewed,
                worker_id,
                &provider_key,
                "provider_task_failed",
                retry_delay_seconds(config.retry_base_seconds, renewed.attempts, true),
                &final_actor,
                evidence_key,
            )
            .await
        {
            Ok(job) if job.state == CommunicationState::Dead => DispatchOutcome::Dead,
            Ok(_) => DispatchOutcome::Retry,
            Err(_) => DispatchOutcome::LeaseDeferred,
        },
    }
}

#[tokio::main]
async fn main() {
    platform_observability::init(SERVICE_NAME);
    let config = WorkerConfig::from_env().expect("invalid email worker configuration");
    let database = Database::try_from_env()
        .expect("invalid DATABASE_URL")
        .expect("DATABASE_URL is required");
    database.ping().await.expect("PostgreSQL is unavailable");
    database
        .communication_repository_ready()
        .await
        .expect("durable communication schema is unavailable");
    AuthConfig::from_env()
        .validate_runtime()
        .expect("invalid authentication configuration");
    let evidence_key = CommunicationEvidenceKey::from_env()
        .expect("COMMUNICATION_EVIDENCE_HMAC_KEY_BASE64 is invalid");
    let provider = HttpEmailProvider::from_env().expect("invalid email provider configuration");
    assert!(
        config.lease_seconds
            >= i64::try_from(provider.request_timeout().as_secs())
                .unwrap_or(i64::MAX)
                .saturating_add(5),
        "EMAIL_WORKER_LEASE_SECONDS must exceed EMAIL_PROVIDER_TIMEOUT_SECONDS by at least 5 seconds"
    );

    tracing::info!(
        service = SERVICE_NAME,
        universe_id = config.universe_id,
        worker_id = %config.worker_id,
        provider_key = provider.provider_key(),
        "durable email worker started"
    );

    loop {
        let actor = match authenticate_dispatch_actor(&config.token_file, config.universe_id) {
            Ok(actor) => actor,
            Err(reason) => {
                tracing::error!(
                    service = SERVICE_NAME,
                    reason,
                    "claim authorization unavailable"
                );
                tokio::time::sleep(config.poll_interval).await;
                continue;
            }
        };
        let jobs = match database
            .claim_communications(
                config.universe_id,
                CommunicationChannel::Email,
                &config.worker_id,
                config.claim_limit,
                config.lease_seconds,
                &actor,
                &evidence_key,
            )
            .await
        {
            Ok(jobs) => jobs,
            Err(_) => {
                tracing::error!(service = SERVICE_NAME, "durable email claim failed");
                Vec::new()
            }
        };
        for job in jobs {
            let job_id = job.id;
            let outcome = dispatch_one(&database, &provider, job, &config, &evidence_key).await;
            tracing::info!(
                service = SERVICE_NAME,
                job_id,
                ?outcome,
                "email job completed"
            );
        }
        tokio::select! {
            _ = signal::ctrl_c() => {
                tracing::info!(service = SERVICE_NAME, "shutdown requested");
                break;
            }
            _ = tokio::time::sleep(config.poll_interval) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_backoff_is_bounded_and_nonretryable_is_not_hot_looped() {
        assert_eq!(retry_delay_seconds(15, 1, true), 15);
        assert_eq!(retry_delay_seconds(15, 4, true), 120);
        assert_eq!(retry_delay_seconds(15, 20, true), 3_600);
        assert_eq!(retry_delay_seconds(15, 1, false), 300);
    }

    #[test]
    fn global_actor_requires_explicit_global_scope() {
        let user = AuthUser {
            user_id: "service:mailer".to_string(),
            username: "mailer".to_string(),
            email: None,
            role: "service".to_string(),
            universe_id: None,
            token_purpose: "service".to_string(),
            scopes: vec![COMMUNICATION_SCOPE_DISPATCH.to_string()],
        };
        assert!(communication_actor(user).is_err());
    }
}
