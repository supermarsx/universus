mod alliance;
mod auth;
mod fleet;
mod galaxy;
mod leaderboard;
mod messages;
mod planets;
mod research;
mod shipyard;
mod shop;

use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

#[derive(Serialize)]
struct ServiceStatus {
    status: &'static str,
    service: &'static str,
}

pub fn build_router(service_name: &'static str) -> Router {
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
        .merge(research::router())
        .merge(shipyard::router())
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
