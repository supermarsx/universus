#![forbid(unsafe_code)]

//! SMS provider adapter.
//!
//! Defines the `SmsProvider` trait and supporting types for dispatching
//! SMS messages. Includes:
//! - `CircuitBreaker` — per-channel failure tracking with auto-open/close
//! - `HistoryStore` — SQLite-backed SMS history with idempotency
//! - `LoggingSmsProvider` — writes dispatches to stdout (for dev/test)
//! - `FailingSmsProvider` — always fails (for testing error paths)
//! - Phone number validation (basic E.164)
//! - Rate limiting via history store
//! - Game-specific SMS templates

pub mod circuit_breaker;
pub mod history_store;
pub mod models;

pub use circuit_breaker::{
    ChannelCircuitState, CircuitBreaker, DEFAULT_CHANNEL_COOLDOWN_MS,
    DEFAULT_CHANNEL_FAILURE_THRESHOLD,
};
pub use history_store::{
    HistoryStore, HistoryStoreError, InsertHistoryError, DEFAULT_HISTORY_DB_PATH,
};
pub use models::{HistoryRecord, HistoryRecordInput, HistoryStatsItem};

use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// An SMS dispatch job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmsJob {
    pub job_id: String,
    pub to: String,
    pub body: String,
    pub from: Option<String>,
    /// Channel identifier (e.g. "twilio", "vonage", "mock").
    pub channel: Option<String>,
    /// Idempotency key for deduplication.
    pub idempotency_key: Option<String>,
}

/// Result of a successful SMS dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsDispatchResult {
    pub provider: String,
    pub job_id: String,
    pub message_id: String,
}

/// Trait for SMS dispatch providers.
pub trait SmsProvider: Send + Sync {
    fn name(&self) -> &str;
    fn dispatch(&self, job: &SmsJob) -> Result<SmsDispatchResult, SmsProviderError>;
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmsProviderError {
    InvalidJob {
        field: &'static str,
        reason: String,
    },
    RateLimited {
        contact: String,
        limit: usize,
        window_seconds: u64,
    },
    CircuitOpen {
        channel: String,
    },
    DispatchFailed(String),
}

impl Display for SmsProviderError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidJob { field, reason } => {
                write!(f, "invalid SMS job field '{field}': {reason}")
            }
            Self::RateLimited {
                contact,
                limit,
                window_seconds,
            } => {
                write!(
                    f,
                    "rate limited: {contact} exceeded {limit} messages in {window_seconds}s"
                )
            }
            Self::CircuitOpen { channel } => {
                write!(f, "circuit open for channel '{channel}'")
            }
            Self::DispatchFailed(message) => write!(f, "dispatch failed: {message}"),
        }
    }
}

impl Error for SmsProviderError {}

// ---------------------------------------------------------------------------
// Phone number validation
// ---------------------------------------------------------------------------

/// Basic E.164 phone number validation.
/// Accepts numbers starting with `+` followed by 7-15 digits.
pub fn validate_phone_number(number: &str) -> Result<(), String> {
    let trimmed = number.trim();
    if trimmed.is_empty() {
        return Err("phone number must not be empty".to_string());
    }
    if !trimmed.starts_with('+') {
        return Err("phone number must start with '+' (E.164 format)".to_string());
    }
    let digits: String = trimmed[1..]
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect();
    if digits.len() < 7 || digits.len() > 15 {
        return Err(format!(
            "phone number must have 7-15 digits after '+', got {}",
            digits.len()
        ));
    }
    // Ensure no non-digit characters besides the leading '+'
    let non_digit_count = trimmed[1..].chars().filter(|c| !c.is_ascii_digit()).count();
    if non_digit_count > 0 {
        return Err("phone number must contain only digits after '+'".to_string());
    }
    Ok(())
}

fn validate_sms_job(job: &SmsJob) -> Result<(), SmsProviderError> {
    if job.job_id.trim().is_empty() {
        return Err(SmsProviderError::InvalidJob {
            field: "job_id",
            reason: "must not be empty".to_string(),
        });
    }
    if let Err(reason) = validate_phone_number(&job.to) {
        return Err(SmsProviderError::InvalidJob {
            field: "to",
            reason,
        });
    }
    if job.body.trim().is_empty() {
        return Err(SmsProviderError::InvalidJob {
            field: "body",
            reason: "must not be empty".to_string(),
        });
    }
    if job.body.len() > 1600 {
        return Err(SmsProviderError::InvalidJob {
            field: "body",
            reason: format!(
                "must not exceed 1600 characters (concatenated SMS limit), got {}",
                job.body.len()
            ),
        });
    }
    if let Some(ref from) = job.from {
        if let Err(reason) = validate_phone_number(from) {
            return Err(SmsProviderError::InvalidJob {
                field: "from",
                reason,
            });
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// LoggingSmsProvider
// ---------------------------------------------------------------------------

/// A provider that logs dispatches to stdout. Useful for development and testing.
#[derive(Debug)]
pub struct LoggingSmsProvider {
    name: String,
    sequence: AtomicU64,
}

impl LoggingSmsProvider {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            sequence: AtomicU64::new(0),
        }
    }

    pub fn dispatch_count(&self) -> u64 {
        self.sequence.load(Ordering::Relaxed)
    }
}

impl Default for LoggingSmsProvider {
    fn default() -> Self {
        Self::new("logging-sms")
    }
}

impl SmsProvider for LoggingSmsProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn dispatch(&self, job: &SmsJob) -> Result<SmsDispatchResult, SmsProviderError> {
        validate_sms_job(job)?;

        let next = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        let message_id = format!("{}-{next}", self.name);

        println!(
            "sms-dispatch provider={} job_id={} to={} body_len={}",
            self.name,
            job.job_id,
            job.to,
            job.body.len()
        );

        Ok(SmsDispatchResult {
            provider: self.name.clone(),
            job_id: job.job_id.clone(),
            message_id,
        })
    }
}

// ---------------------------------------------------------------------------
// FailingSmsProvider
// ---------------------------------------------------------------------------

/// A provider that always fails. Useful for testing error handling paths.
#[derive(Debug, Clone)]
pub struct FailingSmsProvider {
    name: String,
    error_message: String,
}

impl FailingSmsProvider {
    pub fn new(name: impl Into<String>, error_message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            error_message: error_message.into(),
        }
    }
}

impl Default for FailingSmsProvider {
    fn default() -> Self {
        Self::new("failing-sms", "simulated SMS dispatch failure")
    }
}

impl SmsProvider for FailingSmsProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn dispatch(&self, job: &SmsJob) -> Result<SmsDispatchResult, SmsProviderError> {
        validate_sms_job(job)?;
        Err(SmsProviderError::DispatchFailed(format!(
            "{}: {}",
            self.error_message, job.job_id
        )))
    }
}

// ---------------------------------------------------------------------------
// Rate limiter
// ---------------------------------------------------------------------------

/// Rate limiter configuration for SMS dispatch.
#[derive(Debug, Clone)]
pub struct SmsRateLimiter {
    pub max_per_window: usize,
    pub window_seconds: u64,
}

impl Default for SmsRateLimiter {
    fn default() -> Self {
        Self {
            max_per_window: 5,
            window_seconds: 3600,
        }
    }
}

impl SmsRateLimiter {
    pub fn new(max_per_window: usize, window_seconds: u64) -> Self {
        Self {
            max_per_window,
            window_seconds,
        }
    }

    /// Check whether a contact has exceeded the rate limit.
    /// Returns `Ok(current_count)` if under limit, or `Err` if over.
    pub fn check(
        &self,
        store: &HistoryStore,
        contact: &str,
        now_ms: u128,
    ) -> Result<usize, SmsProviderError> {
        let count = store
            .count_recent_for_contact(contact, self.window_seconds, now_ms)
            .map_err(|e| SmsProviderError::DispatchFailed(e.to_string()))?;

        if count >= self.max_per_window {
            return Err(SmsProviderError::RateLimited {
                contact: contact.to_string(),
                limit: self.max_per_window,
                window_seconds: self.window_seconds,
            });
        }

        Ok(count)
    }
}

// ---------------------------------------------------------------------------
// SMS templates
// ---------------------------------------------------------------------------

/// Common game SMS templates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmsTemplate {
    VerificationCode { code: String },
    AttackWarning { attacker: String, planet: String },
    FleetArrived { destination: String },
    AllianceInvite { alliance_name: String },
    AccountLocked { reason: String },
}

impl SmsTemplate {
    /// Render the template to message body text.
    pub fn render(&self) -> String {
        match self {
            Self::VerificationCode { code } => {
                format!("Your Universus verification code is: {code}. Do not share this code.")
            }
            Self::AttackWarning { attacker, planet } => {
                format!("ALERT: {attacker} is attacking your planet {planet}! Defend now!")
            }
            Self::FleetArrived { destination } => {
                format!("Your fleet has arrived at {destination}.")
            }
            Self::AllianceInvite { alliance_name } => {
                format!(
                    "You've been invited to join alliance '{alliance_name}'. Log in to respond."
                )
            }
            Self::AccountLocked { reason } => {
                format!("Your Universus account has been locked: {reason}. Contact support.")
            }
        }
    }

    /// Build an `SmsJob` from a template.
    pub fn to_job(
        &self,
        job_id: impl Into<String>,
        to: impl Into<String>,
        from: Option<String>,
    ) -> SmsJob {
        SmsJob {
            job_id: job_id.into(),
            to: to.into(),
            body: self.render(),
            from,
            channel: None,
            idempotency_key: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_job() -> SmsJob {
        SmsJob {
            job_id: "sms-1".to_string(),
            to: "+12065550123".to_string(),
            body: "Hello world".to_string(),
            from: Some("+12065550199".to_string()),
            channel: Some("twilio".to_string()),
            idempotency_key: None,
        }
    }

    // --- Phone validation ---

    #[test]
    fn validate_phone_number_accepts_valid() {
        assert!(validate_phone_number("+12065550123").is_ok());
        assert!(validate_phone_number("+1234567").is_ok()); // 7 digits
        assert!(validate_phone_number("+123456789012345").is_ok()); // 15 digits
    }

    #[test]
    fn validate_phone_number_rejects_invalid() {
        assert!(validate_phone_number("").is_err());
        assert!(validate_phone_number("12065550123").is_err()); // no +
        assert!(validate_phone_number("+123").is_err()); // too short
        assert!(validate_phone_number("+1234567890123456").is_err()); // too long (16)
        assert!(validate_phone_number("+1234abc567").is_err()); // letters
        assert!(validate_phone_number("+").is_err()); // just +
    }

    // --- Job validation ---

    #[test]
    fn validate_sms_job_valid() {
        assert!(validate_sms_job(&fixture_job()).is_ok());
    }

    #[test]
    fn validate_sms_job_empty_job_id() {
        let mut job = fixture_job();
        job.job_id = "".to_string();
        let err = validate_sms_job(&job).unwrap_err();
        assert!(matches!(
            err,
            SmsProviderError::InvalidJob {
                field: "job_id",
                ..
            }
        ));
    }

    #[test]
    fn validate_sms_job_invalid_to() {
        let mut job = fixture_job();
        job.to = "not-a-number".to_string();
        let err = validate_sms_job(&job).unwrap_err();
        assert!(matches!(
            err,
            SmsProviderError::InvalidJob { field: "to", .. }
        ));
    }

    #[test]
    fn validate_sms_job_empty_body() {
        let mut job = fixture_job();
        job.body = "  ".to_string();
        let err = validate_sms_job(&job).unwrap_err();
        assert!(matches!(
            err,
            SmsProviderError::InvalidJob { field: "body", .. }
        ));
    }

    #[test]
    fn validate_sms_job_body_too_long() {
        let mut job = fixture_job();
        job.body = "x".repeat(1601);
        let err = validate_sms_job(&job).unwrap_err();
        assert!(matches!(
            err,
            SmsProviderError::InvalidJob { field: "body", .. }
        ));
    }

    #[test]
    fn validate_sms_job_invalid_from() {
        let mut job = fixture_job();
        job.from = Some("bad".to_string());
        let err = validate_sms_job(&job).unwrap_err();
        assert!(matches!(
            err,
            SmsProviderError::InvalidJob { field: "from", .. }
        ));
    }

    // --- Logging provider ---

    #[test]
    fn logging_provider_dispatches() {
        let provider = LoggingSmsProvider::new("test-sms");
        let job = fixture_job();

        let result = provider.dispatch(&job).unwrap();
        assert_eq!(result.provider, "test-sms");
        assert_eq!(result.job_id, "sms-1");
        assert_eq!(result.message_id, "test-sms-1");
        assert_eq!(provider.dispatch_count(), 1);

        let result2 = provider.dispatch(&job).unwrap();
        assert_eq!(result2.message_id, "test-sms-2");
        assert_eq!(provider.dispatch_count(), 2);
    }

    #[test]
    fn logging_provider_validates() {
        let provider = LoggingSmsProvider::default();
        let mut job = fixture_job();
        job.to = "invalid".to_string();
        assert!(provider.dispatch(&job).is_err());
        assert_eq!(provider.dispatch_count(), 0);
    }

    // --- Failing provider ---

    #[test]
    fn failing_provider_always_fails() {
        let provider = FailingSmsProvider::default();
        let job = fixture_job();
        let err = provider.dispatch(&job).unwrap_err();
        assert!(matches!(err, SmsProviderError::DispatchFailed(_)));
        assert!(err.to_string().contains("simulated"));
    }

    #[test]
    fn failing_provider_still_validates() {
        let provider = FailingSmsProvider::new("test", "boom");
        let mut job = fixture_job();
        job.to = "".to_string();
        let err = provider.dispatch(&job).unwrap_err();
        assert!(matches!(err, SmsProviderError::InvalidJob { .. }));
    }

    // --- Templates ---

    #[test]
    fn template_verification_code() {
        let tpl = SmsTemplate::VerificationCode {
            code: "123456".to_string(),
        };
        let body = tpl.render();
        assert!(body.contains("123456"));
        assert!(body.contains("verification code"));
    }

    #[test]
    fn template_attack_warning() {
        let tpl = SmsTemplate::AttackWarning {
            attacker: "EvilPlayer".to_string(),
            planet: "4:56:7".to_string(),
        };
        let body = tpl.render();
        assert!(body.contains("EvilPlayer"));
        assert!(body.contains("4:56:7"));
    }

    #[test]
    fn template_to_job() {
        let tpl = SmsTemplate::FleetArrived {
            destination: "1:200:3".to_string(),
        };
        let job = tpl.to_job("sms-fleet-1", "+12065550123", None);
        assert_eq!(job.job_id, "sms-fleet-1");
        assert_eq!(job.to, "+12065550123");
        assert!(job.body.contains("1:200:3"));
        assert!(job.channel.is_none());
    }

    // --- Error display ---

    #[test]
    fn error_display_messages() {
        let e1 = SmsProviderError::InvalidJob {
            field: "to",
            reason: "bad number".to_string(),
        };
        assert!(e1.to_string().contains("to"));

        let e2 = SmsProviderError::RateLimited {
            contact: "+1234567890".to_string(),
            limit: 5,
            window_seconds: 3600,
        };
        assert!(e2.to_string().contains("rate limited"));

        let e3 = SmsProviderError::CircuitOpen {
            channel: "twilio".to_string(),
        };
        assert!(e3.to_string().contains("circuit open"));

        let e4 = SmsProviderError::DispatchFailed("timeout".to_string());
        assert!(e4.to_string().contains("timeout"));
    }

    // --- Serde round-trip ---

    #[test]
    fn sms_job_serde_roundtrip() {
        let job = fixture_job();
        let json = serde_json::to_string(&job).unwrap();
        let deserialized: SmsJob = serde_json::from_str(&json).unwrap();
        assert_eq!(job, deserialized);
    }

    // --- Rate limiter ---

    #[test]
    fn rate_limiter_allows_under_limit() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let store = HistoryStore::new(tmp.path()).unwrap();
        let limiter = SmsRateLimiter::new(3, 60);

        // Insert 2 records within window
        for i in 0..2 {
            store
                .insert_history(&HistoryRecordInput {
                    request_id: format!("req-{i}"),
                    idempotency_key: None,
                    contact: "+12065550123".to_string(),
                    destination: "+12065550123".to_string(),
                    channel: "twilio".to_string(),
                    status: "success".to_string(),
                    error: None,
                    metadata: None,
                    created_at_ms: 50_000 + (i as u128 * 1000),
                })
                .unwrap();
        }

        let result = limiter.check(&store, "+12065550123", 55_000);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 2);
    }

    #[test]
    fn rate_limiter_blocks_over_limit() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let store = HistoryStore::new(tmp.path()).unwrap();
        let limiter = SmsRateLimiter::new(2, 60);

        for i in 0..2 {
            store
                .insert_history(&HistoryRecordInput {
                    request_id: format!("req-{i}"),
                    idempotency_key: None,
                    contact: "+12065550123".to_string(),
                    destination: "+12065550123".to_string(),
                    channel: "twilio".to_string(),
                    status: "success".to_string(),
                    error: None,
                    metadata: None,
                    created_at_ms: 50_000 + (i as u128 * 1000),
                })
                .unwrap();
        }

        let result = limiter.check(&store, "+12065550123", 55_000);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SmsProviderError::RateLimited { .. }
        ));
    }
}
