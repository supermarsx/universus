use axum::extract::{Path, Query};
use axum::response::Response;
use axum::routing::{delete, get, post};
use axum::{Extension, Json, Router};
use serde::{Deserialize, Serialize};

use crate::auth_guard::BearerToken;
use crate::response::{bad_request, not_found, success};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatePlayerBlockRequest {
    blocked_user_id: Option<i64>,
    username: Option<String>,
    scope: Option<String>,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UnblockQuery {
    scope: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlayerBlockPayload {
    blocked_user_id: i64,
    username: String,
    scope: String,
    reason: Option<String>,
}

pub fn router() -> Router {
    Router::new()
        .route("/api/player-blocks", get(list_player_blocks_handler))
        .route("/api/player-blocks", post(create_player_block_handler))
        .route(
            "/api/player-blocks/:target_identifier",
            delete(delete_player_block_handler),
        )
}

async fn list_player_blocks_handler(
    BearerToken(token): BearerToken,
    Extension(app_state): Extension<AppState>,
) -> Response {
    let blocks = app_state.list_player_blocks(&token);
    success(
        blocks
            .into_iter()
            .map(|entry| PlayerBlockPayload {
                blocked_user_id: entry.blocked_user_id,
                username: entry.username,
                scope: entry.scope,
                reason: entry.reason,
            })
            .collect::<Vec<_>>(),
    )
}

async fn create_player_block_handler(
    BearerToken(token): BearerToken,
    Extension(app_state): Extension<AppState>,
    Json(payload): Json<CreatePlayerBlockRequest>,
) -> Response {
    let blocked_user_id = payload.blocked_user_id.unwrap_or(0);
    let username = payload
        .username
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("");
    let scope = payload.scope.as_deref().unwrap_or("all");

    if blocked_user_id <= 0 && username.is_empty() {
        return bad_request("User not found");
    }

    let resolved_user_id = if blocked_user_id > 0 {
        blocked_user_id
    } else {
        10_000
    };
    let resolved_username = if username.is_empty() {
        format!("User-{resolved_user_id}")
    } else {
        username.to_string()
    };

    match app_state.add_player_block(
        &token,
        resolved_user_id,
        &resolved_username,
        scope,
        payload.reason,
    ) {
        Ok(entry) => success(serde_json::json!({
            "blockedUserId": entry.blocked_user_id,
            "username": entry.username,
            "scope": entry.scope,
            "reason": entry.reason,
            "message": "Player blocked"
        })),
        Err(message) => bad_request(message),
    }
}

async fn delete_player_block_handler(
    BearerToken(token): BearerToken,
    Extension(app_state): Extension<AppState>,
    Path(target_identifier): Path<String>,
    Query(query): Query<UnblockQuery>,
) -> Response {
    let _ = query.scope;
    match app_state.remove_player_block(&token, &target_identifier) {
        Ok(_) => success(serde_json::json!({
            "message": "Player unblocked"
        })),
        Err(_) => not_found("Block not found"),
    }
}
