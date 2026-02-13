use axum::extract::rejection::JsonRejection;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::response::{bad_request, success};

#[derive(Debug, Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct RegisterRequest {
    username: String,
    email: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct LogoutRequest {
    token: String,
}

#[derive(Debug, Serialize)]
struct AuthUser {
    id: String,
    username: String,
    email: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthPayload {
    token: String,
    user: AuthUser,
    expires_in_seconds: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LogoutPayload {
    revoked: bool,
    reason: &'static str,
}

pub fn router() -> Router {
    Router::new()
        .route("/api/auth/login", post(login_handler))
        .route("/api/auth/register", post(register_handler))
        .route("/api/auth/logout", post(logout_handler))
        .route("/api/auth/me", get(me_handler))
}

async fn login_handler(payload: Result<Json<LoginRequest>, JsonRejection>) -> Response {
    let Json(input) = match payload {
        Ok(value) => value,
        Err(_) => return bad_request("Invalid login payload"),
    };

    if input.email.trim().is_empty() || input.password.trim().is_empty() {
        return bad_request("Email and password are required");
    }

    success(AuthPayload {
        token: issue_token(&input.email),
        user: AuthUser {
            id: "u-rust-1".to_string(),
            username: "Commander".to_string(),
            email: input.email.to_ascii_lowercase(),
        },
        expires_in_seconds: 7 * 24 * 3600,
    })
}

async fn register_handler(payload: Result<Json<RegisterRequest>, JsonRejection>) -> Response {
    let Json(input) = match payload {
        Ok(value) => value,
        Err(_) => return bad_request("Invalid register payload"),
    };

    if input.email.trim().is_empty()
        || input.password.trim().is_empty()
        || input.username.trim().is_empty()
    {
        return bad_request("Username, email and password are required");
    }

    success(AuthPayload {
        token: issue_token(&input.email),
        user: AuthUser {
            id: "u-rust-new".to_string(),
            username: input.username,
            email: input.email.to_ascii_lowercase(),
        },
        expires_in_seconds: 7 * 24 * 3600,
    })
}

async fn logout_handler(payload: Result<Json<LogoutRequest>, JsonRejection>) -> Response {
    let Json(input) = match payload {
        Ok(value) => value,
        Err(_) => return bad_request("Invalid logout payload"),
    };

    if input.token.trim().is_empty() {
        return bad_request("Token is required");
    }

    success(LogoutPayload {
        revoked: true,
        reason: "manual_logout",
    })
}

async fn me_handler() -> Response {
    success(AuthUser {
        id: "u-rust-1".to_string(),
        username: "Commander".to_string(),
        email: "commander@example.com".to_string(),
    })
}

fn issue_token(seed: &str) -> String {
    let mut hash: u32 = 0x811c9dc5;
    for byte in seed.trim().to_ascii_lowercase().as_bytes() {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(16777619);
    }
    format!("rust-gateway-token-{hash:08x}")
}
