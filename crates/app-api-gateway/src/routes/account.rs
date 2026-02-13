use axum::response::Response;
use axum::routing::get;
use axum::Router;
use serde::Serialize;

use crate::response::success;

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

async fn account_resources_handler() -> Response {
    success(AccountResources {
        metal: 125_000,
        crystal: 94_500,
        deuterium: 40_250,
        dark_matter: 1_500,
    })
}
