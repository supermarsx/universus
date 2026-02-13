mod account;
mod acs;
mod alliance;
mod auth;
mod config;
mod debris;
mod fleet;
mod galaxy;
mod leaderboard;
mod messages;
mod moons;
mod planets;
mod player_blocks;
mod research;
mod rips;
mod shards;
mod shipyard;
mod shop;
mod themes;
mod universe;
mod users;

use axum::routing::get;
use axum::{middleware, Extension, Json, Router};
use serde::Serialize;

use crate::auth_guard::require_bearer_auth;
use crate::state::AppState;

#[derive(Serialize)]
struct ServiceStatus {
    status: &'static str,
    service: &'static str,
}

pub fn build_router(service_name: &'static str) -> Router {
    let protected_routes = Router::new()
        .merge(account::router())
        .merge(acs::router())
        .merge(debris::router())
        .merge(config::router())
        .merge(planets::protected_router())
        .merge(fleet::protected_router())
        .merge(moons::router())
        .merge(player_blocks::router())
        .merge(research::protected_router())
        .merge(rips::router())
        .merge(shipyard::protected_router())
        .merge(shards::router())
        .merge(universe::router())
        .merge(users::router())
        .route_layer(middleware::from_fn(require_bearer_auth));

    Router::new()
        .route("/health", get(move || health(service_name)))
        .route("/ready", get(move || ready(service_name)))
        .merge(auth::router())
        .merge(planets::router())
        .merge(fleet::router())
        .merge(alliance::router())
        .merge(messages::router())
        .merge(leaderboard::router())
        .merge(galaxy::router())
        .merge(shop::router())
        .merge(themes::router())
        .merge(research::router())
        .merge(shipyard::router())
        .merge(protected_routes)
        .layer(Extension(AppState::new()))
}

async fn health(service_name: &'static str) -> Json<ServiceStatus> {
    Json(ServiceStatus {
        status: "ok",
        service: service_name,
    })
}

async fn ready(service_name: &'static str) -> Json<ServiceStatus> {
    Json(ServiceStatus {
        status: "ok",
        service: service_name,
    })
}
