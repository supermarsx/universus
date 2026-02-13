use axum::extract::Path;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::response::{bad_request, success};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Moon {
    id: i64,
    planet_id: i64,
    name: &'static str,
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
        .route("/api/moons/:planet_id", get(get_moon_by_planet_handler))
        .route("/api/moons/:moon_id/jump-gate", post(jump_gate_handler))
}

async fn list_moons_handler() -> Response {
    success(vec![sample_moon(101, 1), sample_moon(102, 2)])
}

async fn get_moon_by_planet_handler(Path(planet_id): Path<i64>) -> Response {
    success(sample_moon(planet_id + 100, planet_id))
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
        name: "Selene",
        diameter: 8_912,
        has_jump_gate: true,
    }
}
