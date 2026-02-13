//! Shared environment configuration helpers.

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
