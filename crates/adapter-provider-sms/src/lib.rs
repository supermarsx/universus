#![forbid(unsafe_code)]

//! Privacy-minimized SMS provider adapter.
//!
//! The public dispatch shape has no arbitrary body. A verified destination is
//! borrowed only for the provider call, while content remains a registered
//! provider template plus an authoritative event identity.

use std::fmt::{Debug, Display, Formatter};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use url::{Host, Url};
use zeroize::Zeroizing;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderEnvironment {
    Production,
    Staging,
    Development,
    Test,
}

impl ProviderEnvironment {
    fn from_env() -> Result<Self, SmsProviderError> {
        let value = ["UNIVERSUS_ENV", "APP_ENV", "ENVIRONMENT", "RUST_ENV"]
            .into_iter()
            .find_map(|name| std::env::var(name).ok())
            .unwrap_or_else(|| "production".to_string());
        Self::parse(&value)
    }

    fn parse(value: &str) -> Result<Self, SmsProviderError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "production" | "prod" => Ok(Self::Production),
            "staging" | "stage" => Ok(Self::Staging),
            "development" | "dev" | "local" => Ok(Self::Development),
            "test" | "testing" => Ok(Self::Test),
            _ => Err(SmsProviderError::Configuration(
                "runtime environment is invalid",
            )),
        }
    }

    const fn permits_loopback_http(self) -> bool {
        matches!(self, Self::Development | Self::Test)
    }
}

#[derive(Clone, Copy)]
pub struct SmsDispatch<'a> {
    pub job_id: i64,
    pub destination: &'a str,
    pub provider_template_key: &'a str,
    pub payload_identity: &'a str,
    pub idempotency_key: &'a str,
}

impl Debug for SmsDispatch<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SmsDispatch")
            .field("job_id", &self.job_id)
            .field("destination", &"[REDACTED]")
            .field("provider_template_key", &self.provider_template_key)
            .field("payload_identity", &self.payload_identity)
            .field("idempotency_key", &self.idempotency_key)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsDispatchResult {
    pub provider_key: String,
    pub provider_message_id: String,
}

pub trait SmsProvider: Send + Sync {
    fn provider_key(&self) -> &str;
    fn dispatch(&self, request: SmsDispatch<'_>) -> Result<SmsDispatchResult, SmsProviderError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmsProviderError {
    Configuration(&'static str),
    InvalidDispatch(&'static str),
    DispatchFailed { code: &'static str, retryable: bool },
}

impl SmsProviderError {
    pub const fn reason_code(&self) -> &'static str {
        match self {
            Self::Configuration(_) => "provider_configuration_invalid",
            Self::InvalidDispatch(_) => "provider_dispatch_invalid",
            Self::DispatchFailed { code, .. } => code,
        }
    }

    pub const fn retryable(&self) -> bool {
        matches!(
            self,
            Self::DispatchFailed {
                retryable: true,
                ..
            }
        )
    }
}

impl Display for SmsProviderError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Configuration(reason) => write!(f, "SMS provider configuration: {reason}"),
            Self::InvalidDispatch(reason) => write!(f, "SMS dispatch contract: {reason}"),
            Self::DispatchFailed { code, retryable } => {
                write!(f, "SMS provider failed: code={code} retryable={retryable}")
            }
        }
    }
}

impl std::error::Error for SmsProviderError {}

#[derive(Clone)]
pub struct HttpSmsProvider {
    provider_key: String,
    endpoint: String,
    bearer_token: Zeroizing<String>,
    request_timeout: Duration,
    agent: ureq::Agent,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SmsProviderRequest<'a> {
    channel: &'static str,
    job_id: i64,
    destination: &'a str,
    template_id: &'a str,
    payload_identity: &'a str,
    idempotency_key: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderResponse {
    message_id: String,
}

impl Debug for HttpSmsProvider {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpSmsProvider")
            .field("provider_key", &self.provider_key)
            .field("endpoint", &"[REDACTED]")
            .field("bearer_token", &"[REDACTED]")
            .field("request_timeout", &self.request_timeout)
            .finish()
    }
}

impl HttpSmsProvider {
    pub fn new(
        provider_key: impl Into<String>,
        endpoint: impl Into<String>,
        bearer_token: impl Into<String>,
        timeout: Duration,
    ) -> Result<Self, SmsProviderError> {
        Self::new_for_environment(
            provider_key,
            endpoint,
            bearer_token,
            timeout,
            ProviderEnvironment::Production,
        )
    }

    pub fn new_for_environment(
        provider_key: impl Into<String>,
        endpoint: impl Into<String>,
        bearer_token: impl Into<String>,
        timeout: Duration,
        environment: ProviderEnvironment,
    ) -> Result<Self, SmsProviderError> {
        let provider_key = provider_key.into();
        let endpoint = endpoint.into();
        let bearer_token = Zeroizing::new(bearer_token.into());
        validate_tokenish(&provider_key, 2, 64, "provider key is invalid")?;
        validate_endpoint(&endpoint, environment)?;
        if bearer_token.trim().len() < 16 || bearer_token.len() > 4096 {
            return Err(SmsProviderError::Configuration(
                "SMS_PROVIDER_BEARER_TOKEN is missing or too short",
            ));
        }
        if timeout.is_zero() || timeout > Duration::from_secs(120) {
            return Err(SmsProviderError::Configuration(
                "SMS provider timeout is invalid",
            ));
        }
        Ok(Self {
            provider_key,
            endpoint,
            bearer_token,
            request_timeout: timeout,
            agent: ureq::AgentBuilder::new().timeout(timeout).build(),
        })
    }

    pub fn from_env() -> Result<Self, SmsProviderError> {
        let endpoint = std::env::var("SMS_PROVIDER_URL")
            .map_err(|_| SmsProviderError::Configuration("SMS_PROVIDER_URL is required"))?;
        let token = std::env::var("SMS_PROVIDER_BEARER_TOKEN").map_err(|_| {
            SmsProviderError::Configuration("SMS_PROVIDER_BEARER_TOKEN is required")
        })?;
        let provider_key =
            std::env::var("SMS_PROVIDER_KEY").unwrap_or_else(|_| "sms_http".to_string());
        let timeout_seconds = std::env::var("SMS_PROVIDER_TIMEOUT_SECONDS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(15);
        Self::new_for_environment(
            provider_key,
            endpoint,
            token,
            Duration::from_secs(timeout_seconds),
            ProviderEnvironment::from_env()?,
        )
    }

    pub const fn request_timeout(&self) -> Duration {
        self.request_timeout
    }
}

fn validate_endpoint(
    endpoint: &str,
    environment: ProviderEnvironment,
) -> Result<(), SmsProviderError> {
    if endpoint.len() > 2048 {
        return Err(SmsProviderError::Configuration(
            "SMS_PROVIDER_URL is invalid",
        ));
    }
    let parsed = Url::parse(endpoint)
        .map_err(|_| SmsProviderError::Configuration("SMS_PROVIDER_URL is invalid"))?;
    if !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || parsed.host().is_none()
    {
        return Err(SmsProviderError::Configuration(
            "SMS_PROVIDER_URL must not contain credentials, query, or fragment",
        ));
    }
    match parsed.scheme() {
        "https" => Ok(()),
        "http" if environment.permits_loopback_http() && is_loopback(parsed.host()) => Ok(()),
        _ => Err(SmsProviderError::Configuration(
            "SMS_PROVIDER_URL requires HTTPS outside explicit loopback development or test mode",
        )),
    }
}

fn is_loopback(host: Option<Host<&str>>) -> bool {
    match host {
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        Some(Host::Domain(domain)) => domain.eq_ignore_ascii_case("localhost"),
        None => false,
    }
}

impl SmsProvider for HttpSmsProvider {
    fn provider_key(&self) -> &str {
        &self.provider_key
    }

    fn dispatch(&self, request: SmsDispatch<'_>) -> Result<SmsDispatchResult, SmsProviderError> {
        validate_dispatch(request)?;
        let authorization = Zeroizing::new(format!("Bearer {}", self.bearer_token.as_str()));
        let body = Zeroizing::new(
            serde_json::to_vec(&SmsProviderRequest {
                channel: "sms",
                job_id: request.job_id,
                destination: request.destination,
                template_id: request.provider_template_key,
                payload_identity: request.payload_identity,
                idempotency_key: request.idempotency_key,
            })
            .map_err(|_| SmsProviderError::InvalidDispatch("provider request is invalid"))?,
        );
        let response = self
            .agent
            .post(&self.endpoint)
            .set("authorization", authorization.as_str())
            .set("content-type", "application/json")
            .send_bytes(body.as_slice());

        let response = match response {
            Ok(response) => response,
            Err(ureq::Error::Status(status, _)) => {
                return Err(SmsProviderError::DispatchFailed {
                    code: if status == 429 {
                        "provider_rate_limited"
                    } else if status >= 500 {
                        "provider_server_error"
                    } else {
                        "provider_rejected"
                    },
                    retryable: status == 429 || status >= 500,
                });
            }
            Err(ureq::Error::Transport(_)) => {
                return Err(SmsProviderError::DispatchFailed {
                    code: "provider_unreachable",
                    retryable: true,
                });
            }
        };
        let body: ProviderResponse =
            response
                .into_json()
                .map_err(|_| SmsProviderError::DispatchFailed {
                    code: "provider_response_invalid",
                    retryable: false,
                })?;
        let message_id = body.message_id;
        if message_id.trim().is_empty() || message_id.len() > 256 {
            return Err(SmsProviderError::DispatchFailed {
                code: "provider_response_invalid",
                retryable: false,
            });
        }
        Ok(SmsDispatchResult {
            provider_key: self.provider_key.clone(),
            provider_message_id: message_id,
        })
    }
}

pub fn validate_phone_number(number: &str) -> Result<(), SmsProviderError> {
    let bytes = number.as_bytes();
    if !(8..=16).contains(&bytes.len())
        || bytes.first() != Some(&b'+')
        || bytes.get(1) == Some(&b'0')
        || !bytes[1..].iter().all(u8::is_ascii_digit)
    {
        return Err(SmsProviderError::InvalidDispatch(
            "resolved destination is not E.164",
        ));
    }
    Ok(())
}

fn validate_dispatch(request: SmsDispatch<'_>) -> Result<(), SmsProviderError> {
    if request.job_id <= 0 {
        return Err(SmsProviderError::InvalidDispatch("job id is invalid"));
    }
    validate_phone_number(request.destination)?;
    validate_tokenish(
        request.provider_template_key,
        2,
        128,
        "provider template identity is invalid",
    )?;
    validate_tokenish(
        request.payload_identity,
        3,
        96,
        "payload identity is invalid",
    )?;
    validate_tokenish(
        request.idempotency_key,
        8,
        128,
        "idempotency key is invalid",
    )?;
    Ok(())
}

fn validate_tokenish(
    value: &str,
    minimum: usize,
    maximum: usize,
    message: &'static str,
) -> Result<(), SmsProviderError> {
    if !(minimum..=maximum).contains(&value.len())
        || value.trim() != value
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
    {
        return Err(SmsProviderError::InvalidDispatch(message));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use super::*;

    fn request<'a>(destination: &'a str) -> SmsDispatch<'a> {
        SmsDispatch {
            job_id: 72,
            destination,
            provider_template_key: "universus.sms.security.v1",
            payload_identity: "security_event:def-456",
            idempotency_key: "sms:test:0001",
        }
    }

    fn mock_server(status: &str, body: &str) -> (String, thread::JoinHandle<String>) {
        mock_server_with_delay(status, body, Duration::ZERO)
    }

    fn mock_server_with_delay(
        status: &str,
        body: &str,
        delay: Duration,
    ) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let status = status.to_string();
        let body = body.to_string();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut bytes = Vec::new();
            let mut chunk = [0_u8; 4096];
            loop {
                let read = stream.read(&mut chunk).unwrap();
                if read == 0 {
                    break;
                }
                bytes.extend_from_slice(&chunk[..read]);
                if let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n")
                {
                    let headers = String::from_utf8_lossy(&bytes[..header_end]);
                    let content_length = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .map(str::to_string)
                        })
                        .and_then(|value| value.parse::<usize>().ok())
                        .unwrap_or(0);
                    if bytes.len() >= header_end + 4 + content_length {
                        break;
                    }
                }
            }
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            thread::sleep(delay);
            let _ = stream.write_all(response.as_bytes());
            String::from_utf8(bytes).unwrap()
        });
        (format!("http://{address}/send"), handle)
    }

    #[test]
    fn http_provider_dispatches_to_local_mock() {
        let (url, server) = mock_server("200 OK", r#"{"messageId":"provider-72"}"#);
        let provider = HttpSmsProvider::new_for_environment(
            "sms_http",
            url,
            "test-bearer-token-long-enough",
            Duration::from_secs(2),
            ProviderEnvironment::Test,
        )
        .unwrap();
        let result = provider.dispatch(request("+12065550123")).unwrap();
        assert_eq!(result.provider_message_id, "provider-72");
        let received = server.join().unwrap();
        assert!(received.contains("+12065550123"));
        assert!(received.contains("universus.sms.security.v1"));
        assert!(received.contains("security_event:def-456"));
    }

    #[test]
    fn errors_and_debug_never_expose_destination_or_secret() {
        let (url, server) = mock_server("429 Too Many Requests", r#"{"error":"+12065550123"}"#);
        let provider = HttpSmsProvider::new_for_environment(
            "sms_http",
            url,
            "secret-bearer-token-long-enough",
            Duration::from_secs(2),
            ProviderEnvironment::Test,
        )
        .unwrap();
        let error = provider.dispatch(request("+12065550123")).unwrap_err();
        server.join().unwrap();
        assert_eq!(error.reason_code(), "provider_rate_limited");
        assert!(error.retryable());
        assert!(!error.to_string().contains("+12065550123"));
        assert!(!format!("{provider:?}").contains("secret-bearer-token-long-enough"));
        assert!(!format!("{provider:?}").contains("127.0.0.1"));
        assert!(!format!("{:?}", request("+12065550123")).contains("+12065550123"));
    }

    #[test]
    fn phone_validation_is_strict_e164() {
        assert!(validate_phone_number("+12065550123").is_ok());
        assert!(validate_phone_number("12065550123").is_err());
        assert!(validate_phone_number("+01234567").is_err());
        assert!(validate_phone_number("+123").is_err());
        assert!(validate_phone_number("+1206-555-0123").is_err());
    }

    #[test]
    fn endpoint_security_is_fail_closed() {
        let make = |endpoint, environment| {
            HttpSmsProvider::new_for_environment(
                "sms_http",
                endpoint,
                "test-bearer-token-long-enough",
                Duration::from_secs(2),
                environment,
            )
        };
        assert!(make(
            "https://provider.example/send",
            ProviderEnvironment::Staging
        )
        .is_ok());
        assert!(make("http://127.0.0.1:9999/send", ProviderEnvironment::Staging).is_err());
        assert!(make(
            "http://provider.example/send",
            ProviderEnvironment::Development
        )
        .is_err());
        assert!(make("http://[::1]:9999/send", ProviderEnvironment::Development).is_ok());
        assert!(make(
            "https://user:secret@provider.example/send",
            ProviderEnvironment::Production
        )
        .is_err());
        assert!(make(
            "https://provider.example/send?token=secret",
            ProviderEnvironment::Production
        )
        .is_err());
        assert!(make(
            "https://provider.example/send#secret",
            ProviderEnvironment::Production
        )
        .is_err());
        assert_eq!(
            ProviderEnvironment::parse("  PRODUCTION ").unwrap(),
            ProviderEnvironment::Production
        );
        assert_eq!(
            ProviderEnvironment::parse("TeStInG").unwrap(),
            ProviderEnvironment::Test
        );
        assert!(ProviderEnvironment::parse("preview").is_err());
        assert!(ProviderEnvironment::parse(" ").is_err());
    }

    #[test]
    fn timeout_is_retryable_and_uncertain_request_carries_durable_idempotency() {
        let (url, server) = mock_server_with_delay(
            "200 OK",
            r#"{"messageId":"late-provider-72"}"#,
            Duration::from_millis(150),
        );
        let provider = HttpSmsProvider::new_for_environment(
            "sms_http",
            url,
            "test-bearer-token-long-enough",
            Duration::from_millis(30),
            ProviderEnvironment::Test,
        )
        .unwrap();
        let error = provider.dispatch(request("+12065550123")).unwrap_err();
        assert_eq!(error.reason_code(), "provider_unreachable");
        assert!(error.retryable());
        let received = server.join().unwrap();
        assert!(received.contains(r#""idempotencyKey":"sms:test:0001""#));
        assert!(!error.to_string().contains("+12065550123"));
    }
}
