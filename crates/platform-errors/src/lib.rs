//! Shared application error types.
//!
//! Provides a comprehensive error handling system with:
//! - `AppError` enum for HTTP-mapped application errors
//! - `ErrorCode` enum for fine-grained numeric error codes
//! - `ErrorResponse` for structured JSON error responses
//! - `ErrorContext` for enriching errors with metadata
//! - `ResultExt` trait for ergonomic error mapping

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// AppResult type alias
// ---------------------------------------------------------------------------

pub type AppResult<T> = Result<T, AppError>;

// ---------------------------------------------------------------------------
// ErrorCode — fine-grained numeric codes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    // Validation 1000-1099
    InvalidInput,
    MissingField,
    OutOfRange,
    InvalidFormat,

    // Auth 1100-1199
    Unauthenticated,
    Forbidden,
    TokenExpired,
    SessionInvalid,

    // Resource 1200-1299
    ResourceNotFound,
    ResourceConflict,
    ResourceExhausted,
    ResourceLocked,

    // Game 1300-1399
    InsufficientResources,
    QueueFull,
    CooldownActive,
    InvalidCoordinates,
    FleetBusy,

    // System 1500-1599
    InternalError,
    ServiceUnavailable,
    RateLimited,
    DatabaseError,
}

impl ErrorCode {
    pub fn code(&self) -> u32 {
        match self {
            // Validation
            Self::InvalidInput => 1001,
            Self::MissingField => 1002,
            Self::OutOfRange => 1003,
            Self::InvalidFormat => 1004,

            // Auth
            Self::Unauthenticated => 1100,
            Self::Forbidden => 1101,
            Self::TokenExpired => 1102,
            Self::SessionInvalid => 1103,

            // Resource
            Self::ResourceNotFound => 1200,
            Self::ResourceConflict => 1201,
            Self::ResourceExhausted => 1202,
            Self::ResourceLocked => 1203,

            // Game
            Self::InsufficientResources => 1300,
            Self::QueueFull => 1301,
            Self::CooldownActive => 1302,
            Self::InvalidCoordinates => 1303,
            Self::FleetBusy => 1304,

            // System
            Self::InternalError => 1500,
            Self::ServiceUnavailable => 1501,
            Self::RateLimited => 1502,
            Self::DatabaseError => 1503,
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::InvalidInput => "Invalid input provided",
            Self::MissingField => "Required field is missing",
            Self::OutOfRange => "Value is out of allowed range",
            Self::InvalidFormat => "Invalid format",

            Self::Unauthenticated => "Authentication required",
            Self::Forbidden => "Access denied",
            Self::TokenExpired => "Authentication token has expired",
            Self::SessionInvalid => "Session is invalid or expired",

            Self::ResourceNotFound => "Resource not found",
            Self::ResourceConflict => "Resource conflict",
            Self::ResourceExhausted => "Resource exhausted",
            Self::ResourceLocked => "Resource is locked",

            Self::InsufficientResources => "Insufficient resources",
            Self::QueueFull => "Queue is full",
            Self::CooldownActive => "Cooldown is active",
            Self::InvalidCoordinates => "Invalid coordinates",
            Self::FleetBusy => "Fleet is busy",

            Self::InternalError => "Internal server error",
            Self::ServiceUnavailable => "Service temporarily unavailable",
            Self::RateLimited => "Rate limit exceeded",
            Self::DatabaseError => "Database error",
        }
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code(), self.message())
    }
}

// ---------------------------------------------------------------------------
// AppError
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum AppError {
    BadRequest(String),
    Unauthorized(String),
    NotFound(String),
    Internal(String),
    Conflict(String),
    RateLimited(String),
    ServiceUnavailable(String),
    Forbidden(String),
    UnprocessableEntity(String),
}

impl AppError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest(message.into())
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::Unauthorized(message.into())
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound(message.into())
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::Conflict(message.into())
    }

    pub fn rate_limited(message: impl Into<String>) -> Self {
        Self::RateLimited(message.into())
    }

    pub fn service_unavailable(message: impl Into<String>) -> Self {
        Self::ServiceUnavailable(message.into())
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::Forbidden(message.into())
    }

    pub fn unprocessable_entity(message: impl Into<String>) -> Self {
        Self::UnprocessableEntity(message.into())
    }

    /// Returns the HTTP status code for this error.
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::RateLimited(_) => StatusCode::TOO_MANY_REQUESTS,
            Self::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::UnprocessableEntity(_) => StatusCode::UNPROCESSABLE_ENTITY,
        }
    }

    /// Returns the message carried by this error.
    pub fn message(&self) -> &str {
        match self {
            Self::BadRequest(m)
            | Self::Unauthorized(m)
            | Self::NotFound(m)
            | Self::Internal(m)
            | Self::Conflict(m)
            | Self::RateLimited(m)
            | Self::ServiceUnavailable(m)
            | Self::Forbidden(m)
            | Self::UnprocessableEntity(m) => m,
        }
    }

    /// Begin building an `ErrorContext` with an explicit error code.
    pub fn with_code(self, code: ErrorCode) -> ErrorContext {
        ErrorContext {
            source: self,
            error_code: code,
            details: None,
            request_id: None,
            timestamp: now_timestamp(),
        }
    }

    /// Begin building an `ErrorContext` with details JSON.
    pub fn with_details(self, details: serde_json::Value) -> ErrorContext {
        ErrorContext {
            error_code: default_error_code(&self),
            source: self,
            details: Some(details),
            request_id: None,
            timestamp: now_timestamp(),
        }
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadRequest(m) => write!(f, "Bad Request: {m}"),
            Self::Unauthorized(m) => write!(f, "Unauthorized: {m}"),
            Self::NotFound(m) => write!(f, "Not Found: {m}"),
            Self::Internal(m) => write!(f, "Internal Error: {m}"),
            Self::Conflict(m) => write!(f, "Conflict: {m}"),
            Self::RateLimited(m) => write!(f, "Rate Limited: {m}"),
            Self::ServiceUnavailable(m) => write!(f, "Service Unavailable: {m}"),
            Self::Forbidden(m) => write!(f, "Forbidden: {m}"),
            Self::UnprocessableEntity(m) => write!(f, "Unprocessable Entity: {m}"),
        }
    }
}

impl std::error::Error for AppError {}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let response = ErrorResponse::from_app_error(&self);
        let status = self.status_code();
        (status, axum::Json(response)).into_response()
    }
}

// ---------------------------------------------------------------------------
// ErrorResponse — structured JSON body
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: String,
    pub error_code: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl ErrorResponse {
    pub fn from_app_error(err: &AppError) -> Self {
        let error_code = default_error_code(err);
        Self {
            success: false,
            error: err.message().to_string(),
            error_code: error_code.code(),
            details: None,
            request_id: None,
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> Response {
        let status = status_from_error_code(self.error_code);
        (status, axum::Json(self)).into_response()
    }
}

// ---------------------------------------------------------------------------
// ErrorContext — enriched error wrapper with builder pattern
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub source: AppError,
    pub error_code: ErrorCode,
    pub details: Option<serde_json::Value>,
    pub request_id: Option<String>,
    pub timestamp: String,
}

impl ErrorContext {
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    pub fn with_code(mut self, code: ErrorCode) -> Self {
        self.error_code = code;
        self
    }

    pub fn to_error_response(&self) -> ErrorResponse {
        ErrorResponse {
            success: false,
            error: self.source.message().to_string(),
            error_code: self.error_code.code(),
            details: self.details.clone(),
            request_id: self.request_id.clone(),
        }
    }
}

impl std::fmt::Display for ErrorContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (code={}, ts={})",
            self.source,
            self.error_code.code(),
            self.timestamp
        )
    }
}

impl std::error::Error for ErrorContext {}

impl IntoResponse for ErrorContext {
    fn into_response(self) -> Response {
        let status = self.source.status_code();
        let body = self.to_error_response();
        (status, axum::Json(body)).into_response()
    }
}

// ---------------------------------------------------------------------------
// ResultExt trait
// ---------------------------------------------------------------------------

pub trait ResultExt<T> {
    /// Map the error to an `AppError` using the provided closure.
    fn map_app_err(self, f: impl FnOnce(Box<dyn std::fmt::Display>) -> AppError) -> AppResult<T>;

    /// Convert any `Display`-able error into `AppError::Internal` with the given context message.
    fn with_context(self, msg: impl Into<String>) -> AppResult<T>;
}

impl<T, E: std::fmt::Display + 'static> ResultExt<T> for Result<T, E> {
    fn map_app_err(self, f: impl FnOnce(Box<dyn std::fmt::Display>) -> AppError) -> AppResult<T> {
        self.map_err(|e| f(Box::new(e)))
    }

    fn with_context(self, msg: impl Into<String>) -> AppResult<T> {
        self.map_err(|e| {
            let msg = msg.into();
            AppError::Internal(format!("{msg}: {e}"))
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Derive a default `ErrorCode` from an `AppError` variant.
fn default_error_code(err: &AppError) -> ErrorCode {
    match err {
        AppError::BadRequest(_) => ErrorCode::InvalidInput,
        AppError::Unauthorized(_) => ErrorCode::Unauthenticated,
        AppError::NotFound(_) => ErrorCode::ResourceNotFound,
        AppError::Internal(_) => ErrorCode::InternalError,
        AppError::Conflict(_) => ErrorCode::ResourceConflict,
        AppError::RateLimited(_) => ErrorCode::RateLimited,
        AppError::ServiceUnavailable(_) => ErrorCode::ServiceUnavailable,
        AppError::Forbidden(_) => ErrorCode::Forbidden,
        AppError::UnprocessableEntity(_) => ErrorCode::InvalidInput,
    }
}

/// Map an error-code numeric value back to an HTTP status code.
fn status_from_error_code(code: u32) -> StatusCode {
    match code {
        1000..=1099 => StatusCode::BAD_REQUEST,
        1100 => StatusCode::UNAUTHORIZED,
        1101 => StatusCode::FORBIDDEN,
        1102..=1103 => StatusCode::UNAUTHORIZED,
        1200 => StatusCode::NOT_FOUND,
        1201 => StatusCode::CONFLICT,
        1202..=1203 => StatusCode::CONFLICT,
        1300..=1399 => StatusCode::UNPROCESSABLE_ENTITY,
        1500 => StatusCode::INTERNAL_SERVER_ERROR,
        1501 => StatusCode::SERVICE_UNAVAILABLE,
        1502 => StatusCode::TOO_MANY_REQUESTS,
        1503 => StatusCode::INTERNAL_SERVER_ERROR,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Produce an ISO-8601-ish timestamp without pulling in `chrono`.
fn now_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    // Simple epoch-seconds string — good enough for ordering / debugging.
    format!("{secs}")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // -- ErrorCode ----------------------------------------------------------

    #[test]
    fn error_code_validation_codes() {
        assert_eq!(ErrorCode::InvalidInput.code(), 1001);
        assert_eq!(ErrorCode::MissingField.code(), 1002);
        assert_eq!(ErrorCode::OutOfRange.code(), 1003);
        assert_eq!(ErrorCode::InvalidFormat.code(), 1004);
    }

    #[test]
    fn error_code_auth_codes() {
        assert_eq!(ErrorCode::Unauthenticated.code(), 1100);
        assert_eq!(ErrorCode::Forbidden.code(), 1101);
        assert_eq!(ErrorCode::TokenExpired.code(), 1102);
        assert_eq!(ErrorCode::SessionInvalid.code(), 1103);
    }

    #[test]
    fn error_code_resource_codes() {
        assert_eq!(ErrorCode::ResourceNotFound.code(), 1200);
        assert_eq!(ErrorCode::ResourceConflict.code(), 1201);
        assert_eq!(ErrorCode::ResourceExhausted.code(), 1202);
        assert_eq!(ErrorCode::ResourceLocked.code(), 1203);
    }

    #[test]
    fn error_code_game_codes() {
        assert_eq!(ErrorCode::InsufficientResources.code(), 1300);
        assert_eq!(ErrorCode::QueueFull.code(), 1301);
        assert_eq!(ErrorCode::CooldownActive.code(), 1302);
        assert_eq!(ErrorCode::InvalidCoordinates.code(), 1303);
        assert_eq!(ErrorCode::FleetBusy.code(), 1304);
    }

    #[test]
    fn error_code_system_codes() {
        assert_eq!(ErrorCode::InternalError.code(), 1500);
        assert_eq!(ErrorCode::ServiceUnavailable.code(), 1501);
        assert_eq!(ErrorCode::RateLimited.code(), 1502);
        assert_eq!(ErrorCode::DatabaseError.code(), 1503);
    }

    #[test]
    fn error_code_messages_not_empty() {
        let codes = [
            ErrorCode::InvalidInput,
            ErrorCode::MissingField,
            ErrorCode::Unauthenticated,
            ErrorCode::ResourceNotFound,
            ErrorCode::InsufficientResources,
            ErrorCode::InternalError,
        ];
        for code in codes {
            assert!(!code.message().is_empty(), "{code:?} has empty message");
        }
    }

    #[test]
    fn error_code_display() {
        let s = format!("{}", ErrorCode::InvalidInput);
        assert!(s.contains("1001"));
        assert!(s.contains("Invalid input"));
    }

    #[test]
    fn error_code_serialize_roundtrip() {
        let code = ErrorCode::FleetBusy;
        let json = serde_json::to_string(&code).unwrap();
        let back: ErrorCode = serde_json::from_str(&json).unwrap();
        assert_eq!(back, code);
    }

    // -- AppError -----------------------------------------------------------

    #[test]
    fn app_error_constructors() {
        let e = AppError::bad_request("oops");
        assert!(matches!(e, AppError::BadRequest(_)));

        let e = AppError::unauthorized("nope");
        assert!(matches!(e, AppError::Unauthorized(_)));

        let e = AppError::not_found("gone");
        assert!(matches!(e, AppError::NotFound(_)));

        let e = AppError::internal("boom");
        assert!(matches!(e, AppError::Internal(_)));
    }

    #[test]
    fn app_error_new_variant_constructors() {
        assert!(matches!(AppError::conflict("dup"), AppError::Conflict(_)));
        assert!(matches!(
            AppError::rate_limited("slow down"),
            AppError::RateLimited(_)
        ));
        assert!(matches!(
            AppError::service_unavailable("later"),
            AppError::ServiceUnavailable(_)
        ));
        assert!(matches!(
            AppError::forbidden("denied"),
            AppError::Forbidden(_)
        ));
        assert!(matches!(
            AppError::unprocessable_entity("bad"),
            AppError::UnprocessableEntity(_)
        ));
    }

    #[test]
    fn app_error_status_codes() {
        assert_eq!(
            AppError::bad_request("x").status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            AppError::unauthorized("x").status_code(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            AppError::not_found("x").status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            AppError::internal("x").status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(AppError::conflict("x").status_code(), StatusCode::CONFLICT);
        assert_eq!(
            AppError::rate_limited("x").status_code(),
            StatusCode::TOO_MANY_REQUESTS
        );
        assert_eq!(
            AppError::service_unavailable("x").status_code(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(
            AppError::forbidden("x").status_code(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            AppError::unprocessable_entity("x").status_code(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }

    #[test]
    fn app_error_message() {
        let e = AppError::bad_request("hello world");
        assert_eq!(e.message(), "hello world");
    }

    #[test]
    fn app_error_display() {
        let e = AppError::not_found("planet 42");
        let s = format!("{e}");
        assert!(s.contains("Not Found"));
        assert!(s.contains("planet 42"));
    }

    #[test]
    fn app_error_implements_std_error() {
        let e = AppError::internal("test");
        let _: &dyn std::error::Error = &e;
    }

    // -- ErrorResponse ------------------------------------------------------

    #[test]
    fn error_response_from_app_error() {
        let e = AppError::not_found("resource missing");
        let resp = ErrorResponse::from_app_error(&e);
        assert!(!resp.success);
        assert_eq!(resp.error, "resource missing");
        assert_eq!(resp.error_code, ErrorCode::ResourceNotFound.code());
        assert!(resp.details.is_none());
        assert!(resp.request_id.is_none());
    }

    #[test]
    fn error_response_with_details() {
        let resp = ErrorResponse::from_app_error(&AppError::bad_request("oops"))
            .with_details(json!({"field": "email"}));
        assert_eq!(resp.details.unwrap()["field"], "email");
    }

    #[test]
    fn error_response_with_request_id() {
        let resp =
            ErrorResponse::from_app_error(&AppError::internal("x")).with_request_id("req-123");
        assert_eq!(resp.request_id.as_deref(), Some("req-123"));
    }

    #[test]
    fn error_response_serializes_to_json() {
        let resp = ErrorResponse::from_app_error(&AppError::unauthorized("bad token"));
        let json_str = serde_json::to_string(&resp).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(v["success"], false);
        assert_eq!(v["error"], "bad token");
        assert_eq!(v["error_code"], 1100);
    }

    #[test]
    fn error_response_skips_none_fields() {
        let resp = ErrorResponse::from_app_error(&AppError::internal("x"));
        let json_str = serde_json::to_string(&resp).unwrap();
        assert!(!json_str.contains("details"));
        assert!(!json_str.contains("request_id"));
    }

    #[test]
    fn error_response_deserialize() {
        let json_str = r#"{"success":false,"error":"test","error_code":1001}"#;
        let resp: ErrorResponse = serde_json::from_str(json_str).unwrap();
        assert!(!resp.success);
        assert_eq!(resp.error, "test");
        assert_eq!(resp.error_code, 1001);
    }

    // -- ErrorContext --------------------------------------------------------

    #[test]
    fn error_context_via_with_code() {
        let ctx = AppError::bad_request("bad email").with_code(ErrorCode::InvalidFormat);
        assert_eq!(ctx.error_code, ErrorCode::InvalidFormat);
        assert_eq!(ctx.source.message(), "bad email");
        assert!(ctx.details.is_none());
        assert!(!ctx.timestamp.is_empty());
    }

    #[test]
    fn error_context_via_with_details() {
        let ctx = AppError::not_found("planet").with_details(json!({"id": 7}));
        assert!(ctx.details.is_some());
        assert_eq!(ctx.details.unwrap()["id"], 7);
        // Should pick the default code for NotFound
        assert_eq!(ctx.error_code, ErrorCode::ResourceNotFound);
    }

    #[test]
    fn error_context_builder_chain() {
        let ctx = AppError::internal("db failure")
            .with_code(ErrorCode::DatabaseError)
            .with_details(json!({"table": "users"}))
            .with_request_id("req-abc");

        assert_eq!(ctx.error_code, ErrorCode::DatabaseError);
        assert_eq!(ctx.request_id.as_deref(), Some("req-abc"));
        assert_eq!(ctx.details.as_ref().unwrap()["table"], "users");
    }

    #[test]
    fn error_context_display() {
        let ctx = AppError::conflict("duplicate").with_code(ErrorCode::ResourceConflict);
        let s = format!("{ctx}");
        assert!(s.contains("Conflict"));
        assert!(s.contains("duplicate"));
        assert!(s.contains("1201"));
    }

    #[test]
    fn error_context_to_error_response() {
        let ctx = AppError::forbidden("no access")
            .with_code(ErrorCode::Forbidden)
            .with_request_id("r1");
        let resp = ctx.to_error_response();
        assert!(!resp.success);
        assert_eq!(resp.error, "no access");
        assert_eq!(resp.error_code, 1101);
        assert_eq!(resp.request_id.as_deref(), Some("r1"));
    }

    #[test]
    fn error_context_implements_std_error() {
        let ctx = AppError::internal("x").with_code(ErrorCode::InternalError);
        let _: &dyn std::error::Error = &ctx;
    }

    // -- ResultExt ----------------------------------------------------------

    #[test]
    fn result_ext_map_app_err_ok() {
        let r: Result<i32, String> = Ok(42);
        let mapped: AppResult<i32> = r.map_app_err(|_| AppError::internal("should not happen"));
        assert_eq!(mapped.unwrap(), 42);
    }

    #[test]
    fn result_ext_map_app_err_err() {
        let r: Result<i32, String> = Err("underlying".to_string());
        let mapped: AppResult<i32> =
            r.map_app_err(|e| AppError::bad_request(format!("mapped: {e}")));
        let err = mapped.unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
        assert!(err.message().contains("mapped"));
    }

    #[test]
    fn result_ext_with_context_ok() {
        let r: Result<i32, String> = Ok(10);
        let mapped: AppResult<i32> = r.with_context("should not appear");
        assert_eq!(mapped.unwrap(), 10);
    }

    #[test]
    fn result_ext_with_context_err() {
        let r: Result<i32, String> = Err("io error".to_string());
        let mapped: AppResult<i32> = r.with_context("reading config");
        let err = mapped.unwrap_err();
        assert!(matches!(err, AppError::Internal(_)));
        assert!(err.message().contains("reading config"));
        assert!(err.message().contains("io error"));
    }

    // -- default_error_code -------------------------------------------------

    #[test]
    fn default_error_code_mapping() {
        assert_eq!(
            default_error_code(&AppError::bad_request("x")),
            ErrorCode::InvalidInput
        );
        assert_eq!(
            default_error_code(&AppError::unauthorized("x")),
            ErrorCode::Unauthenticated
        );
        assert_eq!(
            default_error_code(&AppError::not_found("x")),
            ErrorCode::ResourceNotFound
        );
        assert_eq!(
            default_error_code(&AppError::internal("x")),
            ErrorCode::InternalError
        );
        assert_eq!(
            default_error_code(&AppError::conflict("x")),
            ErrorCode::ResourceConflict
        );
        assert_eq!(
            default_error_code(&AppError::rate_limited("x")),
            ErrorCode::RateLimited
        );
        assert_eq!(
            default_error_code(&AppError::service_unavailable("x")),
            ErrorCode::ServiceUnavailable
        );
        assert_eq!(
            default_error_code(&AppError::forbidden("x")),
            ErrorCode::Forbidden
        );
        assert_eq!(
            default_error_code(&AppError::unprocessable_entity("x")),
            ErrorCode::InvalidInput
        );
    }

    // -- status_from_error_code ---------------------------------------------

    #[test]
    fn status_from_error_code_mapping() {
        assert_eq!(status_from_error_code(1001), StatusCode::BAD_REQUEST);
        assert_eq!(status_from_error_code(1100), StatusCode::UNAUTHORIZED);
        assert_eq!(status_from_error_code(1101), StatusCode::FORBIDDEN);
        assert_eq!(status_from_error_code(1102), StatusCode::UNAUTHORIZED);
        assert_eq!(status_from_error_code(1200), StatusCode::NOT_FOUND);
        assert_eq!(status_from_error_code(1201), StatusCode::CONFLICT);
        assert_eq!(
            status_from_error_code(1300),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert_eq!(
            status_from_error_code(1500),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            status_from_error_code(1501),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(status_from_error_code(1502), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            status_from_error_code(1503),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            status_from_error_code(9999),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    // -- Integration / round-trip -------------------------------------------

    #[test]
    fn error_response_roundtrip_json() {
        let original = ErrorResponse {
            success: false,
            error: "test error".to_string(),
            error_code: 1303,
            details: Some(json!({"x": 1, "y": 2})),
            request_id: Some("req-42".to_string()),
        };
        let json_str = serde_json::to_string(&original).unwrap();
        let decoded: ErrorResponse = serde_json::from_str(&json_str).unwrap();
        assert_eq!(decoded.success, false);
        assert_eq!(decoded.error, "test error");
        assert_eq!(decoded.error_code, 1303);
        assert_eq!(decoded.details.unwrap()["x"], 1);
        assert_eq!(decoded.request_id.as_deref(), Some("req-42"));
    }

    #[test]
    fn app_error_clone() {
        let e = AppError::conflict("dup");
        let e2 = e.clone();
        assert_eq!(e2.message(), "dup");
    }

    #[test]
    fn error_context_with_code_override() {
        let ctx = AppError::bad_request("x")
            .with_code(ErrorCode::InvalidInput)
            .with_code(ErrorCode::MissingField);
        assert_eq!(ctx.error_code, ErrorCode::MissingField);
    }
}
