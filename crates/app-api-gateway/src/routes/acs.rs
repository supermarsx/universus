use axum::extract::Path;
use axum::response::Response;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::response::{bad_request, success};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AcsGroup {
    id: i64,
    mission_type: String,
    target_galaxy: i32,
    target_system: i32,
    target_position: i32,
    member_count: i32,
    departure_window_start: String,
    departure_window_end: String,
    notes: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateAcsGroupRequest {
    mission_type: Option<String>,
    target_galaxy: Option<i32>,
    target_system: Option<i32>,
    target_position: Option<i32>,
    departure_window_start: Option<String>,
    departure_window_end: Option<String>,
    notes: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JoinAcsGroupRequest {
    planet_id: Option<i64>,
}

pub fn router() -> Router {
    Router::new()
        .route("/api/acs", get(list_acs_groups_handler))
        .route("/api/acs", post(create_acs_group_handler))
        .route("/api/acs/:id/join", post(join_acs_group_handler))
        .route("/api/acs/:id/leave", delete(leave_acs_group_handler))
}

async fn list_acs_groups_handler() -> Response {
    success(vec![AcsGroup {
        id: 101,
        mission_type: "attack".to_string(),
        target_galaxy: 1,
        target_system: 223,
        target_position: 9,
        member_count: 3,
        departure_window_start: "2026-02-13T20:00:00Z".to_string(),
        departure_window_end: "2026-02-13T20:10:00Z".to_string(),
        notes: Some("Synchronized strike".to_string()),
    }])
}

async fn create_acs_group_handler(Json(payload): Json<CreateAcsGroupRequest>) -> Response {
    let mission_type = payload
        .mission_type
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let target_galaxy = payload.target_galaxy.unwrap_or(0);
    let target_system = payload.target_system.unwrap_or(0);
    let target_position = payload.target_position.unwrap_or(0);

    if mission_type.is_none() || target_galaxy <= 0 || target_system <= 0 || target_position <= 0 {
        return bad_request("Invalid ACS group request");
    }

    success(serde_json::json!({
        "id": 102,
        "missionType": mission_type,
        "targetGalaxy": target_galaxy,
        "targetSystem": target_system,
        "targetPosition": target_position,
        "departureWindowStart": payload
            .departure_window_start
            .unwrap_or_else(|| "2026-02-13T20:15:00Z".to_string()),
        "departureWindowEnd": payload
            .departure_window_end
            .unwrap_or_else(|| "2026-02-13T20:30:00Z".to_string()),
        "notes": payload.notes,
        "memberCount": 1
    }))
}

async fn join_acs_group_handler(
    Path(id): Path<i64>,
    Json(payload): Json<JoinAcsGroupRequest>,
) -> Response {
    let planet_id = payload.planet_id.unwrap_or(0);
    if id <= 0 || planet_id <= 0 {
        return bad_request("Invalid ACS join request");
    }

    success(serde_json::json!({
        "groupId": id,
        "planetId": planet_id,
        "joined": true
    }))
}

async fn leave_acs_group_handler(Path(id): Path<i64>) -> Response {
    if id <= 0 {
        return bad_request("Invalid ACS leave request");
    }

    success(serde_json::json!({
        "groupId": id,
        "left": true
    }))
}
