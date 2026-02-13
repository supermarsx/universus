use axum::extract::Path;
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;
use serde::Serialize;

use crate::response::success;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UniverseSummary {
    id: i64,
    name: &'static str,
    speed: i32,
    registration_open: bool,
}

pub fn router() -> Router {
    Router::new()
        .route("/api/universe", get(list_universes_handler))
        .route("/api/universe/:id", get(universe_detail_handler))
        .route("/api/universe/:id/stats", get(universe_stats_handler))
        .route(
            "/api/universe/:id/maintenance/start",
            post(universe_maintenance_start_handler),
        )
}

async fn list_universes_handler() -> Response {
    success(vec![
        UniverseSummary {
            id: 1,
            name: "Andromeda",
            speed: 4,
            registration_open: true,
        },
        UniverseSummary {
            id: 2,
            name: "Pegasus",
            speed: 6,
            registration_open: false,
        },
    ])
}

async fn universe_detail_handler(Path(id): Path<i64>) -> Response {
    success(serde_json::json!({
        "id": id,
        "name": format!("Universe-{id}"),
        "speed": 4,
        "registrationOpen": true
    }))
}

async fn universe_stats_handler(Path(id): Path<i64>) -> Response {
    success(serde_json::json!({
        "universeId": id,
        "activePlayers": 1245,
        "occupiedPlanets": 3840,
        "activeWars": 12
    }))
}

async fn universe_maintenance_start_handler(Path(id): Path<i64>) -> Response {
    success(serde_json::json!({
        "universeId": id,
        "maintenance": "started"
    }))
}
