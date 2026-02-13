use axum::response::Response;
use axum::routing::get;
use axum::{Extension, Router};
use serde::Serialize;

use crate::auth_guard::BearerToken;
use crate::response::success;
use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountProfile {
    id: &'static str,
    username: &'static str,
    email: &'static str,
    rank: i64,
    alliance_tag: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountResources {
    metal: i64,
    crystal: i64,
    deuterium: i64,
    dark_matter: i64,
}

pub fn router() -> Router {
    Router::new()
        .route("/api/account/profile", get(account_profile_handler))
        .route("/api/account/resources", get(account_resources_handler))
}

async fn account_profile_handler() -> Response {
    success(AccountProfile {
        id: "u-rust-1",
        username: "Commander",
        email: "commander@example.com",
        rank: 42,
        alliance_tag: "RUST",
    })
}

async fn account_resources_handler(
    BearerToken(token): BearerToken,
    Extension(app_state): Extension<AppState>,
) -> Response {
    let resources = app_state.account_resources(&token);
    success(AccountResources {
        metal: resources.metal,
        crystal: resources.crystal,
        deuterium: resources.deuterium,
        dark_matter: resources.dark_matter,
    })
}
