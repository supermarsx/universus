use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmailJob {
    pub job_id: String,
    pub to: String,
    pub subject: String,
    pub body: String,
    pub from: Option<String>,
    pub reply_to: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailDispatchResult {
    pub provider: String,
    pub job_id: String,
    pub message_id: String,
}

pub trait EmailProvider: Send + Sync {
    fn name(&self) -> &str;
    fn dispatch(&self, job: &EmailJob) -> Result<EmailDispatchResult, EmailProviderError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmailProviderError {
    InvalidJob { field: &'static str, reason: &'static str },
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
        validate_non_empty(&job.job_id, "job_id")?;
        validate_non_empty(&job.to, "to")?;
        validate_non_empty(&job.subject, "subject")?;
        validate_non_empty(&job.body, "body")?;

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

    let root: Value =
        serde_json::from_str(payload).map_err(|err| EmailPayloadParseError::InvalidJson(err.to_string()))?;
    let object = root.as_object().ok_or(EmailPayloadParseError::InvalidFieldType {
        field: "root",
        expected: "a JSON object",
    })?;

    let job_id = parse_required_string(object, "job_id")?;
    let to = parse_required_string(object, "to")?;
    let subject = parse_required_string(object, "subject")?;
    let body = parse_required_string(object, "body")?;
    let from = parse_optional_string(object, "from")?;
    let reply_to = parse_optional_string(object, "reply_to")?;

    Ok(EmailJob {
        job_id,
        to,
        subject,
        body,
        from,
        reply_to,
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

fn validate_non_empty(value: &str, field: &'static str) -> Result<(), EmailProviderError> {
    if value.trim().is_empty() {
        return Err(EmailProviderError::InvalidJob {
            field,
            reason: "must not be empty",
        });
    }
    Ok(())
}

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
        }
    }

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

        assert_eq!(
            parsed,
            EmailJob {
                job_id: "job-123".to_owned(),
                to: "user@example.com".to_owned(),
                subject: "Subject".to_owned(),
                body: "Body".to_owned(),
                from: Some("sender@example.com".to_owned()),
                reply_to: Some("noreply@example.com".to_owned()),
            }
        );
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
    fn parse_email_job_payload_bytes_rejects_non_utf8() {
        let payload = vec![0, 159, 146, 150];

        let error = parse_email_job_payload_bytes(&payload).expect_err("payload should fail");

        assert_eq!(error, EmailPayloadParseError::InvalidUtf8);
    }

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
    fn logging_provider_rejects_invalid_job() {
        let provider = LoggingEmailProvider::default();
        let mut job = fixture_job();
        job.to = "   ".to_owned();

        let error = provider.dispatch(&job).expect_err("dispatch should fail");

        assert_eq!(
            error,
            EmailProviderError::InvalidJob {
                field: "to",
                reason: "must not be empty",
            }
        );
        assert_eq!(provider.dispatch_count(), 0);
    }
}
