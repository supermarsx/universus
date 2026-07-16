//! Durable outbound communication repository.
//!
//! This module intentionally stores no message body or raw destination. Jobs
//! reference registered templates and authoritative event identities; verified
//! contact data is resolved from `users` only at the dispatch boundary.

use std::fmt::{Debug, Display, Formatter};
use std::sync::Arc;

use base64::{engine::general_purpose::STANDARD, Engine};
use deadpool_postgres::GenericClient;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;
use tokio_postgres::{Row, Transaction};
use zeroize::Zeroizing;

use crate::Database;

pub const COMMUNICATION_SCOPE_ENQUEUE: &str = "communications.enqueue";
pub const COMMUNICATION_SCOPE_DISPATCH: &str = "communications.dispatch";
pub const COMMUNICATION_SCOPE_AUDIT_READ: &str = "communications.audit.read";
pub const COMMUNICATION_SCOPE_POLICY_WRITE: &str = "communications.policy.write";
pub const COMMUNICATION_SCOPE_CONTACT_VERIFY: &str = "communications.contact.verify";
pub const COMMUNICATION_SCOPE_GLOBAL: &str = "communications.global";
pub const COMMUNICATION_SCOPE_RETENTION: &str = "communications.retention";

const MAX_CLAIM_LIMIT: i64 = 100;
const MAX_LEASE_SECONDS: i64 = 900;

#[derive(Clone)]
pub struct CommunicationEvidenceKey {
    bytes: Arc<Zeroizing<Vec<u8>>>,
}

impl Debug for CommunicationEvidenceKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommunicationEvidenceKey")
            .field("bytes", &"[REDACTED]")
            .finish()
    }
}

impl CommunicationEvidenceKey {
    pub fn from_base64(encoded: &str) -> Result<Self, CommunicationError> {
        let bytes = STANDARD
            .decode(encoded.trim())
            .map_err(|_| CommunicationError::Configuration("evidence key is not valid base64"))?;
        Self::new(bytes)
    }

    pub fn from_env() -> Result<Self, CommunicationError> {
        let encoded = std::env::var("COMMUNICATION_EVIDENCE_HMAC_KEY_BASE64")
            .map_err(|_| CommunicationError::Configuration("evidence HMAC key is required"))?;
        Self::from_base64(&encoded)
    }

    pub fn new(bytes: Vec<u8>) -> Result<Self, CommunicationError> {
        if bytes.len() < 32 {
            return Err(CommunicationError::Configuration(
                "evidence HMAC key must contain at least 32 bytes",
            ));
        }
        Ok(Self {
            bytes: Arc::new(Zeroizing::new(bytes)),
        })
    }

    pub fn evidence_hmac(&self, domain: &str, value: &str) -> [u8; 32] {
        let mut mac = Hmac::<Sha256>::new_from_slice(self.bytes.as_slice())
            .expect("HMAC accepts keys of every length");
        mac.update(domain.as_bytes());
        mac.update(&[0]);
        mac.update(value.as_bytes());
        mac.finalize().into_bytes().into()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommunicationError {
    InvalidInput(&'static str),
    Unauthorized,
    NotFound,
    Conflict(&'static str),
    Configuration(&'static str),
    Database,
}

impl Display for CommunicationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(reason) => write!(f, "invalid communication request: {reason}"),
            Self::Unauthorized => write!(f, "communication service scope is not authorized"),
            Self::NotFound => write!(f, "communication record was not found"),
            Self::Conflict(reason) => write!(f, "communication state conflict: {reason}"),
            Self::Configuration(reason) => write!(f, "communication configuration error: {reason}"),
            Self::Database => write!(f, "communication database operation failed"),
        }
    }
}

impl std::error::Error for CommunicationError {}

fn database_error<T>(_error: T) -> CommunicationError {
    CommunicationError::Database
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommunicationActor {
    subject: String,
    scopes: Vec<String>,
    allowed_universe_id: Option<i64>,
}

impl CommunicationActor {
    pub fn authenticated_service(
        subject: impl Into<String>,
        allowed_universe_id: i64,
        scopes: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, CommunicationError> {
        let subject = subject.into();
        let scopes = scopes.into_iter().map(Into::into).collect::<Vec<_>>();
        if !subject.starts_with("service:")
            || allowed_universe_id <= 0
            || subject.len() > 128
            || !subject
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
            || scopes.is_empty()
        {
            return Err(CommunicationError::Unauthorized);
        }
        Ok(Self {
            subject,
            scopes,
            allowed_universe_id: Some(allowed_universe_id),
        })
    }

    pub fn authenticated_global_service(
        subject: impl Into<String>,
        scopes: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, CommunicationError> {
        let subject = subject.into();
        let scopes = scopes.into_iter().map(Into::into).collect::<Vec<_>>();
        if !subject.starts_with("service:")
            || subject.len() > 128
            || !subject
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
            || !scopes
                .iter()
                .any(|scope| scope == COMMUNICATION_SCOPE_GLOBAL)
        {
            return Err(CommunicationError::Unauthorized);
        }
        Ok(Self {
            subject,
            scopes,
            allowed_universe_id: None,
        })
    }

    pub fn subject(&self) -> &str {
        &self.subject
    }

    pub fn require_scope(&self, required: &str) -> Result<(), CommunicationError> {
        if self.scopes.iter().any(|scope| scope == required) {
            Ok(())
        } else {
            Err(CommunicationError::Unauthorized)
        }
    }

    pub fn require_universe(&self, universe_id: i64) -> Result<(), CommunicationError> {
        if universe_id <= 0 {
            return Err(CommunicationError::Unauthorized);
        }
        match self.allowed_universe_id {
            Some(allowed) if allowed == universe_id => Ok(()),
            None if self
                .scopes
                .iter()
                .any(|scope| scope == COMMUNICATION_SCOPE_GLOBAL) =>
            {
                Ok(())
            }
            _ => Err(CommunicationError::Unauthorized),
        }
    }

    fn require_global(&self) -> Result<(), CommunicationError> {
        if self.allowed_universe_id.is_none()
            && self
                .scopes
                .iter()
                .any(|scope| scope == COMMUNICATION_SCOPE_GLOBAL)
        {
            Ok(())
        } else {
            Err(CommunicationError::Unauthorized)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommunicationChannel {
    Email,
    Sms,
}

impl CommunicationChannel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Email => "email",
            Self::Sms => "sms",
        }
    }

    pub fn parse(value: &str) -> Result<Self, CommunicationError> {
        match value {
            "email" => Ok(Self::Email),
            "sms" => Ok(Self::Sms),
            _ => Err(CommunicationError::InvalidInput("channel is unsupported")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommunicationCategory {
    Marketing,
    ProductUpdates,
    GameplayDigest,
    Security,
    Transactional,
}

impl CommunicationCategory {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Marketing => "marketing",
            Self::ProductUpdates => "product_updates",
            Self::GameplayDigest => "gameplay_digest",
            Self::Security => "security",
            Self::Transactional => "transactional",
        }
    }

    pub fn parse(value: &str) -> Result<Self, CommunicationError> {
        match value {
            "marketing" => Ok(Self::Marketing),
            "product_updates" => Ok(Self::ProductUpdates),
            "gameplay_digest" => Ok(Self::GameplayDigest),
            "security" => Ok(Self::Security),
            "transactional" => Ok(Self::Transactional),
            _ => Err(CommunicationError::InvalidInput("category is unsupported")),
        }
    }

    pub const fn is_essential(self) -> bool {
        matches!(self, Self::Security | Self::Transactional)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommunicationState {
    Pending,
    Leased,
    Retry,
    Sent,
    Dead,
    Suppressed,
}

impl CommunicationState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Leased => "leased",
            Self::Retry => "retry",
            Self::Sent => "sent",
            Self::Dead => "dead",
            Self::Suppressed => "suppressed",
        }
    }

    fn parse(value: &str) -> Result<Self, CommunicationError> {
        match value {
            "pending" => Ok(Self::Pending),
            "leased" => Ok(Self::Leased),
            "retry" => Ok(Self::Retry),
            "sent" => Ok(Self::Sent),
            "dead" => Ok(Self::Dead),
            "suppressed" => Ok(Self::Suppressed),
            _ => Err(CommunicationError::Database),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommunicationEnqueueInput {
    pub universe_id: i64,
    pub user_id: i32,
    pub channel: CommunicationChannel,
    pub category: CommunicationCategory,
    pub template_key: String,
    pub payload_identity: String,
    pub idempotency_key: String,
    pub max_attempts: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommunicationJob {
    pub id: i64,
    pub universe_id: i64,
    pub user_id: i32,
    pub channel: CommunicationChannel,
    pub category: CommunicationCategory,
    pub template_key: String,
    pub payload_identity: String,
    pub idempotency_key: String,
    pub state: CommunicationState,
    pub attempts: i32,
    pub max_attempts: i32,
    pub lease_owner: Option<String>,
    pub lease_until_unix: Option<i64>,
    pub created_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommunicationEnqueueResult {
    pub job: CommunicationJob,
    pub idempotent_replay: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommunicationDeliveryPolicy {
    pub provider_key: String,
    pub provider_template_key: String,
}

pub struct ResolvedCommunicationContact {
    pub destination: Zeroizing<String>,
    pub destination_hmac: [u8; 32],
    pub destination_masked: String,
}

impl Debug for ResolvedCommunicationContact {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolvedCommunicationContact")
            .field("destination", &"[REDACTED]")
            .field("destination_hmac", &"[REDACTED]")
            .field("destination_masked", &self.destination_masked)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommunicationStatusAggregate {
    pub universe_id: i64,
    pub channel: CommunicationChannel,
    pub category: CommunicationCategory,
    pub state: CommunicationState,
    pub job_count: i64,
    pub oldest_created_at_unix: i64,
    pub newest_updated_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommunicationAuditEvent {
    pub id: i64,
    pub outbox_id: i64,
    pub channel: CommunicationChannel,
    pub category: CommunicationCategory,
    pub event_type: String,
    pub state: CommunicationState,
    pub reason_code: Option<String>,
    pub attempt: i32,
    pub created_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommunicationControlAuditEvent {
    pub id: i64,
    pub universe_id: i64,
    pub user_id: Option<i32>,
    pub control_type: String,
    pub channel: CommunicationChannel,
    pub category: Option<CommunicationCategory>,
    pub action: String,
    pub reason_code: String,
    pub control_version: i64,
    pub created_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommunicationPolicyInput {
    pub universe_id: i64,
    pub channel: CommunicationChannel,
    pub category: CommunicationCategory,
    pub provider_key: String,
    pub enabled: bool,
    pub expected_version: Option<i64>,
}

impl Database {
    pub async fn communication_repository_ready(&self) -> Result<(), CommunicationError> {
        let client = self.pool.get().await.map_err(database_error)?;
        let ready = client
            .query_one(
                "SELECT to_regclass('public.communication_outbox') IS NOT NULL
                    AND to_regclass('public.communication_outbox_events') IS NOT NULL
                    AND to_regclass('public.communication_control_events') IS NOT NULL
                    AND to_regclass('public.communication_verified_contacts') IS NOT NULL
                    AND to_regclass('public.communication_contact_versions') IS NOT NULL
                    AND to_regclass('public.communication_channel_policies') IS NOT NULL",
                &[],
            )
            .await
            .map_err(database_error)?
            .get::<_, bool>(0);
        if ready {
            Ok(())
        } else {
            Err(CommunicationError::Configuration(
                "durable communication schema is missing",
            ))
        }
    }

    pub async fn enqueue_communication(
        &self,
        input: CommunicationEnqueueInput,
        actor: &CommunicationActor,
        evidence_key: &CommunicationEvidenceKey,
    ) -> Result<CommunicationEnqueueResult, CommunicationError> {
        actor.require_scope(COMMUNICATION_SCOPE_ENQUEUE)?;
        actor.require_universe(input.universe_id)?;
        validate_enqueue(&input)?;
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;

        let template_category = transaction
            .query_opt(
                "SELECT category FROM communication_templates
                 WHERE channel = $1 AND template_key = $2 AND active = TRUE",
                &[&input.channel.as_str(), &input.template_key],
            )
            .await
            .map_err(database_error)?
            .ok_or(CommunicationError::InvalidInput(
                "template is not registered for the channel",
            ))?
            .get::<_, String>("category");
        if template_category != input.category.as_str() {
            return Err(CommunicationError::InvalidInput(
                "template category does not match the job category",
            ));
        }

        let inserted = transaction
            .query_opt(
                "INSERT INTO communication_outbox (
                    universe_id, user_id, channel, category, template_key,
                    payload_identity, idempotency_key, max_attempts
                 ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                 ON CONFLICT (universe_id, user_id, channel, idempotency_key) DO NOTHING
                 RETURNING id, universe_id, user_id, channel, category, template_key,
                    payload_identity, idempotency_key, state, attempts, max_attempts,
                    lease_owner, EXTRACT(EPOCH FROM lease_until)::BIGINT AS lease_until_unix,
                    EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at_unix",
                &[
                    &input.universe_id,
                    &input.user_id,
                    &input.channel.as_str(),
                    &input.category.as_str(),
                    &input.template_key,
                    &input.payload_identity,
                    &input.idempotency_key,
                    &input.max_attempts,
                ],
            )
            .await
            .map_err(database_error)?;

        let (job, idempotent_replay) = if let Some(row) = inserted {
            let job = map_job(&row)?;
            insert_event(
                &transaction,
                &job,
                "enqueued",
                CommunicationState::Pending,
                None,
                actor_hmac(actor, evidence_key),
            )
            .await?;
            (job, false)
        } else {
            let row = transaction
                .query_opt(
                    "SELECT id, universe_id, user_id, channel, category, template_key,
                        payload_identity, idempotency_key, state, attempts, max_attempts,
                        lease_owner, EXTRACT(EPOCH FROM lease_until)::BIGINT AS lease_until_unix,
                        EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at_unix
                     FROM communication_outbox
                     WHERE universe_id = $1 AND user_id = $2
                       AND channel = $3 AND idempotency_key = $4",
                    &[
                        &input.universe_id,
                        &input.user_id,
                        &input.channel.as_str(),
                        &input.idempotency_key,
                    ],
                )
                .await
                .map_err(database_error)?
                .ok_or(CommunicationError::Conflict("idempotency lookup failed"))?;
            let job = map_job(&row)?;
            if job.user_id != input.user_id
                || job.category != input.category
                || job.template_key != input.template_key
                || job.payload_identity != input.payload_identity
                || job.max_attempts != input.max_attempts
            {
                return Err(CommunicationError::Conflict(
                    "idempotency key belongs to a different communication",
                ));
            }
            (job, true)
        };
        transaction.commit().await.map_err(database_error)?;
        Ok(CommunicationEnqueueResult {
            job,
            idempotent_replay,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn claim_communications(
        &self,
        universe_id: i64,
        channel: CommunicationChannel,
        worker_id: &str,
        limit: i64,
        lease_seconds: i64,
        actor: &CommunicationActor,
        evidence_key: &CommunicationEvidenceKey,
    ) -> Result<Vec<CommunicationJob>, CommunicationError> {
        actor.require_scope(COMMUNICATION_SCOPE_DISPATCH)?;
        actor.require_universe(universe_id)?;
        validate_tokenish(worker_id, 3, 96, "worker id is invalid")?;
        if !(1..=MAX_CLAIM_LIMIT).contains(&limit)
            || !(1..=MAX_LEASE_SECONDS).contains(&lease_seconds)
        {
            return Err(CommunicationError::InvalidInput(
                "claim limit or lease duration is invalid",
            ));
        }
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        let exhausted_ready = transaction
            .query(
                "SELECT id FROM communication_outbox
                 WHERE universe_id = $1 AND channel = $2 AND state IN ('pending', 'retry')
                   AND available_at <= now() AND attempts >= max_attempts
                 ORDER BY available_at, id
                 FOR UPDATE SKIP LOCKED LIMIT $3",
                &[&universe_id, &channel.as_str(), &limit],
            )
            .await
            .map_err(database_error)?;
        for exhausted in exhausted_ready {
            let id: i64 = exhausted.get("id");
            let row = transaction
                .query_one(
                    "UPDATE communication_outbox
                     SET state = 'dead', terminal_at = now(), updated_at = now(),
                         last_reason_code = 'maximum_attempts_exhausted',
                         lease_owner = NULL, lease_until = NULL
                     WHERE id = $1
                     RETURNING id, universe_id, user_id, channel, category, template_key,
                        payload_identity, idempotency_key, state, attempts, max_attempts,
                        lease_owner, EXTRACT(EPOCH FROM lease_until)::BIGINT AS lease_until_unix,
                        EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at_unix",
                    &[&id],
                )
                .await
                .map_err(database_error)?;
            let job = map_job(&row)?;
            insert_event(
                &transaction,
                &job,
                "dead",
                CommunicationState::Dead,
                Some("maximum_attempts_exhausted"),
                actor_hmac(actor, evidence_key),
            )
            .await?;
        }
        let candidates = transaction
            .query(
                "SELECT id, state, attempts, max_attempts
                 FROM communication_outbox
                 WHERE universe_id = $1 AND channel = $2
                   AND (
                       (state IN ('pending', 'retry') AND available_at <= now()
                        AND attempts < max_attempts)
                       OR (state = 'leased' AND lease_until <= now())
                   )
                 ORDER BY available_at, id
                 FOR UPDATE SKIP LOCKED
                 LIMIT $3",
                &[&universe_id, &channel.as_str(), &limit],
            )
            .await
            .map_err(database_error)?;
        let mut jobs = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            let id: i64 = candidate.get("id");
            let reclaimed = candidate.get::<_, String>("state") == "leased";
            let attempts: i32 = candidate.get("attempts");
            let max_attempts: i32 = candidate.get("max_attempts");
            if reclaimed && attempts >= max_attempts {
                let row = transaction
                    .query_one(
                        "UPDATE communication_outbox
                         SET state = 'dead', terminal_at = now(), updated_at = now(),
                             last_reason_code = 'maximum_attempts_exhausted',
                             lease_owner = NULL, lease_until = NULL
                         WHERE id = $1 AND state = 'leased' AND lease_until <= now()
                         RETURNING id, universe_id, user_id, channel, category, template_key,
                            payload_identity, idempotency_key, state, attempts, max_attempts,
                            lease_owner, EXTRACT(EPOCH FROM lease_until)::BIGINT AS lease_until_unix,
                            EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at_unix",
                        &[&id],
                    )
                    .await
                    .map_err(database_error)?;
                let job = map_job(&row)?;
                insert_event(
                    &transaction,
                    &job,
                    "dead",
                    CommunicationState::Dead,
                    Some("maximum_attempts_exhausted"),
                    actor_hmac(actor, evidence_key),
                )
                .await?;
                continue;
            }
            let row = transaction
                .query_one(
                    "UPDATE communication_outbox
                     SET state = 'leased', attempts = attempts + 1,
                         lease_owner = $2,
                         lease_until = now() + ($3::BIGINT * INTERVAL '1 second'),
                         updated_at = now(), terminal_at = NULL
                     WHERE id = $1
                     RETURNING id, universe_id, user_id, channel, category, template_key,
                        payload_identity, idempotency_key, state, attempts, max_attempts,
                        lease_owner, EXTRACT(EPOCH FROM lease_until)::BIGINT AS lease_until_unix,
                        EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at_unix",
                    &[&id, &worker_id, &lease_seconds],
                )
                .await
                .map_err(database_error)?;
            let job = map_job(&row)?;
            insert_event(
                &transaction,
                &job,
                if reclaimed {
                    "lease_reclaimed"
                } else {
                    "leased"
                },
                CommunicationState::Leased,
                None,
                actor_hmac(actor, evidence_key),
            )
            .await?;
            jobs.push(job);
        }
        transaction.commit().await.map_err(database_error)?;
        Ok(jobs)
    }

    pub async fn renew_communication_lease(
        &self,
        job: &CommunicationJob,
        worker_id: &str,
        lease_seconds: i64,
        actor: &CommunicationActor,
    ) -> Result<CommunicationJob, CommunicationError> {
        actor.require_scope(COMMUNICATION_SCOPE_DISPATCH)?;
        actor.require_universe(job.universe_id)?;
        validate_tokenish(worker_id, 3, 96, "worker id is invalid")?;
        if !(1..=MAX_LEASE_SECONDS).contains(&lease_seconds) {
            return Err(CommunicationError::InvalidInput(
                "lease duration is invalid",
            ));
        }
        let client = self.pool.get().await.map_err(database_error)?;
        ensure_active_job_snapshot(&client, job, worker_id).await?;
        let row = client
            .query_opt(
                "UPDATE communication_outbox
                 SET lease_until = now() + ($3::BIGINT * INTERVAL '1 second'), updated_at = now()
                 WHERE id = $1 AND state = 'leased' AND lease_owner = $2
                   AND lease_until > now() AND attempts <= max_attempts
                 RETURNING id, universe_id, user_id, channel, category, template_key,
                    payload_identity, idempotency_key, state, attempts, max_attempts,
                    lease_owner, EXTRACT(EPOCH FROM lease_until)::BIGINT AS lease_until_unix,
                    EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at_unix",
                &[&job.id, &worker_id, &lease_seconds],
            )
            .await
            .map_err(database_error)?
            .ok_or(CommunicationError::Conflict(
                "communication lease expired or is not owned",
            ))?;
        map_job(&row)
    }

    pub async fn communication_delivery_policy(
        &self,
        job: &CommunicationJob,
        actor: &CommunicationActor,
    ) -> Result<Option<CommunicationDeliveryPolicy>, CommunicationError> {
        actor.require_scope(COMMUNICATION_SCOPE_DISPATCH)?;
        actor.require_universe(job.universe_id)?;
        let worker_id = job
            .lease_owner
            .as_deref()
            .ok_or(CommunicationError::Conflict("communication is not leased"))?;
        let client = self.pool.get().await.map_err(database_error)?;
        let active_job = ensure_active_job_snapshot(&client, job, worker_id).await?;
        let row = client
            .query_opt(
                "SELECT policy.provider_key, template.provider_template_key
                 FROM communication_channel_policies AS policy
                 JOIN communication_templates AS template
                   ON template.channel = policy.channel
                  AND template.template_key = $4
                  AND template.category = policy.category
                  AND template.active = TRUE
                 WHERE policy.universe_id = $1 AND policy.channel = $2
                   AND policy.category = $3 AND policy.enabled = TRUE",
                &[
                    &active_job.universe_id,
                    &active_job.channel.as_str(),
                    &active_job.category.as_str(),
                    &active_job.template_key,
                ],
            )
            .await
            .map_err(database_error)?;
        Ok(row.map(|row| CommunicationDeliveryPolicy {
            provider_key: row.get("provider_key"),
            provider_template_key: row.get("provider_template_key"),
        }))
    }

    pub async fn set_communication_policy(
        &self,
        input: CommunicationPolicyInput,
        reason_code: &str,
        actor: &CommunicationActor,
        evidence_key: &CommunicationEvidenceKey,
    ) -> Result<i64, CommunicationError> {
        actor.require_scope(COMMUNICATION_SCOPE_POLICY_WRITE)?;
        actor.require_universe(input.universe_id)?;
        validate_reason(reason_code)?;
        validate_tokenish(&input.provider_key, 2, 64, "provider key is invalid")?;
        if input.expected_version.is_some_and(|version| version <= 0) {
            return Err(CommunicationError::InvalidInput(
                "expected policy version is invalid",
            ));
        }
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        let current = transaction
            .query_opt(
                "SELECT version FROM communication_channel_policies
                 WHERE universe_id = $1 AND channel = $2 AND category = $3
                 FOR UPDATE",
                &[
                    &input.universe_id,
                    &input.channel.as_str(),
                    &input.category.as_str(),
                ],
            )
            .await
            .map_err(database_error)?;
        let version: i64 = match (current, input.expected_version) {
            (None, None) => transaction
                .query_opt(
                    "INSERT INTO communication_channel_policies (
                        universe_id, channel, category, provider_key, enabled
                     ) VALUES ($1, $2, $3, $4, $5)
                     ON CONFLICT (universe_id, channel, category) DO NOTHING
                     RETURNING version",
                    &[
                        &input.universe_id,
                        &input.channel.as_str(),
                        &input.category.as_str(),
                        &input.provider_key,
                        &input.enabled,
                    ],
                )
                .await
                .map_err(database_error)?
                .ok_or(CommunicationError::Conflict(
                    "communication policy was created concurrently",
                ))?
                .get("version"),
            (Some(row), Some(expected)) if row.get::<_, i64>("version") == expected => transaction
                .query_opt(
                    "UPDATE communication_channel_policies
                     SET provider_key = $4, enabled = $5, version = version + 1,
                         updated_at = now()
                     WHERE universe_id = $1 AND channel = $2 AND category = $3
                       AND version = $6
                     RETURNING version",
                    &[
                        &input.universe_id,
                        &input.channel.as_str(),
                        &input.category.as_str(),
                        &input.provider_key,
                        &input.enabled,
                        &expected,
                    ],
                )
                .await
                .map_err(database_error)?
                .ok_or(CommunicationError::Conflict(
                    "communication policy version changed",
                ))?
                .get("version"),
            _ => {
                return Err(CommunicationError::Conflict(
                    "communication policy version changed",
                ));
            }
        };
        insert_control_event(
            &transaction,
            input.universe_id,
            None,
            "channel_policy",
            input.channel,
            Some(input.category),
            if input.enabled { "enabled" } else { "disabled" },
            reason_code,
            version,
            actor_hmac(actor, evidence_key),
        )
        .await?;
        transaction.commit().await.map_err(database_error)?;
        Ok(version)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn record_current_verified_contact(
        &self,
        universe_id: i64,
        user_id: i32,
        channel: CommunicationChannel,
        verification_method: &str,
        reason_code: &str,
        valid_for_seconds: i64,
        actor: &CommunicationActor,
        evidence_key: &CommunicationEvidenceKey,
    ) -> Result<String, CommunicationError> {
        actor.require_scope(COMMUNICATION_SCOPE_CONTACT_VERIFY)?;
        actor.require_universe(universe_id)?;
        validate_tokenish(verification_method, 2, 64, "verification method is invalid")?;
        validate_reason(reason_code)?;
        if !(60..=31_536_000).contains(&valid_for_seconds) {
            return Err(CommunicationError::InvalidInput(
                "verification lifetime is invalid",
            ));
        }
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        let row = transaction
            .query_opt(
                "SELECT CASE WHEN $3 = 'email' THEN email ELSE phone_number END AS destination
                 FROM users WHERE universe_id = $1 AND id = $2 FOR UPDATE",
                &[&universe_id, &user_id, &channel.as_str()],
            )
            .await
            .map_err(database_error)?
            .ok_or(CommunicationError::NotFound)?;
        let destination = Zeroizing::new(
            row.get::<_, Option<String>>("destination")
                .ok_or(CommunicationError::NotFound)?,
        );
        let digest = evidence_key.evidence_hmac("communication.destination", destination.as_str());
        let masked = mask_destination(channel, destination.as_str())?;
        transaction
            .execute(
                "UPDATE users
                 SET email_verified = CASE WHEN $3 = 'email' THEN TRUE ELSE email_verified END,
                     phone_verified = CASE WHEN $3 = 'sms' THEN TRUE ELSE phone_verified END
                 WHERE universe_id = $1 AND id = $2",
                &[&universe_id, &user_id, &channel.as_str()],
            )
            .await
            .map_err(database_error)?;
        let version: i64 = transaction
            .query_one(
                "INSERT INTO communication_contact_versions (
                    universe_id, user_id, channel
                 ) VALUES ($1, $2, $3)
                 ON CONFLICT (universe_id, user_id, channel) DO UPDATE
                 SET version = communication_contact_versions.version + 1,
                     updated_at = now()
                 RETURNING version",
                &[&universe_id, &user_id, &channel.as_str()],
            )
            .await
            .map_err(database_error)?
            .get("version");
        transaction
            .query_one(
                "INSERT INTO communication_verified_contacts (
                    universe_id, user_id, channel, destination_hmac,
                    destination_masked, verification_method, expires_at, retention_until,
                    version
                 ) VALUES ($1, $2, $3, $4, $5, $6,
                    now() + ($7::BIGINT * INTERVAL '1 second'),
                    now() + ($7::BIGINT * INTERVAL '1 second') + INTERVAL '90 days', $8)
                 ON CONFLICT (universe_id, user_id, channel) DO UPDATE
                 SET destination_hmac = EXCLUDED.destination_hmac,
                     destination_masked = EXCLUDED.destination_masked,
                     verification_method = EXCLUDED.verification_method,
                     verified_at = now(), expires_at = EXCLUDED.expires_at,
                     revoked_at = NULL, version = EXCLUDED.version,
                     retention_until = EXCLUDED.retention_until
                 RETURNING version",
                &[
                    &universe_id,
                    &user_id,
                    &channel.as_str(),
                    &&digest[..],
                    &masked,
                    &verification_method,
                    &valid_for_seconds,
                    &version,
                ],
            )
            .await
            .map_err(database_error)?;
        insert_control_event(
            &transaction,
            universe_id,
            Some(user_id),
            "verified_contact",
            channel,
            None,
            "verified",
            reason_code,
            version,
            actor_hmac(actor, evidence_key),
        )
        .await?;
        transaction.commit().await.map_err(database_error)?;
        Ok(masked)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn replace_communication_contact(
        &self,
        universe_id: i64,
        user_id: i32,
        channel: CommunicationChannel,
        destination: Zeroizing<String>,
        reason_code: &str,
        actor: &CommunicationActor,
        evidence_key: &CommunicationEvidenceKey,
    ) -> Result<String, CommunicationError> {
        actor.require_scope(COMMUNICATION_SCOPE_CONTACT_VERIFY)?;
        actor.require_universe(universe_id)?;
        validate_reason(reason_code)?;
        if universe_id <= 0 || user_id <= 0 {
            return Err(CommunicationError::InvalidInput(
                "tenant and user identifiers must be positive",
            ));
        }
        let masked = mask_destination(channel, destination.as_str())?;
        let actor_hmac_hex = encode_hex(&actor_hmac(actor, evidence_key));
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        transaction
            .query_one(
                "SELECT
                    set_config('app.communication_actor_subject_hmac', $1, TRUE),
                    set_config('app.communication_change_reason', $2, TRUE)",
                &[&actor_hmac_hex, &reason_code],
            )
            .await
            .map_err(database_error)?;
        let changed = match channel {
            CommunicationChannel::Email => {
                transaction
                    .execute(
                        "UPDATE users SET email = $3
                         WHERE universe_id = $1 AND id = $2",
                        &[&universe_id, &user_id, &destination.as_str()],
                    )
                    .await
            }
            CommunicationChannel::Sms => {
                transaction
                    .execute(
                        "UPDATE users SET phone_number = $3
                         WHERE universe_id = $1 AND id = $2",
                        &[&universe_id, &user_id, &destination.as_str()],
                    )
                    .await
            }
        }
        .map_err(database_error)?;
        if changed != 1 {
            return Err(CommunicationError::NotFound);
        }
        transaction.commit().await.map_err(database_error)?;
        Ok(masked)
    }

    pub async fn resolve_verified_communication_contact(
        &self,
        job: &CommunicationJob,
        actor: &CommunicationActor,
        evidence_key: &CommunicationEvidenceKey,
    ) -> Result<Option<ResolvedCommunicationContact>, CommunicationError> {
        actor.require_scope(COMMUNICATION_SCOPE_DISPATCH)?;
        actor.require_universe(job.universe_id)?;
        let worker_id = job
            .lease_owner
            .as_deref()
            .ok_or(CommunicationError::Conflict("communication is not leased"))?;
        let client = self.pool.get().await.map_err(database_error)?;
        ensure_active_job_snapshot(&client, job, worker_id).await?;
        let row = client
            .query_opt(
                "SELECT
                    CASE WHEN $3 = 'email' THEN users.email ELSE users.phone_number END AS destination,
                    CASE WHEN $3 = 'email' THEN users.email_verified ELSE users.phone_verified END AS verified,
                    evidence.destination_hmac, evidence.destination_masked
                 FROM communication_outbox AS active_job
                 JOIN users ON users.universe_id = active_job.universe_id
                           AND users.id = active_job.user_id
                 LEFT JOIN communication_verified_contacts AS evidence
                   ON evidence.universe_id = users.universe_id
                  AND evidence.user_id = users.id
                  AND evidence.channel = $3
                  AND evidence.revoked_at IS NULL
                  AND evidence.expires_at > now()
                 WHERE active_job.id = $4 AND active_job.universe_id = $1
                   AND active_job.user_id = $2 AND active_job.channel = $3
                   AND active_job.state = 'leased' AND active_job.lease_owner = $5
                   AND active_job.lease_until > now()",
                &[
                    &job.universe_id,
                    &job.user_id,
                    &job.channel.as_str(),
                    &job.id,
                    &worker_id,
                ],
            )
            .await
            .map_err(database_error)?;
        let Some(row) = row else {
            return Ok(None);
        };
        if !row.get::<_, bool>("verified") {
            return Ok(None);
        }
        let Some(destination) = row.get::<_, Option<String>>("destination") else {
            return Ok(None);
        };
        let destination = Zeroizing::new(destination);
        let Some(stored_hmac) = row.get::<_, Option<Vec<u8>>>("destination_hmac") else {
            return Ok(None);
        };
        let Some(stored_masked) = row.get::<_, Option<String>>("destination_masked") else {
            return Ok(None);
        };
        let digest = evidence_key.evidence_hmac("communication.destination", destination.as_str());
        if stored_hmac.len() != digest.len()
            || stored_hmac.ct_eq(digest.as_slice()).unwrap_u8() != 1
        {
            return Ok(None);
        }
        let masked = mask_destination(job.channel, destination.as_str())?;
        if masked != stored_masked {
            return Ok(None);
        }
        Ok(Some(ResolvedCommunicationContact {
            destination,
            destination_hmac: digest,
            destination_masked: masked,
        }))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn mark_communication_sent(
        &self,
        job: &CommunicationJob,
        worker_id: &str,
        provider_key: &str,
        provider_message_id: &str,
        destination_hmac: [u8; 32],
        destination_masked: &str,
        actor: &CommunicationActor,
        evidence_key: &CommunicationEvidenceKey,
    ) -> Result<CommunicationJob, CommunicationError> {
        actor.require_scope(COMMUNICATION_SCOPE_DISPATCH)?;
        actor.require_universe(job.universe_id)?;
        validate_tokenish(provider_key, 2, 64, "provider key is invalid")?;
        validate_tokenish(worker_id, 3, 96, "worker id is invalid")?;
        if provider_message_id.trim().is_empty() || provider_message_id.len() > 256 {
            return Err(CommunicationError::InvalidInput(
                "provider receipt identity is invalid",
            ));
        }
        let receipt =
            evidence_key.evidence_hmac("communication.provider_receipt", provider_message_id);
        self.finish_communication(
            job,
            worker_id,
            CommunicationState::Sent,
            provider_key,
            None,
            Some(receipt),
            Some(destination_hmac),
            Some(destination_masked),
            actor,
            evidence_key,
        )
        .await
    }

    pub async fn suppress_communication(
        &self,
        job: &CommunicationJob,
        worker_id: &str,
        reason_code: &str,
        actor: &CommunicationActor,
        evidence_key: &CommunicationEvidenceKey,
    ) -> Result<CommunicationJob, CommunicationError> {
        actor.require_scope(COMMUNICATION_SCOPE_DISPATCH)?;
        actor.require_universe(job.universe_id)?;
        validate_reason(reason_code)?;
        self.finish_communication(
            job,
            worker_id,
            CommunicationState::Suppressed,
            "policy",
            Some(reason_code),
            None,
            None,
            None,
            actor,
            evidence_key,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn fail_communication_attempt(
        &self,
        job: &CommunicationJob,
        worker_id: &str,
        provider_key: &str,
        reason_code: &str,
        retry_delay_seconds: i64,
        actor: &CommunicationActor,
        evidence_key: &CommunicationEvidenceKey,
    ) -> Result<CommunicationJob, CommunicationError> {
        actor.require_scope(COMMUNICATION_SCOPE_DISPATCH)?;
        actor.require_universe(job.universe_id)?;
        validate_reason(reason_code)?;
        validate_tokenish(provider_key, 2, 64, "provider key is invalid")?;
        if !(0..=86_400).contains(&retry_delay_seconds) {
            return Err(CommunicationError::InvalidInput("retry delay is invalid"));
        }
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        let current = ensure_active_job_snapshot(&transaction, job, worker_id).await?;
        let attempts = current.attempts;
        let max_attempts = current.max_attempts;
        let state = if attempts >= max_attempts {
            CommunicationState::Dead
        } else {
            CommunicationState::Retry
        };
        let row = transaction
            .query_opt(
                "UPDATE communication_outbox
                 SET state = $3, provider_key = $4, last_reason_code = $5,
                     lease_owner = NULL, lease_until = NULL, updated_at = now(),
                     available_at = CASE WHEN $3 = 'retry'
                        THEN now() + ($6::BIGINT * INTERVAL '1 second') ELSE available_at END,
                     terminal_at = CASE WHEN $3 = 'dead' THEN now() ELSE NULL END
                 WHERE id = $1 AND state = 'leased' AND lease_owner = $2
                   AND lease_until > now()
                 RETURNING id, universe_id, user_id, channel, category, template_key,
                    payload_identity, idempotency_key, state, attempts, max_attempts,
                    lease_owner, EXTRACT(EPOCH FROM lease_until)::BIGINT AS lease_until_unix,
                    EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at_unix",
                &[
                    &job.id,
                    &worker_id,
                    &state.as_str(),
                    &provider_key,
                    &reason_code,
                    &retry_delay_seconds,
                ],
            )
            .await
            .map_err(database_error)?
            .ok_or(CommunicationError::Conflict(
                "communication lease expired or is not owned",
            ))?;
        let job = map_job(&row)?;
        insert_event(
            &transaction,
            &job,
            if state == CommunicationState::Dead {
                "dead"
            } else {
                "retry_scheduled"
            },
            state,
            Some(reason_code),
            actor_hmac(actor, evidence_key),
        )
        .await?;
        transaction.commit().await.map_err(database_error)?;
        Ok(job)
    }

    #[allow(clippy::too_many_arguments)]
    async fn finish_communication(
        &self,
        job: &CommunicationJob,
        worker_id: &str,
        state: CommunicationState,
        provider_key: &str,
        reason_code: Option<&str>,
        provider_message_hmac: Option<[u8; 32]>,
        destination_hmac: Option<[u8; 32]>,
        destination_masked: Option<&str>,
        actor: &CommunicationActor,
        evidence_key: &CommunicationEvidenceKey,
    ) -> Result<CommunicationJob, CommunicationError> {
        validate_tokenish(worker_id, 3, 96, "worker id is invalid")?;
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        ensure_active_job_snapshot(&transaction, job, worker_id).await?;
        let row = transaction
            .query_opt(
                "UPDATE communication_outbox
                 SET state = $3, provider_key = $4, last_reason_code = $5,
                     provider_message_hmac = $6, destination_hmac = $7,
                     destination_masked = $8,
                     lease_owner = NULL, lease_until = NULL,
                     updated_at = now(), terminal_at = now(),
                     sent_at = CASE WHEN $3 = 'sent' THEN now() ELSE NULL END
                 WHERE id = $1 AND state = 'leased' AND lease_owner = $2
                   AND lease_until > now()
                 RETURNING id, universe_id, user_id, channel, category, template_key,
                    payload_identity, idempotency_key, state, attempts, max_attempts,
                    lease_owner, EXTRACT(EPOCH FROM lease_until)::BIGINT AS lease_until_unix,
                    EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at_unix",
                &[
                    &job.id,
                    &worker_id,
                    &state.as_str(),
                    &provider_key,
                    &reason_code,
                    &provider_message_hmac.as_ref().map(|value| &value[..]),
                    &destination_hmac.as_ref().map(|value| &value[..]),
                    &destination_masked,
                ],
            )
            .await
            .map_err(database_error)?
            .ok_or(CommunicationError::Conflict(
                "communication lease is not owned",
            ))?;
        let job = map_job(&row)?;
        insert_event(
            &transaction,
            &job,
            state.as_str(),
            state,
            reason_code,
            actor_hmac(actor, evidence_key),
        )
        .await?;
        transaction.commit().await.map_err(database_error)?;
        Ok(job)
    }

    pub async fn communication_status_aggregates(
        &self,
        universe_id: i64,
        actor: &CommunicationActor,
    ) -> Result<Vec<CommunicationStatusAggregate>, CommunicationError> {
        actor.require_scope(COMMUNICATION_SCOPE_AUDIT_READ)?;
        actor.require_universe(universe_id)?;
        let client = self.pool.get().await.map_err(database_error)?;
        let rows = client
            .query(
                "SELECT universe_id, channel, category, state, job_count,
                    EXTRACT(EPOCH FROM oldest_created_at)::BIGINT AS oldest_created_at_unix,
                    EXTRACT(EPOCH FROM newest_updated_at)::BIGINT AS newest_updated_at_unix
                 FROM communication_delivery_status_aggregate
                 WHERE universe_id = $1
                 ORDER BY channel, category, state",
                &[&universe_id],
            )
            .await
            .map_err(database_error)?;
        rows.iter().map(map_aggregate).collect()
    }

    pub async fn communication_audit_events(
        &self,
        universe_id: i64,
        limit: i64,
        actor: &CommunicationActor,
    ) -> Result<Vec<CommunicationAuditEvent>, CommunicationError> {
        actor.require_scope(COMMUNICATION_SCOPE_AUDIT_READ)?;
        actor.require_universe(universe_id)?;
        if !(1..=200).contains(&limit) {
            return Err(CommunicationError::InvalidInput("audit limit is invalid"));
        }
        let client = self.pool.get().await.map_err(database_error)?;
        let rows = client
            .query(
                "SELECT id, outbox_id, channel, category, event_type, state,
                    reason_code, attempt,
                    EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at_unix
                 FROM communication_outbox_events
                 WHERE universe_id = $1
                 ORDER BY id DESC LIMIT $2",
                &[&universe_id, &limit],
            )
            .await
            .map_err(database_error)?;
        rows.iter().map(map_audit_event).collect()
    }

    pub async fn communication_control_audit_events(
        &self,
        universe_id: i64,
        limit: i64,
        actor: &CommunicationActor,
    ) -> Result<Vec<CommunicationControlAuditEvent>, CommunicationError> {
        actor.require_scope(COMMUNICATION_SCOPE_AUDIT_READ)?;
        actor.require_universe(universe_id)?;
        if !(1..=200).contains(&limit) {
            return Err(CommunicationError::InvalidInput("audit limit is invalid"));
        }
        let client = self.pool.get().await.map_err(database_error)?;
        let rows = client
            .query(
                "SELECT id, universe_id, user_id, control_type, channel, category,
                    action, reason_code, control_version,
                    EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at_unix
                 FROM communication_control_events
                 WHERE universe_id = $1
                 ORDER BY id DESC LIMIT $2",
                &[&universe_id, &limit],
            )
            .await
            .map_err(database_error)?;
        rows.iter().map(map_control_audit_event).collect()
    }

    pub async fn apply_communication_retention(
        &self,
        actor: &CommunicationActor,
        evidence_key: &CommunicationEvidenceKey,
    ) -> Result<(u64, u64), CommunicationError> {
        actor.require_scope(COMMUNICATION_SCOPE_RETENTION)?;
        actor.require_global()?;
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        let redacted = transaction
            .query(
                "UPDATE communication_outbox AS jobs
                 SET destination_hmac = NULL, destination_masked = NULL,
                     provider_message_hmac = NULL, updated_at = now()
                 WHERE retention_until <= now()
                   AND (destination_hmac IS NOT NULL OR destination_masked IS NOT NULL
                        OR provider_message_hmac IS NOT NULL)
                   AND NOT privacy_subject_has_active_legal_hold(
                       jobs.universe_id, jobs.user_id
                   )
                 RETURNING id, universe_id, user_id, channel, category, template_key,
                    payload_identity, idempotency_key, state, attempts, max_attempts,
                    lease_owner, EXTRACT(EPOCH FROM lease_until)::BIGINT AS lease_until_unix,
                    EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at_unix",
                &[],
            )
            .await
            .map_err(database_error)?;
        for row in &redacted {
            let job = map_job(row)?;
            insert_event(
                &transaction,
                &job,
                "contact_evidence_redacted",
                job.state,
                Some("retention_expired"),
                actor_hmac(actor, evidence_key),
            )
            .await?;
        }
        transaction
            .execute(
                "DELETE FROM communication_verified_contacts AS contacts
                 WHERE retention_until <= now()
                   AND NOT privacy_subject_has_active_legal_hold(
                       contacts.universe_id, contacts.user_id
                   )",
                &[],
            )
            .await
            .map_err(database_error)?;
        transaction
            .execute(
                "SELECT set_config('app.communication_retention_cleanup', 'enabled', TRUE)",
                &[],
            )
            .await
            .map_err(database_error)?;
        let deleted_events = transaction
            .execute(
                "DELETE FROM communication_outbox_events AS events
                 WHERE retention_until <= now()
                   AND NOT EXISTS (
                       SELECT 1 FROM communication_outbox AS jobs
                       WHERE jobs.id = events.outbox_id
                         AND privacy_subject_has_active_legal_hold(
                             jobs.universe_id, jobs.user_id
                         )
                   )",
                &[],
            )
            .await
            .map_err(database_error)?;
        let deleted_control_events = transaction
            .execute(
                "DELETE FROM communication_control_events AS events
                 WHERE retention_until <= now()
                   AND (
                       events.user_id IS NULL
                       OR NOT privacy_subject_has_active_legal_hold(
                           events.universe_id, events.user_id
                       )
                   )",
                &[],
            )
            .await
            .map_err(database_error)?;
        transaction.commit().await.map_err(database_error)?;
        Ok((
            redacted.len() as u64,
            deleted_events.saturating_add(deleted_control_events),
        ))
    }
}

async fn ensure_active_job_snapshot<C>(
    client: &C,
    job: &CommunicationJob,
    worker_id: &str,
) -> Result<CommunicationJob, CommunicationError>
where
    C: GenericClient + Sync,
{
    let row = client
        .query_opt(
            "SELECT id, universe_id, user_id, channel, category, template_key,
                payload_identity, idempotency_key, state, attempts, max_attempts,
                lease_owner, EXTRACT(EPOCH FROM lease_until)::BIGINT AS lease_until_unix,
                EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at_unix
             FROM communication_outbox
             WHERE id = $1 AND state = 'leased' AND lease_owner = $2
               AND lease_until > now()",
            &[&job.id, &worker_id],
        )
        .await
        .map_err(database_error)?
        .ok_or(CommunicationError::Conflict(
            "communication lease expired or is not owned",
        ))?;
    let active = map_job(&row)?;
    if &active != job {
        return Err(CommunicationError::Conflict(
            "communication job snapshot does not match the active lease",
        ));
    }
    Ok(active)
}

async fn insert_event(
    transaction: &Transaction<'_>,
    job: &CommunicationJob,
    event_type: &str,
    state: CommunicationState,
    reason_code: Option<&str>,
    actor_subject_hmac: [u8; 32],
) -> Result<(), CommunicationError> {
    transaction
        .execute(
            "INSERT INTO communication_outbox_events (
                outbox_id, universe_id, channel, category, event_type,
                state, reason_code, attempt, actor_subject_hmac
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
            &[
                &job.id,
                &job.universe_id,
                &job.channel.as_str(),
                &job.category.as_str(),
                &event_type,
                &state.as_str(),
                &reason_code,
                &job.attempts,
                &&actor_subject_hmac[..],
            ],
        )
        .await
        .map_err(database_error)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn insert_control_event(
    transaction: &Transaction<'_>,
    universe_id: i64,
    user_id: Option<i32>,
    control_type: &str,
    channel: CommunicationChannel,
    category: Option<CommunicationCategory>,
    action: &str,
    reason_code: &str,
    control_version: i64,
    actor_subject_hmac: [u8; 32],
) -> Result<(), CommunicationError> {
    transaction
        .execute(
            "INSERT INTO communication_control_events (
                universe_id, user_id, control_type, channel, category, action,
                reason_code, control_version, actor_subject_hmac
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
            &[
                &universe_id,
                &user_id,
                &control_type,
                &channel.as_str(),
                &category.map(CommunicationCategory::as_str),
                &action,
                &reason_code,
                &control_version,
                &&actor_subject_hmac[..],
            ],
        )
        .await
        .map_err(database_error)?;
    Ok(())
}

fn actor_hmac(actor: &CommunicationActor, evidence_key: &CommunicationEvidenceKey) -> [u8; 32] {
    evidence_key.evidence_hmac("communication.actor", actor.subject())
}

fn encode_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
}

fn map_job(row: &Row) -> Result<CommunicationJob, CommunicationError> {
    Ok(CommunicationJob {
        id: row.get("id"),
        universe_id: row.get("universe_id"),
        user_id: row.get("user_id"),
        channel: CommunicationChannel::parse(row.get::<_, String>("channel").as_str())?,
        category: CommunicationCategory::parse(row.get::<_, String>("category").as_str())?,
        template_key: row.get("template_key"),
        payload_identity: row.get("payload_identity"),
        idempotency_key: row.get("idempotency_key"),
        state: CommunicationState::parse(row.get::<_, String>("state").as_str())?,
        attempts: row.get("attempts"),
        max_attempts: row.get("max_attempts"),
        lease_owner: row.get("lease_owner"),
        lease_until_unix: row.get("lease_until_unix"),
        created_at_unix: row.get("created_at_unix"),
    })
}

fn map_aggregate(row: &Row) -> Result<CommunicationStatusAggregate, CommunicationError> {
    Ok(CommunicationStatusAggregate {
        universe_id: row.get("universe_id"),
        channel: CommunicationChannel::parse(row.get::<_, String>("channel").as_str())?,
        category: CommunicationCategory::parse(row.get::<_, String>("category").as_str())?,
        state: CommunicationState::parse(row.get::<_, String>("state").as_str())?,
        job_count: row.get("job_count"),
        oldest_created_at_unix: row.get("oldest_created_at_unix"),
        newest_updated_at_unix: row.get("newest_updated_at_unix"),
    })
}

fn map_audit_event(row: &Row) -> Result<CommunicationAuditEvent, CommunicationError> {
    Ok(CommunicationAuditEvent {
        id: row.get("id"),
        outbox_id: row.get("outbox_id"),
        channel: CommunicationChannel::parse(row.get::<_, String>("channel").as_str())?,
        category: CommunicationCategory::parse(row.get::<_, String>("category").as_str())?,
        event_type: row.get("event_type"),
        state: CommunicationState::parse(row.get::<_, String>("state").as_str())?,
        reason_code: row.get("reason_code"),
        attempt: row.get("attempt"),
        created_at_unix: row.get("created_at_unix"),
    })
}

fn map_control_audit_event(
    row: &Row,
) -> Result<CommunicationControlAuditEvent, CommunicationError> {
    Ok(CommunicationControlAuditEvent {
        id: row.get("id"),
        universe_id: row.get("universe_id"),
        user_id: row.get("user_id"),
        control_type: row.get("control_type"),
        channel: CommunicationChannel::parse(row.get::<_, String>("channel").as_str())?,
        category: row
            .get::<_, Option<String>>("category")
            .map(|value| CommunicationCategory::parse(&value))
            .transpose()?,
        action: row.get("action"),
        reason_code: row.get("reason_code"),
        control_version: row.get("control_version"),
        created_at_unix: row.get("created_at_unix"),
    })
}

fn validate_enqueue(input: &CommunicationEnqueueInput) -> Result<(), CommunicationError> {
    if input.universe_id <= 0 || input.user_id <= 0 {
        return Err(CommunicationError::InvalidInput(
            "tenant and user identifiers must be positive",
        ));
    }
    validate_tokenish(&input.template_key, 2, 64, "template key is invalid")?;
    validate_tokenish(&input.idempotency_key, 8, 128, "idempotency key is invalid")?;
    validate_payload_identity(&input.payload_identity)?;
    if !(1..=20).contains(&input.max_attempts) {
        return Err(CommunicationError::InvalidInput(
            "maximum attempts is invalid",
        ));
    }
    Ok(())
}

fn validate_payload_identity(value: &str) -> Result<(), CommunicationError> {
    let Some((namespace, identifier)) = value.split_once(':') else {
        return Err(CommunicationError::InvalidInput(
            "payload identity must reference an authoritative event",
        ));
    };
    if !matches!(
        namespace,
        "account_event" | "game_event" | "security_event" | "transaction"
    ) || identifier.is_empty()
        || identifier.len() > 64
        || !identifier
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
    {
        return Err(CommunicationError::InvalidInput(
            "payload identity is not an authoritative event reference",
        ));
    }
    Ok(())
}

fn validate_tokenish(
    value: &str,
    minimum: usize,
    maximum: usize,
    error: &'static str,
) -> Result<(), CommunicationError> {
    if !(minimum..=maximum).contains(&value.len())
        || value.trim() != value
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
    {
        return Err(CommunicationError::InvalidInput(error));
    }
    Ok(())
}

fn validate_reason(value: &str) -> Result<(), CommunicationError> {
    validate_tokenish(value, 2, 64, "reason code is invalid")
}

fn mask_destination(
    channel: CommunicationChannel,
    destination: &str,
) -> Result<String, CommunicationError> {
    match channel {
        CommunicationChannel::Email => {
            let (local, domain) =
                destination
                    .split_once('@')
                    .ok_or(CommunicationError::InvalidInput(
                        "authoritative email contact is invalid",
                    ))?;
            if local.is_empty() || domain.len() < 3 {
                return Err(CommunicationError::InvalidInput(
                    "authoritative email contact is invalid",
                ));
            }
            let first = local.chars().next().unwrap_or('*');
            let masked_domain = domain
                .split('.')
                .map(|label| {
                    label
                        .chars()
                        .next()
                        .map(|first| format!("{first}***"))
                        .unwrap_or_else(|| "***".to_string())
                })
                .collect::<Vec<_>>()
                .join(".");
            Ok(format!("{first}***@{masked_domain}"))
        }
        CommunicationChannel::Sms => {
            let bytes = destination.as_bytes();
            if bytes.len() < 8 || bytes[0] != b'+' || !bytes[1..].iter().all(u8::is_ascii_digit) {
                return Err(CommunicationError::InvalidInput(
                    "authoritative phone contact is invalid",
                ));
            }
            let suffix = &destination[destination.len() - 4..];
            Ok(format!("+***{suffix}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_is_keyed_domain_separated_and_debug_redacted() {
        let first = CommunicationEvidenceKey::new(vec![1; 32]).unwrap();
        let second = CommunicationEvidenceKey::new(vec![2; 32]).unwrap();
        assert_ne!(
            first.evidence_hmac("communication.destination", "person@example.test"),
            second.evidence_hmac("communication.destination", "person@example.test")
        );
        assert_ne!(
            first.evidence_hmac("communication.destination", "person@example.test"),
            first.evidence_hmac("communication.actor", "person@example.test")
        );
        assert!(!format!("{first:?}").contains("AQEBAQ"));
    }

    #[test]
    fn authoritative_payload_identity_rejects_free_text_and_pii_shapes() {
        assert!(validate_payload_identity("game_event:1234-abcd").is_ok());
        assert!(validate_payload_identity("hello there").is_err());
        assert!(validate_payload_identity("game_event:person@example.com").is_err());
        assert!(validate_payload_identity("custom:123").is_err());
    }

    #[test]
    fn destination_masking_never_returns_the_raw_contact() {
        assert_eq!(
            mask_destination(CommunicationChannel::Email, "captain@example.test").unwrap(),
            "c***@e***.t***"
        );
        assert_eq!(
            mask_destination(CommunicationChannel::Sms, "+12065550123").unwrap(),
            "+***0123"
        );
    }

    #[test]
    fn actor_requires_service_identity_and_exact_scope() {
        let actor = CommunicationActor::authenticated_service(
            "service:mailer",
            1,
            [COMMUNICATION_SCOPE_DISPATCH],
        )
        .unwrap();
        assert!(actor.require_scope(COMMUNICATION_SCOPE_DISPATCH).is_ok());
        assert!(actor.require_universe(1).is_ok());
        assert_eq!(
            actor.require_universe(2),
            Err(CommunicationError::Unauthorized)
        );
        assert_eq!(
            actor.require_scope(COMMUNICATION_SCOPE_ENQUEUE),
            Err(CommunicationError::Unauthorized)
        );
        assert!(CommunicationActor::authenticated_service(
            "human:1",
            1,
            [COMMUNICATION_SCOPE_DISPATCH]
        )
        .is_err());
        assert!(CommunicationActor::authenticated_global_service(
            "service:retention",
            [COMMUNICATION_SCOPE_RETENTION]
        )
        .is_err());
        assert!(CommunicationActor::authenticated_global_service(
            "service:retention",
            [COMMUNICATION_SCOPE_RETENTION, COMMUNICATION_SCOPE_GLOBAL]
        )
        .is_ok());
    }
}
