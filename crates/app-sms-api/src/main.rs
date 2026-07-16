//! Durable SMS enqueue, dispatch, and aggregate-audit service.
//!
//! Public requests contain no destination or message body. Scoped service JWTs
//! authorize every operation, while PostgreSQL owns leases, retries, dedupe,
//! verified-contact evidence, and append-only delivery history.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use adapter_provider_sms::{HttpSmsProvider, SmsDispatch, SmsProvider};
use axum::extract::{Query, State};
use axum::http::{header::AUTHORIZATION, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use platform_auth::{authenticate_request, require_service_scope, AuthConfig, AuthUser};
use platform_db::{
    CommunicationActor, CommunicationCategory, CommunicationChannel, CommunicationEnqueueInput,
    CommunicationEvidenceKey, CommunicationJob, CommunicationState, Database,
    COMMUNICATION_SCOPE_AUDIT_READ, COMMUNICATION_SCOPE_DISPATCH, COMMUNICATION_SCOPE_ENQUEUE,
    COMMUNICATION_SCOPE_GLOBAL,
};
use serde::{Deserialize, Serialize};
use tokio::sync::watch;
use zeroize::Zeroizing;

const SERVICE_NAME: &str = "app-sms-api";
const DEFAULT_PORT: u16 = 3003;

#[derive(Clone)]
struct AppState {
    database: Database,
    evidence_key: CommunicationEvidenceKey,
    provider: HttpSmsProvider,
    worker_id: String,
    lease_seconds: i64,
    token_file: PathBuf,
    last_db_success_unix: Arc<AtomicI64>,
    background_running: Arc<AtomicBool>,
    readiness_max_staleness_seconds: i64,
}

#[derive(Debug, Clone)]
struct BackgroundConfig {
    universe_ids: Vec<i64>,
    claim_limit: i64,
    poll_interval: Duration,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    code: &'static str,
}

impl ApiError {
    const fn unauthorized() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "service_authorization_required",
        }
    }

    const fn invalid(code: &'static str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code,
        }
    }

    const fn internal(code: &'static str) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code,
        }
    }
}

#[derive(Serialize)]
struct ErrorBody {
    success: bool,
    code: &'static str,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorBody {
                success: false,
                code: self.code,
            }),
        )
            .into_response()
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EnqueueRequest {
    universe_id: i64,
    user_id: i32,
    category: String,
    template_key: String,
    payload_identity: String,
    idempotency_key: String,
    #[serde(default = "default_max_attempts")]
    max_attempts: i32,
}

const fn default_max_attempts() -> i32 {
    5
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EnqueueResponse {
    success: bool,
    job_id: i64,
    state: &'static str,
    idempotent_replay: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DispatchRequest {
    universe_id: i64,
    #[serde(default = "default_dispatch_limit")]
    limit: i64,
}

const fn default_dispatch_limit() -> i64 {
    20
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct DispatchResponse {
    success: bool,
    claimed: u64,
    sent: u64,
    suppressed: u64,
    retry: u64,
    dead: u64,
    lease_deferred: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DispatchOutcome {
    Sent,
    Suppressed,
    Retry,
    Dead,
    LeaseDeferred,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TenantQuery {
    universe_id: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AuditQuery {
    universe_id: i64,
    #[serde(default = "default_audit_limit")]
    limit: i64,
}

const fn default_audit_limit() -> i64 {
    100
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusItem {
    universe_id: i64,
    channel: &'static str,
    category: &'static str,
    state: &'static str,
    job_count: i64,
    oldest_created_at_unix: i64,
    newest_updated_at_unix: i64,
}

#[derive(Serialize)]
struct StatusResponse {
    success: bool,
    items: Vec<StatusItem>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeliveryAuditItem {
    event_id: i64,
    job_id: i64,
    channel: &'static str,
    category: &'static str,
    event_type: String,
    state: &'static str,
    reason_code: Option<String>,
    attempt: i32,
    created_at_unix: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ControlAuditItem {
    event_id: i64,
    control_type: String,
    channel: &'static str,
    category: Option<&'static str>,
    action: String,
    reason_code: String,
    control_version: i64,
    created_at_unix: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuditResponse {
    success: bool,
    delivery_events: Vec<DeliveryAuditItem>,
    control_events: Vec<ControlAuditItem>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
    durable_repository: bool,
    background_dispatch_running: bool,
    last_database_success_unix: i64,
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn communication_actor(user: AuthUser) -> Result<CommunicationActor, ApiError> {
    if let Some(universe_id) = user.universe_id {
        CommunicationActor::authenticated_service(user.user_id, universe_id, user.scopes)
            .map_err(|_| ApiError::unauthorized())
    } else {
        if !user
            .scopes
            .iter()
            .any(|scope| scope == COMMUNICATION_SCOPE_GLOBAL)
        {
            return Err(ApiError::unauthorized());
        }
        CommunicationActor::authenticated_global_service(user.user_id, user.scopes)
            .map_err(|_| ApiError::unauthorized())
    }
}

fn authenticated_actor(
    headers: &HeaderMap,
    required_scope: &str,
    universe_id: i64,
) -> Result<CommunicationActor, ApiError> {
    let auth = AuthConfig::from_env();
    auth.validate_runtime()
        .map_err(|_| ApiError::internal("authentication_configuration_unavailable"))?;
    let authorization = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(ApiError::unauthorized)?;
    let user = authenticate_request(&auth, authorization).map_err(|_| ApiError::unauthorized())?;
    require_service_scope(&user, required_scope).map_err(|_| ApiError::unauthorized())?;
    let actor = communication_actor(user)?;
    actor
        .require_universe(universe_id)
        .map_err(|_| ApiError::unauthorized())?;
    Ok(actor)
}

fn authenticated_file_actor(
    token_file: &Path,
    required_scope: &str,
    universe_id: i64,
) -> Result<CommunicationActor, ApiError> {
    let auth = AuthConfig::from_env();
    auth.validate_runtime()
        .map_err(|_| ApiError::internal("authentication_configuration_unavailable"))?;
    let token = Zeroizing::new(
        std::fs::read_to_string(token_file)
            .map_err(|_| ApiError::internal("service_token_unavailable"))?,
    );
    let authorization = Zeroizing::new(format!("Bearer {}", token.trim()));
    let user = authenticate_request(&auth, authorization.as_str())
        .map_err(|_| ApiError::internal("service_token_invalid"))?;
    require_service_scope(&user, required_scope)
        .map_err(|_| ApiError::internal("service_scope_unavailable"))?;
    let actor = communication_actor(user)?;
    actor
        .require_universe(universe_id)
        .map_err(|_| ApiError::internal("service_tenant_authority_unavailable"))?;
    Ok(actor)
}

enum AuthorizationSource<'a> {
    Request(&'a HeaderMap),
    TokenFile(&'a Path),
}

impl AuthorizationSource<'_> {
    fn actor(
        &self,
        required_scope: &str,
        universe_id: i64,
    ) -> Result<CommunicationActor, ApiError> {
        match self {
            Self::Request(headers) => authenticated_actor(headers, required_scope, universe_id),
            Self::TokenFile(path) => authenticated_file_actor(path, required_scope, universe_id),
        }
    }
}

async fn health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let database_ready = state.database.ping().await.is_ok()
        && state
            .database
            .communication_repository_ready()
            .await
            .is_ok();
    if database_ready {
        state
            .last_db_success_unix
            .store(unix_now(), Ordering::Relaxed);
    }
    let last_database_success_unix = state.last_db_success_unix.load(Ordering::Relaxed);
    let fresh = unix_now().saturating_sub(last_database_success_unix)
        <= state.readiness_max_staleness_seconds;
    let background_dispatch_running = state.background_running.load(Ordering::Relaxed);
    let ready = database_ready && fresh && background_dispatch_running;
    let status = if ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        Json(HealthResponse {
            status: if ready { "ok" } else { "unavailable" },
            service: SERVICE_NAME,
            durable_repository: database_ready,
            background_dispatch_running,
            last_database_success_unix,
        }),
    )
}

async fn enqueue(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<EnqueueRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let actor = authenticated_actor(&headers, COMMUNICATION_SCOPE_ENQUEUE, request.universe_id)?;
    let category = CommunicationCategory::parse(&request.category)
        .map_err(|_| ApiError::invalid("communication_category_invalid"))?;
    let result = state
        .database
        .enqueue_communication(
            CommunicationEnqueueInput {
                universe_id: request.universe_id,
                user_id: request.user_id,
                channel: CommunicationChannel::Sms,
                category,
                template_key: request.template_key,
                payload_identity: request.payload_identity,
                idempotency_key: request.idempotency_key,
                max_attempts: request.max_attempts,
            },
            &actor,
            &state.evidence_key,
        )
        .await
        .map_err(|_| ApiError::invalid("communication_enqueue_rejected"))?;
    Ok((
        StatusCode::ACCEPTED,
        Json(EnqueueResponse {
            success: true,
            job_id: result.job.id,
            state: result.job.state.as_str(),
            idempotent_replay: result.idempotent_replay,
        }),
    ))
}

async fn suppress_job(
    state: &AppState,
    job: &CommunicationJob,
    reason: &'static str,
    actor: &CommunicationActor,
) -> DispatchOutcome {
    match state
        .database
        .suppress_communication(job, &state.worker_id, reason, actor, &state.evidence_key)
        .await
    {
        Ok(_) => DispatchOutcome::Suppressed,
        Err(_) => DispatchOutcome::LeaseDeferred,
    }
}

async fn dispatch_job(
    state: &AppState,
    authorization: &AuthorizationSource<'_>,
    job: CommunicationJob,
) -> DispatchOutcome {
    let actor = match authorization.actor(COMMUNICATION_SCOPE_DISPATCH, job.universe_id) {
        Ok(actor) => actor,
        Err(_) => return DispatchOutcome::LeaseDeferred,
    };
    let renewed = match state
        .database
        .renew_communication_lease(&job, &state.worker_id, state.lease_seconds, &actor)
        .await
    {
        Ok(job) => job,
        Err(_) => return DispatchOutcome::LeaseDeferred,
    };
    let policy = match state
        .database
        .communication_delivery_policy(&renewed, &actor)
        .await
    {
        Ok(Some(policy)) => policy,
        Ok(None) => {
            return suppress_job(state, &renewed, "channel_policy_disabled", &actor).await;
        }
        Err(_) => return DispatchOutcome::LeaseDeferred,
    };
    if policy.provider_key != state.provider.provider_key() {
        return suppress_job(state, &renewed, "provider_policy_mismatch", &actor).await;
    }
    let contact = match state
        .database
        .resolve_verified_communication_contact(&renewed, &actor, &state.evidence_key)
        .await
    {
        Ok(Some(contact)) => contact,
        Ok(None) => {
            return suppress_job(state, &renewed, "verified_contact_unavailable", &actor).await;
        }
        Err(_) => return DispatchOutcome::LeaseDeferred,
    };
    match state
        .database
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
            return suppress_job(state, &renewed, "privacy_policy_denied", &actor).await;
        }
        Err(_) => {
            return suppress_job(state, &renewed, "privacy_policy_unavailable", &actor).await;
        }
    }

    let provider = state.provider.clone();
    let provider_key = provider.provider_key().to_string();
    let template_key = policy.provider_template_key;
    let payload_identity = renewed.payload_identity.clone();
    let idempotency_key = renewed.idempotency_key.clone();
    let job_id = renewed.id;
    let destination = contact.destination;
    let destination_hmac = contact.destination_hmac;
    let destination_masked = contact.destination_masked;
    let provider_result = tokio::task::spawn_blocking(move || {
        provider.dispatch(SmsDispatch {
            job_id,
            destination: destination.as_str(),
            provider_template_key: &template_key,
            payload_identity: &payload_identity,
            idempotency_key: &idempotency_key,
        })
    })
    .await;

    let final_actor = match authorization.actor(COMMUNICATION_SCOPE_DISPATCH, renewed.universe_id) {
        Ok(actor) => actor,
        Err(_) => return DispatchOutcome::LeaseDeferred,
    };
    match provider_result {
        Ok(Ok(result)) => match state
            .database
            .mark_communication_sent(
                &renewed,
                &state.worker_id,
                &result.provider_key,
                &result.provider_message_id,
                destination_hmac,
                &destination_masked,
                &final_actor,
                &state.evidence_key,
            )
            .await
        {
            Ok(_) => DispatchOutcome::Sent,
            Err(_) => DispatchOutcome::LeaseDeferred,
        },
        Ok(Err(error)) => match state
            .database
            .fail_communication_attempt(
                &renewed,
                &state.worker_id,
                &provider_key,
                error.reason_code(),
                retry_delay_seconds(renewed.attempts, error.retryable()),
                &final_actor,
                &state.evidence_key,
            )
            .await
        {
            Ok(job) if job.state == CommunicationState::Dead => DispatchOutcome::Dead,
            Ok(_) => DispatchOutcome::Retry,
            Err(_) => DispatchOutcome::LeaseDeferred,
        },
        Err(_) => match state
            .database
            .fail_communication_attempt(
                &renewed,
                &state.worker_id,
                &provider_key,
                "provider_task_failed",
                retry_delay_seconds(renewed.attempts, true),
                &final_actor,
                &state.evidence_key,
            )
            .await
        {
            Ok(job) if job.state == CommunicationState::Dead => DispatchOutcome::Dead,
            Ok(_) => DispatchOutcome::Retry,
            Err(_) => DispatchOutcome::LeaseDeferred,
        },
    }
}

fn retry_delay_seconds(attempts: i32, retryable: bool) -> i64 {
    let delay =
        15_i64.saturating_mul(2_i64.saturating_pow(attempts.saturating_sub(1).clamp(0, 10) as u32));
    if retryable {
        delay.min(3_600)
    } else {
        delay.clamp(300, 86_400)
    }
}

fn background_config_from_env() -> Result<BackgroundConfig, &'static str> {
    let raw_universes = std::env::var("SMS_WORKER_UNIVERSE_IDS")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| std::env::var("SMS_WORKER_UNIVERSE_ID").ok())
        .ok_or("SMS_WORKER_UNIVERSE_IDS is required")?;
    let mut universe_ids = raw_universes
        .split(',')
        .map(str::trim)
        .map(|value| {
            value
                .parse::<i64>()
                .ok()
                .filter(|value| *value > 0)
                .ok_or("SMS worker universe identifier is invalid")
        })
        .collect::<Result<Vec<_>, _>>()?;
    universe_ids.sort_unstable();
    universe_ids.dedup();
    if universe_ids.is_empty() || universe_ids.len() > 1_000 {
        return Err("SMS worker universe list is invalid");
    }
    let claim_limit = parse_positive_i64("SMS_WORKER_CLAIM_LIMIT", Some(20))?;
    let poll_millis = parse_positive_i64("SMS_WORKER_POLL_MILLIS", Some(1_000))?;
    if claim_limit > 100 || !(50..=60_000).contains(&poll_millis) {
        return Err("SMS background dispatch configuration is invalid");
    }
    Ok(BackgroundConfig {
        universe_ids,
        claim_limit,
        poll_interval: Duration::from_millis(poll_millis as u64),
    })
}

async fn background_dispatch_loop(
    state: Arc<AppState>,
    config: BackgroundConfig,
    mut shutdown: watch::Receiver<bool>,
) {
    state.background_running.store(true, Ordering::Relaxed);
    tracing::info!(
        service = SERVICE_NAME,
        tenant_count = config.universe_ids.len(),
        "background SMS dispatcher started"
    );
    loop {
        if *shutdown.borrow() {
            break;
        }
        for universe_id in &config.universe_ids {
            let actor = match authenticated_file_actor(
                &state.token_file,
                COMMUNICATION_SCOPE_DISPATCH,
                *universe_id,
            ) {
                Ok(actor) => actor,
                Err(_) => {
                    tracing::error!(
                        service = SERVICE_NAME,
                        universe_id,
                        "background SMS authorization unavailable"
                    );
                    continue;
                }
            };
            let jobs = match state
                .database
                .claim_communications(
                    *universe_id,
                    CommunicationChannel::Sms,
                    &state.worker_id,
                    config.claim_limit,
                    state.lease_seconds,
                    &actor,
                    &state.evidence_key,
                )
                .await
            {
                Ok(jobs) => {
                    state
                        .last_db_success_unix
                        .store(unix_now(), Ordering::Relaxed);
                    jobs
                }
                Err(_) => {
                    tracing::error!(
                        service = SERVICE_NAME,
                        universe_id,
                        "background SMS durable claim failed"
                    );
                    continue;
                }
            };
            let authorization = AuthorizationSource::TokenFile(&state.token_file);
            for job in jobs {
                let job_id = job.id;
                let outcome = dispatch_job(state.as_ref(), &authorization, job).await;
                tracing::info!(
                    service = SERVICE_NAME,
                    job_id,
                    ?outcome,
                    "background SMS job completed"
                );
            }
        }
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
            _ = tokio::time::sleep(config.poll_interval) => {}
        }
    }
    state.background_running.store(false, Ordering::Relaxed);
    tracing::info!(service = SERVICE_NAME, "background SMS dispatcher stopped");
}

async fn dispatch(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<DispatchRequest>,
) -> Result<Json<DispatchResponse>, ApiError> {
    let actor = authenticated_actor(&headers, COMMUNICATION_SCOPE_DISPATCH, request.universe_id)?;
    let jobs = state
        .database
        .claim_communications(
            request.universe_id,
            CommunicationChannel::Sms,
            &state.worker_id,
            request.limit,
            state.lease_seconds,
            &actor,
            &state.evidence_key,
        )
        .await
        .map_err(|_| ApiError::invalid("communication_dispatch_rejected"))?;
    state
        .last_db_success_unix
        .store(unix_now(), Ordering::Relaxed);
    let mut response = DispatchResponse {
        success: true,
        claimed: jobs.len() as u64,
        ..DispatchResponse::default()
    };
    let authorization = AuthorizationSource::Request(&headers);
    for job in jobs {
        match dispatch_job(state.as_ref(), &authorization, job).await {
            DispatchOutcome::Sent => response.sent += 1,
            DispatchOutcome::Suppressed => response.suppressed += 1,
            DispatchOutcome::Retry => response.retry += 1,
            DispatchOutcome::Dead => response.dead += 1,
            DispatchOutcome::LeaseDeferred => response.lease_deferred += 1,
        }
    }
    Ok(Json(response))
}

async fn status(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<TenantQuery>,
) -> Result<Json<StatusResponse>, ApiError> {
    let actor = authenticated_actor(&headers, COMMUNICATION_SCOPE_AUDIT_READ, query.universe_id)?;
    let items = state
        .database
        .communication_status_aggregates(query.universe_id, &actor)
        .await
        .map_err(|_| ApiError::internal("communication_status_unavailable"))?
        .into_iter()
        .map(|item| StatusItem {
            universe_id: item.universe_id,
            channel: item.channel.as_str(),
            category: item.category.as_str(),
            state: item.state.as_str(),
            job_count: item.job_count,
            oldest_created_at_unix: item.oldest_created_at_unix,
            newest_updated_at_unix: item.newest_updated_at_unix,
        })
        .collect();
    Ok(Json(StatusResponse {
        success: true,
        items,
    }))
}

async fn audit(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<AuditQuery>,
) -> Result<Json<AuditResponse>, ApiError> {
    let actor = authenticated_actor(&headers, COMMUNICATION_SCOPE_AUDIT_READ, query.universe_id)?;
    let delivery_events = state
        .database
        .communication_audit_events(query.universe_id, query.limit, &actor)
        .await
        .map_err(|_| ApiError::internal("communication_audit_unavailable"))?
        .into_iter()
        .map(|event| DeliveryAuditItem {
            event_id: event.id,
            job_id: event.outbox_id,
            channel: event.channel.as_str(),
            category: event.category.as_str(),
            event_type: event.event_type,
            state: event.state.as_str(),
            reason_code: event.reason_code,
            attempt: event.attempt,
            created_at_unix: event.created_at_unix,
        })
        .collect();
    let control_events = state
        .database
        .communication_control_audit_events(query.universe_id, query.limit, &actor)
        .await
        .map_err(|_| ApiError::internal("communication_audit_unavailable"))?
        .into_iter()
        .map(|event| ControlAuditItem {
            event_id: event.id,
            control_type: event.control_type,
            channel: event.channel.as_str(),
            category: event.category.map(CommunicationCategory::as_str),
            action: event.action,
            reason_code: event.reason_code,
            control_version: event.control_version,
            created_at_unix: event.created_at_unix,
        })
        .collect();
    Ok(Json(AuditResponse {
        success: true,
        delivery_events,
        control_events,
    }))
}

fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/send", post(enqueue))
        .route("/api/dispatch", post(dispatch))
        .route("/api/status", get(status))
        .route("/api/audit", get(audit))
        .with_state(state)
}

fn parse_positive_i64(name: &str, default: Option<i64>) -> Result<i64, &'static str> {
    match std::env::var(name) {
        Ok(value) => value
            .parse::<i64>()
            .ok()
            .filter(|value| *value > 0)
            .ok_or("positive integer configuration is invalid"),
        Err(_) => default.ok_or("required positive integer configuration is missing"),
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    AuthConfig::from_env()
        .validate_runtime()
        .expect("invalid authentication configuration");
    let database = Database::try_from_env()
        .expect("invalid DATABASE_URL")
        .expect("DATABASE_URL is required");
    database.ping().await.expect("PostgreSQL is unavailable");
    database
        .communication_repository_ready()
        .await
        .expect("durable communication schema is unavailable");
    let evidence_key = CommunicationEvidenceKey::from_env()
        .expect("COMMUNICATION_EVIDENCE_HMAC_KEY_BASE64 is invalid");
    let provider = HttpSmsProvider::from_env().expect("invalid SMS provider configuration");
    let background_config =
        background_config_from_env().expect("invalid SMS background worker configuration");
    let token_file = std::env::var("COMMUNICATION_SERVICE_TOKEN_FILE")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .expect("COMMUNICATION_SERVICE_TOKEN_FILE is required");
    let worker_id = std::env::var("SMS_DISPATCH_WORKER_ID")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "sms-api-dispatcher-1".to_string());
    let lease_seconds = parse_positive_i64("SMS_DISPATCH_LEASE_SECONDS", Some(90))
        .expect("invalid SMS dispatch lease");
    assert!(
        lease_seconds <= 900,
        "SMS_DISPATCH_LEASE_SECONDS exceeds the durable lease limit"
    );
    assert!(
        lease_seconds
            >= i64::try_from(provider.request_timeout().as_secs())
                .unwrap_or(i64::MAX)
                .saturating_add(5),
        "SMS_DISPATCH_LEASE_SECONDS must exceed SMS_PROVIDER_TIMEOUT_SECONDS by at least 5 seconds"
    );
    let readiness_max_staleness_seconds =
        parse_positive_i64("SMS_READINESS_MAX_STALENESS_SECONDS", Some(30))
            .expect("invalid SMS readiness threshold");
    let port = parse_positive_i64("PORT", Some(DEFAULT_PORT.into()))
        .ok()
        .and_then(|value| u16::try_from(value).ok())
        .expect("PORT is invalid");
    let provider_key = provider.provider_key().to_string();
    let state = Arc::new(AppState {
        database,
        evidence_key,
        provider,
        worker_id,
        lease_seconds,
        token_file,
        last_db_success_unix: Arc::new(AtomicI64::new(unix_now())),
        background_running: Arc::new(AtomicBool::new(false)),
        readiness_max_staleness_seconds,
    });
    let address = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!(service = SERVICE_NAME, %address, %provider_key, "durable SMS API started");
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let background = tokio::spawn(background_dispatch_loop(
        Arc::clone(&state),
        background_config,
        shutdown_rx,
    ));
    let shutdown_signal = async move {
        if tokio::signal::ctrl_c().await.is_err() {
            tracing::error!(service = SERVICE_NAME, "shutdown signal handler failed");
        }
        let _ = shutdown_tx.send(true);
    };
    axum::Server::bind(&address)
        .serve(router(state).into_make_service())
        .with_graceful_shutdown(shutdown_signal)
        .await
        .expect("SMS API server failed");
    background
        .await
        .expect("background SMS dispatcher task failed");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enqueue_contract_rejects_destination_body_and_arbitrary_variables() {
        let unsafe_request = serde_json::json!({
            "universeId": 1,
            "userId": 7,
            "category": "security",
            "templateKey": "password_reset",
            "payloadIdentity": "security_event:dead-beef",
            "idempotencyKey": "sms:test:0001",
            "contact": "+12065550123",
            "message": "secret body"
        });
        assert!(serde_json::from_value::<EnqueueRequest>(unsafe_request).is_err());
    }

    #[test]
    fn enqueue_response_contains_no_contact_or_message_material() {
        let body = serde_json::to_string(&EnqueueResponse {
            success: true,
            job_id: 4,
            state: "pending",
            idempotent_replay: false,
        })
        .unwrap();
        assert!(!body.contains("destination"));
        assert!(!body.contains("contact"));
        assert!(!body.contains("message"));
    }

    #[test]
    fn provider_backoff_is_bounded() {
        assert_eq!(retry_delay_seconds(1, true), 15);
        assert_eq!(retry_delay_seconds(4, true), 120);
        assert_eq!(retry_delay_seconds(20, true), 3_600);
        assert_eq!(retry_delay_seconds(1, false), 300);
    }
}
