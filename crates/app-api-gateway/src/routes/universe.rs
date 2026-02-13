use axum::extract::Path;
use axum::response::Response;
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use serde::Serialize;
use serde_json::Value;

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
        .route("/api/universe/create", post(universe_create_handler))
        .route("/api/universe/:id", get(universe_detail_handler))
        .route("/api/universe/:id/seed", post(universe_seed_handler))
        .route(
            "/api/universe/:id/place-player",
            post(universe_place_player_handler),
        )
        .route("/api/universe/:id/stats", get(universe_stats_handler))
        .route(
            "/api/universe/:id/maintenance/start",
            post(universe_maintenance_start_handler),
        )
        .route(
            "/api/universe/:id/maintenance/population-balance",
            post(universe_maintenance_population_balance_handler),
        )
        .route(
            "/api/universe/:id/registration",
            patch(universe_registration_patch_handler),
        )
        .route(
            "/api/universe/:id/lifecycle",
            patch(universe_lifecycle_patch_handler),
        )
        .route(
            "/api/universe/:id/speed",
            patch(universe_speed_patch_handler),
        )
        .route(
            "/api/universe/:id/merge",
            patch(universe_merge_patch_handler),
        )
        .route(
            "/api/universe/:id/end-event",
            patch(universe_end_event_patch_handler),
        )
        .route(
            "/api/universe/:id/announcement",
            patch(universe_announcement_patch_handler),
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

async fn universe_create_handler(Json(payload): Json<Value>) -> Response {
    let name = payload
        .get("name")
        .and_then(Value::as_str)
        .or_else(|| payload.get("universeName").and_then(Value::as_str))
        .unwrap_or("Universe");
    success(serde_json::json!({
        "created": true,
        "universeId": 101,
        "message": "Universe created",
        "name": name
    }))
}

async fn universe_seed_handler(Path(id): Path<i64>, Json(payload): Json<Value>) -> Response {
    success(serde_json::json!({
        "success": true,
        "universeId": id,
        "seeded": true,
        "generateGalaxies": payload.get("generateGalaxies").and_then(Value::as_bool).unwrap_or(true),
        "generateBots": payload.get("generateBots").and_then(Value::as_bool).unwrap_or(true),
        "generateAlliances": payload.get("generateAlliances").and_then(Value::as_bool).unwrap_or(true),
        "distributeResources": payload.get("distributeResources").and_then(Value::as_bool).unwrap_or(true)
    }))
}

async fn universe_place_player_handler(
    Path(id): Path<i64>,
    Json(payload): Json<Value>,
) -> Response {
    success(serde_json::json!({
        "success": true,
        "universeId": id,
        "placed": true,
        "playerId": payload.get("playerId").and_then(Value::as_i64).unwrap_or(1),
        "placement": {
            "galaxy": payload.get("customGalaxy").and_then(Value::as_i64).unwrap_or(1),
            "system": payload.get("customSystem").and_then(Value::as_i64).unwrap_or(42),
            "position": 7
        }
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

async fn universe_maintenance_population_balance_handler(Path(id): Path<i64>) -> Response {
    success(serde_json::json!({
        "success": true,
        "universeId": id,
        "operation": "population-balance",
        "balanced": true,
        "adjustments": 0
    }))
}

async fn universe_registration_patch_handler(
    Path(id): Path<i64>,
    Json(payload): Json<Value>,
) -> Response {
    success(serde_json::json!({
        "universeId": id,
        "updated": "registration",
        "changes": payload
    }))
}

async fn universe_lifecycle_patch_handler(
    Path(id): Path<i64>,
    Json(payload): Json<Value>,
) -> Response {
    success(serde_json::json!({
        "universeId": id,
        "updated": "lifecycle",
        "changes": payload
    }))
}

async fn universe_speed_patch_handler(Path(id): Path<i64>, Json(payload): Json<Value>) -> Response {
    success(serde_json::json!({
        "universeId": id,
        "updated": "speed",
        "changes": payload
    }))
}

async fn universe_merge_patch_handler(Path(id): Path<i64>, Json(payload): Json<Value>) -> Response {
    success(serde_json::json!({
        "universeId": id,
        "updated": "merge",
        "changes": payload
    }))
}

async fn universe_end_event_patch_handler(
    Path(id): Path<i64>,
    Json(payload): Json<Value>,
) -> Response {
    success(serde_json::json!({
        "universeId": id,
        "updated": "end-event",
        "changes": payload
    }))
}

async fn universe_announcement_patch_handler(
    Path(id): Path<i64>,
    Json(payload): Json<Value>,
) -> Response {
    success(serde_json::json!({
        "universeId": id,
        "updated": "announcement",
        "changes": payload
    }))
}
