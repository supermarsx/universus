#![forbid(unsafe_code)]

//! Email provider adapter.
//!
//! Defines the `EmailProvider` trait and supporting types for dispatching
//! transactional emails. Includes:
//! - `LoggingEmailProvider` — writes dispatches to stdout (for dev/test)
//! - `FailingEmailProvider` — always fails (for testing error paths)
//! - `EmailJobBuilder` — builder pattern for constructing `EmailJob` instances
//! - `EmailTemplate` — common transactional email templates
//! - Payload parsing from JSON (string or bytes)

use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// An email dispatch job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmailJob {
    pub job_id: String,
    pub to: String,
    pub subject: String,
    pub body: String,
    pub from: Option<String>,
    pub reply_to: Option<String>,
    /// Optional content type hint ("text/plain" or "text/html").
    #[serde(default)]
    pub content_type: Option<String>,
    /// Optional tags for analytics/filtering.
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Result of a successful email dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailDispatchResult {
    pub provider: String,
    pub job_id: String,
    pub message_id: String,
}

/// Trait for email dispatch providers.
pub trait EmailProvider: Send + Sync {
    fn name(&self) -> &str;
    fn dispatch(&self, job: &EmailJob) -> Result<EmailDispatchResult, EmailProviderError>;
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmailProviderError {
    InvalidJob {
        field: &'static str,
        reason: &'static str,
    },
    DispatchFailed(String),
}

impl Display for EmailProviderError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidJob { field, reason } => {
                write!(f, "invalid email job field '{field}': {reason}")
            }
            Self::DispatchFailed(message) => write!(f, "dispatch failed: {message}"),
        }
    }
}

impl Error for EmailProviderError {}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

fn validate_non_empty(value: &str, field: &'static str) -> Result<(), EmailProviderError> {
    if value.trim().is_empty() {
        return Err(EmailProviderError::InvalidJob {
            field,
            reason: "must not be empty",
        });
    }
    Ok(())
}

/// Basic email address validation: non-empty, contains `@`, has parts
/// on both sides.
fn validate_email_address(value: &str, field: &'static str) -> Result<(), EmailProviderError> {
    validate_non_empty(value, field)?;
    let trimmed = value.trim();
    let at_pos = trimmed.find('@');
    match at_pos {
        Some(pos) if pos > 0 && pos < trimmed.len() - 1 => Ok(()),
        _ => Err(EmailProviderError::InvalidJob {
            field,
            reason: "must be a valid email address (user@domain)",
        }),
    }
}

fn validate_job(job: &EmailJob) -> Result<(), EmailProviderError> {
    validate_non_empty(&job.job_id, "job_id")?;
    validate_email_address(&job.to, "to")?;
    validate_non_empty(&job.subject, "subject")?;
    validate_non_empty(&job.body, "body")?;
    if let Some(ref from) = job.from {
        validate_email_address(from, "from")?;
    }
    if let Some(ref reply_to) = job.reply_to {
        validate_email_address(reply_to, "reply_to")?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// LoggingEmailProvider
// ---------------------------------------------------------------------------

/// A provider that logs dispatches to stdout. Useful for development and testing.
#[derive(Debug)]
pub struct LoggingEmailProvider {
    name: String,
    sequence: AtomicU64,
}

impl LoggingEmailProvider {
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

impl Default for LoggingEmailProvider {
    fn default() -> Self {
        Self::new("logging")
    }
}

impl EmailProvider for LoggingEmailProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn dispatch(&self, job: &EmailJob) -> Result<EmailDispatchResult, EmailProviderError> {
        validate_job(job)?;

        let next = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        let message_id = format!("{}-{next}", self.name);

        println!(
            "email-dispatch provider={} job_id={} to={} subject={}",
            self.name, job.job_id, job.to, job.subject
        );

        Ok(EmailDispatchResult {
            provider: self.name.clone(),
            job_id: job.job_id.clone(),
            message_id,
        })
    }
}

// ---------------------------------------------------------------------------
// FailingEmailProvider
// ---------------------------------------------------------------------------

/// A provider that always fails. Useful for testing error handling paths.
#[derive(Debug, Clone)]
pub struct FailingEmailProvider {
    name: String,
    error_message: String,
}

impl FailingEmailProvider {
    pub fn new(name: impl Into<String>, error_message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            error_message: error_message.into(),
        }
    }
}

impl Default for FailingEmailProvider {
    fn default() -> Self {
        Self::new("failing", "simulated dispatch failure")
    }
}

impl EmailProvider for FailingEmailProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn dispatch(&self, job: &EmailJob) -> Result<EmailDispatchResult, EmailProviderError> {
        validate_job(job)?;
        Err(EmailProviderError::DispatchFailed(format!(
            "{}: {}",
            self.error_message, job.job_id
        )))
    }
}

// ---------------------------------------------------------------------------
// EmailJobBuilder
// ---------------------------------------------------------------------------

/// Builder for constructing `EmailJob` instances.
#[derive(Debug, Default)]
pub struct EmailJobBuilder {
    job_id: Option<String>,
    to: Option<String>,
    subject: Option<String>,
    body: Option<String>,
    from: Option<String>,
    reply_to: Option<String>,
    content_type: Option<String>,
    tags: Vec<String>,
}

impl EmailJobBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn job_id(mut self, id: impl Into<String>) -> Self {
        self.job_id = Some(id.into());
        self
    }

    pub fn to(mut self, to: impl Into<String>) -> Self {
        self.to = Some(to.into());
        self
    }

    pub fn subject(mut self, subject: impl Into<String>) -> Self {
        self.subject = Some(subject.into());
        self
    }

    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }

    pub fn from(mut self, from: impl Into<String>) -> Self {
        self.from = Some(from.into());
        self
    }

    pub fn reply_to(mut self, reply_to: impl Into<String>) -> Self {
        self.reply_to = Some(reply_to.into());
        self
    }

    pub fn content_type(mut self, ct: impl Into<String>) -> Self {
        self.content_type = Some(ct.into());
        self
    }

    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn build(self) -> Result<EmailJob, &'static str> {
        Ok(EmailJob {
            job_id: self.job_id.ok_or("job_id is required")?,
            to: self.to.ok_or("to is required")?,
            subject: self.subject.ok_or("subject is required")?,
            body: self.body.ok_or("body is required")?,
            from: self.from,
            reply_to: self.reply_to,
            content_type: self.content_type,
            tags: self.tags,
        })
    }
}

// ---------------------------------------------------------------------------
// Email templates
// ---------------------------------------------------------------------------

/// Common transactional email templates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmailTemplate {
    Welcome {
        username: String,
    },
    PasswordReset {
        reset_link: String,
        expires_minutes: u32,
    },
    AccountVerification {
        verify_link: String,
    },
    FleetArrival {
        fleet_id: String,
        destination: String,
    },
    AttackIncoming {
        attacker: String,
        arrival_time: String,
    },
    AllianceInvite {
        alliance_name: String,
        inviter: String,
    },
}

impl EmailTemplate {
    /// Render the template to a subject and body pair.
    pub fn render(&self) -> (String, String) {
        match self {
            Self::Welcome { username } => (
                "Welcome to Universus!".to_string(),
                format!(
                    "Hello {username},\n\n\
                     Welcome to Universus! Your galactic empire awaits.\n\n\
                     Start building your first planet and explore the universe.\n\n\
                     Good luck, Commander!"
                ),
            ),
            Self::PasswordReset {
                reset_link,
                expires_minutes,
            } => (
                "Password Reset Request".to_string(),
                format!(
                    "You requested a password reset.\n\n\
                     Click the link below to reset your password:\n\
                     {reset_link}\n\n\
                     This link expires in {expires_minutes} minutes.\n\n\
                     If you did not request this, please ignore this email."
                ),
            ),
            Self::AccountVerification { verify_link } => (
                "Verify Your Account".to_string(),
                format!(
                    "Please verify your account by clicking the link below:\n\
                     {verify_link}\n\n\
                     If you did not create an account, please ignore this email."
                ),
            ),
            Self::FleetArrival {
                fleet_id,
                destination,
            } => (
                format!("Fleet {fleet_id} has arrived"),
                format!(
                    "Your fleet {fleet_id} has arrived at {destination}.\n\n\
                     Check your fleet overview for details."
                ),
            ),
            Self::AttackIncoming {
                attacker,
                arrival_time,
            } => (
                "Incoming Attack!".to_string(),
                format!(
                    "WARNING: An attack from {attacker} is incoming!\n\n\
                     Estimated arrival: {arrival_time}\n\n\
                     Prepare your defenses immediately."
                ),
            ),
            Self::AllianceInvite {
                alliance_name,
                inviter,
            } => (
                format!("Alliance Invitation: {alliance_name}"),
                format!(
                    "You have been invited to join the alliance '{alliance_name}' \
                     by {inviter}.\n\n\
                     Log in to accept or decline the invitation."
                ),
            ),
        }
    }

    /// Build an `EmailJob` from a template.
    pub fn to_job(
        &self,
        job_id: impl Into<String>,
        to: impl Into<String>,
        from: Option<String>,
    ) -> EmailJob {
        let (subject, body) = self.render();
        EmailJob {
            job_id: job_id.into(),
            to: to.into(),
            subject,
            body,
            from,
            reply_to: None,
            content_type: Some("text/plain".to_string()),
            tags: vec![self.template_tag().to_string()],
        }
    }

    /// Tag name for analytics/filtering.
    fn template_tag(&self) -> &'static str {
        match self {
            Self::Welcome { .. } => "welcome",
            Self::PasswordReset { .. } => "password_reset",
            Self::AccountVerification { .. } => "account_verification",
            Self::FleetArrival { .. } => "fleet_arrival",
            Self::AttackIncoming { .. } => "attack_incoming",
            Self::AllianceInvite { .. } => "alliance_invite",
        }
    }
}

// ---------------------------------------------------------------------------
// Payload parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmailPayloadParseError {
    EmptyPayload,
    InvalidUtf8,
    InvalidJson(String),
    MissingField(&'static str),
    InvalidFieldType {
        field: &'static str,
        expected: &'static str,
    },
    InvalidFieldValue {
        field: &'static str,
        reason: &'static str,
    },
}

impl Display for EmailPayloadParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyPayload => write!(f, "payload is empty"),
            Self::InvalidUtf8 => write!(f, "payload is not valid UTF-8"),
            Self::InvalidJson(err) => write!(f, "payload is not valid JSON: {err}"),
            Self::MissingField(field) => write!(f, "payload is missing required field '{field}'"),
            Self::InvalidFieldType { field, expected } => {
                write!(f, "payload field '{field}' must be {expected}")
            }
            Self::InvalidFieldValue { field, reason } => {
                write!(f, "payload field '{field}' is invalid: {reason}")
            }
        }
    }
}

impl Error for EmailPayloadParseError {}

pub fn parse_email_job_payload(payload: &str) -> Result<EmailJob, EmailPayloadParseError> {
    if payload.trim().is_empty() {
        return Err(EmailPayloadParseError::EmptyPayload);
    }

    let root: Value = serde_json::from_str(payload)
        .map_err(|err| EmailPayloadParseError::InvalidJson(err.to_string()))?;
    let object = root
        .as_object()
        .ok_or(EmailPayloadParseError::InvalidFieldType {
            field: "root",
            expected: "a JSON object",
        })?;

    let job_id = parse_required_string(object, "job_id")?;
    let to = parse_required_string(object, "to")?;
    let subject = parse_required_string(object, "subject")?;
    let body = parse_required_string(object, "body")?;
    let from = parse_optional_string(object, "from")?;
    let reply_to = parse_optional_string(object, "reply_to")?;
    let content_type = parse_optional_string(object, "content_type")?;
    let tags = parse_string_array(object, "tags");

    Ok(EmailJob {
        job_id,
        to,
        subject,
        body,
        from,
        reply_to,
        content_type,
        tags,
    })
}

pub fn parse_email_job_payload_bytes(payload: &[u8]) -> Result<EmailJob, EmailPayloadParseError> {
    let body = std::str::from_utf8(payload).map_err(|_| EmailPayloadParseError::InvalidUtf8)?;
    parse_email_job_payload(body)
}

fn parse_required_string(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<String, EmailPayloadParseError> {
    let value = object
        .get(field)
        .ok_or(EmailPayloadParseError::MissingField(field))?;
    let parsed = value
        .as_str()
        .ok_or(EmailPayloadParseError::InvalidFieldType {
            field,
            expected: "a string",
        })?
        .trim();

    if parsed.is_empty() {
        return Err(EmailPayloadParseError::InvalidFieldValue {
            field,
            reason: "must not be empty",
        });
    }

    Ok(parsed.to_owned())
}

fn parse_optional_string(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<Option<String>, EmailPayloadParseError> {
    let Some(value) = object.get(field) else {
        return Ok(None);
    };

    if value.is_null() {
        return Ok(None);
    }

    let parsed = value
        .as_str()
        .ok_or(EmailPayloadParseError::InvalidFieldType {
            field,
            expected: "a string or null",
        })?
        .trim();

    if parsed.is_empty() {
        return Err(EmailPayloadParseError::InvalidFieldValue {
            field,
            reason: "must not be empty when present",
        });
    }

    Ok(Some(parsed.to_owned()))
}

fn parse_string_array(object: &serde_json::Map<String, Value>, field: &str) -> Vec<String> {
    object
        .get(field)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_job() -> EmailJob {
        EmailJob {
            job_id: "job-1".to_owned(),
            to: "to@example.com".to_owned(),
            subject: "Welcome".to_owned(),
            body: "Hello".to_owned(),
            from: Some("from@example.com".to_owned()),
            reply_to: Some("reply@example.com".to_owned()),
            content_type: None,
            tags: vec![],
        }
    }

    // --- Payload parsing ---

    #[test]
    fn parse_email_job_payload_parses_valid_json() {
        let payload = r#"{
            "job_id": "job-123",
            "to": "user@example.com",
            "subject": "Subject",
            "body": "Body",
            "from": "sender@example.com",
            "reply_to": "noreply@example.com"
        }"#;

        let parsed = parse_email_job_payload(payload).expect("payload should parse");

        assert_eq!(parsed.job_id, "job-123");
        assert_eq!(parsed.to, "user@example.com");
        assert_eq!(parsed.subject, "Subject");
        assert_eq!(parsed.body, "Body");
        assert_eq!(parsed.from.as_deref(), Some("sender@example.com"));
        assert_eq!(parsed.reply_to.as_deref(), Some("noreply@example.com"));
        assert!(parsed.tags.is_empty());
    }

    #[test]
    fn parse_email_job_payload_with_tags() {
        let payload = r#"{
            "job_id": "job-t",
            "to": "user@example.com",
            "subject": "Sub",
            "body": "Body",
            "tags": ["welcome", "onboarding"]
        }"#;
        let parsed = parse_email_job_payload(payload).unwrap();
        assert_eq!(parsed.tags, vec!["welcome", "onboarding"]);
    }

    #[test]
    fn parse_email_job_payload_with_content_type() {
        let payload = r#"{
            "job_id": "job-ct",
            "to": "user@example.com",
            "subject": "Sub",
            "body": "<h1>Hello</h1>",
            "content_type": "text/html"
        }"#;
        let parsed = parse_email_job_payload(payload).unwrap();
        assert_eq!(parsed.content_type.as_deref(), Some("text/html"));
    }

    #[test]
    fn parse_email_job_payload_returns_missing_field_error() {
        let payload = r#"{
            "job_id": "job-123",
            "subject": "Subject",
            "body": "Body"
        }"#;

        let error = parse_email_job_payload(payload).expect_err("payload should fail");
        assert_eq!(error, EmailPayloadParseError::MissingField("to"));
    }

    #[test]
    fn parse_email_job_payload_rejects_empty_strings() {
        let payload = r#"{
            "job_id": "job-123",
            "to": "user@example.com",
            "subject": "  ",
            "body": "Body"
        }"#;

        let error = parse_email_job_payload(payload).expect_err("payload should fail");
        assert_eq!(
            error,
            EmailPayloadParseError::InvalidFieldValue {
                field: "subject",
                reason: "must not be empty",
            }
        );
    }

    #[test]
    fn parse_email_job_payload_rejects_empty_payload() {
        assert_eq!(
            parse_email_job_payload("").unwrap_err(),
            EmailPayloadParseError::EmptyPayload
        );
        assert_eq!(
            parse_email_job_payload("   ").unwrap_err(),
            EmailPayloadParseError::EmptyPayload
        );
    }

    #[test]
    fn parse_email_job_payload_rejects_invalid_json() {
        let error = parse_email_job_payload("{invalid").unwrap_err();
        matches!(error, EmailPayloadParseError::InvalidJson(_));
    }

    #[test]
    fn parse_email_job_payload_rejects_non_object() {
        let error = parse_email_job_payload("[1,2,3]").unwrap_err();
        assert_eq!(
            error,
            EmailPayloadParseError::InvalidFieldType {
                field: "root",
                expected: "a JSON object",
            }
        );
    }

    #[test]
    fn parse_email_job_payload_null_optional_fields() {
        let payload = r#"{
            "job_id": "job-n",
            "to": "user@example.com",
            "subject": "Sub",
            "body": "Body",
            "from": null,
            "reply_to": null
        }"#;
        let parsed = parse_email_job_payload(payload).unwrap();
        assert_eq!(parsed.from, None);
        assert_eq!(parsed.reply_to, None);
    }

    #[test]
    fn parse_email_job_payload_bytes_rejects_non_utf8() {
        let payload = vec![0, 159, 146, 150];
        let error = parse_email_job_payload_bytes(&payload).expect_err("payload should fail");
        assert_eq!(error, EmailPayloadParseError::InvalidUtf8);
    }

    // --- Logging provider ---

    #[test]
    fn logging_provider_dispatches_and_increments_sequence() {
        let provider = LoggingEmailProvider::new("worker-log");
        let job = fixture_job();

        let first = provider.dispatch(&job).expect("dispatch should pass");
        let second = provider.dispatch(&job).expect("dispatch should pass");

        assert_eq!(first.provider, "worker-log");
        assert_eq!(first.job_id, "job-1");
        assert_eq!(first.message_id, "worker-log-1");
        assert_eq!(second.message_id, "worker-log-2");
        assert_eq!(provider.dispatch_count(), 2);
    }

    #[test]
    fn logging_provider_rejects_empty_job_id() {
        let provider = LoggingEmailProvider::default();
        let mut job = fixture_job();
        job.job_id = "".to_owned();

        let error = provider.dispatch(&job).unwrap_err();
        assert_eq!(
            error,
            EmailProviderError::InvalidJob {
                field: "job_id",
                reason: "must not be empty",
            }
        );
    }

    #[test]
    fn logging_provider_rejects_invalid_email() {
        let provider = LoggingEmailProvider::default();
        let mut job = fixture_job();
        job.to = "no-at-sign".to_owned();

        let error = provider.dispatch(&job).unwrap_err();
        assert_eq!(
            error,
            EmailProviderError::InvalidJob {
                field: "to",
                reason: "must be a valid email address (user@domain)",
            }
        );
    }

    #[test]
    fn logging_provider_rejects_invalid_from_email() {
        let provider = LoggingEmailProvider::default();
        let mut job = fixture_job();
        job.from = Some("bad-email".to_owned());

        let error = provider.dispatch(&job).unwrap_err();
        assert_eq!(
            error,
            EmailProviderError::InvalidJob {
                field: "from",
                reason: "must be a valid email address (user@domain)",
            }
        );
    }

    // --- Failing provider ---

    #[test]
    fn failing_provider_rejects_valid_job() {
        let provider = FailingEmailProvider::default();
        let job = fixture_job();
        let error = provider.dispatch(&job).unwrap_err();
        assert!(matches!(error, EmailProviderError::DispatchFailed(_)));
        let msg = error.to_string();
        assert!(msg.contains("simulated dispatch failure"));
        assert!(msg.contains("job-1"));
    }

    #[test]
    fn failing_provider_still_validates() {
        let provider = FailingEmailProvider::new("test", "boom");
        let mut job = fixture_job();
        job.to = "".to_owned();
        let error = provider.dispatch(&job).unwrap_err();
        assert!(matches!(error, EmailProviderError::InvalidJob { .. }));
    }

    // --- Email validation ---

    #[test]
    fn validate_email_address_accepts_valid() {
        assert!(validate_email_address("user@example.com", "to").is_ok());
        assert!(validate_email_address("a@b", "to").is_ok());
        assert!(validate_email_address("user+tag@domain.co.uk", "to").is_ok());
    }

    #[test]
    fn validate_email_address_rejects_invalid() {
        assert!(validate_email_address("", "to").is_err());
        assert!(validate_email_address("no-at", "to").is_err());
        assert!(validate_email_address("@domain", "to").is_err());
        assert!(validate_email_address("user@", "to").is_err());
    }

    // --- Builder ---

    #[test]
    fn builder_creates_job() {
        let job = EmailJobBuilder::new()
            .job_id("j-1")
            .to("user@example.com")
            .subject("Hello")
            .body("World")
            .from("noreply@example.com")
            .content_type("text/html")
            .tag("welcome")
            .tag("v2")
            .build()
            .unwrap();

        assert_eq!(job.job_id, "j-1");
        assert_eq!(job.to, "user@example.com");
        assert_eq!(job.from.as_deref(), Some("noreply@example.com"));
        assert_eq!(job.content_type.as_deref(), Some("text/html"));
        assert_eq!(job.tags, vec!["welcome", "v2"]);
    }

    #[test]
    fn builder_requires_fields() {
        assert!(EmailJobBuilder::new().build().is_err());
        assert!(EmailJobBuilder::new().job_id("j-1").build().is_err());
        assert!(EmailJobBuilder::new()
            .job_id("j-1")
            .to("x")
            .build()
            .is_err());
        assert!(EmailJobBuilder::new()
            .job_id("j-1")
            .to("x")
            .subject("s")
            .build()
            .is_err());
    }

    // --- Templates ---

    #[test]
    fn template_welcome_render() {
        let tpl = EmailTemplate::Welcome {
            username: "Player1".to_string(),
        };
        let (subject, body) = tpl.render();
        assert_eq!(subject, "Welcome to Universus!");
        assert!(body.contains("Player1"));
        assert!(body.contains("galactic empire"));
    }

    #[test]
    fn template_password_reset_render() {
        let tpl = EmailTemplate::PasswordReset {
            reset_link: "https://example.com/reset/abc".to_string(),
            expires_minutes: 30,
        };
        let (subject, body) = tpl.render();
        assert!(subject.contains("Password Reset"));
        assert!(body.contains("https://example.com/reset/abc"));
        assert!(body.contains("30 minutes"));
    }

    #[test]
    fn template_attack_incoming_render() {
        let tpl = EmailTemplate::AttackIncoming {
            attacker: "EvilPlayer".to_string(),
            arrival_time: "2026-03-08T12:00:00Z".to_string(),
        };
        let (subject, body) = tpl.render();
        assert!(subject.contains("Incoming Attack"));
        assert!(body.contains("EvilPlayer"));
        assert!(body.contains("2026-03-08T12:00:00Z"));
    }

    #[test]
    fn template_to_job() {
        let tpl = EmailTemplate::Welcome {
            username: "TestUser".to_string(),
        };
        let job = tpl.to_job("job-w1", "test@example.com", None);
        assert_eq!(job.job_id, "job-w1");
        assert_eq!(job.to, "test@example.com");
        assert_eq!(job.subject, "Welcome to Universus!");
        assert_eq!(job.content_type.as_deref(), Some("text/plain"));
        assert_eq!(job.tags, vec!["welcome"]);
    }

    #[test]
    fn template_alliance_invite_render() {
        let tpl = EmailTemplate::AllianceInvite {
            alliance_name: "StarForce".to_string(),
            inviter: "LeaderX".to_string(),
        };
        let (subject, body) = tpl.render();
        assert!(subject.contains("StarForce"));
        assert!(body.contains("LeaderX"));
    }

    // --- Display impls ---

    #[test]
    fn error_display_messages() {
        let e1 = EmailProviderError::InvalidJob {
            field: "to",
            reason: "must not be empty",
        };
        assert_eq!(
            e1.to_string(),
            "invalid email job field 'to': must not be empty"
        );

        let e2 = EmailProviderError::DispatchFailed("timeout".to_string());
        assert_eq!(e2.to_string(), "dispatch failed: timeout");

        let e3 = EmailPayloadParseError::EmptyPayload;
        assert_eq!(e3.to_string(), "payload is empty");

        let e4 = EmailPayloadParseError::InvalidUtf8;
        assert_eq!(e4.to_string(), "payload is not valid UTF-8");

        let e5 = EmailPayloadParseError::MissingField("body");
        assert_eq!(e5.to_string(), "payload is missing required field 'body'");
    }

    // --- Serialization round-trip ---

    #[test]
    fn email_job_serde_roundtrip() {
        let job = EmailJobBuilder::new()
            .job_id("rt-1")
            .to("a@b.com")
            .subject("Subj")
            .body("Body text")
            .from("c@d.com")
            .tag("test")
            .build()
            .unwrap();

        let json = serde_json::to_string(&job).unwrap();
        let deserialized: EmailJob = serde_json::from_str(&json).unwrap();
        assert_eq!(job, deserialized);
    }
}
