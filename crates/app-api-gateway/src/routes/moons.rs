use axum::extract::Path;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use platform_db::Database;
use serde::{Deserialize, Serialize};

use crate::response::{bad_request, success};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Moon {
    id: i64,
    planet_id: i64,
    name: String,
    diameter: i32,
    has_jump_gate: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JumpGateRequest {
    to_moon_id: i64,
    fleet_ids: Vec<i64>,
}

pub fn router() -> Router {
    Router::new()
        .route("/api/moons", get(list_moons_handler))
        .route("/api/moons/id/:moon_id", get(get_moon_by_id_handler))
        .route("/api/moons/public/:moon_id", get(get_public_moon_handler))
        .route("/api/moons/:planet_id", get(get_moon_by_planet_handler))
        .route("/api/moons/:moon_id/phalanx", post(phalanx_scan_handler))
        .route("/api/moons/:moon_id/jump-gate", post(jump_gate_handler))
}

async fn list_moons_handler(Extension(db): Extension<Option<Database>>) -> Response {
    if let Some(database) = db {
        if let Ok(rows) = database.list_moons().await {
            if !rows.is_empty() {
                return success(rows.into_iter().map(map_db_moon).collect::<Vec<_>>());
            }
        }
    }

    success(sample_moons())
}

async fn get_moon_by_planet_handler(
    Extension(db): Extension<Option<Database>>,
    Path(planet_id): Path<i64>,
) -> Response {
    if let Some(database) = db {
        if let Ok(Some(row)) = database.moon_by_planet_id(planet_id).await {
            return success(map_db_moon(row));
        }
    }

    success(sample_moon(planet_id + 100, planet_id))
}

async fn get_moon_by_id_handler(
    Extension(db): Extension<Option<Database>>,
    Path(moon_id): Path<i64>,
) -> Response {
    if let Some(database) = db {
        if let Ok(Some(row)) = database.moon_by_id(moon_id).await {
            return success(map_db_moon(row));
        }
    }

    success(sample_moon(moon_id, moon_id.saturating_sub(100).max(1)))
}

async fn get_public_moon_handler(
    Extension(db): Extension<Option<Database>>,
    Path(moon_id): Path<i64>,
) -> Response {
    let moon = if let Some(database) = db {
        database
            .moon_by_id(moon_id)
            .await
            .ok()
            .flatten()
            .map(map_db_moon)
            .unwrap_or_else(|| sample_moon(moon_id, moon_id.saturating_sub(100).max(1)))
    } else {
        sample_moon(moon_id, moon_id.saturating_sub(100).max(1))
    };

    success(serde_json::json!({
        "id": moon.id,
        "diameter": moon.diameter,
        "ownerAlias": "Commander",
        "hasSensorPhalanx": true,
        "hasJumpGate": moon.has_jump_gate,
        "coordinates": {
            "galaxy": 1,
            "system": 120,
            "position": 8
        },
        "createdAt": "2026-02-13T00:00:00Z"
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PhalanxScanRequest {
    target_galaxy: i32,
    target_system: i32,
    target_position: i32,
}

async fn phalanx_scan_handler(
    Path(moon_id): Path<i64>,
    Json(payload): Json<PhalanxScanRequest>,
) -> Response {
    if payload.target_galaxy <= 0 || payload.target_system <= 0 || payload.target_position <= 0 {
        return bad_request("Invalid coordinates");
    }

    success(serde_json::json!({
        "moonId": moon_id,
        "target": {
            "galaxy": payload.target_galaxy,
            "system": payload.target_system,
            "position": payload.target_position
        },
        "missions": [
            {
                "fleetId": "fleet-201",
                "mission": "attack",
                "arrivalInSeconds": 420
            }
        ]
    }))
}

async fn jump_gate_handler(
    Path(moon_id): Path<i64>,
    Json(payload): Json<JumpGateRequest>,
) -> Response {
    if payload.to_moon_id <= 0 || payload.fleet_ids.is_empty() {
        return bad_request("Invalid request");
    }
    success(serde_json::json!({
        "fromMoonId": moon_id,
        "toMoonId": payload.to_moon_id,
        "fleetsMoved": payload.fleet_ids.len(),
        "accepted": true
    }))
}

fn sample_moon(id: i64, planet_id: i64) -> Moon {
    Moon {
        id,
        planet_id,
        name: "Selene".to_string(),
        diameter: 8_912,
        has_jump_gate: true,
    }
}

fn sample_moons() -> Vec<Moon> {
    vec![sample_moon(101, 1), sample_moon(102, 2)]
}

fn map_db_moon(row: platform_db::MoonRow) -> Moon {
    Moon {
        id: row.id,
        planet_id: row.planet_id,
        name: row.name,
        diameter: row.diameter,
        has_jump_gate: row.has_jump_gate,
    }
}
