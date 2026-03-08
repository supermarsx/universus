#![forbid(unsafe_code)]
//! Shared environment configuration helpers and a comprehensive configuration store
//! for game parameters with constraints, history tracking, and JSON import/export.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

// ---------------------------------------------------------------------------
// Existing helpers (unchanged)
// ---------------------------------------------------------------------------

/// Reads an environment variable and returns `None` if it is missing or invalid Unicode.
pub fn env(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

/// Reads an environment variable, returning the provided default when missing.
pub fn env_or(key: &str, default: &str) -> String {
    env(key).unwrap_or_else(|| default.to_string())
}

/// Reads an environment variable as `u16`, returning the provided default on parse failure.
pub fn parse_u16_env(key: &str, default: u16) -> u16 {
    env(key)
        .and_then(|raw| raw.parse::<u16>().ok())
        .unwrap_or(default)
}

// ---------------------------------------------------------------------------
// Type-safe env parsing helpers
// ---------------------------------------------------------------------------

/// Generic env parser: reads an environment variable and parses it as `T`,
/// returning `default` on missing or parse failure.
pub fn parse_env<T: FromStr>(key: &str, default: T) -> T {
    env(key)
        .and_then(|raw| raw.parse::<T>().ok())
        .unwrap_or(default)
}

/// Reads an environment variable as a boolean. Recognises `"true"`, `"1"`, `"yes"`
/// (case-insensitive) as `true`; everything else (including missing) returns `default`.
pub fn parse_bool_env(key: &str, default: bool) -> bool {
    match env(key) {
        Some(raw) => {
            let lower = raw.trim().to_lowercase();
            match lower.as_str() {
                "true" | "1" | "yes" => true,
                "false" | "0" | "no" => false,
                _ => default,
            }
        }
        None => default,
    }
}

/// Reads an environment variable as `u32`.
pub fn parse_u32_env(key: &str, default: u32) -> u32 {
    parse_env(key, default)
}

/// Reads an environment variable as `i64`.
pub fn parse_i64_env(key: &str, default: i64) -> i64 {
    parse_env(key, default)
}

/// Reads an environment variable as `f64`.
pub fn parse_f64_env(key: &str, default: f64) -> f64 {
    parse_env(key, default)
}

/// Reads an environment variable as a duration in seconds.
/// Accepts human-friendly suffixes: `"30s"`, `"5m"`, `"1h"`, or plain seconds (`"120"`).
/// Returns `default_secs` on missing or parse failure.
pub fn parse_duration_env(key: &str, default_secs: u64) -> u64 {
    match env(key) {
        Some(raw) => parse_duration_string(&raw).unwrap_or(default_secs),
        None => default_secs,
    }
}

fn parse_duration_string(raw: &str) -> Option<u64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(num) = trimmed.strip_suffix('s') {
        return num.trim().parse::<u64>().ok();
    }
    if let Some(num) = trimmed.strip_suffix('m') {
        return num.trim().parse::<u64>().ok().map(|v| v * 60);
    }
    if let Some(num) = trimmed.strip_suffix('h') {
        return num.trim().parse::<u64>().ok().map(|v| v * 3600);
    }
    trimmed.parse::<u64>().ok()
}

/// Reads an environment variable as a comma-separated list of strings.
/// Trims whitespace around each element. Returns an empty `Vec` if missing.
pub fn parse_list_env(key: &str) -> Vec<String> {
    match env(key) {
        Some(raw) => raw
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        None => Vec::new(),
    }
}

/// Reads a required environment variable, returning a `ConfigError` if missing.
pub fn require_env(key: &str) -> Result<String, ConfigError> {
    env(key).ok_or_else(|| ConfigError::MissingRequired(key.to_string()))
}

// ---------------------------------------------------------------------------
// ConfigError
// ---------------------------------------------------------------------------

/// Errors produced by the configuration system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConfigError {
    NotFound,
    InvalidValue(String),
    ConstraintViolation(String),
    MissingRequired(String),
    ParseError(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::NotFound => write!(f, "parameter not found"),
            ConfigError::InvalidValue(msg) => write!(f, "invalid value: {msg}"),
            ConfigError::ConstraintViolation(msg) => write!(f, "constraint violation: {msg}"),
            ConfigError::MissingRequired(key) => {
                write!(f, "required environment variable missing: {key}")
            }
            ConfigError::ParseError(msg) => write!(f, "parse error: {msg}"),
        }
    }
}

impl std::error::Error for ConfigError {}

// ---------------------------------------------------------------------------
// DataType
// ---------------------------------------------------------------------------

/// The logical type of a configuration parameter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DataType {
    String,
    Integer,
    Float,
    Boolean,
    Duration,
    List,
}

// ---------------------------------------------------------------------------
// ParameterConstraints
// ---------------------------------------------------------------------------

/// Optional constraints applied when setting a parameter value.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParameterConstraints {
    pub min_value: Option<f64>,
    pub max_value: Option<f64>,
    pub allowed_values: Option<Vec<String>>,
    pub pattern: Option<String>,
}

// ---------------------------------------------------------------------------
// ConfigParameter
// ---------------------------------------------------------------------------

/// A single configuration parameter with metadata and optional constraints.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigParameter {
    pub key: String,
    pub category: String,
    pub value: String,
    pub default_value: String,
    pub data_type: DataType,
    pub description: String,
    pub constraints: Option<ParameterConstraints>,
    pub modified_at: Option<String>,
}

// ---------------------------------------------------------------------------
// ConfigChange / ConfigHistory
// ---------------------------------------------------------------------------

/// A recorded change to a configuration parameter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigChange {
    pub change_id: u64,
    pub parameter_key: String,
    pub old_value: String,
    pub new_value: String,
    pub reason: String,
    pub changed_at: String,
}

/// Append-only history of configuration changes.
#[derive(Debug, Clone, Default)]
pub struct ConfigHistory {
    changes: Vec<ConfigChange>,
    next_id: u64,
}

impl ConfigHistory {
    pub fn new() -> Self {
        Self {
            changes: Vec::new(),
            next_id: 1,
        }
    }

    /// Record a new change entry. `change_id` is assigned automatically.
    pub fn record_change(&mut self, mut change: ConfigChange) {
        change.change_id = self.next_id;
        self.next_id += 1;
        self.changes.push(change);
    }

    /// Return the most recent `limit` changes (newest first).
    pub fn list_changes(&self, limit: usize) -> Vec<ConfigChange> {
        self.changes.iter().rev().take(limit).cloned().collect()
    }

    /// Return the most recent `limit` changes for a specific parameter key (newest first).
    pub fn changes_for_parameter(&self, key: &str, limit: usize) -> Vec<ConfigChange> {
        self.changes
            .iter()
            .rev()
            .filter(|c| c.parameter_key == key)
            .take(limit)
            .cloned()
            .collect()
    }
}

// ---------------------------------------------------------------------------
// ConfigStore
// ---------------------------------------------------------------------------

/// In-memory configuration store backed by a `HashMap`.
#[derive(Debug, Clone)]
pub struct ConfigStore {
    parameters: HashMap<String, ConfigParameter>,
    pub history: ConfigHistory,
}

impl ConfigStore {
    /// Create an empty `ConfigStore`.
    pub fn new() -> Self {
        Self {
            parameters: HashMap::new(),
            history: ConfigHistory::new(),
        }
    }

    /// Create a `ConfigStore` pre-populated with default game parameters.
    pub fn with_defaults() -> Self {
        let mut store = Self::new();

        // -- economy --
        store.insert_default(ConfigParameter {
            key: "economy.resource_multiplier".into(),
            category: "economy".into(),
            value: "1".into(),
            default_value: "1".into(),
            data_type: DataType::Integer,
            description: "Global resource production multiplier".into(),
            constraints: Some(ParameterConstraints {
                min_value: Some(1.0),
                max_value: Some(100.0),
                allowed_values: None,
                pattern: None,
            }),
            modified_at: None,
        });
        store.insert_default(ConfigParameter {
            key: "economy.speed".into(),
            category: "economy".into(),
            value: "1".into(),
            default_value: "1".into(),
            data_type: DataType::Integer,
            description: "Universe speed factor".into(),
            constraints: Some(ParameterConstraints {
                min_value: Some(1.0),
                max_value: Some(10.0),
                allowed_values: None,
                pattern: None,
            }),
            modified_at: None,
        });

        // -- combat --
        store.insert_default(ConfigParameter {
            key: "combat.debris_factor".into(),
            category: "combat".into(),
            value: "0.3".into(),
            default_value: "0.3".into(),
            data_type: DataType::Float,
            description: "Fraction of destroyed fleet that becomes debris".into(),
            constraints: Some(ParameterConstraints {
                min_value: Some(0.0),
                max_value: Some(1.0),
                allowed_values: None,
                pattern: None,
            }),
            modified_at: None,
        });
        store.insert_default(ConfigParameter {
            key: "combat.defense_repair_rate".into(),
            category: "combat".into(),
            value: "0.7".into(),
            default_value: "0.7".into(),
            data_type: DataType::Float,
            description: "Fraction of destroyed defenses that are automatically repaired".into(),
            constraints: Some(ParameterConstraints {
                min_value: Some(0.0),
                max_value: Some(1.0),
                allowed_values: None,
                pattern: None,
            }),
            modified_at: None,
        });
        store.insert_default(ConfigParameter {
            key: "combat.max_rounds".into(),
            category: "combat".into(),
            value: "6".into(),
            default_value: "6".into(),
            data_type: DataType::Integer,
            description: "Maximum combat simulation rounds".into(),
            constraints: Some(ParameterConstraints {
                min_value: Some(1.0),
                max_value: Some(20.0),
                allowed_values: None,
                pattern: None,
            }),
            modified_at: None,
        });

        // -- fleet --
        store.insert_default(ConfigParameter {
            key: "fleet.speed_multiplier".into(),
            category: "fleet".into(),
            value: "1.0".into(),
            default_value: "1.0".into(),
            data_type: DataType::Float,
            description: "Global fleet speed multiplier".into(),
            constraints: Some(ParameterConstraints {
                min_value: Some(0.1),
                max_value: Some(100.0),
                allowed_values: None,
                pattern: None,
            }),
            modified_at: None,
        });
        store.insert_default(ConfigParameter {
            key: "fleet.fuel_multiplier".into(),
            category: "fleet".into(),
            value: "1.0".into(),
            default_value: "1.0".into(),
            data_type: DataType::Float,
            description: "Global fleet fuel consumption multiplier".into(),
            constraints: None,
            modified_at: None,
        });

        // -- galaxy --
        store.insert_default(ConfigParameter {
            key: "galaxy.max_galaxies".into(),
            category: "galaxy".into(),
            value: "9".into(),
            default_value: "9".into(),
            data_type: DataType::Integer,
            description: "Maximum number of galaxies".into(),
            constraints: Some(ParameterConstraints {
                min_value: Some(1.0),
                max_value: Some(99.0),
                allowed_values: None,
                pattern: None,
            }),
            modified_at: None,
        });
        store.insert_default(ConfigParameter {
            key: "galaxy.max_systems".into(),
            category: "galaxy".into(),
            value: "499".into(),
            default_value: "499".into(),
            data_type: DataType::Integer,
            description: "Maximum number of systems per galaxy".into(),
            constraints: Some(ParameterConstraints {
                min_value: Some(50.0),
                max_value: Some(999.0),
                allowed_values: None,
                pattern: None,
            }),
            modified_at: None,
        });
        store.insert_default(ConfigParameter {
            key: "galaxy.max_positions".into(),
            category: "galaxy".into(),
            value: "15".into(),
            default_value: "15".into(),
            data_type: DataType::Integer,
            description: "Maximum number of positions (slots) per system".into(),
            constraints: Some(ParameterConstraints {
                min_value: Some(10.0),
                max_value: Some(25.0),
                allowed_values: None,
                pattern: None,
            }),
            modified_at: None,
        });

        // -- marketplace --
        store.insert_default(ConfigParameter {
            key: "marketplace.tax_rate".into(),
            category: "marketplace".into(),
            value: "0.1".into(),
            default_value: "0.1".into(),
            data_type: DataType::Float,
            description: "Transaction tax rate for marketplace trades".into(),
            constraints: Some(ParameterConstraints {
                min_value: Some(0.0),
                max_value: Some(0.5),
                allowed_values: None,
                pattern: None,
            }),
            modified_at: None,
        });
        store.insert_default(ConfigParameter {
            key: "marketplace.listing_duration_hours".into(),
            category: "marketplace".into(),
            value: "48".into(),
            default_value: "48".into(),
            data_type: DataType::Integer,
            description: "Default listing duration in hours".into(),
            constraints: None,
            modified_at: None,
        });

        // -- security --
        store.insert_default(ConfigParameter {
            key: "security.max_sessions_per_user".into(),
            category: "security".into(),
            value: "5".into(),
            default_value: "5".into(),
            data_type: DataType::Integer,
            description: "Maximum concurrent sessions per user".into(),
            constraints: None,
            modified_at: None,
        });
        store.insert_default(ConfigParameter {
            key: "security.token_expiry_seconds".into(),
            category: "security".into(),
            value: "3600".into(),
            default_value: "3600".into(),
            data_type: DataType::Integer,
            description: "Authentication token expiry in seconds".into(),
            constraints: None,
            modified_at: None,
        });
        store.insert_default(ConfigParameter {
            key: "security.noob_protection_points".into(),
            category: "security".into(),
            value: "50000".into(),
            default_value: "50000".into(),
            data_type: DataType::Integer,
            description: "Points threshold below which new-player protection applies".into(),
            constraints: None,
            modified_at: None,
        });

        store
    }

    /// Internal helper to insert a default parameter.
    fn insert_default(&mut self, param: ConfigParameter) {
        self.parameters.insert(param.key.clone(), param);
    }

    /// Get a reference to a parameter by key.
    pub fn get(&self, key: &str) -> Option<&ConfigParameter> {
        self.parameters.get(key)
    }

    /// Get the current value string for a parameter.
    pub fn get_value(&self, key: &str) -> Option<&str> {
        self.parameters.get(key).map(|p| p.value.as_str())
    }

    /// Get the value parsed as `i64`.
    pub fn get_int(&self, key: &str) -> Option<i64> {
        self.get_value(key).and_then(|v| v.parse::<i64>().ok())
    }

    /// Get the value parsed as `f64`.
    pub fn get_float(&self, key: &str) -> Option<f64> {
        self.get_value(key).and_then(|v| v.parse::<f64>().ok())
    }

    /// Get the value parsed as `bool`.
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get_value(key)
            .and_then(|v| match v.trim().to_lowercase().as_str() {
                "true" | "1" | "yes" => Some(true),
                "false" | "0" | "no" => Some(false),
                _ => None,
            })
    }

    /// Validate a prospective value against a parameter's constraints without
    /// actually setting it.
    pub fn validate_value(&self, key: &str, value: &str) -> Result<(), ConfigError> {
        let param = self.parameters.get(key).ok_or(ConfigError::NotFound)?;
        Self::check_constraints(value, &param.data_type, &param.constraints)
    }

    /// Set a parameter value. Validates against constraints first and records
    /// the change in history. Returns the updated parameter.
    pub fn set(
        &mut self,
        key: &str,
        value: &str,
        reason: &str,
    ) -> Result<ConfigParameter, ConfigError> {
        let param = self.parameters.get(key).ok_or(ConfigError::NotFound)?;
        Self::check_constraints(value, &param.data_type, &param.constraints)?;

        let old_value = param.value.clone();
        let param = self.parameters.get_mut(key).unwrap();
        param.value = value.to_string();
        param.modified_at = Some(now_iso());

        let change = ConfigChange {
            change_id: 0, // assigned by history
            parameter_key: key.to_string(),
            old_value,
            new_value: value.to_string(),
            reason: reason.to_string(),
            changed_at: now_iso(),
        };
        self.history.record_change(change);

        Ok(self.parameters.get(key).unwrap().clone())
    }

    /// Reset a parameter to its default value and record the change.
    pub fn reset_to_default(&mut self, key: &str) -> Result<ConfigParameter, ConfigError> {
        let param = self.parameters.get(key).ok_or(ConfigError::NotFound)?;
        let default = param.default_value.clone();
        self.set(key, &default, "reset to default")
    }

    /// List parameters, optionally filtered by category, sorted by key.
    pub fn list(&self, category: Option<&str>) -> Vec<&ConfigParameter> {
        let mut params: Vec<&ConfigParameter> = self
            .parameters
            .values()
            .filter(|p| match category {
                Some(cat) => p.category == cat,
                None => true,
            })
            .collect();
        params.sort_by(|a, b| a.key.cmp(&b.key));
        params
    }

    /// Return a sorted list of unique category names.
    pub fn categories(&self) -> Vec<String> {
        let mut cats: Vec<String> = self
            .parameters
            .values()
            .map(|p| p.category.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        cats.sort();
        cats
    }

    // -- internal helpers ---------------------------------------------------

    fn check_constraints(
        value: &str,
        data_type: &DataType,
        constraints: &Option<ParameterConstraints>,
    ) -> Result<(), ConfigError> {
        // Type validation
        match data_type {
            DataType::Integer => {
                value.parse::<i64>().map_err(|_| {
                    ConfigError::InvalidValue(format!("expected integer, got '{value}'"))
                })?;
            }
            DataType::Float => {
                value.parse::<f64>().map_err(|_| {
                    ConfigError::InvalidValue(format!("expected float, got '{value}'"))
                })?;
            }
            DataType::Boolean => {
                let lower = value.trim().to_lowercase();
                if !matches!(lower.as_str(), "true" | "false" | "1" | "0" | "yes" | "no") {
                    return Err(ConfigError::InvalidValue(format!(
                        "expected boolean, got '{value}'"
                    )));
                }
            }
            DataType::Duration => {
                if parse_duration_string(value).is_none() {
                    return Err(ConfigError::InvalidValue(format!(
                        "expected duration, got '{value}'"
                    )));
                }
            }
            DataType::String | DataType::List => {}
        }

        // Constraint validation
        if let Some(ref c) = constraints {
            // Numeric range checks (for Integer / Float / Duration)
            let numeric: Option<f64> = match data_type {
                DataType::Integer => value.parse::<i64>().ok().map(|v| v as f64),
                DataType::Float => value.parse::<f64>().ok(),
                DataType::Duration => parse_duration_string(value).map(|v| v as f64),
                _ => None,
            };

            if let Some(num) = numeric {
                if let Some(min) = c.min_value {
                    if num < min {
                        return Err(ConfigError::ConstraintViolation(format!(
                            "value {num} is below minimum {min}"
                        )));
                    }
                }
                if let Some(max) = c.max_value {
                    if num > max {
                        return Err(ConfigError::ConstraintViolation(format!(
                            "value {num} is above maximum {max}"
                        )));
                    }
                }
            }

            if let Some(ref allowed) = c.allowed_values {
                if !allowed.contains(&value.to_string()) {
                    return Err(ConfigError::ConstraintViolation(format!(
                        "value '{value}' is not in the allowed list"
                    )));
                }
            }

            if let Some(ref pattern) = c.pattern {
                if !simple_glob_match(pattern, value) {
                    return Err(ConfigError::ConstraintViolation(format!(
                        "value '{value}' does not match pattern '{pattern}'"
                    )));
                }
            }
        }

        Ok(())
    }
}

impl Default for ConfigStore {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ConfigSnapshot — JSON import / export
// ---------------------------------------------------------------------------

/// Helpers for exporting and importing configuration as JSON.
pub struct ConfigSnapshot;

impl ConfigSnapshot {
    /// Serialize all parameters in the store to a JSON string.
    pub fn export_json(store: &ConfigStore) -> String {
        let params: Vec<&ConfigParameter> = {
            let mut v: Vec<&ConfigParameter> = store.parameters.values().collect();
            v.sort_by(|a, b| a.key.cmp(&b.key));
            v
        };
        serde_json::to_string_pretty(&params).unwrap_or_else(|_| "[]".to_string())
    }

    /// Import parameters from a JSON string into the store.
    /// Only updates parameters that already exist in the store.
    /// Returns the count of parameters successfully updated.
    pub fn import_json(store: &mut ConfigStore, json: &str) -> Result<usize, ConfigError> {
        let incoming: Vec<ConfigParameter> = serde_json::from_str(json)
            .map_err(|e| ConfigError::ParseError(format!("invalid JSON: {e}")))?;

        let mut count = 0;
        for param in incoming {
            if store.parameters.contains_key(&param.key) {
                store.set(&param.key, &param.value, "imported from JSON snapshot")?;
                count += 1;
            }
        }
        Ok(count)
    }
}

// ---------------------------------------------------------------------------
// Utility helpers
// ---------------------------------------------------------------------------

/// Simple glob match supporting `*` (any chars) and `?` (single char).
fn simple_glob_match(pattern: &str, value: &str) -> bool {
    let pat: Vec<char> = pattern.chars().collect();
    let val: Vec<char> = value.chars().collect();
    glob_match_inner(&pat, &val)
}

fn glob_match_inner(pat: &[char], val: &[char]) -> bool {
    if pat.is_empty() {
        return val.is_empty();
    }
    match pat[0] {
        '*' => {
            // Try consuming 0..=n characters from val
            for i in 0..=val.len() {
                if glob_match_inner(&pat[1..], &val[i..]) {
                    return true;
                }
            }
            false
        }
        '?' => {
            if val.is_empty() {
                false
            } else {
                glob_match_inner(&pat[1..], &val[1..])
            }
        }
        c => {
            if val.is_empty() || val[0] != c {
                false
            } else {
                glob_match_inner(&pat[1..], &val[1..])
            }
        }
    }
}

/// Returns a simple ISO-8601-ish timestamp string (no external dependency).
fn now_iso() -> String {
    // We use a monotonic counter approach to keep tests deterministic-friendly.
    // In production you'd use chrono or similar; here we stay dependency-light.
    format!("{:?}", std::time::SystemTime::now())
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- env helpers --------------------------------------------------------

    #[test]
    fn test_env_missing() {
        assert!(env("__PLATFORM_CONFIG_TEST_MISSING__").is_none());
    }

    #[test]
    fn test_env_or_default() {
        let val = env_or("__PLATFORM_CONFIG_TEST_MISSING__", "fallback");
        assert_eq!(val, "fallback");
    }

    #[test]
    fn test_env_or_present() {
        std::env::set_var("__PCFG_TEST_OR__", "hello");
        assert_eq!(env_or("__PCFG_TEST_OR__", "x"), "hello");
        std::env::remove_var("__PCFG_TEST_OR__");
    }

    #[test]
    fn test_parse_u16_env_default() {
        assert_eq!(parse_u16_env("__PCFG_NO_EXIST__", 42), 42);
    }

    #[test]
    fn test_parse_env_generic() {
        std::env::set_var("__PCFG_GEN__", "99");
        assert_eq!(parse_env::<u32>("__PCFG_GEN__", 0), 99);
        std::env::remove_var("__PCFG_GEN__");
    }

    #[test]
    fn test_parse_bool_env_true_variants() {
        for val in &["true", "1", "yes", "TRUE", "Yes"] {
            std::env::set_var("__PCFG_BOOL__", val);
            assert!(parse_bool_env("__PCFG_BOOL__", false));
        }
        std::env::remove_var("__PCFG_BOOL__");
    }

    #[test]
    fn test_parse_bool_env_false_variants() {
        for val in &["false", "0", "no"] {
            std::env::set_var("__PCFG_BOOL2__", val);
            assert!(!parse_bool_env("__PCFG_BOOL2__", true));
        }
        std::env::remove_var("__PCFG_BOOL2__");
    }

    #[test]
    fn test_parse_bool_env_default() {
        assert!(parse_bool_env("__PCFG_BOOL_MISS__", true));
        assert!(!parse_bool_env("__PCFG_BOOL_MISS__", false));
    }

    #[test]
    fn test_parse_u32_env() {
        std::env::set_var("__PCFG_U32__", "12345");
        assert_eq!(parse_u32_env("__PCFG_U32__", 0), 12345);
        std::env::remove_var("__PCFG_U32__");
    }

    #[test]
    fn test_parse_i64_env() {
        std::env::set_var("__PCFG_I64__", "-42");
        assert_eq!(parse_i64_env("__PCFG_I64__", 0), -42);
        std::env::remove_var("__PCFG_I64__");
    }

    #[test]
    fn test_parse_f64_env() {
        std::env::set_var("__PCFG_F64__", "3.14");
        let v = parse_f64_env("__PCFG_F64__", 0.0);
        assert!((v - 3.14).abs() < f64::EPSILON);
        std::env::remove_var("__PCFG_F64__");
    }

    #[test]
    fn test_parse_duration_env_seconds() {
        std::env::set_var("__PCFG_DUR__", "30s");
        assert_eq!(parse_duration_env("__PCFG_DUR__", 0), 30);
        std::env::remove_var("__PCFG_DUR__");
    }

    #[test]
    fn test_parse_duration_env_minutes() {
        std::env::set_var("__PCFG_DUR2__", "5m");
        assert_eq!(parse_duration_env("__PCFG_DUR2__", 0), 300);
        std::env::remove_var("__PCFG_DUR2__");
    }

    #[test]
    fn test_parse_duration_env_hours() {
        std::env::set_var("__PCFG_DUR3__", "1h");
        assert_eq!(parse_duration_env("__PCFG_DUR3__", 0), 3600);
        std::env::remove_var("__PCFG_DUR3__");
    }

    #[test]
    fn test_parse_duration_env_plain() {
        std::env::set_var("__PCFG_DUR4__", "120");
        assert_eq!(parse_duration_env("__PCFG_DUR4__", 0), 120);
        std::env::remove_var("__PCFG_DUR4__");
    }

    #[test]
    fn test_parse_list_env() {
        std::env::set_var("__PCFG_LIST__", "a, b, c");
        let list = parse_list_env("__PCFG_LIST__");
        assert_eq!(list, vec!["a", "b", "c"]);
        std::env::remove_var("__PCFG_LIST__");
    }

    #[test]
    fn test_parse_list_env_empty() {
        let list = parse_list_env("__PCFG_LIST_MISS__");
        assert!(list.is_empty());
    }

    #[test]
    fn test_require_env_missing() {
        let r = require_env("__PCFG_REQ_MISS__");
        assert_eq!(
            r,
            Err(ConfigError::MissingRequired("__PCFG_REQ_MISS__".into()))
        );
    }

    #[test]
    fn test_require_env_present() {
        std::env::set_var("__PCFG_REQ__", "val");
        assert_eq!(require_env("__PCFG_REQ__").unwrap(), "val");
        std::env::remove_var("__PCFG_REQ__");
    }

    // -- ConfigStore basics -------------------------------------------------

    #[test]
    fn test_store_with_defaults_has_parameters() {
        let store = ConfigStore::with_defaults();
        assert!(store.get("economy.speed").is_some());
        assert!(store.get("combat.debris_factor").is_some());
        assert!(store.get("galaxy.max_systems").is_some());
        assert!(store.get("marketplace.tax_rate").is_some());
        assert!(store.get("security.token_expiry_seconds").is_some());
    }

    #[test]
    fn test_store_get_value() {
        let store = ConfigStore::with_defaults();
        assert_eq!(store.get_value("economy.speed"), Some("1"));
    }

    #[test]
    fn test_store_get_int() {
        let store = ConfigStore::with_defaults();
        assert_eq!(store.get_int("galaxy.max_galaxies"), Some(9));
    }

    #[test]
    fn test_store_get_float() {
        let store = ConfigStore::with_defaults();
        let v = store.get_float("combat.debris_factor").unwrap();
        assert!((v - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn test_store_get_bool_none_for_non_bool() {
        let store = ConfigStore::with_defaults();
        // "1" parses as bool true
        assert_eq!(store.get_bool("economy.speed"), Some(true));
    }

    #[test]
    fn test_store_get_missing() {
        let store = ConfigStore::with_defaults();
        assert!(store.get("nonexistent.key").is_none());
        assert!(store.get_value("nonexistent.key").is_none());
        assert!(store.get_int("nonexistent.key").is_none());
    }

    // -- set & constraint validation ----------------------------------------

    #[test]
    fn test_store_set_valid() {
        let mut store = ConfigStore::with_defaults();
        let result = store.set("economy.speed", "5", "testing");
        assert!(result.is_ok());
        assert_eq!(store.get_value("economy.speed"), Some("5"));
    }

    #[test]
    fn test_store_set_constraint_violation_min() {
        let mut store = ConfigStore::with_defaults();
        let result = store.set("economy.speed", "0", "below min");
        assert!(matches!(result, Err(ConfigError::ConstraintViolation(_))));
    }

    #[test]
    fn test_store_set_constraint_violation_max() {
        let mut store = ConfigStore::with_defaults();
        let result = store.set("economy.speed", "999", "above max");
        assert!(matches!(result, Err(ConfigError::ConstraintViolation(_))));
    }

    #[test]
    fn test_store_set_invalid_type() {
        let mut store = ConfigStore::with_defaults();
        let result = store.set("economy.speed", "abc", "not a number");
        assert!(matches!(result, Err(ConfigError::InvalidValue(_))));
    }

    #[test]
    fn test_store_set_not_found() {
        let mut store = ConfigStore::with_defaults();
        let result = store.set("nope.nope", "1", "x");
        assert_eq!(result, Err(ConfigError::NotFound));
    }

    #[test]
    fn test_store_validate_value() {
        let store = ConfigStore::with_defaults();
        assert!(store.validate_value("combat.debris_factor", "0.5").is_ok());
        assert!(store.validate_value("combat.debris_factor", "2.0").is_err());
        assert!(store.validate_value("combat.debris_factor", "abc").is_err());
    }

    // -- reset_to_default ---------------------------------------------------

    #[test]
    fn test_store_reset_to_default() {
        let mut store = ConfigStore::with_defaults();
        store.set("economy.speed", "7", "bump").unwrap();
        assert_eq!(store.get_value("economy.speed"), Some("7"));

        let param = store.reset_to_default("economy.speed").unwrap();
        assert_eq!(param.value, "1");
        assert_eq!(store.get_value("economy.speed"), Some("1"));
    }

    // -- list & categories --------------------------------------------------

    #[test]
    fn test_store_list_all() {
        let store = ConfigStore::with_defaults();
        let all = store.list(None);
        assert!(all.len() >= 15); // we defined 15 defaults
                                  // sorted by key
        for i in 1..all.len() {
            assert!(all[i - 1].key <= all[i].key);
        }
    }

    #[test]
    fn test_store_list_by_category() {
        let store = ConfigStore::with_defaults();
        let combat = store.list(Some("combat"));
        assert_eq!(combat.len(), 3);
        for p in &combat {
            assert_eq!(p.category, "combat");
        }
    }

    #[test]
    fn test_store_categories() {
        let store = ConfigStore::with_defaults();
        let cats = store.categories();
        assert!(cats.contains(&"economy".to_string()));
        assert!(cats.contains(&"combat".to_string()));
        assert!(cats.contains(&"fleet".to_string()));
        assert!(cats.contains(&"galaxy".to_string()));
        assert!(cats.contains(&"marketplace".to_string()));
        assert!(cats.contains(&"security".to_string()));
        // sorted
        for i in 1..cats.len() {
            assert!(cats[i - 1] <= cats[i]);
        }
    }

    // -- history ------------------------------------------------------------

    #[test]
    fn test_history_records_changes() {
        let mut store = ConfigStore::with_defaults();
        store.set("economy.speed", "3", "first").unwrap();
        store.set("economy.speed", "5", "second").unwrap();
        store.set("combat.max_rounds", "10", "more rounds").unwrap();

        let all = store.history.list_changes(10);
        assert_eq!(all.len(), 3);
        // newest first
        assert_eq!(all[0].parameter_key, "combat.max_rounds");
        assert_eq!(all[1].new_value, "5");
    }

    #[test]
    fn test_history_changes_for_parameter() {
        let mut store = ConfigStore::with_defaults();
        store.set("economy.speed", "3", "a").unwrap();
        store.set("combat.max_rounds", "10", "b").unwrap();
        store.set("economy.speed", "5", "c").unwrap();

        let speed_changes = store.history.changes_for_parameter("economy.speed", 10);
        assert_eq!(speed_changes.len(), 2);
        assert_eq!(speed_changes[0].new_value, "5");
        assert_eq!(speed_changes[1].new_value, "3");
    }

    #[test]
    fn test_history_limit() {
        let mut store = ConfigStore::with_defaults();
        for i in 1..=5 {
            store.set("economy.speed", &i.to_string(), "loop").unwrap();
        }
        let limited = store.history.list_changes(2);
        assert_eq!(limited.len(), 2);
    }

    #[test]
    fn test_history_change_ids_increment() {
        let mut store = ConfigStore::with_defaults();
        store.set("economy.speed", "2", "a").unwrap();
        store.set("economy.speed", "3", "b").unwrap();
        let changes = store.history.list_changes(10);
        assert_eq!(changes[0].change_id, 2);
        assert_eq!(changes[1].change_id, 1);
    }

    // -- ConfigSnapshot import/export ---------------------------------------

    #[test]
    fn test_snapshot_export_json() {
        let store = ConfigStore::with_defaults();
        let json = ConfigSnapshot::export_json(&store);
        assert!(json.contains("economy.speed"));
        assert!(json.contains("combat.debris_factor"));
        // Should be valid JSON
        let parsed: Vec<ConfigParameter> = serde_json::from_str(&json).unwrap();
        assert!(parsed.len() >= 15);
    }

    #[test]
    fn test_snapshot_import_json() {
        let mut store = ConfigStore::with_defaults();
        // Export, modify a param, import and verify it's restored
        store.set("economy.speed", "7", "custom").unwrap();
        assert_eq!(store.get_value("economy.speed"), Some("7"));

        // Create a snapshot with speed=3
        let json = r#"[{"key":"economy.speed","category":"economy","value":"3","default_value":"1","data_type":"Integer","description":"Universe speed factor","constraints":{"min_value":1.0,"max_value":10.0,"allowed_values":null,"pattern":null},"modified_at":null}]"#;

        let count = ConfigSnapshot::import_json(&mut store, json).unwrap();
        assert_eq!(count, 1);
        assert_eq!(store.get_value("economy.speed"), Some("3"));
    }

    #[test]
    fn test_snapshot_import_json_invalid() {
        let mut store = ConfigStore::with_defaults();
        let result = ConfigSnapshot::import_json(&mut store, "not json");
        assert!(matches!(result, Err(ConfigError::ParseError(_))));
    }

    #[test]
    fn test_snapshot_import_ignores_unknown_keys() {
        let mut store = ConfigStore::with_defaults();
        let json = r#"[{"key":"unknown.key","category":"x","value":"1","default_value":"1","data_type":"String","description":"","constraints":null,"modified_at":null}]"#;
        let count = ConfigSnapshot::import_json(&mut store, json).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_snapshot_roundtrip() {
        let mut store = ConfigStore::with_defaults();
        store.set("economy.speed", "4", "test").unwrap();
        store.set("combat.max_rounds", "12", "test").unwrap();

        let json = ConfigSnapshot::export_json(&store);

        let mut store2 = ConfigStore::with_defaults();
        let count = ConfigSnapshot::import_json(&mut store2, &json).unwrap();
        assert!(count >= 2);
        assert_eq!(store2.get_value("economy.speed"), Some("4"));
        assert_eq!(store2.get_value("combat.max_rounds"), Some("12"));
    }

    // -- glob matching (internal) -------------------------------------------

    #[test]
    fn test_glob_match() {
        assert!(simple_glob_match("*", "anything"));
        assert!(simple_glob_match("foo*", "foobar"));
        assert!(!simple_glob_match("foo*", "barfoo"));
        assert!(simple_glob_match("f?o", "foo"));
        assert!(!simple_glob_match("f?o", "fooo"));
        assert!(simple_glob_match("*.txt", "readme.txt"));
    }

    // -- constraint with allowed_values -------------------------------------

    #[test]
    fn test_constraint_allowed_values() {
        let mut store = ConfigStore::new();
        store.insert_default(ConfigParameter {
            key: "mode".into(),
            category: "test".into(),
            value: "normal".into(),
            default_value: "normal".into(),
            data_type: DataType::String,
            description: "Game mode".into(),
            constraints: Some(ParameterConstraints {
                min_value: None,
                max_value: None,
                allowed_values: Some(vec!["normal".into(), "hard".into(), "extreme".into()]),
                pattern: None,
            }),
            modified_at: None,
        });

        assert!(store.set("mode", "hard", "ok").is_ok());
        assert!(store.set("mode", "invalid", "nope").is_err());
    }

    // -- constraint with pattern --------------------------------------------

    #[test]
    fn test_constraint_pattern() {
        let mut store = ConfigStore::new();
        store.insert_default(ConfigParameter {
            key: "label".into(),
            category: "test".into(),
            value: "v1".into(),
            default_value: "v1".into(),
            data_type: DataType::String,
            description: "Version label".into(),
            constraints: Some(ParameterConstraints {
                min_value: None,
                max_value: None,
                allowed_values: None,
                pattern: Some("v*".into()),
            }),
            modified_at: None,
        });

        assert!(store.set("label", "v2.0", "ok").is_ok());
        assert!(store.set("label", "release1", "bad").is_err());
    }

    // -- float constraint boundary ------------------------------------------

    #[test]
    fn test_float_constraint_boundaries() {
        let mut store = ConfigStore::with_defaults();
        // debris_factor: min 0.0, max 1.0
        assert!(store.set("combat.debris_factor", "0.0", "min").is_ok());
        assert!(store.set("combat.debris_factor", "1.0", "max").is_ok());
        assert!(store.set("combat.debris_factor", "-0.1", "below").is_err());
        assert!(store.set("combat.debris_factor", "1.1", "above").is_err());
    }

    // -- no constraints on unconstrained params -----------------------------

    #[test]
    fn test_unconstrained_param_accepts_any_valid_type() {
        let mut store = ConfigStore::with_defaults();
        // fleet.fuel_multiplier has no constraints
        assert!(store.set("fleet.fuel_multiplier", "999.9", "ok").is_ok());
    }

    // -- ConfigError display ------------------------------------------------

    #[test]
    fn test_config_error_display() {
        assert_eq!(format!("{}", ConfigError::NotFound), "parameter not found");
        assert!(format!("{}", ConfigError::InvalidValue("x".into())).contains("x"));
        assert!(format!("{}", ConfigError::ConstraintViolation("y".into())).contains("y"));
        assert!(format!("{}", ConfigError::MissingRequired("Z".into())).contains("Z"));
        assert!(format!("{}", ConfigError::ParseError("p".into())).contains("p"));
    }
}
