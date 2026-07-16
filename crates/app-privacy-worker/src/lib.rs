//! Production runtime for the durable privacy outbox.
//!
//! The worker claims database leases, applies access restriction and erasure
//! authorization jobs, and prepares subject-access exports. Export JSON is
//! serialized into a bounded buffer and encrypted with AES-256-GCM before it
//! enters PostgreSQL. Logs and operational events contain only aggregate
//! counts, job kinds, attempts, and stable error codes.

#![forbid(unsafe_code)]

use aes_gcm::{
    aead::{Aead, Payload},
    Aes256Gcm, KeyInit, Nonce,
};
use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use platform_db::{Database, PreparedExportArtifact, PrivacyError, PrivacyJob};
use rand::{rngs::OsRng, RngCore};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream},
    sync::{
        atomic::{AtomicBool, AtomicI64, Ordering},
        Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
use tokio::{sync::oneshot, task::JoinHandle, task::JoinSet, time::timeout};
use zeroize::{Zeroize, Zeroizing};

pub const SERVICE_NAME: &str = "app-privacy-worker";
pub const HEALTH_PATH: &str = "/health";
pub const READINESS_PATH: &str = "/ready";
const EXPORT_AAD_VERSION: &[u8] = b"universus-privacy-export:aes-256-gcm:v1\0";

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum WorkerError {
    #[error("required worker configuration is missing")]
    MissingConfiguration,
    #[error("worker configuration is invalid")]
    InvalidConfiguration,
    #[error("database pool configuration failed")]
    DatabaseConfiguration,
    #[error("privacy repository is not ready")]
    RepositoryNotReady,
    #[error("privacy outbox claim failed")]
    ClaimFailed,
    #[error("health server failed")]
    HealthServer,
    #[error("health probe failed")]
    HealthProbe,
}

impl WorkerError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::MissingConfiguration => "missing_configuration",
            Self::InvalidConfiguration => "invalid_configuration",
            Self::DatabaseConfiguration => "database_configuration_failed",
            Self::RepositoryNotReady => "privacy_repository_not_ready",
            Self::ClaimFailed => "privacy_claim_failed",
            Self::HealthServer => "health_server_failed",
            Self::HealthProbe => "health_probe_failed",
        }
    }
}

/// Validated runtime configuration. Secret fields deliberately do not
/// implement `Debug` and are zeroized on drop.
pub struct WorkerConfig {
    pub worker_id: String,
    pub universe_id: Option<i64>,
    pub claim_limit: i64,
    pub claim_timeout: Duration,
    pub lease_seconds: i64,
    pub job_timeout: Duration,
    pub retry_delay_seconds: i64,
    pub poll_interval: Duration,
    pub run_once: bool,
    pub health_addr: SocketAddr,
    pub readiness_stale_after: Duration,
    pub export_key_id: String,
    pub export_key: Zeroizing<[u8; 32]>,
    pub export_max_plaintext_bytes: usize,
    pub export_expires_in_seconds: i64,
    pub realtime_url: Option<String>,
    pub realtime_token: Option<Zeroizing<String>>,
}

impl WorkerConfig {
    pub fn from_env() -> Result<Self, WorkerError> {
        Self::from_lookup(&|name| std::env::var(name).ok())
    }

    pub fn from_lookup(lookup: &dyn Fn(&str) -> Option<String>) -> Result<Self, WorkerError> {
        let database_url = required_secret(lookup, "DATABASE_URL")?;
        drop(database_url);
        let worker_id = required_text(lookup, "PRIVACY_WORKER_ID")?;
        if worker_id.len() > 200 || !valid_identifier(&worker_id) {
            return Err(WorkerError::InvalidConfiguration);
        }

        let universe_id = optional_value(lookup, "PRIVACY_WORKER_UNIVERSE_ID")
            .map(|value| value.parse::<i64>())
            .transpose()
            .map_err(|_| WorkerError::InvalidConfiguration)?;
        if universe_id.is_some_and(|value| value <= 0) {
            return Err(WorkerError::InvalidConfiguration);
        }

        let claim_limit = bounded_i64(lookup, "PRIVACY_WORKER_CLAIM_LIMIT", 8, 1, 100)?;
        let lease_seconds = bounded_i64(lookup, "PRIVACY_WORKER_LEASE_SECS", 60, 5, 3600)?;
        let default_claim_timeout = (lease_seconds - 1).clamp(1, 5);
        let claim_timeout_seconds = bounded_i64(
            lookup,
            "PRIVACY_WORKER_CLAIM_TIMEOUT_SECS",
            default_claim_timeout,
            1,
            30,
        )?;
        if claim_timeout_seconds >= lease_seconds {
            return Err(WorkerError::InvalidConfiguration);
        }
        let default_timeout = (lease_seconds - 5).max(1);
        let job_timeout_seconds = bounded_i64(
            lookup,
            "PRIVACY_WORKER_JOB_TIMEOUT_SECS",
            default_timeout,
            1,
            3599,
        )?;
        if job_timeout_seconds >= lease_seconds {
            return Err(WorkerError::InvalidConfiguration);
        }
        let retry_delay_seconds = bounded_i64(
            lookup,
            "PRIVACY_WORKER_RETRY_DELAY_SECS",
            30,
            0,
            24 * 60 * 60,
        )?;
        let poll_interval_ms =
            bounded_u64(lookup, "PRIVACY_WORKER_POLL_INTERVAL_MS", 1000, 50, 60_000)?;
        let run_once = strict_bool(lookup, "PRIVACY_WORKER_RUN_ONCE", false)?;
        let health_addr = optional_value(lookup, "PRIVACY_WORKER_HEALTH_ADDR")
            .unwrap_or_else(|| "0.0.0.0:3010".to_string())
            .parse::<SocketAddr>()
            .map_err(|_| WorkerError::InvalidConfiguration)?;
        let default_readiness_stale_seconds =
            poll_interval_ms.div_ceil(1000).saturating_mul(3).max(30);
        let readiness_stale_seconds = bounded_u64(
            lookup,
            "PRIVACY_WORKER_READINESS_STALE_SECS",
            default_readiness_stale_seconds,
            5,
            3600,
        )?;

        let export_key_id = required_text(lookup, "PRIVACY_EXPORT_KEY_ID")?;
        if !valid_export_key_id(&export_key_id) {
            return Err(WorkerError::InvalidConfiguration);
        }
        let encoded_key = required_secret(lookup, "PRIVACY_EXPORT_KEY_BASE64")?;
        let mut decoded_key = Zeroizing::new(
            STANDARD
                .decode(encoded_key.as_bytes())
                .map_err(|_| WorkerError::InvalidConfiguration)?,
        );
        if decoded_key.len() != 32 {
            return Err(WorkerError::InvalidConfiguration);
        }
        let mut export_key = Zeroizing::new([0u8; 32]);
        export_key.copy_from_slice(&decoded_key);
        decoded_key.zeroize();

        let export_max_plaintext_bytes = bounded_u64(
            lookup,
            "PRIVACY_EXPORT_MAX_PLAINTEXT_BYTES",
            16 * 1024 * 1024,
            1024,
            64 * 1024 * 1024,
        )? as usize;
        let export_expires_in_seconds = bounded_i64(
            lookup,
            "PRIVACY_EXPORT_RETENTION_SECS",
            7 * 24 * 60 * 60,
            60,
            30 * 24 * 60 * 60,
        )?;

        let realtime_url = optional_value(lookup, "REALTIME_GATEWAY_URL");
        if realtime_url.as_ref().is_some_and(|url| {
            url.len() > 2048 || !(url.starts_with("http://") || url.starts_with("https://"))
        }) {
            return Err(WorkerError::InvalidConfiguration);
        }
        let realtime_token = if realtime_url.is_some() {
            Some(required_secret(lookup, "PLATFORM_EVENTS_SERVICE_TOKEN")?)
        } else {
            None
        };

        Ok(Self {
            worker_id,
            universe_id,
            claim_limit,
            claim_timeout: Duration::from_secs(claim_timeout_seconds as u64),
            lease_seconds,
            job_timeout: Duration::from_secs(job_timeout_seconds as u64),
            retry_delay_seconds,
            poll_interval: Duration::from_millis(poll_interval_ms),
            run_once,
            health_addr,
            readiness_stale_after: Duration::from_secs(readiness_stale_seconds),
            export_key_id,
            export_key,
            export_max_plaintext_bytes,
            export_expires_in_seconds,
            realtime_url,
            realtime_token,
        })
    }
}

fn required_text(
    lookup: &dyn Fn(&str) -> Option<String>,
    name: &str,
) -> Result<String, WorkerError> {
    optional_value(lookup, name).ok_or(WorkerError::MissingConfiguration)
}

fn required_secret(
    lookup: &dyn Fn(&str) -> Option<String>,
    name: &str,
) -> Result<Zeroizing<String>, WorkerError> {
    let raw = Zeroizing::new(lookup(name).ok_or(WorkerError::MissingConfiguration)?);
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(WorkerError::MissingConfiguration);
    }
    Ok(Zeroizing::new(trimmed.to_string()))
}

fn optional_value(lookup: &dyn Fn(&str) -> Option<String>, name: &str) -> Option<String> {
    lookup(name)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn bounded_i64(
    lookup: &dyn Fn(&str) -> Option<String>,
    name: &str,
    default: i64,
    minimum: i64,
    maximum: i64,
) -> Result<i64, WorkerError> {
    let value = optional_value(lookup, name)
        .map(|value| value.parse::<i64>())
        .transpose()
        .map_err(|_| WorkerError::InvalidConfiguration)?
        .unwrap_or(default);
    if !(minimum..=maximum).contains(&value) {
        return Err(WorkerError::InvalidConfiguration);
    }
    Ok(value)
}

fn bounded_u64(
    lookup: &dyn Fn(&str) -> Option<String>,
    name: &str,
    default: u64,
    minimum: u64,
    maximum: u64,
) -> Result<u64, WorkerError> {
    let value = optional_value(lookup, name)
        .map(|value| value.parse::<u64>())
        .transpose()
        .map_err(|_| WorkerError::InvalidConfiguration)?
        .unwrap_or(default);
    if !(minimum..=maximum).contains(&value) {
        return Err(WorkerError::InvalidConfiguration);
    }
    Ok(value)
}

fn strict_bool(
    lookup: &dyn Fn(&str) -> Option<String>,
    name: &str,
    default: bool,
) -> Result<bool, WorkerError> {
    match optional_value(lookup, name) {
        None => Ok(default),
        Some(value) if value == "1" || value.eq_ignore_ascii_case("true") => Ok(true),
        Some(value) if value == "0" || value.eq_ignore_ascii_case("false") => Ok(false),
        Some(_) => Err(WorkerError::InvalidConfiguration),
    }
}

fn valid_identifier(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

fn valid_export_key_id(value: &str) -> bool {
    let Some(rotation_id) = value.strip_prefix("v1:") else {
        return false;
    };
    !rotation_id.is_empty() && value.len() <= 128 && valid_identifier(rotation_id)
}

pub struct ExportEncryptor {
    key_id: String,
    key: Zeroizing<[u8; 32]>,
    max_plaintext_bytes: usize,
}

impl ExportEncryptor {
    pub fn new(
        key_id: String,
        key: Zeroizing<[u8; 32]>,
        max_plaintext_bytes: usize,
    ) -> Result<Self, WorkerError> {
        if !valid_export_key_id(&key_id) || max_plaintext_bytes == 0 {
            return Err(WorkerError::InvalidConfiguration);
        }
        Ok(Self {
            key_id,
            key,
            max_plaintext_bytes,
        })
    }

    pub fn prepare_artifact(
        &self,
        snapshot: &serde_json::Value,
        expires_in_seconds: i64,
    ) -> Result<PreparedExportArtifact, ExportPreparationError> {
        let mut serialized = BoundedBuffer::new(self.max_plaintext_bytes);
        if serde_json::to_writer(&mut serialized, snapshot).is_err() {
            return Err(if serialized.exceeded {
                ExportPreparationError::TooLarge
            } else {
                ExportPreparationError::Serialization
            });
        }
        let plaintext_size =
            i64::try_from(serialized.bytes.len()).map_err(|_| ExportPreparationError::TooLarge)?;
        let plaintext = serialized.bytes;
        let plaintext_sha256: [u8; 32] = Sha256::digest(plaintext.as_slice()).into();

        let cipher = Aes256Gcm::new_from_slice(self.key.as_slice())
            .map_err(|_| ExportPreparationError::Encryption)?;
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce);
        let mut aad = Vec::with_capacity(EXPORT_AAD_VERSION.len() + self.key_id.len());
        aad.extend_from_slice(EXPORT_AAD_VERSION);
        aad.extend_from_slice(self.key_id.as_bytes());
        let ciphertext = cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: plaintext.as_slice(),
                    aad: &aad,
                },
            )
            .map_err(|_| ExportPreparationError::Encryption)?;
        aad.zeroize();

        Ok(PreparedExportArtifact {
            ciphertext,
            encryption_key_id: self.key_id.clone(),
            encryption_nonce: nonce,
            plaintext_sha256,
            plaintext_size,
            expires_in_seconds,
        })
    }

    pub fn aad_for_key_id(key_id: &str) -> Vec<u8> {
        let mut aad = Vec::with_capacity(EXPORT_AAD_VERSION.len() + key_id.len());
        aad.extend_from_slice(EXPORT_AAD_VERSION);
        aad.extend_from_slice(key_id.as_bytes());
        aad
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportPreparationError {
    TooLarge,
    Serialization,
    Encryption,
}

impl ExportPreparationError {
    const fn code(self) -> &'static str {
        match self {
            Self::TooLarge => "export_too_large",
            Self::Serialization => "export_serialization_failed",
            Self::Encryption => "export_encryption_failed",
        }
    }
}

struct BoundedBuffer {
    bytes: Zeroizing<Vec<u8>>,
    maximum: usize,
    exceeded: bool,
}

impl BoundedBuffer {
    fn new(maximum: usize) -> Self {
        Self {
            bytes: Zeroizing::new(Vec::with_capacity(maximum.min(64 * 1024))),
            maximum,
            exceeded: false,
        }
    }
}

impl Write for BoundedBuffer {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let Some(new_len) = self.bytes.len().checked_add(buffer.len()) else {
            self.exceeded = true;
            return Err(std::io::Error::other("privacy export size limit"));
        };
        if new_len > self.maximum {
            self.exceeded = true;
            return Err(std::io::Error::other("privacy export size limit"));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct ProcessorSettings {
    pub worker_id: String,
    pub universe_id: Option<i64>,
    pub claim_limit: i64,
    pub claim_timeout: Duration,
    pub lease_seconds: i64,
    pub job_timeout: Duration,
    pub retry_delay_seconds: i64,
    pub export_expires_in_seconds: i64,
}

impl ProcessorSettings {
    pub fn validate(&self) -> Result<(), WorkerError> {
        if self.worker_id.is_empty()
            || self.worker_id.len() > 200
            || !valid_identifier(&self.worker_id)
            || !(1..=100).contains(&self.claim_limit)
            || !(5..=3600).contains(&self.lease_seconds)
            || self.claim_timeout.is_zero()
            || self.claim_timeout >= Duration::from_secs(self.lease_seconds as u64)
            || self.job_timeout.is_zero()
            || self.job_timeout >= Duration::from_secs(self.lease_seconds as u64)
            || !(0..=24 * 60 * 60).contains(&self.retry_delay_seconds)
            || !(60..=30 * 24 * 60 * 60).contains(&self.export_expires_in_seconds)
            || self.universe_id.is_some_and(|value| value <= 0)
        {
            return Err(WorkerError::InvalidConfiguration);
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct OpsPublisher {
    endpoint: Option<OpsEndpoint>,
}

#[derive(Clone)]
struct OpsEndpoint {
    base_url: String,
    token: Arc<Zeroizing<String>>,
}

impl OpsPublisher {
    pub fn disabled() -> Self {
        Self { endpoint: None }
    }

    pub fn new(base_url: String, token: Zeroizing<String>) -> Result<Self, WorkerError> {
        if base_url.trim().is_empty() || token.trim().is_empty() {
            return Err(WorkerError::InvalidConfiguration);
        }
        Ok(Self {
            endpoint: Some(OpsEndpoint {
                base_url,
                token: Arc::new(token),
            }),
        })
    }

    pub fn is_enabled(&self) -> bool {
        self.endpoint.is_some()
    }

    pub async fn publish<T: Serialize>(&self, event_type: &'static str, payload: &T) {
        let Some(endpoint) = &self.endpoint else {
            return;
        };
        let event = platform_events::build_event(event_type, payload);
        let delivered = timeout(
            Duration::from_secs(3),
            platform_events::publish_http_with_token(
                &endpoint.base_url,
                "ops.privacy",
                &event,
                endpoint.token.as_str(),
            ),
        )
        .await
        .is_ok_and(|result| result.is_ok());
        if !delivered {
            tracing::warn!(
                service = SERVICE_NAME,
                event_type,
                error_code = "ops_publish_failed",
                "privacy operational event was not delivered"
            );
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CycleReport {
    pub claimed: u64,
    pub completed: u64,
    pub failure_recorded: u64,
    pub lease_lost: u64,
    pub failure_unrecorded: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobOutcome {
    Completed,
    FailureRecorded { error_code: &'static str },
    LeaseLost,
    FailureUnrecorded { error_code: &'static str },
}

#[derive(Clone)]
pub struct PrivacyWorker {
    database: Database,
    settings: Arc<ProcessorSettings>,
    encryptor: Arc<ExportEncryptor>,
    ops: OpsPublisher,
}

impl PrivacyWorker {
    pub fn new(
        database: Database,
        settings: ProcessorSettings,
        encryptor: ExportEncryptor,
        ops: OpsPublisher,
    ) -> Result<Self, WorkerError> {
        settings.validate()?;
        Ok(Self {
            database,
            settings: Arc::new(settings),
            encryptor: Arc::new(encryptor),
            ops,
        })
    }

    pub async fn run_cycle(&self) -> Result<CycleReport, WorkerError> {
        let jobs = timeout(
            self.settings.claim_timeout,
            self.database.claim_privacy_jobs(
                &self.settings.worker_id,
                self.settings.universe_id,
                self.settings.claim_limit,
                self.settings.lease_seconds,
            ),
        )
        .await
        .map_err(|_| WorkerError::ClaimFailed)?
        .map_err(|_| WorkerError::ClaimFailed)?;
        let mut report = CycleReport {
            claimed: jobs.len() as u64,
            ..CycleReport::default()
        };
        let mut tasks = JoinSet::new();
        for job in jobs {
            let worker = self.clone();
            tasks.spawn(async move { worker.process_claimed_job(job).await });
        }
        while let Some(result) = tasks.join_next().await {
            let outcome = result.unwrap_or(JobOutcome::FailureUnrecorded {
                error_code: "worker_task_failed",
            });
            match outcome {
                JobOutcome::Completed => report.completed += 1,
                JobOutcome::FailureRecorded { .. } => report.failure_recorded += 1,
                JobOutcome::LeaseLost => report.lease_lost += 1,
                JobOutcome::FailureUnrecorded { .. } => report.failure_unrecorded += 1,
            }
        }
        Ok(report)
    }

    pub async fn process_claimed_job(&self, job: PrivacyJob) -> JobOutcome {
        let job_type = job.event_type.clone();
        let attempt = job.attempt_count;
        let dispatch = timeout(self.settings.job_timeout, self.dispatch(&job)).await;
        let outcome = match dispatch {
            Ok(Ok(())) => JobOutcome::Completed,
            Ok(Err(failure)) if failure.lease_lost => JobOutcome::LeaseLost,
            Ok(Err(failure)) => self.record_failure(&job, failure.code).await,
            Err(_) => self.record_failure(&job, "job_timeout").await,
        };
        match outcome {
            JobOutcome::Completed => {
                tracing::info!(
                    service = SERVICE_NAME,
                    job_type,
                    attempt,
                    "privacy job completed"
                );
                self.ops
                    .publish(
                        "privacy.worker.job_completed",
                        &serde_json::json!({"jobType": job_type, "attempt": attempt}),
                    )
                    .await;
            }
            JobOutcome::FailureRecorded { error_code } => {
                tracing::warn!(
                    service = SERVICE_NAME,
                    job_type,
                    attempt,
                    error_code,
                    "privacy job failure was recorded for retry or dead letter"
                );
                self.ops
                    .publish(
                        "privacy.worker.job_failed",
                        &serde_json::json!({
                            "jobType": job_type,
                            "attempt": attempt,
                            "errorCode": error_code,
                            "recorded": true
                        }),
                    )
                    .await;
            }
            JobOutcome::LeaseLost => {
                tracing::warn!(
                    service = SERVICE_NAME,
                    job_type,
                    attempt,
                    error_code = "lease_lost",
                    "privacy job lease was lost"
                );
            }
            JobOutcome::FailureUnrecorded { error_code } => {
                tracing::error!(
                    service = SERVICE_NAME,
                    job_type,
                    attempt,
                    error_code,
                    "privacy job failure could not be recorded"
                );
            }
        }
        outcome
    }

    async fn dispatch(&self, job: &PrivacyJob) -> Result<(), DispatchFailure> {
        match job.event_type.as_str() {
            "privacy.restriction.apply" => self
                .database
                .complete_privacy_restriction_job(job.id, &self.settings.worker_id)
                .await
                .map(|_| ())
                .map_err(|error| DispatchFailure::privacy(error, "restriction_apply_failed")),
            "privacy.erasure.invalidate_access" => self
                .database
                .complete_erasure_authorization_job(job.id, &self.settings.worker_id)
                .await
                .map(|_| ())
                .map_err(|error| DispatchFailure::privacy(error, "erasure_access_failed")),
            "privacy.export.prepare" => {
                let mut snapshot = self
                    .database
                    .privacy_export_snapshot(job.universe_id, job.user_id)
                    .await
                    .map_err(|error| DispatchFailure::privacy(error, "export_snapshot_failed"))?;
                let artifact = self
                    .encryptor
                    .prepare_artifact(&snapshot, self.settings.export_expires_in_seconds);
                scrub_snapshot_memory(&mut snapshot);
                let artifact = artifact.map_err(|error| DispatchFailure {
                    code: error.code(),
                    lease_lost: false,
                })?;
                self.database
                    .complete_privacy_export_job(job.id, &self.settings.worker_id, artifact)
                    .await
                    .map(|_| ())
                    .map_err(|error| DispatchFailure::privacy(error, "export_persist_failed"))
            }
            _ => Err(DispatchFailure {
                code: "unsupported_job_type",
                lease_lost: false,
            }),
        }
    }

    async fn record_failure(&self, job: &PrivacyJob, error_code: &'static str) -> JobOutcome {
        let lease_duration = Duration::from_secs(self.settings.lease_seconds as u64);
        let record_timeout = lease_duration
            .checked_sub(self.settings.job_timeout)
            .unwrap_or(Duration::from_secs(1));
        match timeout(
            record_timeout,
            self.database.fail_privacy_job(
                job.id,
                &self.settings.worker_id,
                error_code,
                self.settings.retry_delay_seconds,
            ),
        )
        .await
        {
            Err(_) => JobOutcome::FailureUnrecorded { error_code },
            Ok(Ok(())) => JobOutcome::FailureRecorded { error_code },
            Ok(Err(PrivacyError::LeaseLost)) => JobOutcome::LeaseLost,
            Ok(Err(_)) => JobOutcome::FailureUnrecorded { error_code },
        }
    }
}

fn scrub_snapshot_memory(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(text) => text.zeroize(),
        serde_json::Value::Array(values) => {
            for value in values {
                scrub_snapshot_memory(value);
            }
        }
        serde_json::Value::Object(fields) => {
            for value in fields.values_mut() {
                scrub_snapshot_memory(value);
            }
        }
        _ => {}
    }
    *value = serde_json::Value::Null;
}

struct DispatchFailure {
    code: &'static str,
    lease_lost: bool,
}

impl DispatchFailure {
    fn privacy(error: PrivacyError, default_code: &'static str) -> Self {
        match error {
            PrivacyError::LeaseLost => Self {
                code: "lease_lost",
                lease_lost: true,
            },
            PrivacyError::LegalHold => Self {
                code: "legal_hold_active",
                lease_lost: false,
            },
            PrivacyError::CoolingOff => Self {
                code: "cooling_off_active",
                lease_lost: false,
            },
            PrivacyError::NotFound => Self {
                code: "privacy_job_not_found",
                lease_lost: false,
            },
            _ => Self {
                code: default_code,
                lease_lost: false,
            },
        }
    }
}

pub struct HealthState {
    live: AtomicBool,
    ready: AtomicBool,
    last_database_success_unix: AtomicI64,
    stale_after_seconds: i64,
}

impl HealthState {
    pub fn new(stale_after: Duration) -> Arc<Self> {
        Arc::new(Self {
            live: AtomicBool::new(true),
            ready: AtomicBool::new(false),
            last_database_success_unix: AtomicI64::new(0),
            stale_after_seconds: stale_after.as_secs().min(i64::MAX as u64) as i64,
        })
    }

    pub fn mark_database_success(&self) {
        self.last_database_success_unix
            .store(unix_timestamp(), Ordering::Release);
        self.ready.store(true, Ordering::Release);
    }

    pub fn mark_not_ready(&self) {
        self.ready.store(false, Ordering::Release);
    }

    pub fn mark_shutdown(&self) {
        self.ready.store(false, Ordering::Release);
        self.live.store(false, Ordering::Release);
    }

    pub fn is_live(&self) -> bool {
        self.live.load(Ordering::Acquire)
    }

    pub fn is_ready(&self) -> bool {
        let last_success = self.last_database_success_unix.load(Ordering::Acquire);
        self.is_live()
            && self.ready.load(Ordering::Acquire)
            && last_success > 0
            && unix_timestamp().saturating_sub(last_success) <= self.stale_after_seconds
    }
}

pub struct HealthServer {
    address: SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
    task: JoinHandle<Result<(), WorkerError>>,
}

impl HealthServer {
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    pub async fn shutdown(mut self) -> Result<(), WorkerError> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.task.await.map_err(|_| WorkerError::HealthServer)??;
        Ok(())
    }
}

pub fn spawn_health_server(
    address: SocketAddr,
    state: Arc<HealthState>,
) -> Result<HealthServer, WorkerError> {
    let listener = std::net::TcpListener::bind(address).map_err(|_| WorkerError::HealthServer)?;
    let bound_address = listener
        .local_addr()
        .map_err(|_| WorkerError::HealthServer)?;
    listener
        .set_nonblocking(true)
        .map_err(|_| WorkerError::HealthServer)?;
    let app = Router::new()
        .route(HEALTH_PATH, get(health_handler))
        .route(READINESS_PATH, get(readiness_handler))
        .with_state(state);
    let server = axum::Server::from_tcp(listener)
        .map_err(|_| WorkerError::HealthServer)?
        .serve(app.into_make_service());
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let task = tokio::spawn(async move {
        server
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await
            .map_err(|_| WorkerError::HealthServer)
    });
    Ok(HealthServer {
        address: bound_address,
        shutdown: Some(shutdown_tx),
        task,
    })
}

async fn health_handler(State(state): State<Arc<HealthState>>) -> impl IntoResponse {
    let status = if state.is_live() {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        Json(serde_json::json!({
            "service": SERVICE_NAME,
            "status": if status == StatusCode::OK { "live" } else { "stopping" }
        })),
    )
}

async fn readiness_handler(State(state): State<Arc<HealthState>>) -> impl IntoResponse {
    let status = if state.is_ready() {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        Json(serde_json::json!({
            "service": SERVICE_NAME,
            "status": if status == StatusCode::OK { "ready" } else { "not_ready" }
        })),
    )
}

pub fn healthcheck_from_env() -> Result<(), WorkerError> {
    let address = std::env::var("PRIVACY_WORKER_HEALTH_ADDR")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "0.0.0.0:3010".to_string())
        .parse::<SocketAddr>()
        .map_err(|_| WorkerError::HealthProbe)?;
    healthcheck(address)
}

pub fn healthcheck(mut address: SocketAddr) -> Result<(), WorkerError> {
    match address.ip() {
        IpAddr::V4(address_v4) if address_v4.is_unspecified() => {
            address.set_ip(IpAddr::V4(Ipv4Addr::LOCALHOST));
        }
        IpAddr::V6(address_v6) if address_v6.is_unspecified() => {
            address.set_ip(IpAddr::V6(Ipv6Addr::LOCALHOST));
        }
        _ => {}
    }
    probe_http(address, READINESS_PATH)
}

fn probe_http(address: SocketAddr, path: &str) -> Result<(), WorkerError> {
    let timeout = Duration::from_secs(2);
    let mut stream =
        TcpStream::connect_timeout(&address, timeout).map_err(|_| WorkerError::HealthProbe)?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|_| WorkerError::HealthProbe)?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|_| WorkerError::HealthProbe)?;
    let request = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .map_err(|_| WorkerError::HealthProbe)?;
    let mut response = [0u8; 128];
    let read = stream
        .read(&mut response)
        .map_err(|_| WorkerError::HealthProbe)?;
    let status_line =
        std::str::from_utf8(&response[..read]).map_err(|_| WorkerError::HealthProbe)?;
    if status_line.starts_with("HTTP/1.1 200") || status_line.starts_with("HTTP/1.0 200") {
        Ok(())
    } else {
        Err(WorkerError::HealthProbe)
    }
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().min(i64::MAX as u64) as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aes_gcm::aead::Aead;
    use std::collections::HashMap;

    fn valid_environment() -> HashMap<String, String> {
        HashMap::from([
            (
                "DATABASE_URL".to_string(),
                "postgres://test.invalid/db".to_string(),
            ),
            (
                "PRIVACY_WORKER_ID".to_string(),
                "privacy-worker-test".to_string(),
            ),
            (
                "PRIVACY_EXPORT_KEY_ID".to_string(),
                "v1:test-key".to_string(),
            ),
            (
                "PRIVACY_EXPORT_KEY_BASE64".to_string(),
                STANDARD.encode([7u8; 32]),
            ),
            ("PRIVACY_WORKER_RUN_ONCE".to_string(), "true".to_string()),
        ])
    }

    #[test]
    fn configuration_requires_explicit_valid_key_and_unique_worker_id() {
        let environment = valid_environment();
        let config = WorkerConfig::from_lookup(&|name| environment.get(name).cloned()).unwrap();
        assert!(config.run_once);
        assert_eq!(config.export_key.as_slice(), &[7u8; 32]);

        let mut missing_key = valid_environment();
        missing_key.remove("PRIVACY_EXPORT_KEY_BASE64");
        assert_eq!(
            WorkerConfig::from_lookup(&|name| missing_key.get(name).cloned())
                .err()
                .unwrap(),
            WorkerError::MissingConfiguration
        );

        let mut wrong_size = valid_environment();
        wrong_size.insert(
            "PRIVACY_EXPORT_KEY_BASE64".to_string(),
            STANDARD.encode([9u8; 31]),
        );
        assert_eq!(
            WorkerConfig::from_lookup(&|name| wrong_size.get(name).cloned())
                .err()
                .unwrap(),
            WorkerError::InvalidConfiguration
        );

        let mut unversioned = valid_environment();
        unversioned.insert("PRIVACY_EXPORT_KEY_ID".to_string(), "current".to_string());
        assert!(WorkerConfig::from_lookup(&|name| unversioned.get(name).cloned()).is_err());
    }

    #[test]
    fn configuration_rejects_lease_timeout_and_bounds_mismatches() {
        let mut environment = valid_environment();
        environment.insert("PRIVACY_WORKER_LEASE_SECS".to_string(), "10".to_string());
        environment.insert(
            "PRIVACY_WORKER_JOB_TIMEOUT_SECS".to_string(),
            "10".to_string(),
        );
        assert!(WorkerConfig::from_lookup(&|name| environment.get(name).cloned()).is_err());

        environment.insert(
            "PRIVACY_WORKER_JOB_TIMEOUT_SECS".to_string(),
            "9".to_string(),
        );
        environment.insert(
            "PRIVACY_EXPORT_MAX_PLAINTEXT_BYTES".to_string(),
            "1023".to_string(),
        );
        assert!(WorkerConfig::from_lookup(&|name| environment.get(name).cloned()).is_err());
    }

    #[test]
    fn minimum_lease_has_safe_derived_claim_and_job_timeouts() {
        let mut environment = valid_environment();
        environment.insert("PRIVACY_WORKER_LEASE_SECS".to_string(), "5".to_string());
        let config = WorkerConfig::from_lookup(&|name| environment.get(name).cloned()).unwrap();
        assert_eq!(config.claim_timeout, Duration::from_secs(4));
        assert_eq!(config.job_timeout, Duration::from_secs(1));
    }

    #[test]
    fn export_is_authenticated_randomized_and_bounded() {
        let key = [42u8; 32];
        let encryptor =
            ExportEncryptor::new("v1:unit-test".to_string(), Zeroizing::new(key), 4096).unwrap();
        let snapshot = serde_json::json!({"profile": {"email": "subject@example.test"}});
        let first = encryptor.prepare_artifact(&snapshot, 3600).unwrap();
        let second = encryptor.prepare_artifact(&snapshot, 3600).unwrap();
        assert_ne!(first.encryption_nonce, second.encryption_nonce);
        assert_ne!(first.ciphertext, second.ciphertext);
        assert!(!first
            .ciphertext
            .windows("subject@example.test".len())
            .any(|window| window == b"subject@example.test"));

        let cipher = Aes256Gcm::new_from_slice(&key).unwrap();
        let plaintext = cipher
            .decrypt(
                Nonce::from_slice(&first.encryption_nonce),
                Payload {
                    msg: &first.ciphertext,
                    aad: &ExportEncryptor::aad_for_key_id(&first.encryption_key_id),
                },
            )
            .unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&plaintext).unwrap(),
            snapshot
        );
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(&plaintext)),
            first.plaintext_sha256
        );

        let tiny = ExportEncryptor::new("v1:tiny".to_string(), Zeroizing::new(key), 8).unwrap();
        assert_eq!(
            tiny.prepare_artifact(&snapshot, 3600),
            Err(ExportPreparationError::TooLarge)
        );
    }

    #[test]
    fn readiness_is_fail_closed_and_tracks_shutdown() {
        let state = HealthState::new(Duration::from_secs(30));
        assert!(state.is_live());
        assert!(!state.is_ready());
        state.mark_database_success();
        assert!(state.is_ready());
        state.mark_not_ready();
        assert!(!state.is_ready());
        state.mark_database_success();
        state.mark_shutdown();
        assert!(!state.is_live());
        assert!(!state.is_ready());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn health_server_exposes_live_and_fail_closed_ready_probes() {
        let state = HealthState::new(Duration::from_secs(30));
        let server =
            spawn_health_server("127.0.0.1:0".parse().unwrap(), Arc::clone(&state)).unwrap();
        let address = server.address();
        assert!(
            tokio::task::spawn_blocking(move || probe_http(address, HEALTH_PATH))
                .await
                .unwrap()
                .is_ok()
        );
        let address = server.address();
        assert!(tokio::task::spawn_blocking(move || healthcheck(address))
            .await
            .unwrap()
            .is_err());
        state.mark_database_success();
        let address = server.address();
        assert!(tokio::task::spawn_blocking(move || healthcheck(address))
            .await
            .unwrap()
            .is_ok());
        state.mark_not_ready();
        let address = server.address();
        assert!(tokio::task::spawn_blocking(move || healthcheck(address))
            .await
            .unwrap()
            .is_err());
        server.shutdown().await.unwrap();
    }
}
