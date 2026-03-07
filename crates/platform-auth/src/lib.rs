//! Core building blocks for the platform-auth crate.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthContext {
    pub user_id: i64,
    pub username: String,
    pub roles: Vec<String>,
    pub tenant_id: Option<String>,
    pub issued_at: i64,
    pub expires_at: i64,
}

#[derive(Debug, Clone)]
pub enum AuthError {
    TokenExpired,
    TokenInvalid,
    InsufficientPermissions(String),
    MissingToken,
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TokenExpired => write!(f, "token has expired"),
            Self::TokenInvalid => write!(f, "token is invalid"),
            Self::InsufficientPermissions(role) => {
                write!(f, "insufficient permissions: missing role '{role}'")
            }
            Self::MissingToken => write!(f, "authentication token is missing"),
        }
    }
}

pub struct TokenValidator {
    secret: String,
}

impl TokenValidator {
    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            secret: secret.into(),
        }
    }

    pub fn generate_token(&self, context: &AuthContext) -> String {
        let json = serde_json::to_string(context).expect("failed to serialize AuthContext");
        let encoded = base64_encode(json.as_bytes());
        let signature = compute_signature(self.secret.as_bytes(), encoded.as_bytes());
        format!("{encoded}.{signature}")
    }

    pub fn validate_token(&self, token: &str) -> Result<AuthContext, AuthError> {
        let (encoded, signature) = token.rsplit_once('.').ok_or(AuthError::TokenInvalid)?;

        let expected = compute_signature(self.secret.as_bytes(), encoded.as_bytes());
        if signature != expected {
            return Err(AuthError::TokenInvalid);
        }

        let json_bytes = base64_decode(encoded).map_err(|_| AuthError::TokenInvalid)?;
        let context: AuthContext =
            serde_json::from_slice(&json_bytes).map_err(|_| AuthError::TokenInvalid)?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        if context.expires_at <= now {
            return Err(AuthError::TokenExpired);
        }

        Ok(context)
    }
}

pub fn has_role(context: &AuthContext, role: &str) -> bool {
    context.roles.iter().any(|r| r == role)
}

pub fn require_role(context: &AuthContext, role: &str) -> Result<(), AuthError> {
    if has_role(context, role) {
        Ok(())
    } else {
        Err(AuthError::InsufficientPermissions(role.to_string()))
    }
}

/// Returns the crate name for a basic compile-time sanity check.
pub const fn crate_name() -> &'static str {
    "platform-auth"
}

// ---------------------------------------------------------------------------
// Internal helpers – base64 & HMAC-like signature (std-only, no crypto deps)
// ---------------------------------------------------------------------------

/// Deterministic keyed hash used as a lightweight HMAC substitute.
fn compute_signature(secret: &[u8], data: &[u8]) -> String {
    let mut hash: [u8; 32] = [
        0x6a, 0x09, 0xe6, 0x67, 0xbb, 0x67, 0xae, 0x85, 0x3c, 0x6e, 0xf3, 0x72, 0xa5, 0x4f,
        0xf5, 0x3a, 0x51, 0x0e, 0x52, 0x7f, 0x9b, 0x05, 0x68, 0x8c, 0x1f, 0x83, 0xd9, 0xab,
        0x5b, 0xe0, 0xcd, 0x19,
    ];

    for (i, &b) in secret.iter().enumerate() {
        let idx = i % 32;
        hash[idx] = hash[idx].wrapping_add(b).wrapping_mul(0x6d);
        hash[(idx + 1) % 32] ^= hash[idx];
    }

    for (i, &b) in data.iter().enumerate() {
        let idx = i % 32;
        hash[idx] = hash[idx].wrapping_add(b).wrapping_mul(0x5f);
        hash[(idx + 1) % 32] ^= hash[idx];
    }

    hash.iter().map(|b| format!("{b:02x}")).collect()
}

const BASE64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(input: &[u8]) -> String {
    let mut output = String::new();
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        output.push(BASE64_CHARS[((triple >> 18) & 0x3F) as usize] as char);
        output.push(BASE64_CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            output.push(BASE64_CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            output.push('=');
        }
        if chunk.len() > 2 {
            output.push(BASE64_CHARS[(triple & 0x3F) as usize] as char);
        } else {
            output.push('=');
        }
    }
    output
}

fn base64_decode(input: &str) -> Result<Vec<u8>, &'static str> {
    let input = input.trim_end_matches('=');
    let mut output = Vec::new();
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;

    for c in input.chars() {
        let val = match c {
            'A'..='Z' => c as u32 - b'A' as u32,
            'a'..='z' => c as u32 - b'a' as u32 + 26,
            '0'..='9' => c as u32 - b'0' as u32 + 52,
            '+' => 62,
            '/' => 63,
            _ => return Err("invalid base64 character"),
        };
        buf = (buf << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_context() -> AuthContext {
        AuthContext {
            user_id: 42,
            username: "commander_42".to_string(),
            roles: vec!["player".to_string(), "guild_leader".to_string()],
            tenant_id: Some("realm-7".to_string()),
            issued_at: 1_700_000_000,
            expires_at: i64::MAX,
        }
    }

    #[test]
    fn crate_name_returns_expected() {
        assert_eq!(crate_name(), "platform-auth");
    }

    #[test]
    fn generate_and_validate_round_trip() {
        let validator = TokenValidator::new("test-secret");
        let ctx = sample_context();
        let token = validator.generate_token(&ctx);
        let decoded = validator.validate_token(&token).unwrap();

        assert_eq!(decoded.user_id, ctx.user_id);
        assert_eq!(decoded.username, ctx.username);
        assert_eq!(decoded.roles, ctx.roles);
        assert_eq!(decoded.tenant_id, ctx.tenant_id);
    }

    #[test]
    fn validate_rejects_tampered_payload() {
        let validator = TokenValidator::new("test-secret");
        let token = validator.generate_token(&sample_context());
        let tampered = format!("xxx{}", &token[3..]);
        assert!(matches!(
            validator.validate_token(&tampered),
            Err(AuthError::TokenInvalid)
        ));
    }

    #[test]
    fn validate_rejects_wrong_secret() {
        let v1 = TokenValidator::new("secret-a");
        let v2 = TokenValidator::new("secret-b");
        let token = v1.generate_token(&sample_context());
        assert!(matches!(
            v2.validate_token(&token),
            Err(AuthError::TokenInvalid)
        ));
    }

    #[test]
    fn validate_rejects_expired_token() {
        let validator = TokenValidator::new("test-secret");
        let mut ctx = sample_context();
        ctx.expires_at = 1;
        let token = validator.generate_token(&ctx);
        assert!(matches!(
            validator.validate_token(&token),
            Err(AuthError::TokenExpired)
        ));
    }

    #[test]
    fn validate_rejects_malformed_token() {
        let validator = TokenValidator::new("test-secret");
        assert!(matches!(
            validator.validate_token("not-a-valid-token"),
            Err(AuthError::TokenInvalid)
        ));
    }

    #[test]
    fn has_role_returns_true_for_present_role() {
        let ctx = sample_context();
        assert!(has_role(&ctx, "player"));
        assert!(has_role(&ctx, "guild_leader"));
    }

    #[test]
    fn has_role_returns_false_for_absent_role() {
        assert!(!has_role(&sample_context(), "admin"));
    }

    #[test]
    fn require_role_succeeds_when_present() {
        assert!(require_role(&sample_context(), "player").is_ok());
    }

    #[test]
    fn require_role_fails_when_absent() {
        let err = require_role(&sample_context(), "admin").unwrap_err();
        assert!(
            matches!(err, AuthError::InsufficientPermissions(ref r) if r == "admin")
        );
    }

    #[test]
    fn auth_error_display() {
        assert_eq!(AuthError::TokenExpired.to_string(), "token has expired");
        assert_eq!(AuthError::TokenInvalid.to_string(), "token is invalid");
        assert_eq!(
            AuthError::MissingToken.to_string(),
            "authentication token is missing"
        );
        assert_eq!(
            AuthError::InsufficientPermissions("admin".to_string()).to_string(),
            "insufficient permissions: missing role 'admin'"
        );
    }

    #[test]
    fn base64_round_trip() {
        let original = b"hello, platform-auth!";
        let encoded = base64_encode(original);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn token_with_none_tenant_id() {
        let validator = TokenValidator::new("secret");
        let mut ctx = sample_context();
        ctx.tenant_id = None;
        let token = validator.generate_token(&ctx);
        let decoded = validator.validate_token(&token).unwrap();
        assert_eq!(decoded.tenant_id, None);
    }

    #[test]
    fn signature_is_deterministic() {
        let sig1 = compute_signature(b"key", b"data");
        let sig2 = compute_signature(b"key", b"data");
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn signature_changes_with_different_inputs() {
        let sig1 = compute_signature(b"key", b"data-a");
        let sig2 = compute_signature(b"key", b"data-b");
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn empty_roles_context_round_trip() {
        let validator = TokenValidator::new("secret");
        let mut ctx = sample_context();
        ctx.roles = vec![];
        let token = validator.generate_token(&ctx);
        let decoded = validator.validate_token(&token).unwrap();
        assert!(decoded.roles.is_empty());
    }
}
