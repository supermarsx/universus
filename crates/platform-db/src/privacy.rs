//! Durable, tenant-scoped privacy and communications repository.
//!
//! The repository always requires both `universe_id` and `user_id`; request
//! identifiers alone are never treated as authorization. Regulatory evidence
//! is append-only in PostgreSQL, and raw export delivery tokens are returned
//! once while only their SHA-256 digests are persisted.

use std::fmt;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use tokio_postgres::{types::Json, types::ToSql, Transaction};
use zeroize::{Zeroize, Zeroizing};

use crate::{CommunicationEvidenceKey, Database};

/// Subject-access export inventory. Every category is read from the durable
/// PostgreSQL source of truth when that table exists. Credential material is
/// intentionally excluded: password hashes, session/reset/verification
/// tokens, TOTP secrets, backup codes, encrypted request payloads, export
/// token digests, and worker lease internals never enter an export.
pub const PRIVACY_EXPORT_DATA_INVENTORY: &[&str] = &[
    "profile",
    "planets_resources_buildings_and_inventory",
    "research",
    "construction_research_and_shipyard_queues",
    "fleets",
    "messages_private_and_alliance_chat",
    "alliance_membership_and_authored_content",
    "scores_achievements_badges_and_rewards",
    "notifications_blocks_and_restrictions",
    "purchases_and_enhanced_purchases",
    "security_admin_and_activity_history",
    "account_security_metadata",
    "privacy_requests_decisions_consents_and_preferences",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacyRequestType {
    Export,
    Correction,
    Restriction,
    Erasure,
}

impl PrivacyRequestType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Export => "export",
            Self::Correction => "correction",
            Self::Restriction => "restriction",
            Self::Erasure => "erasure",
        }
    }

    fn parse(value: &str) -> Result<Self, PrivacyError> {
        match value {
            "export" => Ok(Self::Export),
            "correction" => Ok(Self::Correction),
            "restriction" => Ok(Self::Restriction),
            "erasure" => Ok(Self::Erasure),
            _ => Err(PrivacyError::Database(format!(
                "database returned unsupported privacy request type {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacyRequestStatus {
    Pending,
    CoolingOff,
    InReview,
    Approved,
    Queued,
    Processing,
    Completed,
    Cancelled,
    Rejected,
    Failed,
    BlockedLegalHold,
}

impl PrivacyRequestStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::CoolingOff => "cooling_off",
            Self::InReview => "in_review",
            Self::Approved => "approved",
            Self::Queued => "queued",
            Self::Processing => "processing",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Rejected => "rejected",
            Self::Failed => "failed",
            Self::BlockedLegalHold => "blocked_legal_hold",
        }
    }

    fn parse(value: &str) -> Result<Self, PrivacyError> {
        match value {
            "pending" => Ok(Self::Pending),
            "cooling_off" => Ok(Self::CoolingOff),
            "in_review" => Ok(Self::InReview),
            "approved" => Ok(Self::Approved),
            "queued" => Ok(Self::Queued),
            "processing" => Ok(Self::Processing),
            "completed" => Ok(Self::Completed),
            "cancelled" => Ok(Self::Cancelled),
            "rejected" => Ok(Self::Rejected),
            "failed" => Ok(Self::Failed),
            "blocked_legal_hold" => Ok(Self::BlockedLegalHold),
            _ => Err(PrivacyError::Database(format!(
                "database returned unsupported privacy request status {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptedPrivacyPayload {
    pub ciphertext: Vec<u8>,
    pub key_id: String,
    pub nonce: [u8; 12],
    pub plaintext_sha256: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyRequestCreateInput {
    pub universe_id: i64,
    pub user_id: i32,
    pub request_type: PrivacyRequestType,
    pub idempotency_key: String,
    pub request_source: String,
    pub requester_ip_digest: Option<[u8; 32]>,
    pub encrypted_payload: Option<EncryptedPrivacyPayload>,
    /// Erasure cooling-off period. Production callers should normally use the
    /// policy default; tests and policy adapters may supply another value.
    pub erasure_cooling_off_seconds: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyRequestRow {
    pub id: i32,
    pub universe_id: i64,
    pub user_id: i32,
    pub request_type: PrivacyRequestType,
    pub status: PrivacyRequestStatus,
    pub idempotency_key: String,
    pub requested_at_unix: i64,
    pub cooling_off_until_unix: Option<i64>,
    pub completed_at_unix: Option<i64>,
    pub cancelled_at_unix: Option<i64>,
    pub legal_hold_active: bool,
    pub retention_until_unix: i64,
    pub version: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsentStatus {
    Granted,
    Denied,
    Withdrawn,
}

impl ConsentStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Granted => "granted",
            Self::Denied => "denied",
            Self::Withdrawn => "withdrawn",
        }
    }

    fn parse(value: &str) -> Result<Self, PrivacyError> {
        match value {
            "granted" => Ok(Self::Granted),
            "denied" => Ok(Self::Denied),
            "withdrawn" => Ok(Self::Withdrawn),
            _ => Err(PrivacyError::Database(format!(
                "database returned unsupported consent status {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsentUpdate {
    pub universe_id: i64,
    pub user_id: i32,
    pub purpose: String,
    pub channel: String,
    pub status: ConsentStatus,
    pub lawful_basis: String,
    pub policy_version: String,
    pub proof_digest: Option<[u8; 32]>,
    pub expires_at_unix: Option<i64>,
    pub changed_by_user_id: i32,
    pub actor_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommunicationPreferenceUpdate {
    pub universe_id: i64,
    pub user_id: i32,
    pub channel: String,
    pub category: String,
    pub enabled: bool,
    pub changed_by_user_id: i32,
    pub actor_type: String,
}

pub const PRIVACY_COMMUNICATION_CHANNELS: &[&str] = &["email", "in_app", "push", "sms"];
pub const PRIVACY_COMMUNICATION_CATEGORIES: &[&str] = &[
    "marketing",
    "product_updates",
    "gameplay_digest",
    "security",
    "transactional",
];

/// Security and transactional messages are required to operate and protect an
/// account. Self-service controls may describe them, but may never suppress
/// them, including when an older row was persisted with `enabled = FALSE`.
pub fn privacy_communication_category_is_essential(category: &str) -> bool {
    matches!(category, "security" | "transactional")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyExportAvailability {
    pub ready: bool,
    pub expired: bool,
    pub expires_at_unix: i64,
    pub plaintext_size: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyRequestSummary {
    pub request: PrivacyRequestRow,
    pub export: Option<PrivacyExportAvailability>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyRequestEventRow {
    pub id: i64,
    pub event_type: String,
    pub from_status: Option<PrivacyRequestStatus>,
    pub to_status: PrivacyRequestStatus,
    pub actor_type: String,
    pub reason_code: Option<String>,
    pub request_version: i64,
    pub created_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyRequestDetail {
    pub request: PrivacyRequestRow,
    pub timeline: Vec<PrivacyRequestEventRow>,
    pub export: Option<PrivacyExportAvailability>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyConsentRow {
    pub purpose: String,
    pub channel: String,
    pub status: ConsentStatus,
    pub lawful_basis: String,
    pub policy_version: String,
    pub collected_at_unix: i64,
    pub expires_at_unix: Option<i64>,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommunicationPreferenceRow {
    pub channel: String,
    pub category: String,
    pub enabled: bool,
    pub explicitly_configured: bool,
    pub effective_allowed: bool,
    pub essential: bool,
    pub marketing_consent_current: bool,
    pub suppressed_by_restriction: bool,
    pub updated_at_unix: Option<i64>,
    pub version: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacyAdminDecision {
    Approve,
    Reject,
    ApplyLegalHold,
    ReleaseLegalHold,
    CompleteCorrection,
    CompleteErasure,
}

impl PrivacyAdminDecision {
    fn as_str(self) -> &'static str {
        match self {
            Self::Approve => "approve",
            Self::Reject => "reject",
            Self::ApplyLegalHold => "apply_legal_hold",
            Self::ReleaseLegalHold => "release_legal_hold",
            Self::CompleteCorrection => "complete_correction",
            Self::CompleteErasure => "complete_erasure",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyAdminDecisionInput {
    pub universe_id: i64,
    pub request_id: i32,
    pub admin_user_id: i32,
    pub decision: PrivacyAdminDecision,
    pub reason_code: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyAdminRequestFilter {
    pub universe_id: i64,
    pub request_type: Option<PrivacyRequestType>,
    pub status: Option<PrivacyRequestStatus>,
    pub user_id: Option<i32>,
    pub before_request_id: Option<i32>,
    pub limit: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyAdminDecisionRow {
    pub id: i64,
    pub admin_user_id: i32,
    pub decision: String,
    pub reason_code: String,
    pub decided_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyExecutionEventRow {
    pub id: i64,
    pub action: String,
    pub actor_type: String,
    pub actor_user_id: Option<i32>,
    pub reason_code: String,
    pub field_names: Vec<String>,
    pub created_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyAdminRequestDetail {
    pub request: PrivacyRequestRow,
    pub timeline: Vec<PrivacyRequestEventRow>,
    pub decisions: Vec<PrivacyAdminDecisionRow>,
    pub executions: Vec<PrivacyExecutionEventRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyCorrectionPatch {
    pub username: Option<String>,
    pub email: Option<String>,
    /// `None` leaves the phone untouched; `Some(None)` clears it.
    pub phone_number: Option<Option<String>>,
}

impl PrivacyCorrectionPatch {
    pub fn field_names(&self) -> Vec<String> {
        let mut fields = Vec::with_capacity(3);
        if self.username.is_some() {
            fields.push("username".to_string());
        }
        if self.email.is_some() {
            fields.push("email".to_string());
        }
        if self.phone_number.is_some() {
            fields.push("phone_number".to_string());
        }
        fields
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyErasureResult {
    pub already_completed: bool,
    pub credentials_deleted: u64,
    pub sessions_deleted: u64,
    pub personal_content_deleted: u64,
    pub contact_evidence_redacted: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyJob {
    pub id: i64,
    pub request_id: i32,
    pub universe_id: i64,
    pub user_id: i32,
    pub event_type: String,
    pub attempt_count: i32,
    pub lease_expires_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedExportArtifact {
    pub ciphertext: Vec<u8>,
    pub encryption_key_id: String,
    pub encryption_nonce: [u8; 12],
    pub plaintext_sha256: [u8; 32],
    pub plaintext_size: i64,
    pub expires_in_seconds: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDeliveryGrant {
    /// Returned once. This value is never persisted by the repository.
    pub token: String,
    pub expires_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDownload {
    pub ciphertext: Vec<u8>,
    pub encryption_key_id: String,
    pub encryption_nonce: [u8; 12],
    pub plaintext_sha256: [u8; 32],
    pub plaintext_size: i64,
    pub format_version: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacyAuthGuard {
    Allowed,
    StaleEpoch,
    Restricted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrivacyRetentionResult {
    pub artifacts_purged: u64,
    pub request_payloads_redacted: u64,
    pub outbox_rows_deleted: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrivacyRetentionAudit {
    pub universe_id: Option<i64>,
    pub admin_user_id: Option<i32>,
    pub communication_evidence_redacted: u64,
    pub communication_events_deleted: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrivacyError {
    InvalidInput(&'static str),
    NotFound,
    Forbidden,
    Conflict(&'static str),
    CoolingOff,
    LegalHold,
    LeaseLost,
    DeliveryDenied,
    Database(String),
}

impl fmt::Display for PrivacyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(formatter, "invalid privacy input: {message}"),
            Self::NotFound => formatter.write_str("privacy record was not found"),
            Self::Forbidden => formatter.write_str("privacy operation is forbidden"),
            Self::Conflict(message) => write!(formatter, "privacy conflict: {message}"),
            Self::CoolingOff => formatter.write_str("privacy cooling-off period has not elapsed"),
            Self::LegalHold => formatter.write_str("privacy request is under legal hold"),
            Self::LeaseLost => formatter.write_str("privacy worker lease was lost"),
            Self::DeliveryDenied => formatter.write_str("export delivery is unavailable"),
            Self::Database(message) => write!(formatter, "privacy database error: {message}"),
        }
    }
}

impl std::error::Error for PrivacyError {}

impl Database {
    pub async fn privacy_repository_ready(&self) -> Result<(), PrivacyError> {
        let client = self.pool.get().await.map_err(database_error)?;
        let ready = client
            .query_one(
                "SELECT
                    to_regclass('public.gdpr_requests') IS NOT NULL
                    AND to_regclass('public.privacy_request_events') IS NOT NULL
                    AND to_regclass('public.privacy_admin_decisions') IS NOT NULL
                    AND to_regclass('public.privacy_consents') IS NOT NULL
                    AND to_regclass('public.privacy_communication_preferences') IS NOT NULL
                    AND to_regclass('public.privacy_outbox') IS NOT NULL
                    AND to_regclass('public.privacy_export_artifacts') IS NOT NULL
                    AND to_regclass('public.privacy_correction_executions') IS NOT NULL
                    AND to_regclass('public.privacy_erasure_executions') IS NOT NULL
                    AND to_regclass('public.privacy_execution_events') IS NOT NULL
                    AND to_regclass('public.privacy_retention_runs') IS NOT NULL
                    AND to_regclass('public.idx_gdpr_requests_idempotency') IS NOT NULL
                    AND to_regclass('public.idx_privacy_outbox_claim') IS NOT NULL
                    AND EXISTS (
                        SELECT 1 FROM information_schema.columns
                        WHERE table_schema = 'public' AND table_name = 'users'
                          AND column_name = 'auth_epoch' AND is_nullable = 'NO'
                    )
                    AND EXISTS (
                        SELECT 1 FROM information_schema.columns
                        WHERE table_schema = 'public' AND table_name = 'users'
                          AND column_name = 'privacy_erased_at'
                    )
                    AND EXISTS (
                        SELECT 1 FROM information_schema.columns
                        WHERE table_schema = 'public' AND table_name = 'gdpr_requests'
                          AND column_name = 'correction_applied_at'
                    )
                    AND EXISTS (
                        SELECT 1 FROM pg_trigger
                        WHERE tgrelid = 'privacy_request_events'::regclass
                          AND tgname = 'privacy_request_events_immutable'
                          AND tgenabled <> 'D'
                    ) AS ready",
                &[],
            )
            .await
            .map_err(database_error)?
            .get::<_, bool>("ready");
        if ready {
            Ok(())
        } else {
            Err(PrivacyError::Database(
                "privacy schema is incomplete; run ordered database migrations".to_string(),
            ))
        }
    }

    pub async fn create_privacy_request(
        &self,
        input: PrivacyRequestCreateInput,
    ) -> Result<PrivacyRequestRow, PrivacyError> {
        validate_request_input(&input)?;
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_actor(&transaction, "user", Some(input.user_id), "request_created").await?;

        let initial_status = match input.request_type {
            PrivacyRequestType::Export | PrivacyRequestType::Restriction => "queued",
            PrivacyRequestType::Correction => "in_review",
            PrivacyRequestType::Erasure => "cooling_off",
        };
        let cooling_seconds = input
            .erasure_cooling_off_seconds
            .unwrap_or(14 * 24 * 60 * 60);
        if !(0..=90 * 24 * 60 * 60).contains(&cooling_seconds) {
            return Err(PrivacyError::InvalidInput(
                "erasure cooling-off period must be between zero and 90 days",
            ));
        }

        let (ciphertext, key_id, nonce, payload_digest) = match &input.encrypted_payload {
            Some(payload) => (
                Some(payload.ciphertext.as_slice()),
                Some(payload.key_id.as_str()),
                Some(payload.nonce.as_slice()),
                Some(payload.plaintext_sha256.as_slice()),
            ),
            None => (None, None, None, None),
        };
        let ip_digest = input
            .requester_ip_digest
            .as_ref()
            .map(|digest| digest.as_slice());

        // Serialize request creation per tenant subject. The exact
        // idempotency replay is resolved before the active-request bound so a
        // retried network request always receives its original durable row.
        transaction
            .query_opt(
                "SELECT id FROM users
                 WHERE universe_id = $1 AND id = $2
                 FOR UPDATE",
                &[&input.universe_id, &input.user_id],
            )
            .await
            .map_err(database_error)?
            .ok_or(PrivacyError::NotFound)?;
        if let Some(row) = transaction
            .query_opt(
                "SELECT id, universe_id, user_id, request_type, status,
                    idempotency_key, EXTRACT(EPOCH FROM requested_at)::BIGINT,
                    EXTRACT(EPOCH FROM cooling_off_until)::BIGINT,
                    EXTRACT(EPOCH FROM completed_at)::BIGINT,
                    EXTRACT(EPOCH FROM cancelled_at)::BIGINT,
                    legal_hold_active, EXTRACT(EPOCH FROM retention_until)::BIGINT,
                    version, payload_sha256
                 FROM gdpr_requests
                 WHERE universe_id = $1 AND user_id = $2 AND idempotency_key = $3",
                &[
                    &input.universe_id,
                    &input.user_id,
                    &input.idempotency_key.trim(),
                ],
            )
            .await
            .map_err(database_error)?
        {
            let existing_type: String = row.get("request_type");
            let existing_digest: Option<Vec<u8>> = row.get("payload_sha256");
            let expected_digest = input
                .encrypted_payload
                .as_ref()
                .map(|payload| payload.plaintext_sha256.to_vec());
            if existing_type != input.request_type.as_str() || existing_digest != expected_digest {
                return Err(PrivacyError::Conflict(
                    "idempotency key was already used for different request content",
                ));
            }
            let request = map_request_row(&row)?;
            transaction.commit().await.map_err(database_error)?;
            return Ok(request);
        }
        let active_same_type = transaction
            .query_one(
                "SELECT EXISTS (
                    SELECT 1 FROM gdpr_requests
                    WHERE universe_id = $1 AND user_id = $2 AND request_type = $3
                      AND status NOT IN ('completed', 'cancelled', 'rejected')
                 ) AS active",
                &[
                    &input.universe_id,
                    &input.user_id,
                    &input.request_type.as_str(),
                ],
            )
            .await
            .map_err(database_error)?
            .get::<_, bool>("active");
        if active_same_type {
            return Err(PrivacyError::Conflict(
                "an active request of this type already exists",
            ));
        }

        let inserted = transaction
            .query_opt(
                "INSERT INTO gdpr_requests (
                    universe_id, user_id, request_type, status, idempotency_key,
                    request_source, requester_ip_digest, request_payload_ciphertext,
                    payload_key_id, payload_nonce, payload_sha256,
                    cooling_off_until, retention_until
                 ) VALUES (
                    $1, $2, $3::TEXT, $4::TEXT, $5, $6, $7, $8, $9, $10, $11,
                    CASE WHEN $3::TEXT = 'erasure'
                        THEN now() + ($12::BIGINT * interval '1 second') ELSE NULL END,
                    now() + interval '6 years'
                 )
                 ON CONFLICT (universe_id, user_id, idempotency_key) DO NOTHING
                 RETURNING id, universe_id, user_id, request_type, status,
                    idempotency_key, EXTRACT(EPOCH FROM requested_at)::BIGINT,
                    EXTRACT(EPOCH FROM cooling_off_until)::BIGINT,
                    EXTRACT(EPOCH FROM completed_at)::BIGINT,
                    EXTRACT(EPOCH FROM cancelled_at)::BIGINT,
                    legal_hold_active, EXTRACT(EPOCH FROM retention_until)::BIGINT,
                    version",
                &[
                    &input.universe_id,
                    &input.user_id,
                    &input.request_type.as_str(),
                    &initial_status,
                    &input.idempotency_key.trim(),
                    &input.request_source.trim(),
                    &ip_digest,
                    &ciphertext,
                    &key_id,
                    &nonce,
                    &payload_digest,
                    &cooling_seconds,
                ],
            )
            .await
            .map_err(database_error)?;

        let request = if let Some(row) = inserted {
            map_request_row(&row)?
        } else {
            let row = transaction
                .query_opt(
                    "SELECT id, universe_id, user_id, request_type, status,
                        idempotency_key, EXTRACT(EPOCH FROM requested_at)::BIGINT,
                        EXTRACT(EPOCH FROM cooling_off_until)::BIGINT,
                        EXTRACT(EPOCH FROM completed_at)::BIGINT,
                        EXTRACT(EPOCH FROM cancelled_at)::BIGINT,
                        legal_hold_active, EXTRACT(EPOCH FROM retention_until)::BIGINT,
                        version, payload_sha256
                     FROM gdpr_requests
                     WHERE universe_id = $1 AND user_id = $2 AND idempotency_key = $3",
                    &[
                        &input.universe_id,
                        &input.user_id,
                        &input.idempotency_key.trim(),
                    ],
                )
                .await
                .map_err(database_error)?
                .ok_or(PrivacyError::Conflict("idempotency race"))?;
            let existing_type: String = row.get("request_type");
            let existing_digest: Option<Vec<u8>> = row.get("payload_sha256");
            let expected_digest = input
                .encrypted_payload
                .as_ref()
                .map(|payload| payload.plaintext_sha256.to_vec());
            if existing_type != input.request_type.as_str() || existing_digest != expected_digest {
                return Err(PrivacyError::Conflict(
                    "idempotency key was already used for different request content",
                ));
            }
            map_request_row(&row)?
        };

        if let Some(event_type) = initial_outbox_event(input.request_type) {
            let dedupe_key = format!("privacy-request:{}:{event_type}", request.id);
            transaction
                .execute(
                    "INSERT INTO privacy_outbox (
                        request_id, universe_id, user_id, event_type, dedupe_key
                     ) VALUES ($1, $2, $3, $4, $5)
                     ON CONFLICT (dedupe_key) DO NOTHING",
                    &[
                        &request.id,
                        &request.universe_id,
                        &request.user_id,
                        &event_type,
                        &dedupe_key,
                    ],
                )
                .await
                .map_err(database_error)?;
        }
        transaction.commit().await.map_err(database_error)?;
        Ok(request)
    }

    pub async fn privacy_request_for_owner(
        &self,
        universe_id: i64,
        user_id: i32,
        request_id: i32,
    ) -> Result<Option<PrivacyRequestRow>, PrivacyError> {
        let client = self.pool.get().await.map_err(database_error)?;
        let row = client
            .query_opt(
                "SELECT id, universe_id, user_id, request_type, status,
                    idempotency_key, EXTRACT(EPOCH FROM requested_at)::BIGINT,
                    EXTRACT(EPOCH FROM cooling_off_until)::BIGINT,
                    EXTRACT(EPOCH FROM completed_at)::BIGINT,
                    EXTRACT(EPOCH FROM cancelled_at)::BIGINT,
                    legal_hold_active, EXTRACT(EPOCH FROM retention_until)::BIGINT,
                    version
                 FROM gdpr_requests
                 WHERE id = $1 AND universe_id = $2 AND user_id = $3",
                &[&request_id, &universe_id, &user_id],
            )
            .await
            .map_err(database_error)?;
        row.map(|row| map_request_row(&row)).transpose()
    }

    pub async fn list_privacy_requests_for_owner(
        &self,
        universe_id: i64,
        user_id: i32,
        limit: i64,
    ) -> Result<Vec<PrivacyRequestSummary>, PrivacyError> {
        validate_owner(universe_id, user_id)?;
        if !(1..=100).contains(&limit) {
            return Err(PrivacyError::InvalidInput(
                "privacy request list limit is invalid",
            ));
        }
        let client = self.pool.get().await.map_err(database_error)?;
        let rows = client
            .query(
                "SELECT requests.id, requests.universe_id, requests.user_id,
                    requests.request_type, requests.status, requests.idempotency_key,
                    EXTRACT(EPOCH FROM requests.requested_at)::BIGINT,
                    EXTRACT(EPOCH FROM requests.cooling_off_until)::BIGINT,
                    EXTRACT(EPOCH FROM requests.completed_at)::BIGINT,
                    EXTRACT(EPOCH FROM requests.cancelled_at)::BIGINT,
                    requests.legal_hold_active,
                    EXTRACT(EPOCH FROM requests.retention_until)::BIGINT,
                    requests.version,
                    artifacts.id IS NOT NULL AS export_prepared,
                    artifacts.ciphertext IS NOT NULL
                        AND artifacts.purged_at IS NULL
                        AND artifacts.expires_at > now() AS export_ready,
                    artifacts.expires_at <= now() OR artifacts.purged_at IS NOT NULL
                        AS export_expired,
                    EXTRACT(EPOCH FROM artifacts.expires_at)::BIGINT AS export_expires_at,
                    artifacts.plaintext_size AS export_plaintext_size
                 FROM gdpr_requests AS requests
                 LEFT JOIN privacy_export_artifacts AS artifacts
                   ON artifacts.request_id = requests.id
                  AND artifacts.universe_id = requests.universe_id
                  AND artifacts.user_id = requests.user_id
                 WHERE requests.universe_id = $1 AND requests.user_id = $2
                 ORDER BY requests.requested_at DESC, requests.id DESC
                 LIMIT $3",
                &[&universe_id, &user_id, &limit],
            )
            .await
            .map_err(database_error)?;
        rows.iter().map(map_request_summary).collect()
    }

    pub async fn privacy_request_detail_for_owner(
        &self,
        universe_id: i64,
        user_id: i32,
        request_id: i32,
    ) -> Result<Option<PrivacyRequestDetail>, PrivacyError> {
        validate_owner(universe_id, user_id)?;
        if request_id <= 0 {
            return Err(PrivacyError::InvalidInput("privacy request id is invalid"));
        }
        let client = self.pool.get().await.map_err(database_error)?;
        let Some(row) = client
            .query_opt(
                "SELECT requests.id, requests.universe_id, requests.user_id,
                    requests.request_type, requests.status, requests.idempotency_key,
                    EXTRACT(EPOCH FROM requests.requested_at)::BIGINT,
                    EXTRACT(EPOCH FROM requests.cooling_off_until)::BIGINT,
                    EXTRACT(EPOCH FROM requests.completed_at)::BIGINT,
                    EXTRACT(EPOCH FROM requests.cancelled_at)::BIGINT,
                    requests.legal_hold_active,
                    EXTRACT(EPOCH FROM requests.retention_until)::BIGINT,
                    requests.version,
                    artifacts.id IS NOT NULL AS export_prepared,
                    artifacts.ciphertext IS NOT NULL
                        AND artifacts.purged_at IS NULL
                        AND artifacts.expires_at > now() AS export_ready,
                    artifacts.expires_at <= now() OR artifacts.purged_at IS NOT NULL
                        AS export_expired,
                    EXTRACT(EPOCH FROM artifacts.expires_at)::BIGINT AS export_expires_at,
                    artifacts.plaintext_size AS export_plaintext_size
                 FROM gdpr_requests AS requests
                 LEFT JOIN privacy_export_artifacts AS artifacts
                   ON artifacts.request_id = requests.id
                  AND artifacts.universe_id = requests.universe_id
                  AND artifacts.user_id = requests.user_id
                 WHERE requests.id = $1
                   AND requests.universe_id = $2
                   AND requests.user_id = $3",
                &[&request_id, &universe_id, &user_id],
            )
            .await
            .map_err(database_error)?
        else {
            return Ok(None);
        };
        let summary = map_request_summary(&row)?;
        let timeline = client
            .query(
                "SELECT id, event_type, from_status, to_status, actor_type,
                    reason_code, request_version,
                    EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at_unix
                 FROM privacy_request_events
                 WHERE request_id = $1 AND universe_id = $2 AND user_id = $3
                 ORDER BY id",
                &[&request_id, &universe_id, &user_id],
            )
            .await
            .map_err(database_error)?
            .iter()
            .map(map_request_event)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(PrivacyRequestDetail {
            request: summary.request,
            timeline,
            export: summary.export,
        }))
    }

    pub async fn list_privacy_consents_for_owner(
        &self,
        universe_id: i64,
        user_id: i32,
    ) -> Result<Vec<PrivacyConsentRow>, PrivacyError> {
        validate_owner(universe_id, user_id)?;
        let client = self.pool.get().await.map_err(database_error)?;
        let rows = client
            .query(
                "SELECT purpose, channel, status, lawful_basis, policy_version,
                    EXTRACT(EPOCH FROM collected_at)::BIGINT AS collected_at_unix,
                    EXTRACT(EPOCH FROM expires_at)::BIGINT AS expires_at_unix,
                    version
                 FROM privacy_consents
                 WHERE universe_id = $1 AND user_id = $2
                 ORDER BY purpose, channel",
                &[&universe_id, &user_id],
            )
            .await
            .map_err(database_error)?;
        rows.iter().map(map_consent_row).collect()
    }

    pub async fn communication_preferences_for_owner(
        &self,
        universe_id: i64,
        user_id: i32,
    ) -> Result<Vec<CommunicationPreferenceRow>, PrivacyError> {
        validate_owner(universe_id, user_id)?;
        let client = self.pool.get().await.map_err(database_error)?;
        let rows = client
            .query(
                "WITH matrix(channel, category) AS (
                    VALUES
                        ('email', 'marketing'),
                        ('email', 'product_updates'),
                        ('email', 'gameplay_digest'),
                        ('email', 'security'),
                        ('email', 'transactional'),
                        ('in_app', 'marketing'),
                        ('in_app', 'product_updates'),
                        ('in_app', 'gameplay_digest'),
                        ('in_app', 'security'),
                        ('in_app', 'transactional'),
                        ('push', 'marketing'),
                        ('push', 'product_updates'),
                        ('push', 'gameplay_digest'),
                        ('push', 'security'),
                        ('push', 'transactional'),
                        ('sms', 'marketing'),
                        ('sms', 'product_updates'),
                        ('sms', 'gameplay_digest'),
                        ('sms', 'security'),
                        ('sms', 'transactional')
                 ), state AS (
                    SELECT matrix.channel, matrix.category,
                        preferences.enabled AS configured_enabled,
                        preferences.version,
                        EXTRACT(EPOCH FROM preferences.updated_at)::BIGINT AS updated_at_unix,
                        matrix.category IN ('security', 'transactional') AS essential,
                        users.privacy_restriction_active OR users.privacy_erasure_pending
                            AS restricted,
                        COALESCE(
                            (
                                SELECT consents.status = 'granted'
                                   AND consents.lawful_basis = 'consent'
                                   AND (consents.expires_at IS NULL OR consents.expires_at > now())
                                FROM privacy_consents AS consents
                                WHERE consents.universe_id = users.universe_id
                                  AND consents.user_id = users.id
                                  AND consents.purpose = 'marketing'
                                  AND consents.channel = matrix.channel
                            ),
                            (
                                SELECT consents.status = 'granted'
                                   AND consents.lawful_basis = 'consent'
                                   AND (consents.expires_at IS NULL OR consents.expires_at > now())
                                FROM privacy_consents AS consents
                                WHERE consents.universe_id = users.universe_id
                                  AND consents.user_id = users.id
                                  AND consents.purpose = 'marketing'
                                  AND consents.channel = 'all'
                            ),
                            FALSE
                        ) AS marketing_consent_current
                    FROM users CROSS JOIN matrix
                    LEFT JOIN privacy_communication_preferences AS preferences
                      ON preferences.universe_id = users.universe_id
                     AND preferences.user_id = users.id
                     AND preferences.channel = matrix.channel
                     AND preferences.category = matrix.category
                    WHERE users.universe_id = $1 AND users.id = $2
                 )
                 SELECT channel, category,
                    CASE WHEN essential THEN TRUE
                         ELSE COALESCE(configured_enabled, FALSE) END AS enabled,
                    configured_enabled IS NOT NULL AS explicitly_configured,
                    CASE
                        WHEN essential THEN TRUE
                        WHEN NOT essential AND restricted THEN FALSE
                        WHEN NOT COALESCE(configured_enabled, FALSE) THEN FALSE
                        WHEN category = 'marketing' AND NOT marketing_consent_current THEN FALSE
                        ELSE TRUE
                    END AS effective_allowed,
                    essential,
                    marketing_consent_current,
                    NOT essential AND restricted AS suppressed_by_restriction,
                    updated_at_unix,
                    COALESCE(version, 0) AS version
                 FROM state
                 ORDER BY array_position(ARRAY['email', 'in_app', 'push', 'sms'], channel),
                    array_position(ARRAY['marketing', 'product_updates', 'gameplay_digest',
                                         'security', 'transactional'], category)",
                &[&universe_id, &user_id],
            )
            .await
            .map_err(database_error)?;
        if rows.is_empty() {
            return Err(PrivacyError::NotFound);
        }
        Ok(rows.iter().map(map_communication_row).collect())
    }

    pub async fn cancel_privacy_request(
        &self,
        universe_id: i64,
        user_id: i32,
        request_id: i32,
        reason_code: &str,
    ) -> Result<PrivacyRequestRow, PrivacyError> {
        self.cancel_privacy_request_internal(universe_id, user_id, request_id, None, reason_code)
            .await
    }

    pub async fn cancel_privacy_request_if_version(
        &self,
        universe_id: i64,
        user_id: i32,
        request_id: i32,
        expected_version: i64,
        reason_code: &str,
    ) -> Result<PrivacyRequestRow, PrivacyError> {
        if expected_version <= 0 {
            return Err(PrivacyError::InvalidInput(
                "privacy request version is invalid",
            ));
        }
        self.cancel_privacy_request_internal(
            universe_id,
            user_id,
            request_id,
            Some(expected_version),
            reason_code,
        )
        .await
    }

    async fn cancel_privacy_request_internal(
        &self,
        universe_id: i64,
        user_id: i32,
        request_id: i32,
        expected_version: Option<i64>,
        reason_code: &str,
    ) -> Result<PrivacyRequestRow, PrivacyError> {
        validate_reason_code(reason_code)?;
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_actor(&transaction, "user", Some(user_id), reason_code).await?;
        let current = transaction
            .query_opt(
                "SELECT status, legal_hold_active, version
                 FROM gdpr_requests
                 WHERE id = $1 AND universe_id = $2 AND user_id = $3
                 FOR UPDATE",
                &[&request_id, &universe_id, &user_id],
            )
            .await
            .map_err(database_error)?
            .ok_or(PrivacyError::NotFound)?;
        let status: String = current.get("status");
        if expected_version.is_some_and(|version| current.get::<_, i64>("version") != version) {
            return Err(PrivacyError::Conflict("privacy request version changed"));
        }
        if current.get::<_, bool>("legal_hold_active") {
            return Err(PrivacyError::LegalHold);
        }
        let irreversible_job_completed = transaction
            .query_one(
                "SELECT EXISTS (
                    SELECT 1 FROM privacy_outbox
                    WHERE request_id = $1 AND universe_id = $2 AND user_id = $3
                      AND status = 'delivered'
                 ) AS completed",
                &[&request_id, &universe_id, &user_id],
            )
            .await
            .map_err(database_error)?
            .get::<_, bool>("completed");
        if irreversible_job_completed {
            return Err(PrivacyError::Conflict(
                "request side effects have already completed",
            ));
        }
        if !matches!(
            status.as_str(),
            "pending"
                | "cooling_off"
                | "in_review"
                | "approved"
                | "queued"
                | "processing"
                | "failed"
        ) {
            return Err(PrivacyError::Conflict("request can no longer be cancelled"));
        }
        // The canonical lifecycle permits processing -> queued and queued ->
        // cancelled, but not a direct processing -> cancelled edge. Both
        // transitions occur under this request lock and transaction, so a
        // claimed worker cannot observe or act on the intermediate state.
        if status == "processing" {
            transaction
                .execute(
                    "UPDATE gdpr_requests SET status = 'queued'
                     WHERE id = $1 AND universe_id = $2 AND user_id = $3
                       AND status = 'processing'",
                    &[&request_id, &universe_id, &user_id],
                )
                .await
                .map_err(database_error)?;
        }
        let row = transaction
            .query_one(
                "UPDATE gdpr_requests
                 SET status = 'cancelled', cancelled_at = now(),
                     cancelled_by_user_id = $3, cancellation_reason_code = $4
                 WHERE id = $1 AND universe_id = $2 AND user_id = $3
                 RETURNING id, universe_id, user_id, request_type, status,
                    idempotency_key, EXTRACT(EPOCH FROM requested_at)::BIGINT,
                    EXTRACT(EPOCH FROM cooling_off_until)::BIGINT,
                    EXTRACT(EPOCH FROM completed_at)::BIGINT,
                    EXTRACT(EPOCH FROM cancelled_at)::BIGINT,
                    legal_hold_active, EXTRACT(EPOCH FROM retention_until)::BIGINT,
                    version",
                &[&request_id, &universe_id, &user_id, &reason_code],
            )
            .await
            .map_err(database_error)?;
        cancel_active_jobs(&transaction, request_id, universe_id, user_id, reason_code).await?;
        transaction.commit().await.map_err(database_error)?;
        map_request_row(&row)
    }

    pub async fn set_privacy_consent(&self, input: ConsentUpdate) -> Result<(), PrivacyError> {
        self.set_privacy_consent_internal(input, None).await
    }

    pub async fn set_privacy_consent_if_version(
        &self,
        input: ConsentUpdate,
        expected_version: i64,
    ) -> Result<(), PrivacyError> {
        if expected_version < 0 {
            return Err(PrivacyError::InvalidInput("consent version is invalid"));
        }
        self.set_privacy_consent_internal(input, Some(expected_version))
            .await
    }

    async fn set_privacy_consent_internal(
        &self,
        input: ConsentUpdate,
        expected_version: Option<i64>,
    ) -> Result<(), PrivacyError> {
        validate_consent(&input)?;
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_actor(
            &transaction,
            input.actor_type.as_str(),
            Some(input.changed_by_user_id),
            "consent_updated",
        )
        .await?;
        let proof = input.proof_digest.as_ref().map(|value| value.as_slice());
        let consent_values: &[&(dyn tokio_postgres::types::ToSql + Sync)] = &[
            &input.universe_id,
            &input.user_id,
            &input.purpose.trim(),
            &input.channel.as_str(),
            &input.status.as_str(),
            &input.lawful_basis.as_str(),
            &input.policy_version.trim(),
            &proof,
            &input.expires_at_unix,
        ];
        let changed = match expected_version {
            None => {
                transaction
                    .query_opt(
                        "INSERT INTO privacy_consents (
                        universe_id, user_id, purpose, channel, status, lawful_basis,
                        policy_version, proof_digest, collected_at, expires_at
                     ) VALUES (
                        $1, $2, $3, $4, $5, $6, $7, $8, now(),
                        CASE WHEN $9::BIGINT IS NULL THEN NULL
                             ELSE TIMESTAMPTZ 'epoch' + ($9::BIGINT * interval '1 second') END
                     )
                     ON CONFLICT (universe_id, user_id, purpose, channel) DO UPDATE
                     SET status = EXCLUDED.status,
                         lawful_basis = EXCLUDED.lawful_basis,
                         policy_version = EXCLUDED.policy_version,
                         proof_digest = EXCLUDED.proof_digest,
                         collected_at = EXCLUDED.collected_at,
                         expires_at = EXCLUDED.expires_at
                     RETURNING version",
                        consent_values,
                    )
                    .await
            }
            Some(0) => {
                transaction
                    .query_opt(
                        "INSERT INTO privacy_consents (
                        universe_id, user_id, purpose, channel, status, lawful_basis,
                        policy_version, proof_digest, collected_at, expires_at
                     ) VALUES (
                        $1, $2, $3, $4, $5, $6, $7, $8, now(),
                        CASE WHEN $9::BIGINT IS NULL THEN NULL
                             ELSE TIMESTAMPTZ 'epoch' + ($9::BIGINT * interval '1 second') END
                     )
                     ON CONFLICT (universe_id, user_id, purpose, channel) DO NOTHING
                     RETURNING version",
                        consent_values,
                    )
                    .await
            }
            Some(version) => {
                transaction
                    .query_opt(
                        "UPDATE privacy_consents
                     SET status = $5,
                         lawful_basis = $6,
                         policy_version = $7,
                         proof_digest = $8,
                         collected_at = now(),
                         expires_at = CASE WHEN $9::BIGINT IS NULL THEN NULL
                              ELSE TIMESTAMPTZ 'epoch' + ($9::BIGINT * interval '1 second') END
                     WHERE universe_id = $1 AND user_id = $2
                       AND purpose = $3 AND channel = $4 AND version = $10
                     RETURNING version",
                        &[
                            consent_values[0],
                            consent_values[1],
                            consent_values[2],
                            consent_values[3],
                            consent_values[4],
                            consent_values[5],
                            consent_values[6],
                            consent_values[7],
                            consent_values[8],
                            &version,
                        ],
                    )
                    .await
            }
        }
        .map_err(database_error)?;
        if changed.is_none() {
            return Err(PrivacyError::Conflict("consent version changed"));
        }
        transaction.commit().await.map_err(database_error)?;
        Ok(())
    }

    pub async fn set_communication_preference(
        &self,
        input: CommunicationPreferenceUpdate,
    ) -> Result<(), PrivacyError> {
        self.set_communication_preference_internal(input, None)
            .await
    }

    pub async fn set_communication_preference_if_version(
        &self,
        input: CommunicationPreferenceUpdate,
        expected_version: i64,
    ) -> Result<(), PrivacyError> {
        if expected_version < 0 {
            return Err(PrivacyError::InvalidInput(
                "communication preference version is invalid",
            ));
        }
        self.set_communication_preference_internal(input, Some(expected_version))
            .await
    }

    async fn set_communication_preference_internal(
        &self,
        input: CommunicationPreferenceUpdate,
        expected_version: Option<i64>,
    ) -> Result<(), PrivacyError> {
        validate_communication(&input)?;
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_actor(
            &transaction,
            input.actor_type.as_str(),
            Some(input.changed_by_user_id),
            "communication_preference_updated",
        )
        .await?;
        let preference_values: &[&(dyn tokio_postgres::types::ToSql + Sync)] = &[
            &input.universe_id,
            &input.user_id,
            &input.channel.as_str(),
            &input.category.as_str(),
            &input.enabled,
        ];
        let changed = match expected_version {
            None => {
                transaction
                    .query_opt(
                        "INSERT INTO privacy_communication_preferences (
                        universe_id, user_id, channel, category, enabled
                     ) VALUES ($1, $2, $3, $4, $5)
                     ON CONFLICT (universe_id, user_id, channel, category) DO UPDATE
                     SET enabled = EXCLUDED.enabled
                     RETURNING version",
                        preference_values,
                    )
                    .await
            }
            Some(0) => {
                transaction
                    .query_opt(
                        "INSERT INTO privacy_communication_preferences (
                        universe_id, user_id, channel, category, enabled
                     ) VALUES ($1, $2, $3, $4, $5)
                     ON CONFLICT (universe_id, user_id, channel, category) DO NOTHING
                     RETURNING version",
                        preference_values,
                    )
                    .await
            }
            Some(version) => {
                transaction
                    .query_opt(
                        "UPDATE privacy_communication_preferences
                     SET enabled = $5
                     WHERE universe_id = $1 AND user_id = $2
                       AND channel = $3 AND category = $4 AND version = $6
                     RETURNING version",
                        &[
                            preference_values[0],
                            preference_values[1],
                            preference_values[2],
                            preference_values[3],
                            preference_values[4],
                            &version,
                        ],
                    )
                    .await
            }
        }
        .map_err(database_error)?;
        if changed.is_none() {
            return Err(PrivacyError::Conflict(
                "communication preference version changed",
            ));
        }
        transaction.commit().await.map_err(database_error)?;
        Ok(())
    }

    pub async fn communication_allowed(
        &self,
        universe_id: i64,
        user_id: i32,
        channel: &str,
        category: &str,
    ) -> Result<bool, PrivacyError> {
        validate_channel(channel, false)?;
        validate_category(category)?;
        let client = self.pool.get().await.map_err(database_error)?;
        let row = client
            .query_opt(
                "SELECT privacy_restriction_active, privacy_erasure_pending,
                    COALESCE((
                        SELECT enabled
                        FROM privacy_communication_preferences
                        WHERE universe_id = $1 AND user_id = $2
                          AND channel = $3 AND category = $4
                    ), $4 IN ('security', 'transactional')) AS preference_enabled,
                    COALESCE(
                        (
                            SELECT status = 'granted'
                               AND lawful_basis = 'consent'
                               AND (expires_at IS NULL OR expires_at > now())
                            FROM privacy_consents
                            WHERE universe_id = $1 AND user_id = $2
                              AND purpose = 'marketing' AND channel = $3
                        ),
                        (
                            SELECT status = 'granted'
                               AND lawful_basis = 'consent'
                               AND (expires_at IS NULL OR expires_at > now())
                            FROM privacy_consents
                            WHERE universe_id = $1 AND user_id = $2
                              AND purpose = 'marketing' AND channel = 'all'
                        ),
                        FALSE
                    ) AS marketing_consent
                 FROM users
                 WHERE universe_id = $1 AND id = $2",
                &[&universe_id, &user_id, &channel, &category],
            )
            .await
            .map_err(database_error)?
            .ok_or(PrivacyError::NotFound)?;
        let essential = privacy_communication_category_is_essential(category);
        if essential {
            return Ok(true);
        }
        if row.get::<_, bool>("privacy_restriction_active")
            || row.get::<_, bool>("privacy_erasure_pending")
        {
            return Ok(false);
        }
        if !row.get::<_, bool>("preference_enabled") {
            return Ok(false);
        }
        if category == "marketing" && !row.get::<_, bool>("marketing_consent") {
            return Ok(false);
        }
        Ok(true)
    }

    /// Validate a human administrator against the durable session and current
    /// tenant role in one database statement. JWT role claims alone are never
    /// sufficient for GDPR administration.
    pub async fn validate_privacy_admin_session(
        &self,
        account_id: &str,
        session_id: &str,
        auth_epoch: i64,
        universe_id: i64,
    ) -> Result<bool, PrivacyError> {
        let user_id = account_id
            .parse::<i32>()
            .map_err(|_| PrivacyError::Forbidden)?;
        if user_id <= 0
            || universe_id <= 0
            || auth_epoch < 0
            || !(32..=128).contains(&session_id.len())
            || session_id.trim() != session_id
        {
            return Ok(false);
        }
        let client = self.pool.get().await.map_err(database_error)?;
        let updated = client
            .execute(
                "UPDATE auth_sessions AS session
                 SET last_used_at = CASE
                    WHEN session.last_used_at <= now() - interval '60 seconds'
                    THEN now() ELSE session.last_used_at END
                 FROM users
                 WHERE session.session_id = $1
                   AND session.user_id = $2
                   AND session.universe_id = $3
                   AND session.auth_epoch_at_issue = $4
                   AND users.id = session.user_id
                   AND users.universe_id = session.universe_id
                   AND users.auth_epoch = $4
                   AND users.is_admin = TRUE
                   AND users.is_banned = FALSE
                   AND users.privacy_restriction_active = FALSE
                   AND users.privacy_erasure_pending = FALSE
                   AND users.privacy_erased_at IS NULL
                   AND session.revoked_at IS NULL
                   AND session.expires_at > now()",
                &[&session_id, &user_id, &universe_id, &auth_epoch],
            )
            .await
            .map_err(database_error)?;
        Ok(updated == 1)
    }

    pub async fn list_privacy_requests_for_admin(
        &self,
        filter: PrivacyAdminRequestFilter,
    ) -> Result<Vec<PrivacyRequestRow>, PrivacyError> {
        validate_admin_filter(&filter)?;
        let request_type = filter.request_type.map(PrivacyRequestType::as_str);
        let status = filter.status.map(PrivacyRequestStatus::as_str);
        let client = self.pool.get().await.map_err(database_error)?;
        let rows = client
            .query(
                "SELECT id, universe_id, user_id, request_type, status,
                    idempotency_key, EXTRACT(EPOCH FROM requested_at)::BIGINT,
                    EXTRACT(EPOCH FROM cooling_off_until)::BIGINT,
                    EXTRACT(EPOCH FROM completed_at)::BIGINT,
                    EXTRACT(EPOCH FROM cancelled_at)::BIGINT,
                    legal_hold_active, EXTRACT(EPOCH FROM retention_until)::BIGINT,
                    version
                 FROM gdpr_requests
                 WHERE universe_id = $1
                   AND ($2::TEXT IS NULL OR request_type = $2)
                   AND ($3::TEXT IS NULL OR status = $3)
                   AND ($4::INTEGER IS NULL OR user_id = $4)
                   AND ($5::INTEGER IS NULL OR id < $5)
                 ORDER BY id DESC
                 LIMIT $6",
                &[
                    &filter.universe_id,
                    &request_type,
                    &status,
                    &filter.user_id,
                    &filter.before_request_id,
                    &filter.limit,
                ],
            )
            .await
            .map_err(database_error)?;
        rows.iter().map(map_request_row).collect()
    }

    pub async fn privacy_request_detail_for_admin(
        &self,
        universe_id: i64,
        request_id: i32,
    ) -> Result<PrivacyAdminRequestDetail, PrivacyError> {
        if universe_id <= 0 || request_id <= 0 {
            return Err(PrivacyError::InvalidInput(
                "tenant and request ids must be positive",
            ));
        }
        let client = self.pool.get().await.map_err(database_error)?;
        let request = client
            .query_opt(
                "SELECT id, universe_id, user_id, request_type, status,
                    idempotency_key, EXTRACT(EPOCH FROM requested_at)::BIGINT,
                    EXTRACT(EPOCH FROM cooling_off_until)::BIGINT,
                    EXTRACT(EPOCH FROM completed_at)::BIGINT,
                    EXTRACT(EPOCH FROM cancelled_at)::BIGINT,
                    legal_hold_active, EXTRACT(EPOCH FROM retention_until)::BIGINT,
                    version
                 FROM gdpr_requests
                 WHERE universe_id = $1 AND id = $2",
                &[&universe_id, &request_id],
            )
            .await
            .map_err(database_error)?
            .ok_or(PrivacyError::NotFound)
            .and_then(|row| map_request_row(&row))?;
        let timeline_rows = client
            .query(
                "SELECT id, event_type, from_status, to_status, actor_type,
                    reason_code, request_version,
                    EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at_unix
                 FROM privacy_request_events
                 WHERE universe_id = $1 AND request_id = $2
                 ORDER BY id",
                &[&universe_id, &request_id],
            )
            .await
            .map_err(database_error)?;
        let decision_rows = client
            .query(
                "SELECT id, admin_user_id, decision, reason_code,
                    EXTRACT(EPOCH FROM decided_at)::BIGINT AS decided_at_unix
                 FROM privacy_admin_decisions
                 WHERE universe_id = $1 AND request_id = $2
                 ORDER BY id",
                &[&universe_id, &request_id],
            )
            .await
            .map_err(database_error)?;
        let execution_rows = client
            .query(
                "SELECT id, action, actor_type, actor_user_id, reason_code,
                    field_names, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at_unix
                 FROM privacy_execution_events
                 WHERE universe_id = $1 AND request_id = $2
                 ORDER BY id",
                &[&universe_id, &request_id],
            )
            .await
            .map_err(database_error)?;
        Ok(PrivacyAdminRequestDetail {
            request,
            timeline: timeline_rows
                .iter()
                .map(map_request_event)
                .collect::<Result<_, _>>()?,
            decisions: decision_rows
                .iter()
                .map(|row| PrivacyAdminDecisionRow {
                    id: row.get("id"),
                    admin_user_id: row.get("admin_user_id"),
                    decision: row.get("decision"),
                    reason_code: row.get("reason_code"),
                    decided_at_unix: row.get("decided_at_unix"),
                })
                .collect(),
            executions: execution_rows
                .iter()
                .map(|row| PrivacyExecutionEventRow {
                    id: row.get("id"),
                    action: row.get("action"),
                    actor_type: row.get("actor_type"),
                    actor_user_id: row.get("actor_user_id"),
                    reason_code: row.get("reason_code"),
                    field_names: row.get("field_names"),
                    created_at_unix: row.get("created_at_unix"),
                })
                .collect(),
        })
    }

    pub async fn record_privacy_admin_decision(
        &self,
        input: PrivacyAdminDecisionInput,
    ) -> Result<PrivacyRequestRow, PrivacyError> {
        self.record_privacy_admin_decision_internal(input, None)
            .await
    }

    pub async fn record_privacy_admin_decision_if_version(
        &self,
        input: PrivacyAdminDecisionInput,
        expected_version: i64,
    ) -> Result<PrivacyRequestRow, PrivacyError> {
        if expected_version <= 0 {
            return Err(PrivacyError::InvalidInput(
                "expected request version is invalid",
            ));
        }
        self.record_privacy_admin_decision_internal(input, Some(expected_version))
            .await
    }

    async fn record_privacy_admin_decision_internal(
        &self,
        input: PrivacyAdminDecisionInput,
        expected_version: Option<i64>,
    ) -> Result<PrivacyRequestRow, PrivacyError> {
        validate_reason_code(&input.reason_code)?;
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_actor(
            &transaction,
            "admin",
            Some(input.admin_user_id),
            input.reason_code.as_str(),
        )
        .await?;
        let request = transaction
            .query_opt(
                "SELECT id, universe_id, user_id, request_type, status,
                    cooling_off_until, legal_hold_active, status_before_legal_hold
                 FROM gdpr_requests
                 WHERE id = $1 AND universe_id = $2
                   AND ($3::BIGINT IS NULL OR version = $3)
                 FOR UPDATE",
                &[&input.request_id, &input.universe_id, &expected_version],
            )
            .await
            .map_err(database_error)?
            .ok_or_else(|| {
                if expected_version.is_some() {
                    PrivacyError::Conflict("privacy request version is stale")
                } else {
                    PrivacyError::NotFound
                }
            })?;
        let user_id: i32 = request.get("user_id");
        let request_type: String = request.get("request_type");
        let status: String = request.get("status");
        let legal_hold: bool = request.get("legal_hold_active");

        if legal_hold && input.decision != PrivacyAdminDecision::ReleaseLegalHold {
            return Err(PrivacyError::LegalHold);
        }
        match input.decision {
            PrivacyAdminDecision::ApplyLegalHold => {
                if matches!(status.as_str(), "completed" | "cancelled" | "rejected") {
                    return Err(PrivacyError::Conflict(
                        "a terminal request cannot receive a legal hold",
                    ));
                }
            }
            PrivacyAdminDecision::ReleaseLegalHold => {
                if !legal_hold {
                    return Err(PrivacyError::Conflict("request has no active legal hold"));
                }
            }
            PrivacyAdminDecision::Reject => {
                if status != "in_review" {
                    return Err(PrivacyError::Conflict(
                        "request is not awaiting an administrative decision",
                    ));
                }
            }
            PrivacyAdminDecision::Approve => {
                let approvable = if request_type == "erasure" {
                    matches!(status.as_str(), "cooling_off" | "in_review")
                } else {
                    status == "in_review"
                };
                if !approvable {
                    return Err(PrivacyError::Conflict(
                        "request is not awaiting an administrative decision",
                    ));
                }
            }
            PrivacyAdminDecision::CompleteCorrection | PrivacyAdminDecision::CompleteErasure => {
                return Err(PrivacyError::Forbidden);
            }
        }

        let inserted = transaction
            .execute(
                "INSERT INTO privacy_admin_decisions (
                    request_id, universe_id, user_id, admin_user_id, decision, reason_code
                 ) VALUES ($1, $2, $3, $4, $5, $6)
                 ON CONFLICT (request_id, admin_user_id, decision) DO NOTHING",
                &[
                    &input.request_id,
                    &input.universe_id,
                    &user_id,
                    &input.admin_user_id,
                    &input.decision.as_str(),
                    &input.reason_code.trim(),
                ],
            )
            .await
            .map_err(database_error)?;

        if inserted > 0 {
            match input.decision {
                PrivacyAdminDecision::ApplyLegalHold => {
                    transaction
                        .execute(
                            "UPDATE gdpr_requests
                             SET legal_hold_active = TRUE, legal_hold_at = now(),
                                 legal_hold_by_admin_id = $3,
                                 legal_hold_reason_code = $4,
                                 status_before_legal_hold = status,
                                 status = 'blocked_legal_hold'
                             WHERE id = $1 AND universe_id = $2",
                            &[
                                &input.request_id,
                                &input.universe_id,
                                &input.admin_user_id,
                                &input.reason_code.trim(),
                            ],
                        )
                        .await
                        .map_err(database_error)?;
                    invalidate_processing_jobs_for_hold(
                        &transaction,
                        input.request_id,
                        input.universe_id,
                        user_id,
                    )
                    .await?;
                }
                PrivacyAdminDecision::ReleaseLegalHold => {
                    transaction
                        .execute(
                            "UPDATE gdpr_requests
                             SET legal_hold_active = FALSE,
                                 legal_hold_released_at = now(),
                                 legal_hold_released_by_admin_id = $3,
                                 status = CASE status_before_legal_hold
                                    WHEN 'cooling_off' THEN 'cooling_off'
                                    WHEN 'approved' THEN 'approved'
                                    WHEN 'queued' THEN 'queued'
                                    WHEN 'processing' THEN 'queued'
                                    WHEN 'failed' THEN 'queued'
                                    ELSE 'in_review'
                                 END
                             WHERE id = $1 AND universe_id = $2",
                            &[&input.request_id, &input.universe_id, &input.admin_user_id],
                        )
                        .await
                        .map_err(database_error)?;
                }
                PrivacyAdminDecision::Reject => {
                    if legal_hold {
                        return Err(PrivacyError::LegalHold);
                    }
                    transaction
                        .execute(
                            "UPDATE gdpr_requests SET status = 'rejected'
                             WHERE id = $1 AND universe_id = $2 AND status = 'in_review'",
                            &[&input.request_id, &input.universe_id],
                        )
                        .await
                        .map_err(database_error)?;
                    cancel_active_jobs(
                        &transaction,
                        input.request_id,
                        input.universe_id,
                        user_id,
                        "request_rejected",
                    )
                    .await?;
                }
                PrivacyAdminDecision::Approve => {
                    if legal_hold {
                        return Err(PrivacyError::LegalHold);
                    }
                    if request_type == "erasure" {
                        let cooling_elapsed = transaction
                            .query_one(
                                "SELECT cooling_off_until <= now() AS elapsed
                                 FROM gdpr_requests WHERE id = $1",
                                &[&input.request_id],
                            )
                            .await
                            .map_err(database_error)?
                            .get::<_, bool>("elapsed");
                        if !cooling_elapsed {
                            return Err(PrivacyError::CoolingOff);
                        }
                        if status == "cooling_off" {
                            transaction
                                .execute(
                                    "UPDATE gdpr_requests SET status = 'in_review'
                                     WHERE id = $1 AND universe_id = $2",
                                    &[&input.request_id, &input.universe_id],
                                )
                                .await
                                .map_err(database_error)?;
                        }
                        let approvals = transaction
                            .query_one(
                                "SELECT COUNT(DISTINCT admin_user_id)::INTEGER AS approvals
                                 FROM privacy_admin_decisions
                                 WHERE request_id = $1 AND decision = 'approve'",
                                &[&input.request_id],
                            )
                            .await
                            .map_err(database_error)?
                            .get::<_, i32>("approvals");
                        if approvals >= 2 {
                            transaction
                                .execute(
                                    "UPDATE gdpr_requests SET status = 'approved'
                                     WHERE id = $1 AND universe_id = $2",
                                    &[&input.request_id, &input.universe_id],
                                )
                                .await
                                .map_err(database_error)?;
                            enqueue_outbox(
                                &transaction,
                                input.request_id,
                                input.universe_id,
                                user_id,
                                "privacy.erasure.execute",
                            )
                            .await?;
                            transaction
                                .execute(
                                    "UPDATE gdpr_requests SET status = 'queued'
                                     WHERE id = $1 AND universe_id = $2",
                                    &[&input.request_id, &input.universe_id],
                                )
                                .await
                                .map_err(database_error)?;
                        }
                    } else if request_type == "correction" {
                        transaction
                            .execute(
                                "UPDATE gdpr_requests SET status = 'approved'
                                 WHERE id = $1 AND universe_id = $2 AND status = 'in_review'",
                                &[&input.request_id, &input.universe_id],
                            )
                            .await
                            .map_err(database_error)?;
                        let has_payload = transaction
                            .query_one(
                                "SELECT request_payload_ciphertext IS NOT NULL AS has_payload
                                 FROM gdpr_requests WHERE id = $1 AND universe_id = $2",
                                &[&input.request_id, &input.universe_id],
                            )
                            .await
                            .map_err(database_error)?
                            .get::<_, bool>("has_payload");
                        if !has_payload {
                            return Err(PrivacyError::Conflict(
                                "correction request has no encrypted change set",
                            ));
                        }
                        enqueue_outbox(
                            &transaction,
                            input.request_id,
                            input.universe_id,
                            user_id,
                            "privacy.correction.apply",
                        )
                        .await?;
                        transaction
                            .execute(
                                "UPDATE gdpr_requests SET status = 'queued'
                                 WHERE id = $1 AND universe_id = $2 AND status = 'approved'",
                                &[&input.request_id, &input.universe_id],
                            )
                            .await
                            .map_err(database_error)?;
                    } else {
                        transaction
                            .execute(
                                "UPDATE gdpr_requests SET status = 'approved'
                                 WHERE id = $1 AND universe_id = $2 AND status = 'in_review'",
                                &[&input.request_id, &input.universe_id],
                            )
                            .await
                            .map_err(database_error)?;
                    }
                }
                PrivacyAdminDecision::CompleteCorrection
                | PrivacyAdminDecision::CompleteErasure => {
                    return Err(PrivacyError::Forbidden);
                }
            }
        }

        let row = request_row_by_id(&transaction, input.universe_id, input.request_id).await?;
        transaction.commit().await.map_err(database_error)?;
        Ok(row)
    }

    pub async fn claim_privacy_jobs(
        &self,
        worker_id: &str,
        universe_id: Option<i64>,
        limit: i64,
        lease_seconds: i64,
    ) -> Result<Vec<PrivacyJob>, PrivacyError> {
        if worker_id.trim().is_empty() || worker_id.len() > 200 {
            return Err(PrivacyError::InvalidInput("worker id is invalid"));
        }
        if !(1..=1000).contains(&limit) || !(1..=3600).contains(&lease_seconds) {
            return Err(PrivacyError::InvalidInput(
                "worker claim bounds are invalid",
            ));
        }
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_actor(&transaction, "worker", None, "worker_claimed").await?;
        let rows = transaction
            .query(
                "WITH candidates AS (
                    SELECT outbox.id
                    FROM privacy_outbox AS outbox
                    JOIN gdpr_requests AS request
                      ON request.id = outbox.request_id
                     AND request.universe_id = outbox.universe_id
                     AND request.user_id = outbox.user_id
                    WHERE ($1::BIGINT IS NULL OR outbox.universe_id = $1)
                      AND request.legal_hold_active = FALSE
                      AND request.status IN ('queued', 'processing', 'failed')
                      AND (
                        (outbox.status IN ('pending', 'retry') AND outbox.available_at <= now())
                        OR
                        (outbox.status = 'processing' AND outbox.lease_expires_at <= now())
                      )
                      AND outbox.attempt_count < outbox.max_attempts
                    ORDER BY outbox.available_at, outbox.id
                    FOR UPDATE OF request SKIP LOCKED
                    LIMIT $2
                 ), claimed AS (
                    UPDATE privacy_outbox AS outbox
                    SET status = 'processing', lease_owner = $3,
                        lease_expires_at = now() + ($4::BIGINT * interval '1 second'),
                        attempt_count = outbox.attempt_count + 1,
                        last_error_code = NULL, updated_at = now()
                    FROM candidates
                    WHERE outbox.id = candidates.id
                    RETURNING outbox.id, outbox.request_id, outbox.universe_id,
                        outbox.user_id, outbox.event_type, outbox.attempt_count,
                        EXTRACT(EPOCH FROM outbox.lease_expires_at)::BIGINT AS lease_expires_at_unix
                 )
                 SELECT * FROM claimed ORDER BY id",
                &[&universe_id, &limit, &worker_id.trim(), &lease_seconds],
            )
            .await
            .map_err(database_error)?;
        if !rows.is_empty() {
            let ids: Vec<i64> = rows.iter().map(|row| row.get("id")).collect();
            transaction
                .execute(
                    "UPDATE gdpr_requests AS request
                     SET status = 'processing', processed_at = COALESCE(processed_at, now())
                     FROM privacy_outbox AS outbox
                     WHERE outbox.id = ANY($1)
                       AND request.id = outbox.request_id
                       AND request.universe_id = outbox.universe_id
                       AND request.user_id = outbox.user_id
                       AND request.status IN ('queued', 'failed')",
                    &[&ids],
                )
                .await
                .map_err(database_error)?;
        }
        transaction.commit().await.map_err(database_error)?;
        Ok(rows
            .iter()
            .map(|row| PrivacyJob {
                id: row.get("id"),
                request_id: row.get("request_id"),
                universe_id: row.get("universe_id"),
                user_id: row.get("user_id"),
                event_type: row.get("event_type"),
                attempt_count: row.get("attempt_count"),
                lease_expires_at_unix: row.get("lease_expires_at_unix"),
            })
            .collect())
    }

    pub async fn complete_privacy_restriction_job(
        &self,
        job_id: i64,
        worker_id: &str,
    ) -> Result<bool, PrivacyError> {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_actor(&transaction, "worker", None, "restriction_applied").await?;
        let Some((request, job)) = lock_request_and_job(&transaction, job_id).await? else {
            return Err(PrivacyError::NotFound);
        };
        if job.status == "delivered" {
            return Ok(false);
        }
        validate_job_lease(&job, worker_id, "privacy.restriction.apply")?;
        validate_request_for_job(&request, "privacy.restriction.apply")?;
        transaction
            .execute(
                "UPDATE users
                 SET privacy_restriction_active = TRUE,
                     privacy_restricted_at = COALESCE(privacy_restricted_at, now()),
                     auth_epoch = auth_epoch + 1
                 WHERE universe_id = $1 AND id = $2",
                &[&job.universe_id, &job.user_id],
            )
            .await
            .map_err(database_error)?;
        revoke_sessions(&transaction, job.user_id).await?;
        let completed = transaction
            .execute(
                "UPDATE gdpr_requests SET status = 'completed'
                 WHERE id = $1 AND universe_id = $2 AND user_id = $3
                   AND status = 'processing' AND legal_hold_active = FALSE",
                &[&job.request_id, &job.universe_id, &job.user_id],
            )
            .await
            .map_err(database_error)?;
        if completed != 1 {
            return Err(PrivacyError::Conflict(
                "privacy request is no longer executable",
            ));
        }
        mark_job_delivered(&transaction, job_id, worker_id).await?;
        transaction.commit().await.map_err(database_error)?;
        Ok(true)
    }

    pub async fn complete_erasure_authorization_job(
        &self,
        job_id: i64,
        worker_id: &str,
    ) -> Result<bool, PrivacyError> {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_actor(&transaction, "worker", None, "erasure_access_invalidated").await?;
        let Some((request, job)) = lock_request_and_job(&transaction, job_id).await? else {
            return Err(PrivacyError::NotFound);
        };
        if job.status == "delivered" {
            return Ok(false);
        }
        validate_job_lease(&job, worker_id, "privacy.erasure.invalidate_access")?;
        validate_request_for_job(&request, "privacy.erasure.invalidate_access")?;
        transaction
            .execute(
                "UPDATE users
                 SET privacy_erasure_pending = TRUE,
                     privacy_restriction_active = TRUE,
                     privacy_restricted_at = COALESCE(privacy_restricted_at, now()),
                     auth_epoch = auth_epoch + 1
                 WHERE universe_id = $1 AND id = $2",
                &[&job.universe_id, &job.user_id],
            )
            .await
            .map_err(database_error)?;
        revoke_sessions(&transaction, job.user_id).await?;
        let authorized = transaction
            .execute(
                "UPDATE gdpr_requests SET status = 'approved'
                 WHERE id = $1 AND universe_id = $2 AND user_id = $3
                   AND status = 'processing' AND legal_hold_active = FALSE",
                &[&job.request_id, &job.universe_id, &job.user_id],
            )
            .await
            .map_err(database_error)?;
        if authorized != 1 {
            return Err(PrivacyError::Conflict(
                "privacy request is no longer executable",
            ));
        }
        mark_job_delivered(&transaction, job_id, worker_id).await?;
        transaction.commit().await.map_err(database_error)?;
        Ok(true)
    }

    pub async fn privacy_correction_payload_for_job(
        &self,
        job_id: i64,
        worker_id: &str,
    ) -> Result<EncryptedPrivacyPayload, PrivacyError> {
        if job_id <= 0 || worker_id.trim().is_empty() || worker_id.len() > 200 {
            return Err(PrivacyError::InvalidInput(
                "privacy correction worker identity is invalid",
            ));
        }
        let client = self.pool.get().await.map_err(database_error)?;
        let row = client
            .query_opt(
                "SELECT request.request_payload_ciphertext, request.payload_key_id,
                    request.payload_nonce, request.payload_sha256
                 FROM privacy_outbox AS outbox
                 JOIN gdpr_requests AS request
                   ON request.id = outbox.request_id
                  AND request.universe_id = outbox.universe_id
                  AND request.user_id = outbox.user_id
                 WHERE outbox.id = $1
                   AND outbox.event_type = 'privacy.correction.apply'
                   AND outbox.status = 'processing'
                   AND outbox.lease_owner = $2
                   AND outbox.lease_expires_at > now()
                   AND request.request_type = 'correction'
                   AND request.status = 'processing'
                   AND request.legal_hold_active = FALSE",
                &[&job_id, &worker_id.trim()],
            )
            .await
            .map_err(database_error)?
            .ok_or(PrivacyError::LeaseLost)?;
        let ciphertext: Option<Vec<u8>> = row.get("request_payload_ciphertext");
        let key_id: Option<String> = row.get("payload_key_id");
        let nonce: Option<Vec<u8>> = row.get("payload_nonce");
        let digest: Option<Vec<u8>> = row.get("payload_sha256");
        Ok(EncryptedPrivacyPayload {
            ciphertext: ciphertext.ok_or(PrivacyError::Conflict(
                "correction request has no encrypted change set",
            ))?,
            key_id: key_id.ok_or(PrivacyError::Conflict(
                "correction request has no encryption key id",
            ))?,
            nonce: fixed_array::<12>(nonce.ok_or(PrivacyError::Conflict(
                "correction request has no encryption nonce",
            ))?)?,
            plaintext_sha256: fixed_array::<32>(digest.ok_or(PrivacyError::Conflict(
                "correction request has no plaintext digest",
            ))?)?,
        })
    }

    pub async fn complete_privacy_correction_job(
        &self,
        job_id: i64,
        worker_id: &str,
        mut patch: PrivacyCorrectionPatch,
        evidence_key: &CommunicationEvidenceKey,
    ) -> Result<bool, PrivacyError> {
        normalize_correction_patch(&mut patch)?;
        let mut fields = patch.field_names();
        fields.sort_unstable();
        fields.dedup();
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_actor(&transaction, "worker", None, "correction_applied").await?;
        let Some((request, job)) = lock_request_and_job(&transaction, job_id).await? else {
            return Err(PrivacyError::NotFound);
        };
        if job.status == "delivered" {
            return Ok(false);
        }
        validate_job_lease(&job, worker_id, "privacy.correction.apply")?;
        validate_request_for_job(&request, "privacy.correction.apply")?;
        let actor_hmac =
            evidence_key.evidence_hmac("communication-actor-v1", "service:app-privacy-worker");
        set_communication_contact_actor(&transaction, &actor_hmac, "privacy_correction").await?;
        let phone_requested = patch.phone_number.is_some();
        let phone_value = patch.phone_number.clone().flatten();

        let changed = transaction
            .execute(
                "UPDATE users
                 SET username = COALESCE($3, username),
                     email = COALESCE($4, email),
                     phone_number = CASE WHEN $5 THEN $6 ELSE phone_number END
                 WHERE universe_id = $1 AND id = $2
                   AND privacy_erased_at IS NULL",
                &[
                    &job.universe_id,
                    &job.user_id,
                    &patch.username,
                    &patch.email,
                    &phone_requested,
                    &phone_value,
                ],
            )
            .await
            .map_err(correction_database_error)?;
        if changed != 1 {
            return Err(PrivacyError::NotFound);
        }
        transaction
            .execute(
                "INSERT INTO privacy_correction_executions (
                    request_id, universe_id, user_id, applied_fields
                 ) VALUES ($1, $2, $3, $4)
                 ON CONFLICT (request_id) DO NOTHING",
                &[&job.request_id, &job.universe_id, &job.user_id, &fields],
            )
            .await
            .map_err(database_error)?;
        let completed = transaction
            .query_opt(
                "UPDATE gdpr_requests
                 SET status = 'completed', correction_applied_at = now(),
                     request_payload_ciphertext = NULL, payload_key_id = NULL,
                     payload_nonce = NULL, payload_sha256 = NULL
                 WHERE id = $1 AND universe_id = $2 AND user_id = $3
                   AND status = 'processing' AND legal_hold_active = FALSE
                 RETURNING version",
                &[&job.request_id, &job.universe_id, &job.user_id],
            )
            .await
            .map_err(database_error)?
            .ok_or(PrivacyError::Conflict(
                "privacy request is no longer executable",
            ))?;
        let version: i64 = completed.get("version");
        insert_privacy_execution_event(
            &transaction,
            &job,
            "correction_applied",
            "worker",
            None,
            "correction_applied",
            &fields,
            None,
        )
        .await?;
        insert_privacy_request_execution_event(
            &transaction,
            &job,
            "correction_applied",
            version,
            "correction_applied",
        )
        .await?;
        mark_job_delivered(&transaction, job_id, worker_id).await?;
        transaction.commit().await.map_err(database_error)?;
        Ok(true)
    }

    pub async fn complete_privacy_erasure_job(
        &self,
        job_id: i64,
        worker_id: &str,
        evidence_key: &CommunicationEvidenceKey,
    ) -> Result<PrivacyErasureResult, PrivacyError> {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_actor(&transaction, "worker", None, "erasure_completed").await?;
        let Some((request, job)) = lock_request_and_job(&transaction, job_id).await? else {
            return Err(PrivacyError::NotFound);
        };
        if job.status == "delivered" {
            let existing = erasure_execution_result(&transaction, job.request_id).await?;
            return Ok(PrivacyErasureResult {
                already_completed: true,
                ..existing
            });
        }
        validate_job_lease_any_type(&job, worker_id)?;
        if !matches!(
            job.event_type.as_str(),
            "privacy.erasure.execute" | "privacy.erasure.invalidate_access"
        ) {
            return Err(PrivacyError::Conflict(
                "privacy outbox event type does not match erasure execution",
            ));
        }
        validate_request_for_job(&request, "privacy.erasure.execute")?;

        let subject = transaction
            .query_opt(
                "SELECT username, email, phone_number, privacy_erased_at IS NOT NULL AS erased
                 FROM users
                 WHERE universe_id = $1 AND id = $2
                 FOR UPDATE",
                &[&job.universe_id, &job.user_id],
            )
            .await
            .map_err(database_error)?
            .ok_or(PrivacyError::NotFound)?;
        if subject.get::<_, bool>("erased") {
            let existing = erasure_execution_result(&transaction, job.request_id).await?;
            mark_job_delivered(&transaction, job_id, worker_id).await?;
            transaction.commit().await.map_err(database_error)?;
            return Ok(PrivacyErasureResult {
                already_completed: true,
                ..existing
            });
        }

        let mut subject_material = Zeroizing::new(format!(
            "{}\0{}\0{}\0{}\0{}",
            job.universe_id,
            job.user_id,
            subject.get::<_, String>("username"),
            subject.get::<_, String>("email"),
            subject
                .get::<_, Option<String>>("phone_number")
                .unwrap_or_default()
        ));
        let subject_hmac =
            evidence_key.evidence_hmac("privacy-erased-subject-v1", subject_material.as_str());
        subject_material.zeroize();
        let actor_hmac =
            evidence_key.evidence_hmac("communication-actor-v1", "service:app-privacy-worker");
        set_communication_contact_actor(&transaction, &actor_hmac, "privacy_erasure").await?;

        let pseudonym_suffix = encode_hex(&subject_hmac[..8]);
        let pseudonym = format!("erased-{}-{pseudonym_suffix}", job.request_id);
        let pseudonymous_email = format!("{pseudonym}@privacy.invalid");

        let mut credentials_deleted = 0u64;
        for (table, column) in [
            ("account_transfers", "user_id"),
            ("account_data_backups", "user_id"),
            ("email_verifications", "user_id"),
            ("sms_verifications", "user_id"),
            ("password_resets", "user_id"),
            ("two_factor_auth", "user_id"),
            ("oauth_accounts", "user_id"),
            ("oauth_connections", "user_id"),
            ("oauth_tokens", "user_id"),
            ("user_devices", "user_id"),
            ("trusted_devices", "user_id"),
            ("device_sessions", "user_id"),
        ] {
            credentials_deleted = credentials_deleted.saturating_add(
                delete_subject_rows_if_present(
                    &transaction,
                    table,
                    column,
                    job.universe_id,
                    job.user_id,
                )
                .await?,
            );
        }

        let mut sessions_deleted = 0u64;
        for (table, column) in [("auth_sessions", "user_id"), ("user_sessions", "user_id")] {
            sessions_deleted = sessions_deleted.saturating_add(
                delete_subject_rows_if_present(
                    &transaction,
                    table,
                    column,
                    job.universe_id,
                    job.user_id,
                )
                .await?,
            );
        }

        let mut personal_content_deleted = 0u64;
        personal_content_deleted = personal_content_deleted.saturating_add(
            transaction
                .execute(
                    "DELETE FROM private_conversations
                     WHERE user1_id = $1 OR user2_id = $1",
                    &[&job.user_id],
                )
                .await
                .map_err(database_error)?,
        );
        for (table, column) in [
            ("private_messages", "sender_id"),
            ("alliance_chat", "user_id"),
            ("alliance_messages", "sender_id"),
            ("chat_messages", "user_id"),
            ("notifications", "user_id"),
            ("user_activity_logs", "user_id"),
        ] {
            personal_content_deleted = personal_content_deleted.saturating_add(
                delete_subject_rows_if_present(
                    &transaction,
                    table,
                    column,
                    job.universe_id,
                    job.user_id,
                )
                .await?,
            );
        }
        personal_content_deleted = personal_content_deleted.saturating_add(
            transaction
                .execute(
                    "DELETE FROM messages
                     WHERE from_user_id = $1 OR to_user_id = $1",
                    &[&job.user_id],
                )
                .await
                .map_err(database_error)?,
        );
        personal_content_deleted = personal_content_deleted.saturating_add(
            transaction
                .execute(
                    "DELETE FROM shard_chat_messages
                     WHERE sender_id = $1 OR recipient_id = $1",
                    &[&job.user_id],
                )
                .await
                .map_err(database_error)?,
        );

        personal_content_deleted = personal_content_deleted.saturating_add(
            redact_subject_metadata(&transaction, job.universe_id, job.user_id, &pseudonym).await?,
        );

        let user_updated = transaction
            .execute(
                "UPDATE users
                 SET username = $3,
                     email = $4,
                     password_hash = '!privacy-erased!',
                     phone_number = NULL,
                     email_verified = FALSE,
                     email_verified_at = NULL,
                     phone_verified = FALSE,
                     phone_verified_at = NULL,
                     last_login = NULL,
                     last_login_at = NULL,
                     last_ip = NULL,
                     last_login_ip = NULL,
                     country_code = NULL,
                     referral_source = NULL,
                     admin_notes = NULL,
                     admin_flags = NULL,
                     alliance_id = NULL,
                     is_admin = FALSE,
                     is_banned = TRUE,
                     is_locked = TRUE,
                     locked_at = CURRENT_TIMESTAMP,
                     locked_reason = 'privacy_erasure',
                     account_status = 'deleted',
                     deleted_at = CURRENT_TIMESTAMP,
                     deletion_reason = 'privacy_erasure',
                     privacy_restriction_active = TRUE,
                     privacy_restricted_at = COALESCE(privacy_restricted_at, now()),
                     privacy_erasure_pending = FALSE,
                     privacy_erased_at = now(),
                     privacy_subject_hmac = $5,
                     privacy_erasure_request_id = $6,
                     auth_epoch = auth_epoch + 1
                 WHERE universe_id = $1 AND id = $2
                   AND privacy_erased_at IS NULL",
                &[
                    &job.universe_id,
                    &job.user_id,
                    &pseudonym,
                    &pseudonymous_email,
                    &subject_hmac.as_slice(),
                    &job.request_id,
                ],
            )
            .await
            .map_err(database_error)?;
        if user_updated != 1 {
            return Err(PrivacyError::Conflict(
                "privacy subject changed during erasure",
            ));
        }

        let contact_rows = transaction
            .execute(
                "DELETE FROM communication_verified_contacts
                 WHERE universe_id = $1 AND user_id = $2",
                &[&job.universe_id, &job.user_id],
            )
            .await
            .map_err(database_error)?;
        transaction
            .execute(
                "DELETE FROM communication_contact_versions
                 WHERE universe_id = $1 AND user_id = $2",
                &[&job.universe_id, &job.user_id],
            )
            .await
            .map_err(database_error)?;
        let communication_jobs = transaction
            .query(
                "UPDATE communication_outbox
                 SET state = CASE
                        WHEN state IN ('pending', 'leased', 'retry') THEN 'suppressed'
                        ELSE state END,
                     lease_owner = NULL,
                     lease_until = NULL,
                     sent_at = CASE
                        WHEN state IN ('pending', 'leased', 'retry') THEN NULL
                        ELSE sent_at END,
                     terminal_at = CASE
                        WHEN state IN ('pending', 'leased', 'retry')
                        THEN COALESCE(terminal_at, now()) ELSE terminal_at END,
                     provider_message_hmac = NULL,
                     destination_hmac = NULL,
                     destination_masked = NULL,
                     last_reason_code = CASE
                        WHEN state IN ('pending', 'leased', 'retry', 'suppressed')
                        THEN 'privacy_erasure' ELSE last_reason_code END,
                     updated_at = now()
                 WHERE universe_id = $1 AND user_id = $2
                 RETURNING id, universe_id, channel, category, state, attempts",
                &[&job.universe_id, &job.user_id],
            )
            .await
            .map_err(database_error)?;
        for row in &communication_jobs {
            transaction
                .execute(
                    "INSERT INTO communication_outbox_events (
                        outbox_id, universe_id, channel, category, event_type,
                        state, reason_code, attempt, actor_subject_hmac
                     ) VALUES ($1, $2, $3, $4, 'contact_evidence_redacted',
                        $5, 'privacy_erasure', $6, $7)",
                    &[
                        &row.get::<_, i64>("id"),
                        &row.get::<_, i64>("universe_id"),
                        &row.get::<_, String>("channel"),
                        &row.get::<_, String>("category"),
                        &row.get::<_, String>("state"),
                        &row.get::<_, i32>("attempts"),
                        &actor_hmac.as_slice(),
                    ],
                )
                .await
                .map_err(database_error)?;
        }
        let contact_evidence_redacted =
            contact_rows.saturating_add(communication_jobs.len() as u64);

        transaction
            .execute(
                "INSERT INTO privacy_erasure_executions (
                    request_id, universe_id, user_id, subject_hmac,
                    credentials_deleted, sessions_deleted, personal_content_deleted,
                    contact_evidence_redacted
                 ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                &[
                    &job.request_id,
                    &job.universe_id,
                    &job.user_id,
                    &subject_hmac.as_slice(),
                    &(credentials_deleted as i64),
                    &(sessions_deleted as i64),
                    &(personal_content_deleted as i64),
                    &(contact_evidence_redacted as i64),
                ],
            )
            .await
            .map_err(database_error)?;
        let completed = transaction
            .query_opt(
                "UPDATE gdpr_requests
                 SET status = 'completed', erasure_executed_at = now(),
                     request_payload_ciphertext = NULL, payload_key_id = NULL,
                     payload_nonce = NULL, payload_sha256 = NULL
                 WHERE id = $1 AND universe_id = $2 AND user_id = $3
                   AND status = 'processing' AND legal_hold_active = FALSE
                 RETURNING version",
                &[&job.request_id, &job.universe_id, &job.user_id],
            )
            .await
            .map_err(database_error)?
            .ok_or(PrivacyError::Conflict(
                "privacy request is no longer executable",
            ))?;
        let version: i64 = completed.get("version");
        insert_privacy_execution_event(
            &transaction,
            &job,
            "erasure_completed",
            "worker",
            None,
            "erasure_completed",
            &[],
            Some(&subject_hmac),
        )
        .await?;
        insert_privacy_request_execution_event(
            &transaction,
            &job,
            "erasure_completed",
            version,
            "erasure_completed",
        )
        .await?;
        mark_job_delivered(&transaction, job_id, worker_id).await?;
        transaction.commit().await.map_err(database_error)?;
        Ok(PrivacyErasureResult {
            already_completed: false,
            credentials_deleted,
            sessions_deleted,
            personal_content_deleted,
            contact_evidence_redacted,
        })
    }

    pub async fn privacy_export_snapshot(
        &self,
        universe_id: i64,
        user_id: i32,
    ) -> Result<serde_json::Value, PrivacyError> {
        let client = self.pool.get().await.map_err(database_error)?;
        let profile = client
            .query_opt(
                "SELECT jsonb_build_object(
                    'id', id,
                    'universeId', universe_id,
                    'username', username,
                    'email', email,
                    'createdAt', created_at,
                    'lastLogin', last_login,
                    'accountStatus', account_status,
                    'emailVerified', email_verified,
                    'allianceId', alliance_id,
                    'privacyRestricted', privacy_restriction_active,
                    'erasurePending', privacy_erasure_pending
                 ) AS profile
                 FROM users
                 WHERE universe_id = $1 AND id = $2",
                &[&universe_id, &user_id],
            )
            .await
            .map_err(database_error)?
            .ok_or(PrivacyError::NotFound)?;
        let generated_at = client
            .query_one("SELECT EXTRACT(EPOCH FROM clock_timestamp())::BIGINT", &[])
            .await
            .map_err(database_error)?
            .get::<_, i64>(0);

        let mut export = serde_json::Map::new();
        export.insert("schemaVersion".to_string(), serde_json::json!(1));
        export.insert(
            "generatedAtUnix".to_string(),
            serde_json::json!(generated_at),
        );
        export.insert(
            "inventory".to_string(),
            serde_json::json!(PRIVACY_EXPORT_DATA_INVENTORY),
        );
        export.insert(
            "profile".to_string(),
            profile.get::<_, Json<serde_json::Value>>("profile").0,
        );

        let sources: &[(&str, &str, &str)] = &[
            (
                "planets",
                "planets",
                "SELECT COALESCE(jsonb_agg(to_jsonb(row_data) ORDER BY row_data.id), '[]'::jsonb) AS data
                 FROM (SELECT planets.* FROM planets
                       WHERE universe_id = $1 AND user_id = $2) AS row_data",
            ),
            (
                "research",
                "research",
                "SELECT COALESCE(jsonb_agg(to_jsonb(row_data)), '[]'::jsonb) AS data
                 FROM (SELECT research.* FROM research
                       JOIN users ON users.id = research.user_id
                       WHERE users.universe_id = $1 AND research.user_id = $2) AS row_data",
            ),
            (
                "constructionQueue",
                "construction_queue",
                "SELECT COALESCE(jsonb_agg(to_jsonb(row_data) ORDER BY row_data.id), '[]'::jsonb) AS data
                 FROM (SELECT queue.* FROM construction_queue AS queue
                       JOIN planets ON planets.id = queue.planet_id
                       WHERE planets.universe_id = $1 AND planets.user_id = $2) AS row_data",
            ),
            (
                "researchQueue",
                "research_queue",
                "SELECT COALESCE(jsonb_agg(to_jsonb(row_data) ORDER BY row_data.id), '[]'::jsonb) AS data
                 FROM (SELECT queue.* FROM research_queue AS queue
                       JOIN users ON users.id = queue.user_id
                       WHERE users.universe_id = $1 AND queue.user_id = $2) AS row_data",
            ),
            (
                "shipyardQueue",
                "shipyard_queue",
                "SELECT COALESCE(jsonb_agg(to_jsonb(row_data) ORDER BY row_data.id), '[]'::jsonb) AS data
                 FROM (SELECT queue.* FROM shipyard_queue AS queue
                       JOIN planets ON planets.id = queue.planet_id
                       WHERE planets.universe_id = $1 AND planets.user_id = $2) AS row_data",
            ),
            (
                "fleets",
                "fleets",
                "SELECT COALESCE(jsonb_agg(to_jsonb(row_data) ORDER BY row_data.id), '[]'::jsonb) AS data
                 FROM (SELECT fleets.* FROM fleets
                       JOIN users ON users.id = fleets.user_id
                       WHERE users.universe_id = $1 AND fleets.user_id = $2) AS row_data",
            ),
            (
                "messages",
                "messages",
                "SELECT COALESCE(jsonb_agg(to_jsonb(row_data) ORDER BY row_data.id), '[]'::jsonb) AS data
                 FROM (SELECT messages.* FROM messages
                       WHERE (from_user_id = $2 OR to_user_id = $2)
                         AND EXISTS (SELECT 1 FROM users
                                     WHERE universe_id = $1 AND id = $2)) AS row_data",
            ),
            (
                "privateConversations",
                "private_conversations",
                "SELECT COALESCE(jsonb_agg(to_jsonb(row_data) ORDER BY row_data.id), '[]'::jsonb) AS data
                 FROM (SELECT conversations.* FROM private_conversations AS conversations
                       WHERE (conversations.user1_id = $2 OR conversations.user2_id = $2)
                         AND EXISTS (SELECT 1 FROM users
                                     WHERE universe_id = $1 AND id = $2)) AS row_data",
            ),
            (
                "privateMessages",
                "private_messages",
                "SELECT COALESCE(jsonb_agg(to_jsonb(row_data) ORDER BY row_data.id), '[]'::jsonb) AS data
                 FROM (SELECT private_messages.* FROM private_messages
                       JOIN private_conversations AS conversations
                         ON conversations.id = private_messages.conversation_id
                       WHERE (conversations.user1_id = $2 OR conversations.user2_id = $2)
                         AND EXISTS (SELECT 1 FROM users
                                     WHERE universe_id = $1 AND id = $2)) AS row_data",
            ),
            (
                "chatMessages",
                "chat_messages",
                "SELECT COALESCE(jsonb_agg(to_jsonb(row_data) ORDER BY row_data.id), '[]'::jsonb) AS data
                 FROM (SELECT chat_messages.* FROM chat_messages
                       JOIN users ON users.id = chat_messages.user_id
                       WHERE users.universe_id = $1 AND chat_messages.user_id = $2) AS row_data",
            ),
            (
                "allianceChat",
                "alliance_chat",
                "SELECT COALESCE(jsonb_agg(to_jsonb(row_data) ORDER BY row_data.id), '[]'::jsonb) AS data
                 FROM (SELECT chat.* FROM alliance_chat AS chat
                       JOIN users ON users.id = chat.user_id
                       WHERE users.universe_id = $1 AND chat.user_id = $2) AS row_data",
            ),
            (
                "allianceMessages",
                "alliance_messages",
                "SELECT COALESCE(jsonb_agg(to_jsonb(row_data) ORDER BY row_data.id), '[]'::jsonb) AS data
                 FROM (SELECT messages.* FROM alliance_messages AS messages
                       JOIN users ON users.id = messages.sender_id
                       WHERE users.universe_id = $1 AND messages.sender_id = $2) AS row_data",
            ),
            (
                "allianceMemberships",
                "alliance_members",
                "SELECT COALESCE(jsonb_agg(to_jsonb(row_data)), '[]'::jsonb) AS data
                 FROM (SELECT members.*, to_jsonb(alliances)
                                      - ARRAY['admin_notes', 'auto_application_notes'] AS alliance
                       FROM alliance_members AS members
                       JOIN alliances ON alliances.id = members.alliance_id
                       JOIN users ON users.id = members.user_id
                       WHERE users.universe_id = $1 AND members.user_id = $2) AS row_data",
            ),
            (
                "scores",
                "player_scores",
                "SELECT COALESCE(jsonb_agg(to_jsonb(row_data)), '[]'::jsonb) AS data
                 FROM (SELECT scores.* FROM player_scores AS scores
                       JOIN users ON users.id = scores.user_id
                       WHERE users.universe_id = $1 AND scores.user_id = $2) AS row_data",
            ),
            (
                "achievements",
                "user_achievements",
                "SELECT COALESCE(jsonb_agg(to_jsonb(row_data)), '[]'::jsonb) AS data
                 FROM (SELECT achievements.* FROM user_achievements AS achievements
                       JOIN users ON users.id = achievements.user_id
                       WHERE users.universe_id = $1 AND achievements.user_id = $2) AS row_data",
            ),
            (
                "badges",
                "user_badges",
                "SELECT COALESCE(jsonb_agg(to_jsonb(row_data)), '[]'::jsonb) AS data
                 FROM (SELECT user_badges.user_id, user_badges.badge_id,
                              user_badges.earned_at, badges.code, badges.name,
                              badges.description, badges.icon_url
                       FROM user_badges
                       JOIN badges ON badges.id = user_badges.badge_id
                       JOIN users ON users.id = user_badges.user_id
                       WHERE users.universe_id = $1 AND user_badges.user_id = $2) AS row_data",
            ),
            (
                "rewards",
                "user_rewards",
                "SELECT COALESCE(jsonb_agg(to_jsonb(row_data)), '[]'::jsonb) AS data
                 FROM (SELECT user_rewards.user_id, user_rewards.reward_id,
                              user_rewards.granted_at, rewards.code, rewards.name,
                              rewards.description, rewards.reward_type, rewards.value
                       FROM user_rewards
                       JOIN rewards ON rewards.id = user_rewards.reward_id
                       JOIN users ON users.id = user_rewards.user_id
                       WHERE users.universe_id = $1 AND user_rewards.user_id = $2) AS row_data",
            ),
            (
                "notifications",
                "notifications",
                "SELECT COALESCE(jsonb_agg(to_jsonb(row_data) ORDER BY row_data.id), '[]'::jsonb) AS data
                 FROM (SELECT notifications.* FROM notifications
                       JOIN users ON users.id = notifications.user_id
                       WHERE users.universe_id = $1 AND notifications.user_id = $2) AS row_data",
            ),
            (
                "blocks",
                "player_blocks",
                "SELECT COALESCE(jsonb_agg(to_jsonb(row_data) ORDER BY row_data.id), '[]'::jsonb) AS data
                 FROM (SELECT blocks.* FROM player_blocks AS blocks
                       WHERE (blocks.user_id = $2 OR blocks.blocked_user_id = $2)
                         AND EXISTS (SELECT 1 FROM users
                                     WHERE universe_id = $1 AND id = $2)) AS row_data",
            ),
            (
                "chatRestrictions",
                "chat_restrictions",
                "SELECT COALESCE(jsonb_agg(to_jsonb(row_data) ORDER BY row_data.id), '[]'::jsonb) AS data
                 FROM (SELECT restrictions.id, restrictions.user_id,
                              restrictions.channel_id, restrictions.restriction_type,
                              restrictions.reason, restrictions.restricted_by,
                              restrictions.expires_at, restrictions.created_at
                       FROM chat_restrictions AS restrictions
                       JOIN users ON users.id = restrictions.user_id
                       WHERE users.universe_id = $1 AND restrictions.user_id = $2) AS row_data",
            ),
            (
                "accountBlocks",
                "user_blocks",
                "SELECT COALESCE(jsonb_agg(to_jsonb(row_data) ORDER BY row_data.id), '[]'::jsonb) AS data
                 FROM (SELECT blocks.id, blocks.user_id, blocks.block_type,
                              blocks.reason, blocks.duration_minutes, blocks.start_time,
                              blocks.end_time, blocks.is_permanent, blocks.is_active,
                              blocks.blocked_by, blocks.unblocked_by, blocks.unblock_time,
                              blocks.unblock_reason, blocks.appeal_status,
                              blocks.severity_level
                       FROM user_blocks AS blocks
                       JOIN users ON users.id = blocks.user_id
                       WHERE users.universe_id = $1 AND blocks.user_id = $2) AS row_data",
            ),
            (
                "purchases",
                "purchases",
                "SELECT COALESCE(jsonb_agg(row_data.data ORDER BY row_data.id), '[]'::jsonb) AS data
                 FROM (SELECT purchases.id,
                              to_jsonb(purchases) - ARRAY['stripe_payment_intent_id'] AS data
                       FROM purchases
                       JOIN users ON users.id = purchases.user_id
                       WHERE users.universe_id = $1 AND purchases.user_id = $2) AS row_data",
            ),
            (
                "enhancedPurchases",
                "shop_purchases_enhanced",
                "SELECT COALESCE(jsonb_agg(to_jsonb(row_data) ORDER BY row_data.id), '[]'::jsonb) AS data
                 FROM (SELECT purchases.id, purchases.user_id, purchases.item_type,
                              purchases.item_id, purchases.quantity, purchases.price_usd,
                              purchases.currency, purchases.payment_method,
                              purchases.promotion_id, purchases.discount_applied,
                              purchases.final_price, purchases.status, purchases.ip_address,
                              purchases.user_agent, purchases.device_type, purchases.referrer,
                              purchases.created_at, purchases.completed_at, purchases.refunded_at
                       FROM shop_purchases_enhanced AS purchases
                       JOIN users ON users.id = purchases.user_id
                       WHERE users.universe_id = $1 AND purchases.user_id = $2) AS row_data",
            ),
            (
                "securityAudit",
                "security_audit_logs",
                "SELECT COALESCE(jsonb_agg(to_jsonb(row_data) ORDER BY row_data.id), '[]'::jsonb) AS data
                 FROM (SELECT audit.id, audit.user_id, audit.event_type,
                              audit.event_description, audit.severity, audit.ip_address,
                              audit.user_agent, audit.created_at
                       FROM security_audit_logs AS audit
                       JOIN users ON users.id = audit.user_id
                       WHERE users.universe_id = $1 AND audit.user_id = $2) AS row_data",
            ),
            (
                "activityHistory",
                "user_activity_logs",
                "SELECT COALESCE(jsonb_agg(to_jsonb(row_data) ORDER BY row_data.id), '[]'::jsonb) AS data
                 FROM (SELECT activity.id, activity.user_id, activity.activity_type,
                              activity.description, activity.ip_address, activity.created_at
                       FROM user_activity_logs AS activity
                       JOIN users ON users.id = activity.user_id
                       WHERE users.universe_id = $1 AND activity.user_id = $2) AS row_data",
            ),
            (
                "adminAudit",
                "admin_audit_logs",
                "SELECT COALESCE(jsonb_agg(to_jsonb(row_data) ORDER BY row_data.id), '[]'::jsonb) AS data
                 FROM (SELECT audit.id, audit.admin_id, audit.admin_username,
                              audit.action_type, audit.action_category, audit.target_type,
                              audit.target_id, audit.target_identifier, audit.timestamp,
                              audit.severity, audit.success, audit.error_message,
                              audit.action, audit.resource_type, audit.created_at
                       FROM admin_audit_logs AS audit
                       WHERE (audit.admin_id = $2
                              OR (audit.target_id = $2 AND LOWER(COALESCE(audit.target_type, ''))
                                  IN ('user', 'users', 'account', 'user_account',
                                      'user-account', 'user account', 'account_user')))
                         AND EXISTS (SELECT 1 FROM users
                                     WHERE universe_id = $1 AND id = $2)) AS row_data",
            ),
            (
                "suspensions",
                "account_suspensions",
                "SELECT COALESCE(jsonb_agg(to_jsonb(row_data) ORDER BY row_data.id), '[]'::jsonb) AS data
                 FROM (SELECT suspensions.* FROM account_suspensions AS suspensions
                       JOIN users ON users.id = suspensions.user_id
                       WHERE users.universe_id = $1 AND suspensions.user_id = $2) AS row_data",
            ),
            (
                "accountTransfers",
                "account_transfers",
                "SELECT COALESCE(jsonb_agg(row_data.data ORDER BY row_data.id), '[]'::jsonb) AS data
                 FROM (SELECT transfers.id,
                              to_jsonb(transfers) - ARRAY['verification_token'] AS data
                       FROM account_transfers AS transfers
                       JOIN users ON users.id = transfers.user_id
                       WHERE users.universe_id = $1 AND transfers.user_id = $2) AS row_data",
            ),
            (
                "emailVerifications",
                "email_verifications",
                "SELECT COALESCE(jsonb_agg(row_data.data ORDER BY row_data.id), '[]'::jsonb) AS data
                 FROM (SELECT verifications.id,
                              to_jsonb(verifications) - ARRAY['verification_token'] AS data
                       FROM email_verifications AS verifications
                       JOIN users ON users.id = verifications.user_id
                       WHERE users.universe_id = $1 AND verifications.user_id = $2) AS row_data",
            ),
            (
                "passwordResetHistory",
                "password_resets",
                "SELECT COALESCE(jsonb_agg(row_data.data ORDER BY row_data.id), '[]'::jsonb) AS data
                 FROM (SELECT resets.id,
                              to_jsonb(resets) - ARRAY['reset_token'] AS data
                       FROM password_resets AS resets
                       JOIN users ON users.id = resets.user_id
                       WHERE users.universe_id = $1 AND resets.user_id = $2) AS row_data",
            ),
            (
                "twoFactorMetadata",
                "two_factor_auth",
                "SELECT COALESCE(jsonb_agg(row_data.data ORDER BY row_data.id), '[]'::jsonb) AS data
                 FROM (SELECT factors.id,
                              to_jsonb(factors) - ARRAY['secret', 'backup_codes'] AS data
                       FROM two_factor_auth AS factors
                       JOIN users ON users.id = factors.user_id
                       WHERE users.universe_id = $1 AND factors.user_id = $2) AS row_data",
            ),
            (
                "sessions",
                "user_sessions",
                "SELECT COALESCE(jsonb_agg(row_data.data ORDER BY row_data.id), '[]'::jsonb) AS data
                 FROM (SELECT sessions.id,
                              to_jsonb(sessions) - ARRAY['session_token'] AS data
                       FROM user_sessions AS sessions
                       JOIN users ON users.id = sessions.user_id
                       WHERE users.universe_id = $1 AND sessions.user_id = $2) AS row_data",
            ),
            (
                "privacyRequests",
                "gdpr_requests",
                "SELECT COALESCE(jsonb_agg(row_data.data ORDER BY row_data.id), '[]'::jsonb) AS data
                 FROM (SELECT requests.id,
                              to_jsonb(requests) - ARRAY[
                                'requester_ip_digest', 'request_payload_ciphertext',
                                'payload_key_id', 'payload_nonce', 'payload_sha256'
                              ] AS data
                       FROM gdpr_requests AS requests
                       WHERE requests.universe_id = $1 AND requests.user_id = $2) AS row_data",
            ),
            (
                "privacyRequestEvents",
                "privacy_request_events",
                "SELECT COALESCE(jsonb_agg(to_jsonb(row_data) ORDER BY row_data.id), '[]'::jsonb) AS data
                 FROM (SELECT events.* FROM privacy_request_events AS events
                       WHERE events.universe_id = $1 AND events.user_id = $2) AS row_data",
            ),
            (
                "privacyAdminDecisions",
                "privacy_admin_decisions",
                "SELECT COALESCE(jsonb_agg(to_jsonb(row_data) ORDER BY row_data.id), '[]'::jsonb) AS data
                 FROM (SELECT decisions.* FROM privacy_admin_decisions AS decisions
                       WHERE decisions.universe_id = $1 AND decisions.user_id = $2) AS row_data",
            ),
            (
                "consents",
                "privacy_consents",
                "SELECT COALESCE(jsonb_agg(row_data.data), '[]'::jsonb) AS data
                 FROM (SELECT to_jsonb(consents) - ARRAY['proof_digest'] AS data
                       FROM privacy_consents AS consents
                       WHERE consents.universe_id = $1 AND consents.user_id = $2) AS row_data",
            ),
            (
                "communicationPreferences",
                "privacy_communication_preferences",
                "SELECT COALESCE(jsonb_agg(to_jsonb(row_data)), '[]'::jsonb) AS data
                 FROM (SELECT preferences.*
                       FROM privacy_communication_preferences AS preferences
                       WHERE preferences.universe_id = $1 AND preferences.user_id = $2) AS row_data",
            ),
        ];
        let params: &[&(dyn ToSql + Sync)] = &[&universe_id, &user_id];
        for (key, table, query) in sources {
            let value = export_array_if_table(&client, table, query, params).await?;
            export.insert((*key).to_string(), value);
        }
        Ok(serde_json::Value::Object(export))
    }

    pub async fn complete_privacy_export_job(
        &self,
        job_id: i64,
        worker_id: &str,
        artifact: PreparedExportArtifact,
    ) -> Result<bool, PrivacyError> {
        validate_export_artifact(&artifact)?;
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_actor(&transaction, "worker", None, "export_prepared").await?;
        let Some((request, job)) = lock_request_and_job(&transaction, job_id).await? else {
            return Err(PrivacyError::NotFound);
        };
        if job.status == "delivered" {
            return Ok(false);
        }
        validate_job_lease(&job, worker_id, "privacy.export.prepare")?;
        validate_request_for_job(&request, "privacy.export.prepare")?;
        transaction
            .execute(
                "INSERT INTO privacy_export_artifacts (
                    request_id, universe_id, user_id, ciphertext, encryption_key_id,
                    encryption_nonce, plaintext_sha256, plaintext_size, expires_at
                 ) VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8,
                    now() + ($9::BIGINT * interval '1 second')
                 )
                 ON CONFLICT (request_id) DO NOTHING",
                &[
                    &job.request_id,
                    &job.universe_id,
                    &job.user_id,
                    &artifact.ciphertext.as_slice(),
                    &artifact.encryption_key_id.trim(),
                    &artifact.encryption_nonce.as_slice(),
                    &artifact.plaintext_sha256.as_slice(),
                    &artifact.plaintext_size,
                    &artifact.expires_in_seconds,
                ],
            )
            .await
            .map_err(database_error)?;
        let completed = transaction
            .execute(
                "UPDATE gdpr_requests SET status = 'completed'
                 WHERE id = $1 AND universe_id = $2 AND user_id = $3
                   AND status = 'processing' AND legal_hold_active = FALSE",
                &[&job.request_id, &job.universe_id, &job.user_id],
            )
            .await
            .map_err(database_error)?;
        if completed != 1 {
            return Err(PrivacyError::Conflict(
                "privacy request is no longer executable",
            ));
        }
        mark_job_delivered(&transaction, job_id, worker_id).await?;
        transaction.commit().await.map_err(database_error)?;
        Ok(true)
    }

    pub async fn fail_privacy_job(
        &self,
        job_id: i64,
        worker_id: &str,
        error_code: &str,
        retry_delay_seconds: i64,
    ) -> Result<(), PrivacyError> {
        validate_reason_code(error_code)?;
        if !(0..=24 * 60 * 60).contains(&retry_delay_seconds) {
            return Err(PrivacyError::InvalidInput("retry delay is invalid"));
        }
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_actor(&transaction, "worker", None, error_code).await?;
        let Some((request, job)) = lock_request_and_job(&transaction, job_id).await? else {
            return Err(PrivacyError::NotFound);
        };
        validate_job_lease_any_type(&job, worker_id)?;
        validate_request_for_job(&request, &job.event_type)?;
        let terminal = job.attempt_count >= job.max_attempts;
        let updated = transaction
            .execute(
                "UPDATE privacy_outbox
                 SET status = CASE WHEN $4 THEN 'dead' ELSE 'retry' END,
                     available_at = now() + ($5::BIGINT * interval '1 second'),
                     lease_owner = NULL, lease_expires_at = NULL,
                     last_error_code = $3, updated_at = now()
                 WHERE id = $1 AND status = 'processing' AND lease_owner = $2",
                &[
                    &job_id,
                    &worker_id.trim(),
                    &error_code.trim(),
                    &terminal,
                    &retry_delay_seconds,
                ],
            )
            .await
            .map_err(database_error)?;
        if updated != 1 {
            return Err(PrivacyError::LeaseLost);
        }
        let request_updated = transaction
            .execute(
                "UPDATE gdpr_requests
                 SET status = CASE WHEN $4 THEN 'failed' ELSE 'queued' END
                 WHERE id = $1 AND universe_id = $2 AND user_id = $3
                   AND status = 'processing' AND legal_hold_active = FALSE",
                &[&job.request_id, &job.universe_id, &job.user_id, &terminal],
            )
            .await
            .map_err(database_error)?;
        if request_updated != 1 {
            return Err(PrivacyError::Conflict(
                "privacy request is no longer executable",
            ));
        }
        transaction.commit().await.map_err(database_error)?;
        Ok(())
    }

    pub async fn issue_export_delivery(
        &self,
        universe_id: i64,
        user_id: i32,
        request_id: i32,
        ttl_seconds: i64,
    ) -> Result<ExportDeliveryGrant, PrivacyError> {
        if !(60..=24 * 60 * 60).contains(&ttl_seconds) {
            return Err(PrivacyError::InvalidInput(
                "export token lifetime must be between one minute and one day",
            ));
        }
        let mut token_bytes = [0u8; 32];
        OsRng.fill_bytes(&mut token_bytes);
        let token = URL_SAFE_NO_PAD.encode(token_bytes);
        let digest = Sha256::digest(token.as_bytes());
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        let row = transaction
            .query_opt(
                "UPDATE privacy_export_artifacts AS artifact
                 SET download_token_digest = $4, token_issued_at = now(),
                     token_expires_at = LEAST(
                        artifact.expires_at,
                        now() + ($5::BIGINT * interval '1 second')
                     )
                 FROM gdpr_requests AS request
                 WHERE artifact.request_id = $1
                   AND artifact.universe_id = $2 AND artifact.user_id = $3
                   AND request.id = artifact.request_id
                   AND request.universe_id = artifact.universe_id
                   AND request.user_id = artifact.user_id
                   AND request.status = 'completed'
                   AND request.legal_hold_active = FALSE
                   AND artifact.purged_at IS NULL
                   AND artifact.downloaded_at IS NULL
                   AND artifact.expires_at > now()
                 RETURNING EXTRACT(EPOCH FROM artifact.token_expires_at)::BIGINT AS expires_at",
                &[
                    &request_id,
                    &universe_id,
                    &user_id,
                    &digest.as_slice(),
                    &ttl_seconds,
                ],
            )
            .await
            .map_err(database_error)?
            .ok_or(PrivacyError::DeliveryDenied)?;
        let delivery_dedupe = format!(
            "privacy-execution:{request_id}:export_delivery_issued:{}",
            encode_hex(&digest[..8])
        );
        transaction
            .execute(
                "INSERT INTO privacy_execution_events (
                    request_id, universe_id, user_id, action, actor_type,
                    actor_user_id, reason_code, field_names, dedupe_key
                 ) VALUES ($1, $2, $3, 'export_delivery_issued', 'user',
                    $3, 'export_delivery_issued', ARRAY[]::TEXT[], $4)
                 ON CONFLICT (dedupe_key) DO NOTHING",
                &[&request_id, &universe_id, &user_id, &delivery_dedupe],
            )
            .await
            .map_err(database_error)?;
        transaction.commit().await.map_err(database_error)?;
        Ok(ExportDeliveryGrant {
            token,
            expires_at_unix: row.get("expires_at"),
        })
    }

    /// Validates a one-time delivery grant and returns the encrypted artifact
    /// without consuming the grant. Callers must verify/decrypt the artifact
    /// successfully before calling [`Self::finalize_export_delivery`].
    pub async fn prepare_export_delivery(
        &self,
        universe_id: i64,
        user_id: i32,
        request_id: i32,
        token: &str,
    ) -> Result<ExportDownload, PrivacyError> {
        if token.is_empty() || token.len() > 200 {
            return Err(PrivacyError::DeliveryDenied);
        }
        let candidate = Sha256::digest(token.as_bytes());
        let client = self.pool.get().await.map_err(database_error)?;
        let row = client
            .query_opt(
                "SELECT ciphertext, encryption_key_id, encryption_nonce,
                    plaintext_sha256, plaintext_size, download_token_digest,
                    format_version,
                    token_expires_at > now() AS token_current,
                    downloaded_at IS NULL AS not_downloaded,
                    purged_at IS NULL AND expires_at > now() AS artifact_current
                 FROM privacy_export_artifacts
                 WHERE request_id = $1 AND universe_id = $2 AND user_id = $3",
                &[&request_id, &universe_id, &user_id],
            )
            .await
            .map_err(database_error)?
            .ok_or(PrivacyError::DeliveryDenied)?;
        let stored: Option<Vec<u8>> = row.get("download_token_digest");
        let valid_digest = stored
            .as_deref()
            .filter(|value| value.len() == 32)
            .map(|value| candidate.as_slice().ct_eq(value).unwrap_u8() == 1)
            .unwrap_or(false);
        if !valid_digest
            || !row.get::<_, bool>("token_current")
            || !row.get::<_, bool>("not_downloaded")
            || !row.get::<_, bool>("artifact_current")
        {
            return Err(PrivacyError::DeliveryDenied);
        }
        let ciphertext: Option<Vec<u8>> = row.get("ciphertext");
        let key_id: Option<String> = row.get("encryption_key_id");
        let nonce: Option<Vec<u8>> = row.get("encryption_nonce");
        let plaintext_digest: Option<Vec<u8>> = row.get("plaintext_sha256");
        let download = ExportDownload {
            ciphertext: ciphertext.ok_or(PrivacyError::DeliveryDenied)?,
            encryption_key_id: key_id.ok_or(PrivacyError::DeliveryDenied)?,
            encryption_nonce: fixed_array::<12>(nonce.ok_or(PrivacyError::DeliveryDenied)?)?,
            plaintext_sha256: fixed_array::<32>(
                plaintext_digest.ok_or(PrivacyError::DeliveryDenied)?,
            )?,
            plaintext_size: row.get("plaintext_size"),
            format_version: row.get("format_version"),
        };
        Ok(download)
    }

    /// Atomically consumes a previously verified delivery grant. The digest
    /// comparison is repeated in the update so concurrent download attempts
    /// can prepare in parallel but only one can finalize and receive bytes.
    pub async fn finalize_export_delivery(
        &self,
        universe_id: i64,
        user_id: i32,
        request_id: i32,
        token: &str,
    ) -> Result<(), PrivacyError> {
        if token.is_empty() || token.len() > 200 {
            return Err(PrivacyError::DeliveryDenied);
        }
        let candidate = Sha256::digest(token.as_bytes());
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        let consumed = transaction
            .execute(
                "UPDATE privacy_export_artifacts
                 SET downloaded_at = now(), download_token_digest = NULL,
                     token_issued_at = NULL, token_expires_at = NULL
                 WHERE request_id = $1 AND universe_id = $2 AND user_id = $3
                   AND downloaded_at IS NULL
                   AND download_token_digest = $4
                   AND token_expires_at > now()
                   AND purged_at IS NULL AND expires_at > now()",
                &[&request_id, &universe_id, &user_id, &candidate.as_slice()],
            )
            .await
            .map_err(database_error)?;
        if consumed != 1 {
            return Err(PrivacyError::DeliveryDenied);
        }
        let consume_dedupe = format!("privacy-execution:{request_id}:export_consumed");
        transaction
            .execute(
                "INSERT INTO privacy_execution_events (
                    request_id, universe_id, user_id, action, actor_type,
                    actor_user_id, reason_code, field_names, dedupe_key
                 ) VALUES ($1, $2, $3, 'export_consumed', 'user',
                    $3, 'export_consumed', ARRAY[]::TEXT[], $4)
                 ON CONFLICT (dedupe_key) DO NOTHING",
                &[&request_id, &universe_id, &user_id, &consume_dedupe],
            )
            .await
            .map_err(database_error)?;
        transaction.commit().await.map_err(database_error)?;
        Ok(())
    }

    pub async fn privacy_auth_guard(
        &self,
        universe_id: i64,
        user_id: i32,
        presented_auth_epoch: i64,
    ) -> Result<PrivacyAuthGuard, PrivacyError> {
        if presented_auth_epoch < 0 {
            return Err(PrivacyError::Forbidden);
        }
        let client = self.pool.get().await.map_err(database_error)?;
        let row = client
            .query_opt(
                "SELECT auth_epoch, privacy_restriction_active, privacy_erasure_pending,
                    is_banned
                 FROM users WHERE universe_id = $1 AND id = $2",
                &[&universe_id, &user_id],
            )
            .await
            .map_err(database_error)?
            .ok_or(PrivacyError::NotFound)?;
        if row.get::<_, i64>("auth_epoch") != presented_auth_epoch {
            return Ok(PrivacyAuthGuard::StaleEpoch);
        }
        if row.get::<_, bool>("privacy_restriction_active")
            || row.get::<_, bool>("privacy_erasure_pending")
            || row.get::<_, bool>("is_banned")
        {
            return Ok(PrivacyAuthGuard::Restricted);
        }
        Ok(PrivacyAuthGuard::Allowed)
    }

    pub async fn purge_privacy_retention(
        &self,
        delivered_outbox_retention_days: i32,
    ) -> Result<PrivacyRetentionResult, PrivacyError> {
        self.run_privacy_retention(
            delivered_outbox_retention_days,
            PrivacyRetentionAudit {
                universe_id: None,
                admin_user_id: None,
                communication_evidence_redacted: 0,
                communication_events_deleted: 0,
            },
        )
        .await
        .map(|(result, _)| result)
    }

    /// Applies privacy retention and appends its immutable audit row in the
    /// same transaction. Any audit failure rolls all destructive work back.
    pub async fn run_privacy_retention(
        &self,
        delivered_outbox_retention_days: i32,
        audit: PrivacyRetentionAudit,
    ) -> Result<(PrivacyRetentionResult, i64), PrivacyError> {
        if !(1..=3650).contains(&delivered_outbox_retention_days) {
            return Err(PrivacyError::InvalidInput("outbox retention is invalid"));
        }
        if audit.universe_id.is_some_and(|value| value <= 0)
            || audit.admin_user_id.is_some_and(|value| value <= 0)
            || audit.admin_user_id.is_some() != audit.universe_id.is_some()
        {
            return Err(PrivacyError::InvalidInput(
                "privacy retention actor is invalid",
            ));
        }
        let universe_id = audit.universe_id;
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_actor(
            &transaction,
            if audit.admin_user_id.is_some() {
                "admin"
            } else {
                "system"
            },
            audit.admin_user_id,
            "retention_expired",
        )
        .await?;
        let artifacts_purged = transaction
            .execute(
                "UPDATE privacy_export_artifacts AS artifact
                 SET ciphertext = NULL, encryption_key_id = NULL,
                     encryption_nonce = NULL, plaintext_sha256 = NULL,
                     download_token_digest = NULL, token_issued_at = NULL,
                     token_expires_at = NULL, purged_at = now()
                 FROM gdpr_requests AS request
                 WHERE request.id = artifact.request_id
                   AND request.universe_id = artifact.universe_id
                   AND request.user_id = artifact.user_id
                   AND request.legal_hold_active = FALSE
                   AND ($1::BIGINT IS NULL OR request.universe_id = $1)
                   AND artifact.purged_at IS NULL
                   AND artifact.expires_at <= now()",
                &[&universe_id],
            )
            .await
            .map_err(database_error)?;
        let request_payloads_redacted = transaction
            .execute(
                "UPDATE gdpr_requests
                 SET request_payload_ciphertext = NULL, payload_key_id = NULL,
                     payload_nonce = NULL, payload_sha256 = NULL
                 WHERE legal_hold_active = FALSE
                   AND ($1::BIGINT IS NULL OR universe_id = $1)
                   AND retention_until <= now()
                   AND status IN ('completed', 'cancelled', 'rejected')
                   AND request_payload_ciphertext IS NOT NULL",
                &[&universe_id],
            )
            .await
            .map_err(database_error)?;
        let outbox_rows_deleted = transaction
            .execute(
                "DELETE FROM privacy_outbox AS outbox
                 WHERE outbox.status IN ('delivered', 'cancelled', 'dead')
                   AND outbox.updated_at < now() - ($1::INTEGER * interval '1 day')
                   AND ($2::BIGINT IS NULL OR outbox.universe_id = $2)
                   AND NOT EXISTS (
                       SELECT 1 FROM gdpr_requests AS request
                       WHERE request.id = outbox.request_id
                         AND request.universe_id = outbox.universe_id
                         AND request.user_id = outbox.user_id
                         AND request.legal_hold_active = TRUE
                   )",
                &[&delivered_outbox_retention_days, &universe_id],
            )
            .await
            .map_err(database_error)?;
        let result = PrivacyRetentionResult {
            artifacts_purged,
            request_payloads_redacted,
            outbox_rows_deleted,
        };
        let run_id: i64 = transaction
            .query_one(
                "INSERT INTO privacy_retention_runs (
                    universe_id, actor_type, actor_user_id, reason_code,
                    artifacts_purged, request_payloads_redacted,
                    privacy_outbox_rows_deleted,
                    communication_evidence_redacted,
                    communication_events_deleted
                 ) VALUES (
                    $1, CASE WHEN $2::INTEGER IS NULL THEN 'system' ELSE 'admin' END,
                    $2, 'retention_expired', $3, $4, $5, $6, $7
                 ) RETURNING id",
                &[
                    &audit.universe_id,
                    &audit.admin_user_id,
                    &(result.artifacts_purged as i64),
                    &(result.request_payloads_redacted as i64),
                    &(result.outbox_rows_deleted as i64),
                    &(audit.communication_evidence_redacted as i64),
                    &(audit.communication_events_deleted as i64),
                ],
            )
            .await
            .map_err(database_error)?
            .get("id");
        transaction.commit().await.map_err(database_error)?;
        Ok((result, run_id))
    }
}

pub fn validate_privacy_correction_patch(
    mut patch: PrivacyCorrectionPatch,
) -> Result<PrivacyCorrectionPatch, PrivacyError> {
    normalize_correction_patch(&mut patch)?;
    Ok(patch)
}

fn normalize_correction_patch(patch: &mut PrivacyCorrectionPatch) -> Result<(), PrivacyError> {
    if let Some(username) = &mut patch.username {
        *username = username.trim().to_string();
        if !(3..=32).contains(&username.len())
            || !username
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return Err(PrivacyError::InvalidInput("correction username is invalid"));
        }
    }
    if let Some(email) = &mut patch.email {
        *email = email.trim().to_ascii_lowercase();
        let mut parts = email.split('@');
        let local = parts.next().unwrap_or_default();
        let domain = parts.next().unwrap_or_default();
        if !(3..=254).contains(&email.len())
            || local.is_empty()
            || domain.is_empty()
            || !domain.contains('.')
            || parts.next().is_some()
            || email.bytes().any(|byte| byte.is_ascii_whitespace())
        {
            return Err(PrivacyError::InvalidInput("correction email is invalid"));
        }
    }
    if let Some(Some(phone)) = &mut patch.phone_number {
        *phone = phone.trim().to_string();
        if !(8..=16).contains(&phone.len())
            || !phone.starts_with('+')
            || !phone[1..].bytes().all(|byte| byte.is_ascii_digit())
            || phone.as_bytes().get(1) == Some(&b'0')
        {
            return Err(PrivacyError::InvalidInput(
                "correction phone number is invalid",
            ));
        }
    }
    if patch.field_names().is_empty() {
        return Err(PrivacyError::InvalidInput("correction change set is empty"));
    }
    Ok(())
}

fn correction_database_error(error: tokio_postgres::Error) -> PrivacyError {
    if error.code().is_some_and(|code| code.code() == "23505") {
        PrivacyError::Conflict("corrected identity is already in use")
    } else {
        database_error(error)
    }
}

async fn set_communication_contact_actor(
    transaction: &Transaction<'_>,
    actor_hmac: &[u8; 32],
    reason_code: &str,
) -> Result<(), PrivacyError> {
    transaction
        .execute(
            "SELECT set_config('app.communication_actor_subject_hmac', $1, TRUE),
                    set_config('app.communication_change_reason', $2, TRUE)",
            &[&encode_hex(actor_hmac), &reason_code],
        )
        .await
        .map_err(database_error)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn insert_privacy_execution_event(
    transaction: &Transaction<'_>,
    job: &LockedJob,
    action: &str,
    actor_type: &str,
    actor_user_id: Option<i32>,
    reason_code: &str,
    field_names: &[String],
    subject_hmac: Option<&[u8; 32]>,
) -> Result<(), PrivacyError> {
    let dedupe_key = format!("privacy-execution:{}:{action}", job.request_id);
    let subject_hmac = subject_hmac.map(|value| value.as_slice());
    transaction
        .execute(
            "INSERT INTO privacy_execution_events (
                request_id, universe_id, user_id, action, actor_type,
                actor_user_id, reason_code, field_names, subject_hmac, dedupe_key
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
             ON CONFLICT (dedupe_key) DO NOTHING",
            &[
                &job.request_id,
                &job.universe_id,
                &job.user_id,
                &action,
                &actor_type,
                &actor_user_id,
                &reason_code,
                &field_names,
                &subject_hmac,
                &dedupe_key,
            ],
        )
        .await
        .map_err(database_error)?;
    Ok(())
}

async fn insert_privacy_request_execution_event(
    transaction: &Transaction<'_>,
    job: &LockedJob,
    event_type: &str,
    request_version: i64,
    reason_code: &str,
) -> Result<(), PrivacyError> {
    let dedupe_key = format!("{}:{request_version}:{event_type}", job.request_id);
    transaction
        .execute(
            "INSERT INTO privacy_request_events (
                request_id, universe_id, user_id, event_type, from_status,
                to_status, actor_type, actor_user_id, reason_code,
                request_version, dedupe_key
             ) VALUES ($1, $2, $3, $4, 'processing', 'completed',
                'worker', NULL, $5, $6, $7)
             ON CONFLICT (dedupe_key) DO NOTHING",
            &[
                &job.request_id,
                &job.universe_id,
                &job.user_id,
                &event_type,
                &reason_code,
                &request_version,
                &dedupe_key,
            ],
        )
        .await
        .map_err(database_error)?;
    Ok(())
}

async fn erasure_execution_result(
    transaction: &Transaction<'_>,
    request_id: i32,
) -> Result<PrivacyErasureResult, PrivacyError> {
    let row = transaction
        .query_opt(
            "SELECT credentials_deleted, sessions_deleted,
                personal_content_deleted, contact_evidence_redacted
             FROM privacy_erasure_executions WHERE request_id = $1",
            &[&request_id],
        )
        .await
        .map_err(database_error)?
        .ok_or(PrivacyError::Conflict(
            "erased subject has no execution evidence",
        ))?;
    Ok(PrivacyErasureResult {
        already_completed: false,
        credentials_deleted: row.get::<_, i64>("credentials_deleted") as u64,
        sessions_deleted: row.get::<_, i64>("sessions_deleted") as u64,
        personal_content_deleted: row.get::<_, i64>("personal_content_deleted") as u64,
        contact_evidence_redacted: row.get::<_, i64>("contact_evidence_redacted") as u64,
    })
}

async fn delete_subject_rows_if_present(
    transaction: &Transaction<'_>,
    table: &str,
    user_column: &str,
    universe_id: i64,
    user_id: i32,
) -> Result<u64, PrivacyError> {
    if !valid_sql_identifier(table) || !valid_sql_identifier(user_column) {
        return Err(PrivacyError::InvalidInput(
            "privacy erasure inventory is invalid",
        ));
    }
    let shape = transaction
        .query_one(
            "SELECT
                EXISTS (
                    SELECT 1 FROM information_schema.columns
                    WHERE table_schema = 'public' AND table_name = $1
                      AND column_name = $2
                ) AS has_user,
                EXISTS (
                    SELECT 1 FROM information_schema.columns
                    WHERE table_schema = 'public' AND table_name = $1
                      AND column_name = 'universe_id'
                ) AS has_universe",
            &[&table, &user_column],
        )
        .await
        .map_err(database_error)?;
    if !shape.get::<_, bool>("has_user") {
        return Ok(0);
    }
    let statement = if shape.get::<_, bool>("has_universe") {
        format!("DELETE FROM \"{table}\" WHERE \"{user_column}\" = $1 AND universe_id = $2")
    } else {
        format!("DELETE FROM \"{table}\" WHERE \"{user_column}\" = $1")
    };
    if shape.get::<_, bool>("has_universe") {
        transaction
            .execute(&statement, &[&user_id, &universe_id])
            .await
            .map_err(database_error)
    } else {
        transaction
            .execute(&statement, &[&user_id])
            .await
            .map_err(database_error)
    }
}

async fn redact_subject_metadata(
    transaction: &Transaction<'_>,
    _universe_id: i64,
    user_id: i32,
    pseudonym: &str,
) -> Result<u64, PrivacyError> {
    let mut changed = 0u64;
    changed = changed.saturating_add(
        transaction
            .execute(
                "UPDATE analytics_events
                 SET user_id = NULL, ip_address = NULL, user_agent = NULL,
                     event_properties = '{}'::jsonb
                 WHERE user_id = $1",
                &[&user_id],
            )
            .await
            .map_err(database_error)?,
    );
    changed = changed.saturating_add(
        transaction
            .execute(
                "UPDATE security_audit_logs
                 SET user_id = NULL, ip_address = NULL, user_agent = NULL,
                     metadata = NULL, event_description = 'privacy_erased_subject_event'
                 WHERE user_id = $1",
                &[&user_id],
            )
            .await
            .map_err(database_error)?,
    );
    changed = changed.saturating_add(
        transaction
            .execute(
                "UPDATE shop_security_logs
                 SET user_id = NULL, ip_address = NULL, user_agent = NULL,
                     metadata = NULL
                 WHERE user_id = $1",
                &[&user_id],
            )
            .await
            .map_err(database_error)?,
    );
    changed = changed.saturating_add(
        transaction
            .execute(
                "UPDATE shop_purchases_enhanced
                 SET ip_address = NULL, user_agent = NULL, device_type = NULL,
                     referrer = NULL
                 WHERE user_id = $1",
                &[&user_id],
            )
            .await
            .map_err(database_error)?,
    );
    changed = changed.saturating_add(
        transaction
            .execute(
                "UPDATE shop_gifts
                 SET recipient_email = NULL, personal_message = NULL, gift_code = NULL
                 WHERE sender_user_id = $1 OR recipient_user_id = $1",
                &[&user_id],
            )
            .await
            .map_err(database_error)?,
    );
    changed = changed.saturating_add(
        transaction
            .execute(
                "UPDATE player_leaderboard_snapshots
                 SET username = $2 WHERE user_id = $1",
                &[&user_id, &pseudonym],
            )
            .await
            .map_err(database_error)?,
    );
    changed = changed.saturating_add(
        transaction
            .execute(
                "UPDATE planets SET name = 'Erased planet ' || id::TEXT
                 WHERE user_id = $1",
                &[&user_id],
            )
            .await
            .map_err(database_error)?,
    );
    changed = changed.saturating_add(
        transaction
            .execute(
                "UPDATE moons SET name = 'Erased moon ' || id::TEXT
                 WHERE user_id = $1",
                &[&user_id],
            )
            .await
            .map_err(database_error)?,
    );
    Ok(changed)
}

fn valid_sql_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 63
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

#[derive(Debug)]
struct LockedRequest {
    request_type: String,
    status: String,
    legal_hold_active: bool,
}

#[derive(Debug)]
struct LockedJob {
    request_id: i32,
    universe_id: i64,
    user_id: i32,
    event_type: String,
    status: String,
    lease_owner: Option<String>,
    lease_current: bool,
    attempt_count: i32,
    max_attempts: i32,
}

/// Locks lifecycle rows in the global order used by cancellation, legal-hold,
/// claim, failure and completion transactions: request first, outbox second.
/// Reading immutable identity before either lock lets a worker locate the
/// request without ever taking the outbox lock first.
async fn lock_request_and_job(
    transaction: &Transaction<'_>,
    job_id: i64,
) -> Result<Option<(LockedRequest, LockedJob)>, PrivacyError> {
    let Some(identity) = transaction
        .query_opt(
            "SELECT request_id, universe_id, user_id
             FROM privacy_outbox WHERE id = $1",
            &[&job_id],
        )
        .await
        .map_err(database_error)?
    else {
        return Ok(None);
    };
    let request_id = identity.get::<_, i32>("request_id");
    let universe_id = identity.get::<_, i64>("universe_id");
    let user_id = identity.get::<_, i32>("user_id");
    let request = transaction
        .query_opt(
            "SELECT request_type, status, legal_hold_active
             FROM gdpr_requests
             WHERE id = $1 AND universe_id = $2 AND user_id = $3
             FOR UPDATE",
            &[&request_id, &universe_id, &user_id],
        )
        .await
        .map_err(database_error)?
        .ok_or(PrivacyError::NotFound)?;
    let Some(job) = lock_job(transaction, job_id).await? else {
        return Ok(None);
    };
    if job.request_id != request_id || job.universe_id != universe_id || job.user_id != user_id {
        return Err(PrivacyError::Conflict("privacy job identity changed"));
    }
    Ok(Some((
        LockedRequest {
            request_type: request.get("request_type"),
            status: request.get("status"),
            legal_hold_active: request.get("legal_hold_active"),
        },
        job,
    )))
}

async fn lock_job(
    transaction: &Transaction<'_>,
    job_id: i64,
) -> Result<Option<LockedJob>, PrivacyError> {
    let row = transaction
        .query_opt(
            "SELECT request_id, universe_id, user_id, event_type, status,
                lease_owner, COALESCE(lease_expires_at > now(), FALSE) AS lease_current,
                attempt_count, max_attempts
             FROM privacy_outbox WHERE id = $1 FOR UPDATE",
            &[&job_id],
        )
        .await
        .map_err(database_error)?;
    Ok(row.map(|row| LockedJob {
        request_id: row.get("request_id"),
        universe_id: row.get("universe_id"),
        user_id: row.get("user_id"),
        event_type: row.get("event_type"),
        status: row.get("status"),
        lease_owner: row.get("lease_owner"),
        lease_current: row.get("lease_current"),
        attempt_count: row.get("attempt_count"),
        max_attempts: row.get("max_attempts"),
    }))
}

fn validate_job_lease(
    job: &LockedJob,
    worker_id: &str,
    expected_event: &str,
) -> Result<(), PrivacyError> {
    validate_job_lease_any_type(job, worker_id)?;
    if job.event_type != expected_event {
        return Err(PrivacyError::Conflict(
            "worker handler does not match job type",
        ));
    }
    Ok(())
}

fn validate_job_lease_any_type(job: &LockedJob, worker_id: &str) -> Result<(), PrivacyError> {
    if job.status != "processing"
        || job.lease_owner.as_deref() != Some(worker_id.trim())
        || !job.lease_current
    {
        return Err(PrivacyError::LeaseLost);
    }
    Ok(())
}

fn validate_request_for_job(request: &LockedRequest, event_type: &str) -> Result<(), PrivacyError> {
    if request.legal_hold_active {
        return Err(PrivacyError::LegalHold);
    }
    if request.status != "processing" {
        return Err(PrivacyError::Conflict(
            "privacy request is no longer executable",
        ));
    }
    let expected_request_type = match event_type {
        "privacy.export.prepare" => "export",
        "privacy.restriction.apply" => "restriction",
        "privacy.erasure.invalidate_access" | "privacy.erasure.execute" => "erasure",
        "privacy.correction.apply" => "correction",
        _ => {
            return Err(PrivacyError::Conflict(
                "worker handler does not match job type",
            ))
        }
    };
    if request.request_type != expected_request_type {
        return Err(PrivacyError::Conflict(
            "worker handler does not match request type",
        ));
    }
    Ok(())
}

async fn mark_job_delivered(
    transaction: &Transaction<'_>,
    job_id: i64,
    worker_id: &str,
) -> Result<(), PrivacyError> {
    let updated = transaction
        .execute(
            "UPDATE privacy_outbox
             SET status = 'delivered', delivered_at = now(),
                 lease_owner = NULL, lease_expires_at = NULL, updated_at = now()
             WHERE id = $1 AND status = 'processing' AND lease_owner = $2
               AND lease_expires_at > now()",
            &[&job_id, &worker_id.trim()],
        )
        .await
        .map_err(database_error)?;
    if updated == 1 {
        Ok(())
    } else {
        Err(PrivacyError::LeaseLost)
    }
}

async fn cancel_active_jobs(
    transaction: &Transaction<'_>,
    request_id: i32,
    universe_id: i64,
    user_id: i32,
    reason_code: &str,
) -> Result<u64, PrivacyError> {
    transaction
        .execute(
            "UPDATE privacy_outbox
             SET status = 'cancelled', lease_owner = NULL, lease_expires_at = NULL,
                 last_error_code = $4, updated_at = now()
             WHERE request_id = $1 AND universe_id = $2 AND user_id = $3
               AND status IN ('pending', 'retry', 'processing')",
            &[&request_id, &universe_id, &user_id, &reason_code.trim()],
        )
        .await
        .map_err(database_error)
}

async fn invalidate_processing_jobs_for_hold(
    transaction: &Transaction<'_>,
    request_id: i32,
    universe_id: i64,
    user_id: i32,
) -> Result<u64, PrivacyError> {
    transaction
        .execute(
            "UPDATE privacy_outbox
             SET status = 'retry', available_at = now(),
                 lease_owner = NULL, lease_expires_at = NULL,
                 last_error_code = 'legal_hold', updated_at = now()
             WHERE request_id = $1 AND universe_id = $2 AND user_id = $3
               AND status = 'processing'",
            &[&request_id, &universe_id, &user_id],
        )
        .await
        .map_err(database_error)
}

async fn revoke_sessions(transaction: &Transaction<'_>, user_id: i32) -> Result<(), PrivacyError> {
    transaction
        .execute(
            "UPDATE user_sessions
             SET status = 'revoked', last_activity = now()
             WHERE user_id = $1 AND status = 'active'",
            &[&user_id],
        )
        .await
        .map_err(database_error)?;
    Ok(())
}

async fn enqueue_outbox(
    transaction: &Transaction<'_>,
    request_id: i32,
    universe_id: i64,
    user_id: i32,
    event_type: &str,
) -> Result<(), PrivacyError> {
    let dedupe_key = format!("privacy-request:{request_id}:{event_type}");
    transaction
        .execute(
            "INSERT INTO privacy_outbox (
                request_id, universe_id, user_id, event_type, dedupe_key
             ) VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (dedupe_key) DO NOTHING",
            &[
                &request_id,
                &universe_id,
                &user_id,
                &event_type,
                &dedupe_key,
            ],
        )
        .await
        .map_err(database_error)?;
    Ok(())
}

async fn request_row_by_id(
    transaction: &Transaction<'_>,
    universe_id: i64,
    request_id: i32,
) -> Result<PrivacyRequestRow, PrivacyError> {
    let row = transaction
        .query_opt(
            "SELECT id, universe_id, user_id, request_type, status,
                idempotency_key, EXTRACT(EPOCH FROM requested_at)::BIGINT,
                EXTRACT(EPOCH FROM cooling_off_until)::BIGINT,
                EXTRACT(EPOCH FROM completed_at)::BIGINT,
                EXTRACT(EPOCH FROM cancelled_at)::BIGINT,
                legal_hold_active, EXTRACT(EPOCH FROM retention_until)::BIGINT,
                version
             FROM gdpr_requests WHERE id = $1 AND universe_id = $2",
            &[&request_id, &universe_id],
        )
        .await
        .map_err(database_error)?
        .ok_or(PrivacyError::NotFound)?;
    map_request_row(&row)
}

async fn set_actor(
    transaction: &Transaction<'_>,
    actor_type: &str,
    actor_user_id: Option<i32>,
    reason_code: &str,
) -> Result<(), PrivacyError> {
    transaction
        .query_one(
            "SELECT set_config('app.privacy_actor_type', $1, TRUE),
                    set_config('app.privacy_actor_user_id', COALESCE($2::INTEGER::TEXT, ''), TRUE),
                    set_config('app.privacy_reason_code', $3, TRUE)",
            &[&actor_type, &actor_user_id, &reason_code],
        )
        .await
        .map_err(database_error)?;
    Ok(())
}

fn map_request_row(row: &tokio_postgres::Row) -> Result<PrivacyRequestRow, PrivacyError> {
    Ok(PrivacyRequestRow {
        id: row.get("id"),
        universe_id: row.get("universe_id"),
        user_id: row.get("user_id"),
        request_type: PrivacyRequestType::parse(row.get::<_, String>("request_type").as_str())?,
        status: PrivacyRequestStatus::parse(row.get::<_, String>("status").as_str())?,
        idempotency_key: row.get("idempotency_key"),
        requested_at_unix: row.get(6),
        cooling_off_until_unix: row.get(7),
        completed_at_unix: row.get(8),
        cancelled_at_unix: row.get(9),
        legal_hold_active: row.get("legal_hold_active"),
        retention_until_unix: row.get(11),
        version: row.get("version"),
    })
}

fn map_request_summary(row: &tokio_postgres::Row) -> Result<PrivacyRequestSummary, PrivacyError> {
    let request = map_request_row(row)?;
    let export = if row.get::<_, bool>("export_prepared") {
        Some(PrivacyExportAvailability {
            ready: row.get("export_ready"),
            expired: row.get("export_expired"),
            expires_at_unix: row.get("export_expires_at"),
            plaintext_size: row.get("export_plaintext_size"),
        })
    } else {
        None
    };
    Ok(PrivacyRequestSummary { request, export })
}

fn map_request_event(row: &tokio_postgres::Row) -> Result<PrivacyRequestEventRow, PrivacyError> {
    let from_status = row
        .get::<_, Option<String>>("from_status")
        .map(|status| PrivacyRequestStatus::parse(&status))
        .transpose()?;
    Ok(PrivacyRequestEventRow {
        id: row.get("id"),
        event_type: row.get("event_type"),
        from_status,
        to_status: PrivacyRequestStatus::parse(&row.get::<_, String>("to_status"))?,
        actor_type: row.get("actor_type"),
        reason_code: row.get("reason_code"),
        request_version: row.get("request_version"),
        created_at_unix: row.get("created_at_unix"),
    })
}

fn map_consent_row(row: &tokio_postgres::Row) -> Result<PrivacyConsentRow, PrivacyError> {
    Ok(PrivacyConsentRow {
        purpose: row.get("purpose"),
        channel: row.get("channel"),
        status: ConsentStatus::parse(&row.get::<_, String>("status"))?,
        lawful_basis: row.get("lawful_basis"),
        policy_version: row.get("policy_version"),
        collected_at_unix: row.get("collected_at_unix"),
        expires_at_unix: row.get("expires_at_unix"),
        version: row.get("version"),
    })
}

fn map_communication_row(row: &tokio_postgres::Row) -> CommunicationPreferenceRow {
    CommunicationPreferenceRow {
        channel: row.get("channel"),
        category: row.get("category"),
        enabled: row.get("enabled"),
        explicitly_configured: row.get("explicitly_configured"),
        effective_allowed: row.get("effective_allowed"),
        essential: row.get("essential"),
        marketing_consent_current: row.get("marketing_consent_current"),
        suppressed_by_restriction: row.get("suppressed_by_restriction"),
        updated_at_unix: row.get("updated_at_unix"),
        version: row.get("version"),
    }
}

/// Produce a domain-separated keyed SHA-256 digest for privacy evidence.
/// Callers persist only the returned digest and must not log the evidence or
/// pepper. The production pepper must be independently generated and at least
/// 256 bits.
pub fn privacy_evidence_digest(pepper: &[u8], evidence: &[u8]) -> Result<[u8; 32], PrivacyError> {
    if !(32..=1024).contains(&pepper.len()) {
        return Err(PrivacyError::InvalidInput(
            "privacy evidence pepper is invalid",
        ));
    }
    if evidence.is_empty() || evidence.len() > 4096 {
        return Err(PrivacyError::InvalidInput("privacy evidence is invalid"));
    }

    let mut key = [0u8; 64];
    if pepper.len() > key.len() {
        key[..32].copy_from_slice(&Sha256::digest(pepper));
    } else {
        key[..pepper.len()].copy_from_slice(pepper);
    }
    let mut inner_pad = [0x36u8; 64];
    let mut outer_pad = [0x5cu8; 64];
    for index in 0..key.len() {
        inner_pad[index] ^= key[index];
        outer_pad[index] ^= key[index];
    }
    let mut inner = Sha256::new();
    inner.update(inner_pad);
    inner.update(b"universus-privacy-evidence:v1\0");
    inner.update(evidence);
    let mut inner_digest = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner_digest.as_slice());
    let digest: [u8; 32] = outer.finalize().into();
    key.fill(0);
    inner_pad.fill(0);
    outer_pad.fill(0);
    inner_digest.fill(0);
    Ok(digest)
}

fn validate_owner(universe_id: i64, user_id: i32) -> Result<(), PrivacyError> {
    if universe_id <= 0 || user_id <= 0 {
        return Err(PrivacyError::InvalidInput(
            "tenant and user ids must be positive",
        ));
    }
    Ok(())
}

fn validate_admin_filter(filter: &PrivacyAdminRequestFilter) -> Result<(), PrivacyError> {
    if filter.universe_id <= 0
        || !(1..=200).contains(&filter.limit)
        || filter.user_id.is_some_and(|value| value <= 0)
        || filter.before_request_id.is_some_and(|value| value <= 0)
    {
        return Err(PrivacyError::InvalidInput(
            "privacy administration filter is invalid",
        ));
    }
    Ok(())
}

fn validate_request_input(input: &PrivacyRequestCreateInput) -> Result<(), PrivacyError> {
    validate_owner(input.universe_id, input.user_id)?;
    if input.idempotency_key.trim().is_empty() || input.idempotency_key.len() > 200 {
        return Err(PrivacyError::InvalidInput("idempotency key is invalid"));
    }
    if input.request_source.trim().is_empty() || input.request_source.len() > 100 {
        return Err(PrivacyError::InvalidInput("request source is invalid"));
    }
    if let Some(payload) = &input.encrypted_payload {
        if payload.ciphertext.is_empty() || payload.key_id.trim().is_empty() {
            return Err(PrivacyError::InvalidInput(
                "encrypted request payload is incomplete",
            ));
        }
    }
    if (input.request_type == PrivacyRequestType::Correction) != input.encrypted_payload.is_some() {
        return Err(PrivacyError::InvalidInput(
            "correction requests require exactly one encrypted change set",
        ));
    }
    Ok(())
}

fn validate_consent(input: &ConsentUpdate) -> Result<(), PrivacyError> {
    if input.universe_id <= 0 || input.user_id <= 0 || input.changed_by_user_id <= 0 {
        return Err(PrivacyError::InvalidInput("consent actor ids are invalid"));
    }
    validate_short_identifier(&input.purpose, "consent purpose")?;
    validate_channel(&input.channel, true)?;
    if !matches!(
        input.lawful_basis.as_str(),
        "consent"
            | "contract"
            | "legal_obligation"
            | "vital_interests"
            | "public_task"
            | "legitimate_interests"
    ) {
        return Err(PrivacyError::InvalidInput("lawful basis is invalid"));
    }
    validate_short_identifier(&input.policy_version, "policy version")?;
    if !matches!(input.actor_type.as_str(), "user" | "admin" | "system") {
        return Err(PrivacyError::InvalidInput("consent actor type is invalid"));
    }
    if input.status == ConsentStatus::Granted
        && input.lawful_basis == "consent"
        && input.proof_digest.is_none()
    {
        return Err(PrivacyError::InvalidInput(
            "explicit consent requires a proof digest",
        ));
    }
    Ok(())
}

fn validate_communication(input: &CommunicationPreferenceUpdate) -> Result<(), PrivacyError> {
    if input.universe_id <= 0 || input.user_id <= 0 || input.changed_by_user_id <= 0 {
        return Err(PrivacyError::InvalidInput(
            "communication preference actor ids are invalid",
        ));
    }
    validate_channel(&input.channel, false)?;
    validate_category(&input.category)?;
    if privacy_communication_category_is_essential(&input.category) && !input.enabled {
        return Err(PrivacyError::InvalidInput(
            "essential communications cannot be disabled",
        ));
    }
    if !matches!(input.actor_type.as_str(), "user" | "admin" | "system") {
        return Err(PrivacyError::InvalidInput(
            "communication preference actor type is invalid",
        ));
    }
    Ok(())
}

fn validate_channel(channel: &str, allow_all: bool) -> Result<(), PrivacyError> {
    if matches!(channel, "email" | "in_app" | "push" | "sms") || (allow_all && channel == "all") {
        Ok(())
    } else {
        Err(PrivacyError::InvalidInput(
            "communication channel is invalid",
        ))
    }
}

fn validate_category(category: &str) -> Result<(), PrivacyError> {
    if matches!(
        category,
        "marketing" | "product_updates" | "gameplay_digest" | "security" | "transactional"
    ) {
        Ok(())
    } else {
        Err(PrivacyError::InvalidInput(
            "communication category is invalid",
        ))
    }
}

fn validate_reason_code(reason: &str) -> Result<(), PrivacyError> {
    if reason.trim().is_empty()
        || reason.len() > 100
        || !reason
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err(PrivacyError::InvalidInput("reason code is invalid"));
    }
    Ok(())
}

fn validate_short_identifier(value: &str, field: &'static str) -> Result<(), PrivacyError> {
    if value.trim().is_empty() || value.len() > 100 {
        return Err(PrivacyError::InvalidInput(field));
    }
    Ok(())
}

fn validate_export_artifact(artifact: &PreparedExportArtifact) -> Result<(), PrivacyError> {
    if artifact.ciphertext.is_empty()
        || artifact.encryption_key_id.trim().is_empty()
        || artifact.plaintext_size < 0
        || artifact.plaintext_size as usize == 0
        || !(60..=30 * 24 * 60 * 60).contains(&artifact.expires_in_seconds)
    {
        return Err(PrivacyError::InvalidInput(
            "export artifact metadata is invalid",
        ));
    }
    Ok(())
}

fn initial_outbox_event(request_type: PrivacyRequestType) -> Option<&'static str> {
    match request_type {
        PrivacyRequestType::Export => Some("privacy.export.prepare"),
        PrivacyRequestType::Restriction => Some("privacy.restriction.apply"),
        PrivacyRequestType::Correction | PrivacyRequestType::Erasure => None,
    }
}

fn fixed_array<const N: usize>(value: Vec<u8>) -> Result<[u8; N], PrivacyError> {
    value
        .try_into()
        .map_err(|_| PrivacyError::Database("invalid fixed-length digest metadata".to_string()))
}

async fn export_array_if_table(
    client: &tokio_postgres::Client,
    table: &str,
    query: &str,
    params: &[&(dyn ToSql + Sync)],
) -> Result<serde_json::Value, PrivacyError> {
    let relation = format!("public.{table}");
    let exists = client
        .query_one("SELECT to_regclass($1) IS NOT NULL", &[&relation])
        .await
        .map_err(database_error)?
        .get::<_, bool>(0);
    if !exists {
        return Ok(serde_json::json!([]));
    }
    let row = client
        .query_one(query, params)
        .await
        .map_err(database_error)?;
    let mut value = row.get::<_, Json<serde_json::Value>>("data").0;
    scrub_forbidden_export_fields(&mut value);
    Ok(value)
}

fn scrub_forbidden_export_fields(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                scrub_forbidden_export_fields(value);
            }
        }
        serde_json::Value::Object(fields) => {
            fields.retain(|name, _| !forbidden_export_field(name));
            for value in fields.values_mut() {
                scrub_forbidden_export_fields(value);
            }
        }
        _ => {}
    }
}

fn forbidden_export_field(name: &str) -> bool {
    let normalized = name.to_ascii_lowercase();
    normalized.contains("password")
        || normalized.contains("secret")
        || normalized.contains("token")
        || matches!(
            normalized.as_str(),
            "backup_codes"
                | "requester_ip_digest"
                | "proof_digest"
                | "request_payload_ciphertext"
                | "payload_key_id"
                | "payload_nonce"
                | "payload_sha256"
                | "stripe_payment_intent_id"
                | "stripe_charge_id"
                | "lease_owner"
                | "lease_expires_at"
                | "attempt_count"
                | "max_attempts"
                | "last_error_code"
        )
}

fn database_error(error: impl fmt::Display) -> PrivacyError {
    PrivacyError::Database(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn privacy_evidence_digest_is_keyed_domain_separated_and_bounded() {
        let evidence = b"192.0.2.10";
        let first = privacy_evidence_digest(&[7u8; 32], evidence).unwrap();
        assert_eq!(
            first,
            privacy_evidence_digest(&[7u8; 32], evidence).unwrap()
        );
        assert_ne!(
            first,
            privacy_evidence_digest(&[8u8; 32], evidence).unwrap()
        );
        assert_ne!(
            first,
            privacy_evidence_digest(&[7u8; 32], b"192.0.2.11").unwrap()
        );
        assert!(privacy_evidence_digest(&[7u8; 31], evidence).is_err());
        assert!(privacy_evidence_digest(&[7u8; 32], &[]).is_err());
    }

    #[test]
    fn communication_contract_is_the_complete_four_by_five_matrix() {
        assert_eq!(PRIVACY_COMMUNICATION_CHANNELS.len(), 4);
        assert_eq!(PRIVACY_COMMUNICATION_CATEGORIES.len(), 5);
        assert!(privacy_communication_category_is_essential("security"));
        assert!(privacy_communication_category_is_essential("transactional"));
        assert!(!privacy_communication_category_is_essential("marketing"));
    }
}
