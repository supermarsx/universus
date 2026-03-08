#![forbid(unsafe_code)]

//! `platform-common` — shared utilities used across all Universus crates.
//!
//! Provides ID generation, time utilities, validation, pagination, sorting,
//! string helpers, math utilities, error types, and environment helpers.

use serde::{Deserialize, Serialize};
use std::env;
use std::fmt;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// 1. ID Generation
// ---------------------------------------------------------------------------

/// Global counter for `generate_id` uniqueness within a process.
static ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generates a unique ID composed of unix-millis, a monotonic counter, and a
/// pseudo-random component derived from hashing the counter with the timestamp.
pub fn generate_id() -> String {
    let ts = unix_timestamp_ms() as u64;
    let seq = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    // Simple hash-mix for the random component (no external RNG crate needed).
    let mix = ts
        .wrapping_mul(6364136223846793005)
        .wrapping_add(seq ^ 0xBEEF);
    format!("{ts}-{seq}-{mix:08x}")
}

/// Generates a short 8-character alphanumeric ID.
///
/// Uniqueness relies on timestamp + counter mixing; not cryptographically
/// random but sufficient for user-facing short codes.
pub fn generate_short_id() -> String {
    const ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let ts = unix_timestamp_ms() as u64;
    let seq = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut hash = ts.wrapping_mul(6364136223846793005).wrapping_add(seq);
    let mut buf = String::with_capacity(8);
    for _ in 0..8 {
        let idx = (hash % ALPHABET.len() as u64) as usize;
        buf.push(ALPHABET[idx] as char);
        hash /= ALPHABET.len() as u64;
        hash = hash.wrapping_add(seq).wrapping_mul(2654435761);
    }
    buf
}

/// Sequential ID generator with a configurable prefix.
///
/// ```
/// use platform_common::IdGenerator;
/// let gen = IdGenerator::new("planet");
/// assert!(gen.next().starts_with("planet-"));
/// ```
#[derive(Debug, Clone)]
pub struct IdGenerator {
    prefix: String,
    counter: std::sync::Arc<AtomicU64>,
}

impl IdGenerator {
    pub fn new(prefix: &str) -> Self {
        Self {
            prefix: prefix.to_owned(),
            counter: std::sync::Arc::new(AtomicU64::new(1)),
        }
    }

    pub fn next(&self) -> String {
        let n = self.counter.fetch_add(1, Ordering::Relaxed);
        format!("{}-{n}", self.prefix)
    }
}

// ---------------------------------------------------------------------------
// 2. Time Utilities
// ---------------------------------------------------------------------------

/// Current UTC time as unix seconds.
pub fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before UNIX epoch")
        .as_secs() as i64
}

/// Current UTC time as unix milliseconds.
pub fn unix_timestamp_ms() -> i128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before UNIX epoch")
        .as_millis() as i128
}

/// Current UTC time formatted as ISO 8601 (`YYYY-MM-DDTHH:MM:SSZ`).
///
/// Implemented without `chrono` to keep deps minimal.
pub fn iso8601_now() -> String {
    let secs = unix_timestamp();
    format_epoch_as_iso8601(secs)
}

/// Internal: format epoch seconds as ISO 8601.
fn format_epoch_as_iso8601(epoch: i64) -> String {
    let secs = epoch;
    let days = secs.div_euclid(86400);
    let day_secs = secs.rem_euclid(86400);
    let h = day_secs / 3600;
    let m = (day_secs % 3600) / 60;
    let s = day_secs % 60;

    // Civil date from days since epoch (algorithm from Howard Hinnant).
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let yr = if mo <= 2 { y + 1 } else { y };

    format!("{yr:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

/// Formats a duration in seconds to a human-readable string.
///
/// ```
/// use platform_common::format_duration;
/// assert_eq!(format_duration(3600 + 23 * 60 + 45), "1h 23m 45s");
/// assert_eq!(format_duration(90061), "1d 1h 1m 1s");
/// ```
pub fn format_duration(seconds: i64) -> String {
    if seconds < 0 {
        return format!("-{}", format_duration(-seconds));
    }
    let d = seconds / 86400;
    let h = (seconds % 86400) / 3600;
    let m = (seconds % 3600) / 60;
    let s = seconds % 60;

    let mut parts = Vec::new();
    if d > 0 {
        parts.push(format!("{d}d"));
    }
    if h > 0 {
        parts.push(format!("{h}h"));
    }
    if m > 0 {
        parts.push(format!("{m}m"));
    }
    if s > 0 || parts.is_empty() {
        parts.push(format!("{s}s"));
    }
    parts.join(" ")
}

/// Parses a human-readable duration string into total seconds.
///
/// Supports `d`, `h`, `m`, `s` suffixes. Components can appear in any order
/// and may be separated by whitespace.
///
/// ```
/// use platform_common::parse_duration;
/// assert_eq!(parse_duration("1h"), Some(3600));
/// assert_eq!(parse_duration("1d 2h"), Some(86400 + 7200));
/// ```
pub fn parse_duration(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    let mut total: i64 = 0;
    let mut num_buf = String::new();
    let mut found_any = false;

    for ch in s.chars() {
        if ch.is_ascii_digit() {
            num_buf.push(ch);
        } else if ch == 'd' || ch == 'h' || ch == 'm' || ch == 's' {
            if num_buf.is_empty() {
                return None;
            }
            let n: i64 = num_buf.parse().ok()?;
            num_buf.clear();
            let multiplier = match ch {
                'd' => 86400,
                'h' => 3600,
                'm' => 60,
                's' => 1,
                _ => unreachable!(),
            };
            total += n * multiplier;
            found_any = true;
        } else if ch.is_whitespace() {
            // skip
        } else {
            return None;
        }
    }

    // Trailing digits without suffix -> invalid
    if !num_buf.is_empty() {
        return None;
    }

    if found_any {
        Some(total)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// 3. Validation Utilities
// ---------------------------------------------------------------------------

/// Validation error kinds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationError {
    TooShort {
        field: String,
        min: usize,
        actual: usize,
    },
    TooLong {
        field: String,
        max: usize,
        actual: usize,
    },
    InvalidFormat {
        field: String,
        details: String,
    },
    InvalidCharacters {
        field: String,
        details: String,
    },
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort {
                field, min, actual, ..
            } => {
                write!(f, "{field}: too short (min {min}, got {actual})")
            }
            Self::TooLong {
                field, max, actual, ..
            } => {
                write!(f, "{field}: too long (max {max}, got {actual})")
            }
            Self::InvalidFormat { field, details } => {
                write!(f, "{field}: invalid format — {details}")
            }
            Self::InvalidCharacters { field, details } => {
                write!(f, "{field}: invalid characters — {details}")
            }
        }
    }
}

impl std::error::Error for ValidationError {}

/// Validates a username: 3–20 characters, alphanumeric + underscore only.
pub fn validate_username(name: &str) -> Result<(), ValidationError> {
    let len = name.len();
    if len < 3 {
        return Err(ValidationError::TooShort {
            field: "username".into(),
            min: 3,
            actual: len,
        });
    }
    if len > 20 {
        return Err(ValidationError::TooLong {
            field: "username".into(),
            max: 20,
            actual: len,
        });
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(ValidationError::InvalidCharacters {
            field: "username".into(),
            details: "only ASCII alphanumeric and underscore allowed".into(),
        });
    }
    Ok(())
}

/// Validates an email address (basic format check: `local@domain.tld`).
pub fn validate_email(email: &str) -> Result<(), ValidationError> {
    let parts: Vec<&str> = email.splitn(2, '@').collect();
    if parts.len() != 2 {
        return Err(ValidationError::InvalidFormat {
            field: "email".into(),
            details: "missing '@'".into(),
        });
    }
    let local = parts[0];
    let domain = parts[1];
    if local.is_empty() {
        return Err(ValidationError::InvalidFormat {
            field: "email".into(),
            details: "empty local part".into(),
        });
    }
    if !domain.contains('.') || domain.starts_with('.') || domain.ends_with('.') {
        return Err(ValidationError::InvalidFormat {
            field: "email".into(),
            details: "invalid domain".into(),
        });
    }
    if domain.len() < 3 {
        return Err(ValidationError::InvalidFormat {
            field: "email".into(),
            details: "domain too short".into(),
        });
    }
    Ok(())
}

/// Validates a password: min 8 chars, must contain uppercase, lowercase, digit.
pub fn validate_password(password: &str) -> Result<(), ValidationError> {
    let len = password.len();
    if len < 8 {
        return Err(ValidationError::TooShort {
            field: "password".into(),
            min: 8,
            actual: len,
        });
    }
    let has_upper = password.chars().any(|c| c.is_ascii_uppercase());
    let has_lower = password.chars().any(|c| c.is_ascii_lowercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    if !has_upper || !has_lower || !has_digit {
        return Err(ValidationError::InvalidFormat {
            field: "password".into(),
            details: "must contain uppercase, lowercase, and digit".into(),
        });
    }
    Ok(())
}

/// Validates an alliance tag: 2–8 characters, uppercase ASCII alphanumeric.
pub fn validate_alliance_tag(tag: &str) -> Result<(), ValidationError> {
    let len = tag.len();
    if len < 2 {
        return Err(ValidationError::TooShort {
            field: "alliance_tag".into(),
            min: 2,
            actual: len,
        });
    }
    if len > 8 {
        return Err(ValidationError::TooLong {
            field: "alliance_tag".into(),
            max: 8,
            actual: len,
        });
    }
    if !tag
        .chars()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
    {
        return Err(ValidationError::InvalidCharacters {
            field: "alliance_tag".into(),
            details: "only uppercase ASCII letters and digits allowed".into(),
        });
    }
    Ok(())
}

/// Strips HTML tags from input, returning only the text content.
pub fn sanitize_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut inside_tag = false;
    for ch in input.chars() {
        if ch == '<' {
            inside_tag = true;
        } else if ch == '>' {
            inside_tag = false;
        } else if !inside_tag {
            out.push(ch);
        }
    }
    out
}

/// Truncates a string to `max_len` characters, appending "..." if truncated.
///
/// If `max_len <= 3`, the result is simply the first `max_len` characters
/// (no room for the ellipsis suffix).
pub fn truncate(s: &str, max_len: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_len {
        return s.to_owned();
    }
    if max_len <= 3 {
        return s.chars().take(max_len).collect();
    }
    let truncated: String = s.chars().take(max_len - 3).collect();
    format!("{truncated}...")
}

// ---------------------------------------------------------------------------
// 4. Pagination
// ---------------------------------------------------------------------------

/// Parameters for paginated queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationParams {
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    20
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self {
            offset: 0,
            limit: 20,
        }
    }
}

impl PaginationParams {
    /// Clamps `limit` to the allowed maximum of 100.
    pub fn clamped(&self) -> Self {
        Self {
            offset: self.offset,
            limit: self.limit.min(100),
        }
    }
}

/// A paginated response envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub has_more: bool,
}

/// Paginates an in-memory slice according to `params`.
///
/// `limit` is clamped to a maximum of 100.
pub fn paginate<T: Clone>(items: &[T], params: &PaginationParams) -> PaginatedResponse<T> {
    let limit = params.limit.min(100);
    let total = items.len();
    let offset = params.offset.min(total);
    let end = (offset + limit).min(total);
    let page = items[offset..end].to_vec();
    let has_more = end < total;
    PaginatedResponse {
        items: page,
        total,
        offset,
        limit,
        has_more,
    }
}

// ---------------------------------------------------------------------------
// 5. Sorting / Filtering
// ---------------------------------------------------------------------------

/// Sort direction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortOrder {
    Ascending,
    Descending,
}

/// Sort parameters: field name + direction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortParams {
    pub field: String,
    pub order: SortOrder,
}

// ---------------------------------------------------------------------------
// 6. String Utilities
// ---------------------------------------------------------------------------

/// Converts a string to a URL-friendly slug.
///
/// ```
/// use platform_common::slugify;
/// assert_eq!(slugify("Hello World!"), "hello-world");
/// ```
pub fn slugify(s: &str) -> String {
    let mut slug = String::with_capacity(s.len());
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if ch == ' ' || ch == '-' || ch == '_' {
            // collapse consecutive hyphens later
            slug.push('-');
        }
        // else: strip
    }
    // Collapse consecutive hyphens and trim leading/trailing hyphens.
    let mut result = String::with_capacity(slug.len());
    let mut prev_hyphen = true; // treat start as hyphen to trim leading
    for ch in slug.chars() {
        if ch == '-' {
            if !prev_hyphen {
                result.push('-');
            }
            prev_hyphen = true;
        } else {
            result.push(ch);
            prev_hyphen = false;
        }
    }
    // Trim trailing hyphen.
    if result.ends_with('-') {
        result.pop();
    }
    result
}

/// Collapses runs of whitespace into a single space and trims.
pub fn normalize_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = true; // trim leading
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
            }
            prev_space = true;
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    // Trim trailing space.
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

/// Masks an email for display: `"user@example.com"` → `"u***@example.com"`.
///
/// If the local part is empty or there is no `@`, returns the input unchanged.
pub fn mask_email(email: &str) -> String {
    match email.split_once('@') {
        Some((local, domain)) if !local.is_empty() => {
            let first: String = local.chars().take(1).collect();
            format!("{first}***@{domain}")
        }
        _ => email.to_owned(),
    }
}

/// Masks an IP address, hiding the last two octets (IPv4).
///
/// `"192.168.1.42"` → `"192.168.***.**"`.
/// Non-IPv4 strings are returned unchanged.
pub fn mask_ip(ip: &str) -> String {
    let parts: Vec<&str> = ip.split('.').collect();
    if parts.len() == 4 {
        let masked_3 = "*".repeat(parts[2].len());
        let masked_4 = "*".repeat(parts[3].len());
        format!("{}.{}.{}.{}", parts[0], parts[1], masked_3, masked_4)
    } else {
        ip.to_owned()
    }
}

// ---------------------------------------------------------------------------
// 7. Math Utilities
// ---------------------------------------------------------------------------

/// Clamps `value` to `[min, max]`.
pub fn clamp<T: Ord>(value: T, min: T, max: T) -> T {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

/// Rounds `value` to `decimals` decimal places.
pub fn round_to(value: f64, decimals: u32) -> f64 {
    let factor = 10_f64.powi(decimals as i32);
    (value * factor).round() / factor
}

/// Returns the percentage `part / total * 100`. Returns `0.0` if `total` is
/// zero.
pub fn percentage(part: f64, total: f64) -> f64 {
    if total == 0.0 {
        0.0
    } else {
        (part / total) * 100.0
    }
}

/// Linear interpolation between `a` and `b` by factor `t` (`0.0..=1.0`).
pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

// ---------------------------------------------------------------------------
// 8. Result / Error Helpers
// ---------------------------------------------------------------------------

/// Application-level error with a machine-readable code, message, and HTTP
/// status hint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub status: u16,
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {} (HTTP {})", self.code, self.message, self.status)
    }
}

impl std::error::Error for AppError {}

/// Convenience alias for results carrying [`AppError`].
pub type AppResult<T> = Result<T, AppError>;

// ---------------------------------------------------------------------------
// 9. Environment Helpers
// ---------------------------------------------------------------------------

/// Returns the value of the environment variable `key`, or `default` if unset
/// or empty.
pub fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_owned())
}

/// Returns the value of `key` parsed as `T`, falling back to `default` on
/// any failure.
pub fn env_or_parse<T: FromStr>(key: &str, default: T) -> T {
    env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Returns the value of `key` or an error message if it is unset.
pub fn require_env(key: &str) -> Result<String, String> {
    env::var(key).map_err(|_| format!("required environment variable '{key}' is not set"))
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- ID generation ------------------------------------------------------

    #[test]
    fn test_generate_id_unique() {
        let a = generate_id();
        let b = generate_id();
        assert_ne!(a, b, "generated IDs must be unique");
    }

    #[test]
    fn test_generate_id_format() {
        let id = generate_id();
        // Should have three parts separated by '-'.
        let parts: Vec<&str> = id.splitn(3, '-').collect();
        assert_eq!(parts.len(), 3);
    }

    #[test]
    fn test_generate_short_id_length() {
        let id = generate_short_id();
        assert_eq!(id.len(), 8);
        assert!(id.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn test_generate_short_id_unique() {
        let a = generate_short_id();
        let b = generate_short_id();
        assert_ne!(a, b);
    }

    #[test]
    fn test_id_generator_sequential() {
        let gen = IdGenerator::new("ship");
        assert_eq!(gen.next(), "ship-1");
        assert_eq!(gen.next(), "ship-2");
        assert_eq!(gen.next(), "ship-3");
    }

    #[test]
    fn test_id_generator_clone_shares_counter() {
        let gen = IdGenerator::new("fleet");
        let gen2 = gen.clone();
        let _ = gen.next(); // fleet-1
        assert_eq!(gen2.next(), "fleet-2");
    }

    // -- Time utilities -----------------------------------------------------

    #[test]
    fn test_unix_timestamp_positive() {
        assert!(unix_timestamp() > 1_700_000_000);
    }

    #[test]
    fn test_unix_timestamp_ms_positive() {
        assert!(unix_timestamp_ms() > 1_700_000_000_000);
    }

    #[test]
    fn test_iso8601_now_format() {
        let ts = iso8601_now();
        // Basic structural check: YYYY-MM-DDTHH:MM:SSZ
        assert!(ts.ends_with('Z'));
        assert_eq!(ts.len(), 20);
        assert_eq!(&ts[4..5], "-");
        assert_eq!(&ts[10..11], "T");
    }

    #[test]
    fn test_format_duration_basic() {
        assert_eq!(format_duration(0), "0s");
        assert_eq!(format_duration(59), "59s");
        assert_eq!(format_duration(60), "1m");
        assert_eq!(format_duration(3661), "1h 1m 1s");
        assert_eq!(format_duration(86400 + 3600 + 60 + 1), "1d 1h 1m 1s");
    }

    #[test]
    fn test_format_duration_negative() {
        assert_eq!(format_duration(-3661), "-1h 1m 1s");
    }

    #[test]
    fn test_parse_duration_valid() {
        assert_eq!(parse_duration("1h"), Some(3600));
        assert_eq!(parse_duration("30m"), Some(1800));
        assert_eq!(parse_duration("1d 2h"), Some(86400 + 7200));
        assert_eq!(parse_duration("1h 30m 10s"), Some(3600 + 1800 + 10));
        assert_eq!(parse_duration("0s"), Some(0));
    }

    #[test]
    fn test_parse_duration_invalid() {
        assert_eq!(parse_duration(""), None);
        assert_eq!(parse_duration("abc"), None);
        assert_eq!(parse_duration("123"), None); // no suffix
        assert_eq!(parse_duration("h"), None); // no number
    }

    // -- Validation ---------------------------------------------------------

    #[test]
    fn test_validate_username_valid() {
        assert!(validate_username("ace").is_ok());
        assert!(validate_username("player_01").is_ok());
        assert!(validate_username("A_very_long_name1234").is_ok()); // 20 chars
    }

    #[test]
    fn test_validate_username_invalid() {
        assert!(matches!(
            validate_username("ab"),
            Err(ValidationError::TooShort { .. })
        ));
        assert!(matches!(
            validate_username("a]b"),
            Err(ValidationError::InvalidCharacters { .. })
        ));
        let long = "a".repeat(21);
        assert!(matches!(
            validate_username(&long),
            Err(ValidationError::TooLong { .. })
        ));
    }

    #[test]
    fn test_validate_email_valid() {
        assert!(validate_email("user@example.com").is_ok());
        assert!(validate_email("a@b.co").is_ok());
    }

    #[test]
    fn test_validate_email_invalid() {
        assert!(validate_email("noatsign").is_err());
        assert!(validate_email("@domain.com").is_err());
        assert!(validate_email("user@.com").is_err());
        assert!(validate_email("user@com").is_err());
    }

    #[test]
    fn test_validate_password_valid() {
        assert!(validate_password("Abcdef1!").is_ok());
        assert!(validate_password("StrongP4ss").is_ok());
    }

    #[test]
    fn test_validate_password_invalid() {
        assert!(matches!(
            validate_password("short1A"),
            Err(ValidationError::TooShort { .. })
        ));
        assert!(validate_password("alllowercase1").is_err()); // no uppercase
        assert!(validate_password("ALLUPPERCASE1").is_err()); // no lowercase
        assert!(validate_password("NoDigitsHere").is_err());
    }

    #[test]
    fn test_validate_alliance_tag() {
        assert!(validate_alliance_tag("AB").is_ok());
        assert!(validate_alliance_tag("ABCD1234").is_ok());
        assert!(validate_alliance_tag("A").is_err()); // too short
        assert!(validate_alliance_tag("ABCDEFGHI").is_err()); // too long (9)
        assert!(validate_alliance_tag("abcd").is_err()); // lowercase
    }

    #[test]
    fn test_sanitize_html() {
        assert_eq!(sanitize_html("<b>bold</b>"), "bold");
        assert_eq!(sanitize_html("no tags"), "no tags");
        assert_eq!(
            sanitize_html("<script>alert('xss')</script>hi"),
            "alert('xss')hi"
        );
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 8), "hello...");
        assert_eq!(truncate("abc", 3), "abc");
        assert_eq!(truncate("abcde", 3), "abc"); // max_len<=3, no room for ...
    }

    // -- Pagination ---------------------------------------------------------

    #[test]
    fn test_paginate_basic() {
        let items: Vec<i32> = (1..=50).collect();
        let params = PaginationParams {
            offset: 0,
            limit: 10,
        };
        let page = paginate(&items, &params);
        assert_eq!(page.items.len(), 10);
        assert_eq!(page.total, 50);
        assert_eq!(page.offset, 0);
        assert_eq!(page.limit, 10);
        assert!(page.has_more);
        assert_eq!(page.items[0], 1);
        assert_eq!(page.items[9], 10);
    }

    #[test]
    fn test_paginate_last_page() {
        let items: Vec<i32> = (1..=25).collect();
        let params = PaginationParams {
            offset: 20,
            limit: 20,
        };
        let page = paginate(&items, &params);
        assert_eq!(page.items.len(), 5);
        assert!(!page.has_more);
    }

    #[test]
    fn test_paginate_limit_clamped() {
        let items: Vec<i32> = (1..=200).collect();
        let params = PaginationParams {
            offset: 0,
            limit: 999,
        };
        let page = paginate(&items, &params);
        assert_eq!(page.items.len(), 100);
        assert_eq!(page.limit, 100);
    }

    #[test]
    fn test_pagination_params_default() {
        let p = PaginationParams::default();
        assert_eq!(p.offset, 0);
        assert_eq!(p.limit, 20);
    }

    // -- String utilities ---------------------------------------------------

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Hello World!"), "hello-world");
        assert_eq!(slugify("  Foo  BAR  "), "foo-bar");
        assert_eq!(slugify("already-slug"), "already-slug");
        assert_eq!(slugify("Special #$% chars"), "special-chars");
    }

    #[test]
    fn test_normalize_whitespace() {
        assert_eq!(normalize_whitespace("  hello   world  "), "hello world");
        assert_eq!(normalize_whitespace("ok"), "ok");
        assert_eq!(normalize_whitespace("\t\nnewlines\t"), "newlines");
    }

    #[test]
    fn test_mask_email() {
        assert_eq!(mask_email("user@example.com"), "u***@example.com");
        assert_eq!(mask_email("a@b.co"), "a***@b.co");
        assert_eq!(mask_email("noatsign"), "noatsign");
    }

    #[test]
    fn test_mask_ip() {
        assert_eq!(mask_ip("192.168.1.42"), "192.168.*.**");
        assert_eq!(mask_ip("10.0.100.200"), "10.0.***.***");
        assert_eq!(mask_ip("not-an-ip"), "not-an-ip");
    }

    // -- Math utilities -----------------------------------------------------

    #[test]
    fn test_clamp() {
        assert_eq!(clamp(5, 1, 10), 5);
        assert_eq!(clamp(-3, 0, 100), 0);
        assert_eq!(clamp(200, 0, 100), 100);
    }

    #[test]
    fn test_round_to() {
        assert!((round_to(3.14159, 2) - 3.14).abs() < 1e-10);
        assert!((round_to(2.5, 0) - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_percentage() {
        assert!((percentage(1.0, 4.0) - 25.0).abs() < 1e-10);
        assert!((percentage(0.0, 0.0)).abs() < 1e-10); // avoid NaN
    }

    #[test]
    fn test_lerp() {
        assert!((lerp(0.0, 10.0, 0.0) - 0.0).abs() < 1e-10);
        assert!((lerp(0.0, 10.0, 0.5) - 5.0).abs() < 1e-10);
        assert!((lerp(0.0, 10.0, 1.0) - 10.0).abs() < 1e-10);
    }

    // -- Error helpers ------------------------------------------------------

    #[test]
    fn test_app_error_display() {
        let err = AppError {
            code: "NOT_FOUND".into(),
            message: "planet not found".into(),
            status: 404,
        };
        let s = err.to_string();
        assert!(s.contains("NOT_FOUND"));
        assert!(s.contains("404"));
    }

    // -- Environment helpers ------------------------------------------------

    #[test]
    fn test_env_or_default() {
        // Use a key that is almost certainly not set.
        let val = env_or("__UNIVERSUS_TEST_MISSING_KEY__", "fallback");
        assert_eq!(val, "fallback");
    }

    #[test]
    fn test_env_or_parse_default() {
        let val: u16 = env_or_parse("__UNIVERSUS_TEST_MISSING_PORT__", 8080);
        assert_eq!(val, 8080);
    }

    #[test]
    fn test_require_env_missing() {
        let result = require_env("__UNIVERSUS_TEST_MISSING__");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not set"));
    }

    #[test]
    fn test_require_env_present() {
        // Set it, test it, clean up.
        env::set_var("__UNIVERSUS_TEST_PRESENT__", "hello");
        let result = require_env("__UNIVERSUS_TEST_PRESENT__");
        assert_eq!(result.unwrap(), "hello");
        env::remove_var("__UNIVERSUS_TEST_PRESENT__");
    }

    // -- Sort types ---------------------------------------------------------

    #[test]
    fn test_sort_order_serde() {
        let json = serde_json::to_string(&SortOrder::Descending).unwrap();
        let parsed: SortOrder = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, SortOrder::Descending);
    }

    // -- Pagination serde ---------------------------------------------------

    #[test]
    fn test_pagination_params_serde_defaults() {
        let json = "{}";
        let params: PaginationParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.offset, 0);
        assert_eq!(params.limit, 20);
    }

    // -- ISO 8601 internal --------------------------------------------------

    #[test]
    fn test_format_epoch_known_date() {
        // 2024-01-01T00:00:00Z = 1704067200
        let iso = format_epoch_as_iso8601(1_704_067_200);
        assert_eq!(iso, "2024-01-01T00:00:00Z");
    }
}
