use axum::extract::Path;
use axum::response::Response;
use axum::routing::{delete, get, post};
use axum::{Extension, Json, Router};
use platform_db::{AcsGroupCreateInput, AcsGroupRow, Database};
use serde::{Deserialize, Serialize};

use crate::response::{bad_request, success};
use crate::state::{AcsGroupSnapshot, AppState, CreateAcsGroupInput};

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

async fn list_acs_groups_handler(
    Extension(db): Extension<Option<Database>>,
    Extension(app_state): Extension<AppState>,
) -> Response {
    let groups = if let Some(database) = db {
        database
            .list_acs_groups()
            .await
            .map(|rows| rows.into_iter().map(acs_group_from_row).collect::<Vec<_>>())
            .unwrap_or_else(|_| {
                app_state
                    .list_acs_groups()
                    .into_iter()
                    .map(acs_group_from_snapshot)
                    .collect::<Vec<_>>()
            })
    } else {
        app_state
            .list_acs_groups()
            .into_iter()
            .map(acs_group_from_snapshot)
            .collect::<Vec<_>>()
    };

    success(groups)
}

async fn create_acs_group_handler(
    Extension(db): Extension<Option<Database>>,
    Extension(app_state): Extension<AppState>,
    Json(payload): Json<CreateAcsGroupRequest>,
) -> Response {
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
    let mission_type = mission_type.expect("validated mission_type");

    let departure_window_start = payload
        .departure_window_start
        .clone()
        .unwrap_or_else(|| "2026-02-13T20:15:00Z".to_string());
    let departure_window_end = payload
        .departure_window_end
        .clone()
        .unwrap_or_else(|| "2026-02-13T20:30:00Z".to_string());

    let group = if let Some(database) = db {
        database
            .create_acs_group(AcsGroupCreateInput {
                mission_type: mission_type.clone(),
                target_galaxy,
                target_system,
                target_position,
                departure_window_start: departure_window_start.clone(),
                departure_window_end: departure_window_end.clone(),
                notes: payload.notes.clone(),
            })
            .await
            .map(acs_group_from_row)
            .unwrap_or_else(|_| {
                acs_group_from_snapshot(app_state.create_acs_group(CreateAcsGroupInput {
                    mission_type: mission_type.clone(),
                    target_galaxy,
                    target_system,
                    target_position,
                    departure_window_start: Some(departure_window_start.clone()),
                    departure_window_end: Some(departure_window_end.clone()),
                    notes: payload.notes.clone(),
                }))
            })
    } else {
        acs_group_from_snapshot(app_state.create_acs_group(CreateAcsGroupInput {
            mission_type: mission_type.clone(),
            target_galaxy,
            target_system,
            target_position,
            departure_window_start: Some(departure_window_start.clone()),
            departure_window_end: Some(departure_window_end.clone()),
            notes: payload.notes.clone(),
        }))
    };

    success(serde_json::json!({
        "id": group.id,
        "missionType": group.mission_type,
        "targetGalaxy": group.target_galaxy,
        "targetSystem": group.target_system,
        "targetPosition": group.target_position,
        "departureWindowStart": group.departure_window_start,
        "departureWindowEnd": group.departure_window_end,
        "notes": group.notes,
        "memberCount": group.member_count
    }))
}

async fn join_acs_group_handler(
    Extension(db): Extension<Option<Database>>,
    Extension(app_state): Extension<AppState>,
    Path(id): Path<i64>,
    Json(payload): Json<JoinAcsGroupRequest>,
) -> Response {
    let planet_id = payload.planet_id.unwrap_or(0);
    if id <= 0 || planet_id <= 0 {
        return bad_request("Invalid ACS join request");
    }
    let joined = if let Some(database) = db {
        database
            .join_acs_group(id, planet_id)
            .await
            .unwrap_or_else(|_| app_state.join_acs_group(id, planet_id).is_ok())
    } else {
        app_state.join_acs_group(id, planet_id).is_ok()
    };
    if !joined {
        return bad_request("Invalid ACS join request");
    }

    success(serde_json::json!({
        "groupId": id,
        "planetId": planet_id,
        "joined": true
    }))
}

async fn leave_acs_group_handler(
    Extension(db): Extension<Option<Database>>,
    Extension(app_state): Extension<AppState>,
    Path(id): Path<i64>,
) -> Response {
    if id <= 0 {
        return bad_request("Invalid ACS leave request");
    }
    let left = if let Some(database) = db {
        database
            .leave_acs_group(id)
            .await
            .unwrap_or_else(|_| app_state.leave_acs_group(id).is_ok())
    } else {
        app_state.leave_acs_group(id).is_ok()
    };
    if !left {
        return bad_request("Invalid ACS leave request");
    }

    success(serde_json::json!({
        "groupId": id,
        "left": true
    }))
}

fn acs_group_from_row(entry: AcsGroupRow) -> AcsGroup {
    AcsGroup {
        id: entry.id,
        mission_type: entry.mission_type,
        target_galaxy: entry.target_galaxy,
        target_system: entry.target_system,
        target_position: entry.target_position,
        member_count: entry.member_count,
        departure_window_start: entry.departure_window_start,
        departure_window_end: entry.departure_window_end,
        notes: entry.notes,
    }
}

fn acs_group_from_snapshot(entry: AcsGroupSnapshot) -> AcsGroup {
    AcsGroup {
        id: entry.id,
        mission_type: entry.mission_type,
        target_galaxy: entry.target_galaxy,
        target_system: entry.target_system,
        target_position: entry.target_position,
        member_count: entry.member_count,
        departure_window_start: entry.departure_window_start,
        departure_window_end: entry.departure_window_end,
        notes: entry.notes,
    }
}
