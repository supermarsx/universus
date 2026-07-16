mod account;
mod achievements;
mod alliance;
mod analytics;
mod auth;
mod config;
mod debris;
mod fleet;
mod galaxy;
mod gameplay;
mod leaderboard;
mod marketplace;
mod messages;
mod moons;
mod notifications;
mod planets;
mod player_blocks;
mod privacy;
mod research;
mod rips;
mod shards;
mod shipyard;
mod shop;
mod shop_enhanced;
mod themes;
mod universe;
mod users;

use axum::http::{header, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{middleware, Extension, Json, Router};
use platform_db::Database;
use serde::Serialize;

use crate::accounts::AccountRepository;
use crate::authorization::enforce_route_authorization;
use crate::state::AppState;

#[derive(Serialize)]
struct ServiceStatus {
    status: &'static str,
    service: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    dependency: Option<&'static str>,
}

pub fn build_router(service_name: &'static str) -> Router {
    let db = Database::from_env();
    let accounts = AccountRepository::from_environment(db.clone());
    build_router_with_dependencies(service_name, db, accounts)
}

pub fn build_router_with_dependencies(
    service_name: &'static str,
    db: Option<Database>,
    accounts: AccountRepository,
) -> Router {
    let protected_routes = Router::new()
        .merge(account::router())
        .merge(achievements::protected_router())
        .merge(debris::router())
        .merge(config::router())
        .merge(planets::router())
        .merge(planets::protected_router())
        .merge(fleet::router())
        .merge(fleet::protected_router())
        .merge(marketplace::router())
        .merge(messages::router())
        .merge(moons::router())
        .merge(notifications::protected_router())
        .merge(player_blocks::router())
        .merge(privacy::router())
        .merge(research::router())
        .merge(research::protected_router())
        .merge(rips::router())
        .merge(shipyard::router())
        .merge(shipyard::protected_router())
        .merge(shards::router())
        .merge(universe::router())
        .merge(users::router());

    Router::new()
        .route("/health", get(move || health(service_name)))
        .route("/api/health", get(move || health(service_name)))
        .route("/metrics", get(metrics))
        .route("/ready", get(move |accounts| ready(service_name, accounts)))
        .merge(auth::router())
        .merge(achievements::router())
        .merge(alliance::router())
        .merge(leaderboard::router())
        .merge(galaxy::router())
        .merge(shop::router())
        .merge(shop_enhanced::router())
        .merge(analytics::router())
        .merge(themes::router())
        .merge(protected_routes)
        .layer(middleware::from_fn(enforce_route_authorization))
        .layer(Extension(AppState::new()))
        .layer(Extension(accounts))
        .layer(Extension(db))
}

async fn health(service_name: &'static str) -> Json<ServiceStatus> {
    Json(ServiceStatus {
        status: "ok",
        service: service_name,
        dependency: None,
    })
}

async fn ready(
    service_name: &'static str,
    Extension(accounts): Extension<AccountRepository>,
) -> impl IntoResponse {
    let ready = accounts.ready().await.is_ok();
    let status = if ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        Json(ServiceStatus {
            status: if ready { "ok" } else { "unavailable" },
            service: service_name,
            dependency: Some("account-repository"),
        }),
    )
}

async fn metrics() -> impl IntoResponse {
    let body = "# HELP universus_gateway_up Service availability\n# TYPE universus_gateway_up gauge\nuniversus_gateway_up 1\n";
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; version=0.0.4"),
        )],
        body,
    )
}
