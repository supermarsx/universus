#![forbid(unsafe_code)]
//! Core building blocks for the platform-common crate.

use serde::Deserialize;
use std::time::SystemTime;

/// Returns the crate name for a basic compile-time sanity check.
pub const fn crate_name() -> &'static str {
    "platform-common"
}

/// Generates a unique ID using the current timestamp in nanoseconds combined
/// with a pseudo-random component derived from memory addresses.
pub fn generate_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    // Use a stack variable's address as a cheap source of entropy.
    let entropy: u64 = {
        let local = 0u8;
        let addr = &local as *const u8 as u64;
        addr.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1)
    };

    format!("{nanos:x}-{entropy:x}")
}

/// Returns the current unix timestamp in seconds.
pub fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Returns the current unix timestamp in milliseconds.
pub fn unix_timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Formats a unix timestamp (seconds) as an ISO-like string `"unix:{secs}"`.
pub fn format_timestamp(secs: i64) -> String {
    format!("unix:{secs}")
}

/// Clamps `value` so that it is at least `min` and at most `max`.
pub fn clamp<T: Ord>(value: T, min: T, max: T) -> T {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

/// Safely truncates a string to at most `max_len` bytes on a valid UTF-8
/// boundary, returning a new `String`.
pub fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        return s.to_string();
    }
    // Walk backwards from max_len to find a char boundary.
    let mut end = max_len;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

/// Pagination parameters for list endpoints.
#[derive(Debug, Clone, Deserialize)]
pub struct Pagination {
    pub page: usize,
    pub per_page: usize,
}

impl Pagination {
    /// Returns the zero-based offset for database queries.
    pub fn offset(&self) -> usize {
        self.page.saturating_sub(1) * self.per_page
    }

    /// Returns the limit (number of items per page).
    pub fn limit(&self) -> usize {
        self.per_page
    }
}

/// Sort direction for ordered queries.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub enum SortDirection {
    Asc,
    Desc,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── crate_name ──────────────────────────────────────────────────

    #[test]
    fn test_crate_name() {
        assert_eq!(crate_name(), "platform-common");
    }

    // ── generate_id ─────────────────────────────────────────────────

    #[test]
    fn test_generate_id_not_empty() {
        let id = generate_id();
        assert!(!id.is_empty());
    }

    #[test]
    fn test_generate_id_contains_separator() {
        let id = generate_id();
        assert!(id.contains('-'), "ID should contain a '-' separator");
    }

    #[test]
    fn test_generate_id_uniqueness() {
        let ids: Vec<String> = (0..50).map(|_| generate_id()).collect();
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                assert_ne!(ids[i], ids[j], "IDs should be unique");
            }
        }
    }

    // ── unix_timestamp ──────────────────────────────────────────────

    #[test]
    fn test_unix_timestamp_positive() {
        assert!(unix_timestamp() > 0);
    }

    #[test]
    fn test_unix_timestamp_reasonable_range() {
        let ts = unix_timestamp();
        // After 2020-01-01 and before 2100-01-01
        assert!(ts > 1_577_836_800);
        assert!(ts < 4_102_444_800);
    }

    // ── unix_timestamp_ms ───────────────────────────────────────────

    #[test]
    fn test_unix_timestamp_ms_positive() {
        assert!(unix_timestamp_ms() > 0);
    }

    #[test]
    fn test_unix_timestamp_ms_greater_than_seconds() {
        let ms = unix_timestamp_ms();
        let secs = unix_timestamp() as u128;
        assert!(ms >= secs * 1000);
    }

    // ── format_timestamp ────────────────────────────────────────────

    #[test]
    fn test_format_timestamp_basic() {
        assert_eq!(format_timestamp(0), "unix:0");
    }

    #[test]
    fn test_format_timestamp_positive() {
        assert_eq!(format_timestamp(1_700_000_000), "unix:1700000000");
    }

    #[test]
    fn test_format_timestamp_negative() {
        assert_eq!(format_timestamp(-100), "unix:-100");
    }

    // ── clamp ───────────────────────────────────────────────────────

    #[test]
    fn test_clamp_within_range() {
        assert_eq!(clamp(5, 1, 10), 5);
    }

    #[test]
    fn test_clamp_below_min() {
        assert_eq!(clamp(-3, 0, 10), 0);
    }

    #[test]
    fn test_clamp_above_max() {
        assert_eq!(clamp(100, 0, 10), 10);
    }

    #[test]
    fn test_clamp_at_boundaries() {
        assert_eq!(clamp(0, 0, 10), 0);
        assert_eq!(clamp(10, 0, 10), 10);
    }

    #[test]
    fn test_clamp_single_value_range() {
        assert_eq!(clamp(5, 3, 3), 3);
    }

    // ── truncate_string ─────────────────────────────────────────────

    #[test]
    fn test_truncate_string_shorter_than_max() {
        assert_eq!(truncate_string("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_string_exact_length() {
        assert_eq!(truncate_string("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_string_longer_than_max() {
        assert_eq!(truncate_string("hello world", 5), "hello");
    }

    #[test]
    fn test_truncate_string_empty() {
        assert_eq!(truncate_string("", 5), "");
    }

    #[test]
    fn test_truncate_string_zero_max() {
        assert_eq!(truncate_string("hello", 0), "");
    }

    #[test]
    fn test_truncate_string_multibyte_char() {
        // 'é' is 2 bytes in UTF-8; truncating at 1 byte must not split it.
        let s = "é";
        let result = truncate_string(s, 1);
        assert!(result.is_empty());
    }

    #[test]
    fn test_truncate_string_multibyte_boundary() {
        // "aé" = 3 bytes ('a'=1, 'é'=2). Truncate at 2 should give "a".
        assert_eq!(truncate_string("aé", 2), "a");
    }

    // ── Pagination ──────────────────────────────────────────────────

    #[test]
    fn test_pagination_offset_page_one() {
        let p = Pagination { page: 1, per_page: 20 };
        assert_eq!(p.offset(), 0);
    }

    #[test]
    fn test_pagination_offset_page_three() {
        let p = Pagination { page: 3, per_page: 10 };
        assert_eq!(p.offset(), 20);
    }

    #[test]
    fn test_pagination_offset_page_zero() {
        // page 0 is treated the same as page 1 via saturating_sub
        let p = Pagination { page: 0, per_page: 10 };
        assert_eq!(p.offset(), 0);
    }

    #[test]
    fn test_pagination_limit() {
        let p = Pagination { page: 1, per_page: 25 };
        assert_eq!(p.limit(), 25);
    }

    // ── SortDirection ───────────────────────────────────────────────

    #[test]
    fn test_sort_direction_equality() {
        assert_eq!(SortDirection::Asc, SortDirection::Asc);
        assert_eq!(SortDirection::Desc, SortDirection::Desc);
        assert_ne!(SortDirection::Asc, SortDirection::Desc);
    }

    #[test]
    fn test_sort_direction_debug() {
        assert_eq!(format!("{:?}", SortDirection::Asc), "Asc");
        assert_eq!(format!("{:?}", SortDirection::Desc), "Desc");
    }
}
