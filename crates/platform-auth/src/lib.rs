#![forbid(unsafe_code)]
//! Authentication primitives for the Universus platform.
//!
//! Provides JWT token management, password hashing, session management,
//! role-based access control, and auth middleware helpers for Axum services.

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use subtle::ConstantTimeEq;

// ---------------------------------------------------------------------------
// AuthError
// ---------------------------------------------------------------------------

/// Errors that can occur during authentication operations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AuthError {
    InvalidCredentials,
    TokenExpired,
    TokenInvalid,
    TokenMissing,
    WeakPassword(String),
    PasswordHashFailed,
    SessionExpired,
    SessionRevoked,
    TooManySessions,
    SigningUnavailable,
}

/// Invalid authentication configuration detected before a service starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthConfigError {
    LegacySymmetricSigningForbidden,
    MissingVerificationKeys,
    InvalidVerificationKey,
    MissingSigningKey,
    PrivateKeyOnVerifier,
    SigningKeyMismatch,
    MissingIssuer,
    MissingAudience,
}

impl fmt::Display for AuthConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LegacySymmetricSigningForbidden => write!(
                f,
                "legacy JWT_SECRET/HS256 signing is forbidden in production-like environments"
            ),
            Self::MissingVerificationKeys => write!(
                f,
                "AUTH_JWT_VERIFICATION_KEYS must contain at least one kid:base64url Ed25519 public key"
            ),
            Self::InvalidVerificationKey => {
                write!(f, "AUTH_JWT_VERIFICATION_KEYS contains invalid Ed25519 key material")
            }
            Self::MissingSigningKey => write!(
                f,
                "the configured token issuer requires AUTH_JWT_SIGNING_KEY_ID and AUTH_JWT_PRIVATE_KEY_BASE64"
            ),
            Self::PrivateKeyOnVerifier => write!(
                f,
                "verification-only services must not receive AUTH_JWT_PRIVATE_KEY_BASE64"
            ),
            Self::SigningKeyMismatch => write!(
                f,
                "the signing private key does not match its public verification key"
            ),
            Self::MissingIssuer => write!(f, "AUTH_JWT_ISSUER must be configured"),
            Self::MissingAudience => write!(
                f,
                "AUTH_EXPECTED_AUDIENCE and issuer AUTH_TOKEN_AUDIENCES must be configured"
            ),
        }
    }
}

impl std::error::Error for AuthConfigError {}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCredentials => write!(f, "invalid credentials"),
            Self::TokenExpired => write!(f, "token expired"),
            Self::TokenInvalid => write!(f, "token invalid"),
            Self::TokenMissing => write!(f, "token missing"),
            Self::WeakPassword(reason) => write!(f, "weak password: {reason}"),
            Self::PasswordHashFailed => write!(f, "password hashing failed"),
            Self::SessionExpired => write!(f, "session expired"),
            Self::SessionRevoked => write!(f, "session revoked"),
            Self::TooManySessions => write!(f, "too many sessions"),
            Self::SigningUnavailable => write!(f, "token signing is unavailable"),
        }
    }
}

// ---------------------------------------------------------------------------
// AuthConfig
// ---------------------------------------------------------------------------

/// Configuration for authentication behaviour.
#[derive(Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Development/test-only symmetric secret. Production rejects any value.
    #[serde(skip_serializing)]
    pub jwt_secret: String,
    pub legacy_hmac_enabled: bool,
    pub issuer: String,
    pub expected_audience: String,
    pub token_audiences: Vec<String>,
    pub signing_key_id: Option<String>,
    #[serde(skip_serializing)]
    pub signing_private_key_base64: Option<String>,
    pub verification_keys: HashMap<String, String>,
    pub token_issuer: bool,
    pub jwt_expiry_seconds: i64,
    pub refresh_expiry_seconds: i64,
    pub bcrypt_cost: u32,
    pub max_sessions_per_user: usize,
}

/// Authentication configuration may be included in startup diagnostics, but
/// signing material must never be emitted to logs or serialized snapshots.
impl fmt::Debug for AuthConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthConfig")
            .field("jwt_secret", &"[REDACTED]")
            .field("legacy_hmac_enabled", &self.legacy_hmac_enabled)
            .field("issuer", &self.issuer)
            .field("expected_audience", &self.expected_audience)
            .field("token_audiences", &self.token_audiences)
            .field("signing_key_id", &self.signing_key_id)
            .field("signing_private_key_base64", &"[REDACTED]")
            .field(
                "verification_key_ids",
                &self.verification_keys.keys().collect::<Vec<_>>(),
            )
            .field("token_issuer", &self.token_issuer)
            .field("jwt_expiry_seconds", &self.jwt_expiry_seconds)
            .field("refresh_expiry_seconds", &self.refresh_expiry_seconds)
            .field("bcrypt_cost", &self.bcrypt_cost)
            .field("max_sessions_per_user", &self.max_sessions_per_user)
            .finish()
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: "default-secret".to_string(),
            legacy_hmac_enabled: true,
            issuer: "universus-dev".to_string(),
            expected_audience: "universus".to_string(),
            token_audiences: vec!["universus".to_string()],
            signing_key_id: None,
            signing_private_key_base64: None,
            verification_keys: HashMap::new(),
            token_issuer: false,
            jwt_expiry_seconds: 86_400,
            refresh_expiry_seconds: 604_800,
            bcrypt_cost: 12,
            max_sessions_per_user: 5,
        }
    }
}

impl AuthConfig {
    /// Build an [`AuthConfig`] by reading environment variables, falling back
    /// to sensible defaults.
    pub fn from_env() -> Self {
        let environment = runtime_environment();
        let production_like = production_like_environment(&environment);
        let configured_secret = std::env::var("JWT_SECRET").ok();
        let jwt_secret = configured_secret.clone().unwrap_or_else(|| {
            if production_like {
                String::new()
            } else {
                "default-secret".to_string()
            }
        });
        let legacy_hmac_enabled = parse_env_bool("AUTH_ALLOW_LEGACY_HS256")
            .unwrap_or(!production_like || configured_secret.is_some());
        let issuer =
            std::env::var("AUTH_JWT_ISSUER").unwrap_or_else(|_| "universus-dev".to_string());
        let expected_audience =
            std::env::var("AUTH_EXPECTED_AUDIENCE").unwrap_or_else(|_| "universus".to_string());
        let token_audiences = std::env::var("AUTH_TOKEN_AUDIENCES")
            .ok()
            .map(|value| split_csv(&value))
            .filter(|values| !values.is_empty())
            .unwrap_or_else(|| vec![expected_audience.clone()]);
        let signing_key_id = std::env::var("AUTH_JWT_SIGNING_KEY_ID")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let signing_private_key_base64 = std::env::var("AUTH_JWT_PRIVATE_KEY_BASE64")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let verification_keys = std::env::var("AUTH_JWT_VERIFICATION_KEYS")
            .ok()
            .map(|value| parse_verification_keys(&value))
            .unwrap_or_default();
        let token_issuer = parse_env_bool("AUTH_TOKEN_ISSUER").unwrap_or(false);
        let jwt_expiry_seconds = std::env::var("JWT_EXPIRY_SECONDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(86_400);
        let refresh_expiry_seconds = std::env::var("REFRESH_EXPIRY_SECONDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(604_800);
        let bcrypt_cost = std::env::var("BCRYPT_COST")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(12);
        let max_sessions_per_user = std::env::var("MAX_SESSIONS_PER_USER")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5);
        Self {
            jwt_secret,
            legacy_hmac_enabled,
            issuer,
            expected_audience,
            token_audiences,
            signing_key_id,
            signing_private_key_base64,
            verification_keys,
            token_issuer,
            jwt_expiry_seconds,
            refresh_expiry_seconds,
            bcrypt_cost,
            max_sessions_per_user,
        }
    }

    /// Reject development/default signing secrets in production-like environments.
    ///
    /// Local development intentionally retains the existing default so tests and
    /// one-command development remain compatible. Deployments identify themselves
    /// with `UNIVERSUS_ENV`, `APP_ENV`, `ENVIRONMENT`, or `RUST_ENV`.
    pub fn validate_runtime(&self) -> Result<(), AuthConfigError> {
        self.validate_for_environment(&runtime_environment())
    }

    /// Validate a configuration for an explicit deployment environment.
    pub fn validate_for_environment(&self, environment: &str) -> Result<(), AuthConfigError> {
        let production_like = production_like_environment(environment);
        if !production_like {
            return Ok(());
        }

        if self.legacy_hmac_enabled || !self.jwt_secret.trim().is_empty() {
            return Err(AuthConfigError::LegacySymmetricSigningForbidden);
        }
        if self.issuer.trim().is_empty() {
            return Err(AuthConfigError::MissingIssuer);
        }
        if self.expected_audience.trim().is_empty()
            || (self.token_issuer
                && (self.token_audiences.is_empty()
                    || self
                        .token_audiences
                        .iter()
                        .any(|audience| audience.trim().is_empty())))
        {
            return Err(AuthConfigError::MissingAudience);
        }
        if self.verification_keys.is_empty() {
            return Err(AuthConfigError::MissingVerificationKeys);
        }
        for (key_id, encoded_key) in &self.verification_keys {
            if key_id.trim().is_empty() || decode_verifying_key(encoded_key).is_err() {
                return Err(AuthConfigError::InvalidVerificationKey);
            }
        }

        if self.token_issuer {
            let key_id = self
                .signing_key_id
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .ok_or(AuthConfigError::MissingSigningKey)?;
            let private = self
                .signing_private_key_base64
                .as_deref()
                .ok_or(AuthConfigError::MissingSigningKey)?;
            let signing_key =
                decode_signing_key(private).map_err(|_| AuthConfigError::MissingSigningKey)?;
            let expected_public = self
                .verification_keys
                .get(key_id)
                .ok_or(AuthConfigError::MissingSigningKey)
                .and_then(|value| {
                    decode_verifying_key(value).map_err(|_| AuthConfigError::InvalidVerificationKey)
                })?;
            if signing_key.verifying_key() != expected_public {
                return Err(AuthConfigError::SigningKeyMismatch);
            }
        } else if self.signing_private_key_base64.is_some() || self.signing_key_id.is_some() {
            return Err(AuthConfigError::PrivateKeyOnVerifier);
        }

        Ok(())
    }
}

fn runtime_environment() -> String {
    ["UNIVERSUS_ENV", "APP_ENV", "ENVIRONMENT", "RUST_ENV"]
        .into_iter()
        .find_map(|name| std::env::var(name).ok())
        .unwrap_or_else(|| "development".to_string())
}

fn production_like_environment(environment: &str) -> bool {
    matches!(
        environment.trim().to_ascii_lowercase().as_str(),
        "production" | "prod" | "staging" | "stage"
    )
}

fn parse_env_bool(name: &str) -> Option<bool> {
    std::env::var(name)
        .ok()
        .and_then(|value| match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        })
}

fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn parse_verification_keys(value: &str) -> HashMap<String, String> {
    value
        .split(',')
        .map(|entry| {
            let Some((key_id, key)) = entry.trim().split_once(':') else {
                // Preserve malformed entries so production validation fails
                // closed instead of silently dropping a rotation key typo.
                return (String::new(), entry.trim().to_string());
            };
            (key_id.trim().to_string(), key.trim().to_string())
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Minimal SHA-256 implementation (no unsafe, no external crate)
// ---------------------------------------------------------------------------

/// Pure-Rust SHA-256 operating on byte slices.
struct Sha256 {
    state: [u32; 8],
    buffer: Vec<u8>,
    total_len: u64,
}

const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

const SHA256_INIT: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

impl Sha256 {
    fn new() -> Self {
        Self {
            state: SHA256_INIT,
            buffer: Vec::new(),
            total_len: 0,
        }
    }

    fn update(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
        self.total_len += data.len() as u64;

        while self.buffer.len() >= 64 {
            let block: Vec<u8> = self.buffer.drain(..64).collect();
            self.process_block(&block);
        }
    }

    fn process_block(&mut self, block: &[u8]) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(SHA256_K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }

    fn finalize(mut self) -> [u8; 32] {
        let bit_len = self.total_len * 8;
        self.buffer.push(0x80);
        while (self.buffer.len() % 64) != 56 {
            self.buffer.push(0);
        }
        self.buffer.extend_from_slice(&bit_len.to_be_bytes());

        // Process remaining blocks (1 or 2).
        let remaining = self.buffer.clone();
        for chunk in remaining.chunks(64) {
            self.process_block(chunk);
        }

        let mut out = [0u8; 32];
        for (i, word) in self.state.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
        }
        out
    }
}

/// Convenience: hash arbitrary bytes and return the 32-byte digest.
fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(data);
    h.finalize()
}

/// HMAC-SHA256 as defined in RFC 2104.
fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let block_size = 64;

    // If key is longer than block size, hash it first.
    let key_prime: Vec<u8> = if key.len() > block_size {
        sha256(key).to_vec()
    } else {
        key.to_vec()
    };

    let mut ipad = vec![0x36u8; block_size];
    let mut opad = vec![0x5cu8; block_size];
    for (i, &b) in key_prime.iter().enumerate() {
        ipad[i] ^= b;
        opad[i] ^= b;
    }

    // inner hash
    ipad.extend_from_slice(message);
    let inner = sha256(&ipad);

    // outer hash
    opad.extend_from_slice(&inner);
    sha256(&opad)
}

// ---------------------------------------------------------------------------
// Base64-URL encoding / decoding (no padding, URL-safe)
// ---------------------------------------------------------------------------

const BASE64URL_CHARS: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn base64url_encode(data: &[u8]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i + 2 < data.len() {
        let n = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8) | (data[i + 2] as u32);
        out.push(BASE64URL_CHARS[((n >> 18) & 0x3f) as usize] as char);
        out.push(BASE64URL_CHARS[((n >> 12) & 0x3f) as usize] as char);
        out.push(BASE64URL_CHARS[((n >> 6) & 0x3f) as usize] as char);
        out.push(BASE64URL_CHARS[(n & 0x3f) as usize] as char);
        i += 3;
    }
    let remaining = data.len() - i;
    if remaining == 2 {
        let n = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8);
        out.push(BASE64URL_CHARS[((n >> 18) & 0x3f) as usize] as char);
        out.push(BASE64URL_CHARS[((n >> 12) & 0x3f) as usize] as char);
        out.push(BASE64URL_CHARS[((n >> 6) & 0x3f) as usize] as char);
    } else if remaining == 1 {
        let n = (data[i] as u32) << 16;
        out.push(BASE64URL_CHARS[((n >> 18) & 0x3f) as usize] as char);
        out.push(BASE64URL_CHARS[((n >> 12) & 0x3f) as usize] as char);
    }
    out
}

fn base64url_decode(input: &str) -> Result<Vec<u8>, AuthError> {
    fn val(c: u8) -> Result<u32, AuthError> {
        match c {
            b'A'..=b'Z' => Ok((c - b'A') as u32),
            b'a'..=b'z' => Ok((c - b'a' + 26) as u32),
            b'0'..=b'9' => Ok((c - b'0' + 52) as u32),
            b'-' => Ok(62),
            b'_' => Ok(63),
            _ => Err(AuthError::TokenInvalid),
        }
    }

    let bytes = input.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 3 < bytes.len() {
        let n = (val(bytes[i])? << 18)
            | (val(bytes[i + 1])? << 12)
            | (val(bytes[i + 2])? << 6)
            | val(bytes[i + 3])?;
        out.push((n >> 16) as u8);
        out.push((n >> 8) as u8);
        out.push(n as u8);
        i += 4;
    }
    let remaining = bytes.len() - i;
    if remaining == 3 {
        let n = (val(bytes[i])? << 18) | (val(bytes[i + 1])? << 12) | (val(bytes[i + 2])? << 6);
        out.push((n >> 16) as u8);
        out.push((n >> 8) as u8);
    } else if remaining == 2 {
        let n = (val(bytes[i])? << 18) | (val(bytes[i + 1])? << 12);
        out.push((n >> 16) as u8);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Simple counter-based unique ID (no unsafe, no deps)
// ---------------------------------------------------------------------------

fn generate_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let raw = format!("{ts:016x}{seq:08x}");
    // Hash for uniform distribution.
    let hash = sha256(raw.as_bytes());
    hex_encode(&hash[..16])
}

fn hex_encode(data: &[u8]) -> String {
    let mut s = String::with_capacity(data.len() * 2);
    for &b in data {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn now_timestamp() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

// ---------------------------------------------------------------------------
// JWT Claims
// ---------------------------------------------------------------------------

pub const TOKEN_PURPOSE_ACCESS: &str = "access";
pub const TOKEN_PURPOSE_REFRESH: &str = "refresh";
pub const TOKEN_PURPOSE_SERVICE: &str = "service";

/// JWT claims embedded in access, refresh, and scoped service tokens.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Claims {
    pub iss: String,
    pub aud: Vec<String>,
    pub sub: String,
    pub username: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    pub role: String,
    pub universe_id: Option<i64>,
    pub exp: i64,
    pub iat: i64,
    pub jti: String,
    pub purpose: String,
    pub scopes: Vec<String>,
}

impl Claims {
    pub fn is_access_token(&self) -> bool {
        self.purpose == TOKEN_PURPOSE_ACCESS
    }

    pub fn is_service_token(&self) -> bool {
        self.purpose == TOKEN_PURPOSE_SERVICE && self.role == "service"
    }

    pub fn has_scope(&self, required: &str) -> bool {
        self.scopes.iter().any(|scope| scope == required)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct JwtHeader {
    alg: String,
    typ: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    kid: Option<String>,
}

fn encoded_jwt_header(alg: &str, kid: Option<&str>) -> Result<String, AuthError> {
    let header = JwtHeader {
        alg: alg.to_string(),
        typ: "JWT".to_string(),
        kid: kid.map(str::to_string),
    };
    serde_json::to_vec(&header)
        .map(|bytes| base64url_encode(&bytes))
        .map_err(|_| AuthError::TokenInvalid)
}

fn decode_signing_key(encoded: &str) -> Result<SigningKey, AuthError> {
    let bytes = base64url_decode(encoded.trim()).map_err(|_| AuthError::TokenInvalid)?;
    let seed: [u8; 32] = bytes.try_into().map_err(|_| AuthError::TokenInvalid)?;
    Ok(SigningKey::from_bytes(&seed))
}

fn decode_verifying_key(encoded: &str) -> Result<VerifyingKey, AuthError> {
    let bytes = base64url_decode(encoded.trim()).map_err(|_| AuthError::TokenInvalid)?;
    let public: [u8; 32] = bytes.try_into().map_err(|_| AuthError::TokenInvalid)?;
    VerifyingKey::from_bytes(&public).map_err(|_| AuthError::TokenInvalid)
}

/// Create a signed JWT from the given [`Claims`]. Production issuers use
/// Ed25519; HS256 is retained only for explicit development/test compatibility.
fn encode_jwt(config: &AuthConfig, claims: &Claims) -> Result<String, AuthError> {
    let payload_json = serde_json::to_string(claims).map_err(|_| AuthError::TokenInvalid)?;
    let payload = base64url_encode(payload_json.as_bytes());

    if config.token_issuer {
        let key_id = config
            .signing_key_id
            .as_deref()
            .ok_or(AuthError::SigningUnavailable)?;
        let private_key = config
            .signing_private_key_base64
            .as_deref()
            .ok_or(AuthError::SigningUnavailable)?;
        let signing_key =
            decode_signing_key(private_key).map_err(|_| AuthError::SigningUnavailable)?;
        let header = encoded_jwt_header("EdDSA", Some(key_id))?;
        let signing_input = format!("{header}.{payload}");
        let signature = signing_key.sign(signing_input.as_bytes());
        return Ok(format!(
            "{signing_input}.{}",
            base64url_encode(&signature.to_bytes())
        ));
    }

    if config.legacy_hmac_enabled && !config.jwt_secret.is_empty() {
        let header = encoded_jwt_header("HS256", None)?;
        let signing_input = format!("{header}.{payload}");
        let signature = hmac_sha256(config.jwt_secret.as_bytes(), signing_input.as_bytes());
        return Ok(format!("{signing_input}.{}", base64url_encode(&signature)));
    }

    Err(AuthError::SigningUnavailable)
}

/// Decode and verify a JWT, returning the embedded [`Claims`].
fn decode_jwt(config: &AuthConfig, token: &str) -> Result<Claims, AuthError> {
    let mut parts = token.split('.');
    let Some(encoded_header) = parts.next() else {
        return Err(AuthError::TokenInvalid);
    };
    let Some(encoded_payload) = parts.next() else {
        return Err(AuthError::TokenInvalid);
    };
    let Some(encoded_signature) = parts.next() else {
        return Err(AuthError::TokenInvalid);
    };
    if parts.next().is_some() {
        return Err(AuthError::TokenInvalid);
    }

    let header_bytes = base64url_decode(encoded_header)?;
    let header: JwtHeader =
        serde_json::from_slice(&header_bytes).map_err(|_| AuthError::TokenInvalid)?;
    if header.typ != "JWT" {
        return Err(AuthError::TokenInvalid);
    }
    let signing_input = format!("{encoded_header}.{encoded_payload}");
    let provided_signature = base64url_decode(encoded_signature)?;

    match header.alg.as_str() {
        "EdDSA" => {
            let key_id = header.kid.as_deref().ok_or(AuthError::TokenInvalid)?;
            let encoded_key = config
                .verification_keys
                .get(key_id)
                .ok_or(AuthError::TokenInvalid)?;
            let verifying_key = decode_verifying_key(encoded_key)?;
            let signature =
                Signature::from_slice(&provided_signature).map_err(|_| AuthError::TokenInvalid)?;
            verifying_key
                .verify(signing_input.as_bytes(), &signature)
                .map_err(|_| AuthError::TokenInvalid)?;
        }
        "HS256" => {
            if !config.legacy_hmac_enabled || config.jwt_secret.is_empty() || header.kid.is_some() {
                return Err(AuthError::TokenInvalid);
            }
            let expected = hmac_sha256(config.jwt_secret.as_bytes(), signing_input.as_bytes());
            if expected.len() != provided_signature.len()
                || expected.ct_eq(&provided_signature).unwrap_u8() != 1
            {
                return Err(AuthError::TokenInvalid);
            }
        }
        _ => return Err(AuthError::TokenInvalid),
    }

    let payload_bytes = base64url_decode(encoded_payload)?;
    let claims: Claims =
        serde_json::from_slice(&payload_bytes).map_err(|_| AuthError::TokenInvalid)?;
    Ok(claims)
}

fn validate_registered_claims(
    config: &AuthConfig,
    claims: &Claims,
    accepted_purposes: &[&str],
) -> Result<(), AuthError> {
    let now = now_timestamp();
    if claims.exp <= now {
        return Err(AuthError::TokenExpired);
    }
    if claims.iat <= 0
        || claims.iat > now.saturating_add(60)
        || claims.exp <= claims.iat
        || claims.jti.trim().is_empty()
        || claims.iss != config.issuer
        || !claims
            .aud
            .iter()
            .any(|audience| audience == &config.expected_audience)
        || !accepted_purposes.contains(&claims.purpose.as_str())
    {
        return Err(AuthError::TokenInvalid);
    }

    match claims.purpose.as_str() {
        TOKEN_PURPOSE_ACCESS if matches!(claims.role.as_str(), "service" | "refresh") => {
            Err(AuthError::TokenInvalid)
        }
        TOKEN_PURPOSE_SERVICE
            if claims.role != "service"
                || claims.scopes.is_empty()
                || claims.scopes.iter().any(|scope| scope.trim().is_empty()) =>
        {
            Err(AuthError::TokenInvalid)
        }
        TOKEN_PURPOSE_REFRESH if claims.role != "refresh" || !claims.scopes.is_empty() => {
            Err(AuthError::TokenInvalid)
        }
        _ => Ok(()),
    }
}

/// Generate a signed access token (JWT) for the given user.
pub fn generate_token(
    config: &AuthConfig,
    user_id: &str,
    username: &str,
    role: &str,
    universe_id: Option<i64>,
) -> Result<String, AuthError> {
    generate_token_with_email(config, user_id, username, None, role, universe_id)
}

/// Produce a deterministic positive numeric bridge for legacy stores that
/// cannot yet represent the platform's string user subjects.
///
/// Authorization must still be based on the signed string subject; this value
/// is only an internal compatibility identifier.
pub fn stable_numeric_subject_id(subject: &str) -> u64 {
    let digest = sha256(subject.as_bytes());
    let value = u64::from_be_bytes([
        digest[0], digest[1], digest[2], digest[3], digest[4], digest[5], digest[6], digest[7],
    ]);
    (value & i64::MAX as u64).max(1)
}

/// Generate a signed access token that also carries the authenticated email.
///
/// The email claim is optional so tokens issued by older callers remain
/// compatible with [`validate_token`].
pub fn generate_token_with_email(
    config: &AuthConfig,
    user_id: &str,
    username: &str,
    email: Option<&str>,
    role: &str,
    universe_id: Option<i64>,
) -> Result<String, AuthError> {
    if matches!(role, "service" | "refresh") {
        return Err(AuthError::InvalidCredentials);
    }
    let now = now_timestamp();
    let claims = Claims {
        iss: config.issuer.clone(),
        aud: config.token_audiences.clone(),
        sub: user_id.to_string(),
        username: username.to_string(),
        email: email.map(str::to_owned),
        role: role.to_string(),
        universe_id,
        iat: now,
        exp: now + config.jwt_expiry_seconds,
        jti: generate_id(),
        purpose: TOKEN_PURPOSE_ACCESS.to_string(),
        scopes: Vec::new(),
    };
    encode_jwt(config, &claims)
}

/// Issue a least-privilege service credential. Production callers must be the
/// dedicated issuer and should provision the resulting token out-of-band.
pub fn generate_service_token(
    config: &AuthConfig,
    service_id: &str,
    audience: &str,
    scopes: &[&str],
    expiry_seconds: i64,
) -> Result<String, AuthError> {
    if service_id.trim().is_empty()
        || audience.trim().is_empty()
        || scopes.is_empty()
        || scopes.iter().any(|scope| scope.trim().is_empty())
        || expiry_seconds <= 0
    {
        return Err(AuthError::InvalidCredentials);
    }
    let now = now_timestamp();
    encode_jwt(
        config,
        &Claims {
            iss: config.issuer.clone(),
            aud: vec![audience.to_string()],
            sub: format!("service:{service_id}"),
            username: service_id.to_string(),
            email: None,
            role: "service".to_string(),
            universe_id: None,
            exp: now.saturating_add(expiry_seconds),
            iat: now,
            jti: generate_id(),
            purpose: TOKEN_PURPOSE_SERVICE.to_string(),
            scopes: scopes.iter().map(|scope| (*scope).to_string()).collect(),
        },
    )
}

/// Validate and decode an access token, returning the [`Claims`].
///
/// Returns [`AuthError::TokenExpired`] when the `exp` claim is in the past,
/// or [`AuthError::TokenInvalid`] if the signature / structure is wrong.
pub fn validate_token(config: &AuthConfig, token: &str) -> Result<Claims, AuthError> {
    let claims = decode_jwt(config, token)?;
    validate_registered_claims(
        config,
        &claims,
        &[TOKEN_PURPOSE_ACCESS, TOKEN_PURPOSE_SERVICE],
    )?;
    Ok(claims)
}

/// Generate a longer-lived refresh token for a user.
///
/// The refresh token is a JWT with an extended expiry and a `refresh` role
/// marker so it cannot be confused with a regular access token.
pub fn generate_refresh_token(config: &AuthConfig, user_id: &str) -> Result<String, AuthError> {
    let now = now_timestamp();
    let claims = Claims {
        iss: config.issuer.clone(),
        aud: config.token_audiences.clone(),
        sub: user_id.to_string(),
        username: String::new(),
        email: None,
        role: "refresh".to_string(),
        universe_id: None,
        iat: now,
        exp: now + config.refresh_expiry_seconds,
        jti: generate_id(),
        purpose: TOKEN_PURPOSE_REFRESH.to_string(),
        scopes: Vec::new(),
    };
    encode_jwt(config, &claims)
}

/// Validate a refresh token and produce a fresh access token.
pub fn refresh_access_token(
    config: &AuthConfig,
    refresh_token: &str,
    username: &str,
    role: &str,
) -> Result<String, AuthError> {
    let claims = decode_jwt(config, refresh_token)?;
    validate_registered_claims(config, &claims, &[TOKEN_PURPOSE_REFRESH])?;
    generate_token_with_email(
        config,
        &claims.sub,
        username,
        claims.email.as_deref(),
        role,
        None,
    )
}

// ---------------------------------------------------------------------------
// Password hashing (Argon2id PHC format)
// ---------------------------------------------------------------------------

/// Hash a password with Argon2id and a cryptographically random salt.
///
/// The Argon2 crate's reviewed defaults define the memory and time parameters.
/// Callers must run this CPU- and memory-intensive operation outside async
/// executor worker threads.
pub fn hash_password(password: &str) -> Result<String, AuthError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| AuthError::PasswordHashFailed)
}

/// Verify a password against its hash produced by [`hash_password`].
pub fn verify_password(password: &str, stored: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(stored) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

/// Validate that a password meets minimum complexity requirements.
///
/// Rules: >= 8 characters, at least one uppercase, one lowercase, one digit.
pub fn validate_password_strength(password: &str) -> Result<(), AuthError> {
    if password.len() < 8 {
        return Err(AuthError::WeakPassword(
            "must be at least 8 characters".to_string(),
        ));
    }
    if !password.chars().any(|c| c.is_uppercase()) {
        return Err(AuthError::WeakPassword(
            "must contain at least one uppercase letter".to_string(),
        ));
    }
    if !password.chars().any(|c| c.is_lowercase()) {
        return Err(AuthError::WeakPassword(
            "must contain at least one lowercase letter".to_string(),
        ));
    }
    if !password.chars().any(|c| c.is_ascii_digit()) {
        return Err(AuthError::WeakPassword(
            "must contain at least one digit".to_string(),
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Session management
// ---------------------------------------------------------------------------

/// An authentication session tied to a single JWT.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub user_id: String,
    pub token_jti: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: i64,
    pub expires_at: i64,
    pub is_revoked: bool,
}

/// In-memory session store. Thread-safety is left to the caller (e.g. wrap in
/// `Arc<Mutex<_>>`).
#[derive(Debug, Clone)]
pub struct SessionStore {
    pub sessions: HashMap<String, Session>,
    pub max_sessions_per_user: usize,
    creation_order: HashMap<String, u64>,
    next_creation_order: u64,
}

impl SessionStore {
    pub fn new(max_sessions_per_user: usize) -> Self {
        Self {
            sessions: HashMap::new(),
            max_sessions_per_user,
            creation_order: HashMap::new(),
            next_creation_order: 0,
        }
    }

    /// Create a new session, enforcing the per-user limit by revoking the
    /// oldest sessions when the limit is exceeded.
    pub fn create_session(
        &mut self,
        user_id: &str,
        token_jti: &str,
        ip: Option<&str>,
        ua: Option<&str>,
        expires_at: i64,
    ) -> Session {
        // Enforce per-user limit.
        let mut user_sessions: Vec<(String, u64)> = self
            .sessions
            .iter()
            .filter(|(_, s)| s.user_id == user_id && !s.is_revoked)
            .map(|(id, _)| {
                (
                    id.clone(),
                    self.creation_order.get(id).copied().unwrap_or(u64::MAX),
                )
            })
            .collect();

        if user_sessions.len() >= self.max_sessions_per_user {
            // Sort oldest-first, revoke enough to make room.
            user_sessions.sort_by_key(|(_, order)| *order);
            let to_revoke = user_sessions.len() - self.max_sessions_per_user + 1;
            for (sid, _) in user_sessions.iter().take(to_revoke) {
                if let Some(s) = self.sessions.get_mut(sid) {
                    s.is_revoked = true;
                }
            }
        }

        let session = Session {
            id: generate_id(),
            user_id: user_id.to_string(),
            token_jti: token_jti.to_string(),
            ip_address: ip.map(String::from),
            user_agent: ua.map(String::from),
            created_at: now_timestamp(),
            expires_at,
            is_revoked: false,
        };
        let id = session.id.clone();
        self.creation_order
            .insert(id.clone(), self.next_creation_order);
        self.next_creation_order = self.next_creation_order.saturating_add(1);
        self.sessions.insert(id, session.clone());
        session
    }

    pub fn get_session(&self, session_id: &str) -> Option<&Session> {
        self.sessions.get(session_id)
    }

    /// Returns `true` when the session exists, is not revoked, and has not
    /// expired.
    pub fn validate_session(&self, session_id: &str, now: i64) -> bool {
        match self.sessions.get(session_id) {
            Some(s) => !s.is_revoked && s.expires_at > now,
            None => false,
        }
    }

    /// Revoke a single session. Returns `true` if the session existed and was
    /// not already revoked.
    pub fn revoke_session(&mut self, session_id: &str) -> bool {
        if let Some(s) = self.sessions.get_mut(session_id) {
            if !s.is_revoked {
                s.is_revoked = true;
                return true;
            }
        }
        false
    }

    /// Revoke all sessions belonging to a user. Returns the number of sessions
    /// revoked.
    pub fn revoke_all_sessions(&mut self, user_id: &str) -> usize {
        let mut count = 0;
        for s in self.sessions.values_mut() {
            if s.user_id == user_id && !s.is_revoked {
                s.is_revoked = true;
                count += 1;
            }
        }
        count
    }

    pub fn list_user_sessions(&self, user_id: &str) -> Vec<&Session> {
        self.sessions
            .values()
            .filter(|s| s.user_id == user_id)
            .collect()
    }

    /// Remove expired sessions from the store. Returns how many were removed.
    pub fn cleanup_expired(&mut self, now: i64) -> usize {
        let before = self.sessions.len();
        self.sessions.retain(|_, s| s.expires_at > now);
        self.creation_order
            .retain(|session_id, _| self.sessions.contains_key(session_id));
        before - self.sessions.len()
    }
}

// ---------------------------------------------------------------------------
// Auth middleware helpers
// ---------------------------------------------------------------------------

/// Extract the token value from an `Authorization: Bearer <token>` header.
pub fn extract_bearer_token(authorization_header: &str) -> Option<&str> {
    let trimmed = authorization_header.trim();
    if trimmed.len() > 7 && trimmed[..7].eq_ignore_ascii_case("bearer ") {
        Some(trimmed[7..].trim())
    } else {
        None
    }
}

/// Lightweight representation of an authenticated user extracted from a JWT.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuthUser {
    pub user_id: String,
    pub username: String,
    pub email: Option<String>,
    pub role: String,
    pub universe_id: Option<i64>,
    pub token_purpose: String,
    pub scopes: Vec<String>,
}

/// Validate the `Authorization` header and return an [`AuthUser`] on success.
pub fn authenticate_request(
    config: &AuthConfig,
    authorization_header: &str,
) -> Result<AuthUser, AuthError> {
    let token = extract_bearer_token(authorization_header).ok_or(AuthError::TokenMissing)?;
    let claims = validate_token(config, token)?;
    Ok(AuthUser {
        user_id: claims.sub,
        username: claims.username,
        email: claims.email,
        role: claims.role,
        universe_id: claims.universe_id,
        token_purpose: claims.purpose,
        scopes: claims.scopes,
    })
}

// ---------------------------------------------------------------------------
// Role-based access control
// ---------------------------------------------------------------------------

/// Supported user roles with hierarchical permissions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum UserRole {
    Player,
    Moderator,
    Admin,
    SuperAdmin,
}

impl UserRole {
    /// Numeric privilege level (higher = more privileges).
    fn level(&self) -> u8 {
        match self {
            Self::Player => 0,
            Self::Moderator => 1,
            Self::Admin => 2,
            Self::SuperAdmin => 3,
        }
    }
}

impl fmt::Display for UserRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Player => "player",
            Self::Moderator => "moderator",
            Self::Admin => "admin",
            Self::SuperAdmin => "superadmin",
        };
        write!(f, "{s}")
    }
}

impl FromStr for UserRole {
    type Err = AuthError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "player" => Ok(Self::Player),
            "moderator" => Ok(Self::Moderator),
            "admin" => Ok(Self::Admin),
            "superadmin" => Ok(Self::SuperAdmin),
            _ => Err(AuthError::InvalidCredentials),
        }
    }
}

/// Check whether `role` has at least the privilege level of `required`.
pub fn has_permission(role: &UserRole, required: &UserRole) -> bool {
    role.level() >= required.level()
}

/// Returns `Ok(())` if the user holds a role at or above `required`, otherwise
/// returns [`AuthError::InvalidCredentials`].
pub fn require_role(user: &AuthUser, required: UserRole) -> Result<(), AuthError> {
    if user.token_purpose != TOKEN_PURPOSE_ACCESS {
        return Err(AuthError::InvalidCredentials);
    }
    let user_role = UserRole::from_str(&user.role)?;
    if has_permission(&user_role, &required) {
        Ok(())
    } else {
        Err(AuthError::InvalidCredentials)
    }
}

/// Require a non-human service identity with one exact endpoint scope.
pub fn require_service_scope(user: &AuthUser, required_scope: &str) -> Result<(), AuthError> {
    if user.token_purpose == TOKEN_PURPOSE_SERVICE
        && user.role == "service"
        && user.scopes.iter().any(|scope| scope == required_scope)
    {
        Ok(())
    } else {
        Err(AuthError::InvalidCredentials)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> AuthConfig {
        AuthConfig {
            jwt_secret: "test-secret-key-for-unit-tests".to_string(),
            jwt_expiry_seconds: 3600,
            refresh_expiry_seconds: 86_400,
            bcrypt_cost: 4, // low cost for fast tests
            max_sessions_per_user: 3,
            ..AuthConfig::default()
        }
    }

    fn ed25519_issuer_config(seed: [u8; 32], key_id: &str) -> AuthConfig {
        let signing_key = SigningKey::from_bytes(&seed);
        AuthConfig {
            jwt_secret: String::new(),
            legacy_hmac_enabled: false,
            issuer: "https://auth.universus.test".to_string(),
            expected_audience: "app-api-gateway".to_string(),
            token_audiences: vec![
                "app-api-gateway".to_string(),
                "app-web-frontend".to_string(),
                "app-realtime-gateway".to_string(),
            ],
            signing_key_id: Some(key_id.to_string()),
            signing_private_key_base64: Some(base64url_encode(&seed)),
            verification_keys: HashMap::from([(
                key_id.to_string(),
                base64url_encode(&signing_key.verifying_key().to_bytes()),
            )]),
            token_issuer: true,
            jwt_expiry_seconds: 3_600,
            refresh_expiry_seconds: 86_400,
            bcrypt_cost: 4,
            max_sessions_per_user: 3,
        }
    }

    fn verifier_config(issuer: &AuthConfig, audience: &str) -> AuthConfig {
        AuthConfig {
            expected_audience: audience.to_string(),
            signing_key_id: None,
            signing_private_key_base64: None,
            token_issuer: false,
            ..issuer.clone()
        }
    }

    #[test]
    fn production_rejects_all_legacy_symmetric_signing() {
        let mut config = test_config();
        config.jwt_secret = "default-secret".to_string();
        assert_eq!(
            config.validate_for_environment("production"),
            Err(AuthConfigError::LegacySymmetricSigningForbidden)
        );

        config.jwt_secret = "0123456789abcdef0123456789abcdef".to_string();
        assert_eq!(
            config.validate_for_environment("staging"),
            Err(AuthConfigError::LegacySymmetricSigningForbidden)
        );
    }

    #[test]
    fn development_keeps_explicit_legacy_compatibility() {
        let config = test_config();
        assert_eq!(config.validate_for_environment("development"), Ok(()));
    }

    #[test]
    fn production_enforces_private_public_key_separation() {
        let issuer = ed25519_issuer_config([7; 32], "primary-2026-07");
        assert_eq!(issuer.validate_for_environment("production"), Ok(()));

        let verifier = verifier_config(&issuer, "app-realtime-gateway");
        assert_eq!(verifier.validate_for_environment("production"), Ok(()));

        let leaked_private = AuthConfig {
            signing_private_key_base64: issuer.signing_private_key_base64.clone(),
            ..verifier.clone()
        };
        assert_eq!(
            leaked_private.validate_for_environment("production"),
            Err(AuthConfigError::PrivateKeyOnVerifier)
        );

        let no_public_keys = AuthConfig {
            verification_keys: HashMap::new(),
            ..verifier
        };
        assert_eq!(
            no_public_keys.validate_for_environment("production"),
            Err(AuthConfigError::MissingVerificationKeys)
        );
    }

    #[test]
    fn production_rejects_malformed_rotation_maps_and_mismatched_signers() {
        let issuer = ed25519_issuer_config([7; 32], "primary");
        let public = issuer.verification_keys.get("primary").expect("public key");
        let malformed = AuthConfig {
            verification_keys: parse_verification_keys(&format!(
                "primary:{public},entry-without-a-colon"
            )),
            signing_key_id: None,
            signing_private_key_base64: None,
            token_issuer: false,
            ..issuer.clone()
        };
        assert_eq!(
            malformed.validate_for_environment("production"),
            Err(AuthConfigError::InvalidVerificationKey)
        );

        let mismatched = AuthConfig {
            signing_private_key_base64: Some(base64url_encode(&[8; 32])),
            ..issuer
        };
        assert_eq!(
            mismatched.validate_for_environment("production"),
            Err(AuthConfigError::SigningKeyMismatch)
        );
    }

    #[test]
    fn auth_config_debug_and_serialization_redact_signing_material() {
        let config = AuthConfig {
            jwt_secret: "legacy-secret-marker".to_string(),
            signing_private_key_base64: Some("private-seed-marker".to_string()),
            ..AuthConfig::default()
        };

        let debug = format!("{config:?}");
        let serialized = serde_json::to_string(&config).expect("serialize config");
        for secret in ["legacy-secret-marker", "private-seed-marker"] {
            assert!(!debug.contains(secret));
            assert!(!serialized.contains(secret));
        }
        assert!(debug.contains("[REDACTED]"));
    }

    // -- SHA-256 sanity ---------------------------------------------------

    #[test]
    fn sha256_empty_input() {
        let digest = sha256(b"");
        assert_eq!(
            hex_encode(&digest),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn sha256_known_vector() {
        let digest = sha256(b"abc");
        assert_eq!(
            hex_encode(&digest),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn stable_numeric_subject_id_is_positive_and_deterministic() {
        let first = stable_numeric_subject_id("user:alpha");
        assert_eq!(first, stable_numeric_subject_id("user:alpha"));
        assert_ne!(first, stable_numeric_subject_id("user:beta"));
        assert!((1..=i64::MAX as u64).contains(&first));
    }

    // -- Base64-URL -------------------------------------------------------

    #[test]
    fn base64url_roundtrip() {
        let data = b"hello, universus!";
        let encoded = base64url_encode(data);
        let decoded = base64url_decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    // -- Token generation / validation ------------------------------------

    #[test]
    fn generate_and_validate_token() {
        let cfg = test_config();
        let token = generate_token(&cfg, "user-42", "alice", "player", Some(7)).unwrap();
        let claims = validate_token(&cfg, &token).unwrap();
        assert_eq!(claims.sub, "user-42");
        assert_eq!(claims.username, "alice");
        assert_eq!(claims.role, "player");
        assert_eq!(claims.universe_id, Some(7));
    }

    #[test]
    fn token_with_wrong_secret_fails() {
        let cfg = test_config();
        let token = generate_token(&cfg, "u1", "bob", "admin", None).unwrap();
        let bad_cfg = AuthConfig {
            jwt_secret: "wrong-secret".to_string(),
            ..cfg.clone()
        };
        assert_eq!(
            validate_token(&bad_cfg, &token),
            Err(AuthError::TokenInvalid)
        );
    }

    #[test]
    fn expired_token_fails() {
        let cfg = AuthConfig {
            jwt_expiry_seconds: -10, // already expired
            ..test_config()
        };
        let token = generate_token(&cfg, "u1", "bob", "player", None).unwrap();
        assert_eq!(validate_token(&cfg, &token), Err(AuthError::TokenExpired));
    }

    #[test]
    fn malformed_token_fails() {
        let cfg = test_config();
        assert_eq!(
            validate_token(&cfg, "not.a.jwt"),
            Err(AuthError::TokenInvalid)
        );
        assert_eq!(
            validate_token(&cfg, "only-one-part"),
            Err(AuthError::TokenInvalid)
        );
    }

    // -- Refresh token ----------------------------------------------------

    #[test]
    fn refresh_token_roundtrip() {
        let cfg = test_config();
        let refresh = generate_refresh_token(&cfg, "user-99").unwrap();
        let access = refresh_access_token(&cfg, &refresh, "charlie", "moderator").unwrap();
        let claims = validate_token(&cfg, &access).unwrap();
        assert_eq!(claims.sub, "user-99");
        assert_eq!(claims.username, "charlie");
        assert_eq!(claims.role, "moderator");
    }

    #[test]
    fn refresh_with_access_token_fails() {
        let cfg = test_config();
        let access = generate_token(&cfg, "u1", "dave", "player", None).unwrap();
        // An access token has role != "refresh", so using it as a refresh token must fail.
        assert_eq!(
            refresh_access_token(&cfg, &access, "dave", "player"),
            Err(AuthError::TokenInvalid)
        );
    }

    // -- Password hashing -------------------------------------------------

    #[test]
    fn hash_and_verify_password() {
        let hash = hash_password("Str0ngP@ss").unwrap();
        assert!(hash.starts_with("$argon2id$"));
        assert!(verify_password("Str0ngP@ss", &hash));
        assert!(!verify_password("wrongpassword", &hash));
        assert!(!verify_password("Str0ngP@ss", "$iter$4$salt$legacy"));
    }

    #[test]
    fn ed25519_access_token_supports_intentional_multi_audience_validation() {
        let issuer = ed25519_issuer_config([11; 32], "primary");
        let token = generate_token(&issuer, "user-42", "alice", "player", Some(7)).unwrap();

        for audience in [
            "app-api-gateway",
            "app-web-frontend",
            "app-realtime-gateway",
        ] {
            let verifier = verifier_config(&issuer, audience);
            let claims = validate_token(&verifier, &token).unwrap();
            assert_eq!(claims.purpose, TOKEN_PURPOSE_ACCESS);
            assert!(claims.aud.iter().any(|candidate| candidate == audience));
            assert!(!claims.jti.is_empty());
            assert!(claims.iat > 0);
        }

        let wrong_audience = verifier_config(&issuer, "app-admin-api");
        assert_eq!(
            validate_token(&wrong_audience, &token),
            Err(AuthError::TokenInvalid)
        );
        let wrong_issuer = AuthConfig {
            issuer: "https://attacker.invalid".to_string(),
            ..verifier_config(&issuer, "app-api-gateway")
        };
        assert_eq!(
            validate_token(&wrong_issuer, &token),
            Err(AuthError::TokenInvalid)
        );
    }

    #[test]
    fn ed25519_verification_rejects_forgery_and_algorithm_confusion() {
        let issuer = ed25519_issuer_config([13; 32], "primary");
        let verifier = verifier_config(&issuer, "app-api-gateway");
        let token = generate_token(&issuer, "u1", "alice", "admin", None).unwrap();
        let mut parts = token.split('.').map(str::to_string).collect::<Vec<_>>();
        let mut payload = base64url_decode(&parts[1]).unwrap();
        payload[0] ^= 1;
        parts[1] = base64url_encode(&payload);
        assert_eq!(
            validate_token(&verifier, &parts.join(".")),
            Err(AuthError::TokenInvalid)
        );

        let mut parts = token.split('.').collect::<Vec<_>>();
        let confused_header = encoded_jwt_header("HS256", None).unwrap();
        parts[0] = &confused_header;
        assert_eq!(
            validate_token(&verifier, &parts.join(".")),
            Err(AuthError::TokenInvalid)
        );
    }

    #[test]
    fn overlapping_verification_keys_support_rotation_without_signing_key_leakage() {
        let old_issuer = ed25519_issuer_config([17; 32], "old");
        let new_issuer = ed25519_issuer_config([19; 32], "new");
        let mut verifier = verifier_config(&new_issuer, "app-api-gateway");
        verifier
            .verification_keys
            .extend(old_issuer.verification_keys.clone());

        let old_token = generate_token(&old_issuer, "old-user", "old", "player", None).unwrap();
        let new_token = generate_token(&new_issuer, "new-user", "new", "player", None).unwrap();
        assert_eq!(
            validate_token(&verifier, &old_token).unwrap().sub,
            "old-user"
        );
        assert_eq!(
            validate_token(&verifier, &new_token).unwrap().sub,
            "new-user"
        );
        assert!(verifier.signing_private_key_base64.is_none());
    }

    #[test]
    fn service_credentials_are_scoped_and_cannot_escalate_to_human_admin() {
        let issuer = ed25519_issuer_config([23; 32], "primary");
        let verifier = verifier_config(&issuer, "app-realtime-gateway");
        let token = generate_service_token(
            &issuer,
            "notifications-worker",
            "app-realtime-gateway",
            &["realtime.publish"],
            600,
        )
        .unwrap();
        let user = authenticate_request(&verifier, &format!("Bearer {token}")).unwrap();
        assert!(require_service_scope(&user, "realtime.publish").is_ok());
        assert!(require_service_scope(&user, "bot.process").is_err());
        assert!(require_role(&user, UserRole::Admin).is_err());
        assert_eq!(
            generate_token(&issuer, "service:forged", "forged", "service", None),
            Err(AuthError::InvalidCredentials)
        );
    }

    #[test]
    fn refresh_and_future_issued_tokens_are_never_api_credentials() {
        let issuer = ed25519_issuer_config([29; 32], "primary");
        let verifier = verifier_config(&issuer, "app-api-gateway");
        let refresh = generate_refresh_token(&issuer, "u1").unwrap();
        assert_eq!(
            validate_token(&verifier, &refresh),
            Err(AuthError::TokenInvalid)
        );

        let now = now_timestamp();
        let future = Claims {
            iss: issuer.issuer.clone(),
            aud: issuer.token_audiences.clone(),
            sub: "u1".to_string(),
            username: "future".to_string(),
            email: None,
            role: "player".to_string(),
            universe_id: None,
            exp: now + 7_200,
            iat: now + 3_600,
            jti: "future-jti".to_string(),
            purpose: TOKEN_PURPOSE_ACCESS.to_string(),
            scopes: Vec::new(),
        };
        let token = encode_jwt(&issuer, &future).unwrap();
        assert_eq!(
            validate_token(&verifier, &token),
            Err(AuthError::TokenInvalid)
        );
    }

    #[test]
    fn password_strength_validation() {
        assert!(validate_password_strength("Abcdefg1").is_ok());
        assert!(matches!(
            validate_password_strength("short"),
            Err(AuthError::WeakPassword(_))
        ));
        assert!(matches!(
            validate_password_strength("alllowercase1"),
            Err(AuthError::WeakPassword(_))
        ));
        assert!(matches!(
            validate_password_strength("ALLUPPERCASE1"),
            Err(AuthError::WeakPassword(_))
        ));
        assert!(matches!(
            validate_password_strength("NoDigitsHere"),
            Err(AuthError::WeakPassword(_))
        ));
    }

    // -- Session management -----------------------------------------------

    #[test]
    fn session_create_and_validate() {
        let mut store = SessionStore::new(5);
        let session = store.create_session("u1", "jti-1", Some("127.0.0.1"), None, 9999999999);
        assert!(store.validate_session(&session.id, 1000));
        assert!(!store.validate_session(&session.id, 99999999999)); // expired
    }

    #[test]
    fn session_revoke() {
        let mut store = SessionStore::new(5);
        let s = store.create_session("u1", "jti-1", None, None, 9999999999);
        assert!(store.revoke_session(&s.id));
        assert!(!store.validate_session(&s.id, 1000));
        // Revoking again returns false.
        assert!(!store.revoke_session(&s.id));
    }

    #[test]
    fn session_revoke_all_for_user() {
        let mut store = SessionStore::new(10);
        store.create_session("u1", "j1", None, None, 9999999999);
        store.create_session("u1", "j2", None, None, 9999999999);
        store.create_session("u2", "j3", None, None, 9999999999);
        assert_eq!(store.revoke_all_sessions("u1"), 2);
        assert_eq!(
            store
                .list_user_sessions("u1")
                .iter()
                .filter(|s| !s.is_revoked)
                .count(),
            0
        );
        // u2 is unaffected.
        assert_eq!(
            store
                .list_user_sessions("u2")
                .iter()
                .filter(|s| !s.is_revoked)
                .count(),
            1
        );
    }

    #[test]
    fn session_max_per_user_enforced() {
        let mut store = SessionStore::new(2);
        let s1 = store.create_session("u1", "j1", None, None, 9999999999);
        let _s2 = store.create_session("u1", "j2", None, None, 9999999999);
        // Third session should revoke the oldest (s1).
        let _s3 = store.create_session("u1", "j3", None, None, 9999999999);
        let s1_ref = store.get_session(&s1.id).unwrap();
        assert!(s1_ref.is_revoked);
        // Active (non-revoked) session count should be <= max.
        let active = store
            .list_user_sessions("u1")
            .iter()
            .filter(|s| !s.is_revoked)
            .count();
        assert!(active <= 2);
    }

    #[test]
    fn session_cleanup_expired() {
        let mut store = SessionStore::new(10);
        store.create_session("u1", "j1", None, None, 100);
        store.create_session("u1", "j2", None, None, 200);
        store.create_session("u1", "j3", None, None, 99999);
        let removed = store.cleanup_expired(150);
        assert_eq!(removed, 1);
        assert_eq!(store.sessions.len(), 2);
    }

    // -- Bearer extraction ------------------------------------------------

    #[test]
    fn extract_bearer_valid() {
        assert_eq!(extract_bearer_token("Bearer abc123"), Some("abc123"));
        assert_eq!(extract_bearer_token("bearer abc123"), Some("abc123"));
        assert_eq!(extract_bearer_token("BEARER abc123"), Some("abc123"));
    }

    #[test]
    fn extract_bearer_invalid() {
        assert_eq!(extract_bearer_token("Basic abc"), None);
        assert_eq!(extract_bearer_token("Bear abc"), None);
        assert_eq!(extract_bearer_token(""), None);
    }

    // -- authenticate_request ---------------------------------------------

    #[test]
    fn authenticate_request_success() {
        let cfg = test_config();
        let token = generate_token(&cfg, "u7", "eve", "admin", Some(3)).unwrap();
        let header = format!("Bearer {token}");
        let user = authenticate_request(&cfg, &header).unwrap();
        assert_eq!(user.user_id, "u7");
        assert_eq!(user.username, "eve");
        assert_eq!(user.role, "admin");
        assert_eq!(user.universe_id, Some(3));
    }

    #[test]
    fn authenticate_request_missing_token() {
        let cfg = test_config();
        assert_eq!(
            authenticate_request(&cfg, "no-bearer-here"),
            Err(AuthError::TokenMissing)
        );
    }

    // -- Role hierarchy ---------------------------------------------------

    #[test]
    fn role_hierarchy() {
        assert!(has_permission(&UserRole::SuperAdmin, &UserRole::Admin));
        assert!(has_permission(&UserRole::Admin, &UserRole::Moderator));
        assert!(has_permission(&UserRole::Moderator, &UserRole::Player));
        assert!(has_permission(&UserRole::Player, &UserRole::Player));
        assert!(!has_permission(&UserRole::Player, &UserRole::Admin));
        assert!(!has_permission(&UserRole::Moderator, &UserRole::Admin));
    }

    #[test]
    fn role_display_and_parse() {
        assert_eq!(UserRole::Admin.to_string(), "admin");
        assert_eq!(
            UserRole::from_str("moderator").unwrap(),
            UserRole::Moderator
        );
        assert_eq!(
            UserRole::from_str("SUPERADMIN").unwrap(),
            UserRole::SuperAdmin
        );
        assert!(UserRole::from_str("unknown").is_err());
    }

    #[test]
    fn require_role_check() {
        let admin_user = AuthUser {
            user_id: "u1".into(),
            username: "admin_alice".into(),
            email: None,
            role: "admin".into(),
            universe_id: None,
            token_purpose: TOKEN_PURPOSE_ACCESS.into(),
            scopes: Vec::new(),
        };
        assert!(require_role(&admin_user, UserRole::Moderator).is_ok());
        assert!(require_role(&admin_user, UserRole::Admin).is_ok());
        assert!(require_role(&admin_user, UserRole::SuperAdmin).is_err());

        let player_user = AuthUser {
            user_id: "u2".into(),
            username: "player_bob".into(),
            email: None,
            role: "player".into(),
            universe_id: None,
            token_purpose: TOKEN_PURPOSE_ACCESS.into(),
            scopes: Vec::new(),
        };
        assert!(require_role(&player_user, UserRole::Player).is_ok());
        assert!(require_role(&player_user, UserRole::Moderator).is_err());
    }

    // -- AuthConfig from_env defaults -------------------------------------

    #[test]
    fn auth_config_defaults() {
        let cfg = AuthConfig::default();
        assert_eq!(cfg.jwt_expiry_seconds, 86_400);
        assert_eq!(cfg.refresh_expiry_seconds, 604_800);
        assert_eq!(cfg.bcrypt_cost, 12);
        assert_eq!(cfg.max_sessions_per_user, 5);
    }

    // -- Unique ID generation ---------------------------------------------

    #[test]
    fn generated_ids_are_unique() {
        let ids: Vec<String> = (0..100).map(|_| generate_id()).collect();
        let unique: std::collections::HashSet<&String> = ids.iter().collect();
        assert_eq!(ids.len(), unique.len());
    }
}
