use axum::extract::Path;
use axum::response::Response;
use axum::routing::{get, patch, post};
use axum::{Extension, Json, Router};
use platform_db::{Database, UniverseCreateInput, UniverseRow};
use serde::Serialize;
use serde_json::Value;

use crate::response::success;
use crate::state::{AppState, UniverseSnapshot};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UniverseSummary {
    id: i64,
    name: String,
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

async fn list_universes_handler(
    Extension(db): Extension<Option<Database>>,
    Extension(app_state): Extension<AppState>,
) -> Response {
    let universes: Vec<UniverseSummary> = if let Some(database) = db {
        database
            .list_universes()
            .await
            .map(|rows| rows.into_iter().map(universe_summary_from_row).collect())
            .unwrap_or_else(|_| {
                app_state
                    .list_universes()
                    .into_iter()
                    .map(universe_summary_from_snapshot)
                    .collect()
            })
    } else {
        app_state
            .list_universes()
            .into_iter()
            .map(universe_summary_from_snapshot)
            .collect()
    };

    success(universes)
}

async fn universe_detail_handler(
    Extension(db): Extension<Option<Database>>,
    Extension(app_state): Extension<AppState>,
    Path(id): Path<i64>,
) -> Response {
    if let Some(database) = db {
        if let Ok(Some(universe)) = database.get_universe(id).await {
            return success(serde_json::json!({
                "id": universe.id,
                "name": universe.name,
                "speed": universe.speed,
                "registrationOpen": universe.registration_open
            }));
        }
    }

    if let Some(universe) = app_state.get_universe(id) {
        return success(serde_json::json!({
            "id": universe.id,
            "name": universe.name,
            "speed": universe.speed,
            "registrationOpen": universe.registration_open
        }));
    }

    success(serde_json::json!({
        "id": id,
        "name": format!("Universe-{id}"),
        "speed": 4,
        "registrationOpen": true
    }))
}

async fn universe_create_handler(
    Extension(db): Extension<Option<Database>>,
    Extension(app_state): Extension<AppState>,
    Json(payload): Json<Value>,
) -> Response {
    let name = payload
        .get("name")
        .and_then(Value::as_str)
        .or_else(|| payload.get("universeName").and_then(Value::as_str))
        .unwrap_or("Universe");
    let speed = payload
        .get("speed")
        .and_then(Value::as_i64)
        .or_else(|| payload.get("speedMultiplier").and_then(Value::as_i64))
        .unwrap_or(4) as i32;

    let universe_id = if let Some(database) = db {
        database
            .create_universe(UniverseCreateInput {
                name: name.to_string(),
                speed,
                registration_open: true,
            })
            .await
            .map(|row| row.id)
            .unwrap_or_else(|_| app_state.create_universe(name, speed, true).id)
    } else {
        app_state.create_universe(name, speed, true).id
    };

    success(serde_json::json!({
        "created": true,
        "universeId": universe_id,
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

async fn universe_stats_handler(
    Extension(db): Extension<Option<Database>>,
    Path(id): Path<i64>,
) -> Response {
    if let Some(database) = db {
        if let Ok(stats) = database.universe_stats(id).await {
            return success(serde_json::json!({
                "universeId": stats.universe_id,
                "activePlayers": stats.active_players,
                "occupiedPlanets": stats.occupied_planets,
                "activeWars": stats.active_wars
            }));
        }
    }

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

fn universe_summary_from_row(entry: UniverseRow) -> UniverseSummary {
    UniverseSummary {
        id: entry.id,
        name: entry.name,
        speed: entry.speed,
        registration_open: entry.registration_open,
    }
}

fn universe_summary_from_snapshot(entry: UniverseSnapshot) -> UniverseSummary {
    UniverseSummary {
        id: entry.id,
        name: entry.name,
        speed: entry.speed,
        registration_open: entry.registration_open,
    }
}
