use axum::extract::Path;
use axum::response::Response;
use axum::routing::get;
use axum::{Extension, Router};
use serde::Serialize;

use crate::response::{bad_request, success};
use crate::state::AppState;

// ---------------------------------------------------------------------------
// API response types (camelCase for JSON consumers)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GalaxyOverview {
    galaxy: i32,
    systems: i32,
    active_players: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SystemSlot {
    position: i32,
    occupant: String,
    status: String,
    planet_name: Option<String>,
    moon_id: Option<i64>,
    debris_metal: i64,
    debris_crystal: i64,
    alliance_tag: Option<String>,
    is_inactive: bool,
    is_vacation: bool,
    is_banned: bool,
    icon_url: Option<String>,
    visual_seed: Option<u64>,
    visual_version: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SystemView {
    galaxy: i32,
    system: i32,
    slots: Vec<SystemSlot>,
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router {
    Router::new()
        .route("/api/galaxy", get(galaxy_overview_handler))
        .route("/api/galaxy/:galaxy/:system", get(system_view_handler))
        .route(
            "/api/galaxy/:galaxy/:system/:position",
            get(position_view_handler),
        )
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn galaxy_overview_handler(Extension(state): Extension<AppState>) -> Response {
    let snapshots = state.galaxy_overview();
    let result: Vec<GalaxyOverview> = snapshots
        .into_iter()
        .map(|s| GalaxyOverview {
            galaxy: s.galaxy,
            systems: s.systems,
            active_players: s.active_players,
        })
        .collect();
    success(result)
}

async fn system_view_handler(
    Extension(state): Extension<AppState>,
    Path((galaxy, system)): Path<(i32, i32)>,
) -> Response {
    match state.galaxy_system_view(galaxy, system) {
        Ok(view) => {
            let slots = view
                .slots
                .into_iter()
                .map(system_slot_from_snapshot)
                .collect();
            success(SystemView {
                galaxy: view.galaxy,
                system: view.system,
                slots,
            })
        }
        Err(msg) => bad_request(&msg),
    }
}

async fn position_view_handler(
    Extension(state): Extension<AppState>,
    Path((galaxy, system, position)): Path<(i32, i32, i32)>,
) -> Response {
    match state.galaxy_position(galaxy, system, position) {
        Ok(s) => success(system_slot_from_snapshot(s)),
        Err(msg) => bad_request(&msg),
    }
}

fn system_slot_from_snapshot(s: crate::state::GalaxySlotSnapshot) -> SystemSlot {
    SystemSlot {
        position: s.position,
        occupant: s.occupant,
        status: s.status,
        planet_name: s.planet_name,
        moon_id: s.moon_id,
        debris_metal: s.debris_metal,
        debris_crystal: s.debris_crystal,
        alliance_tag: s.alliance_tag,
        is_inactive: s.is_inactive,
        is_vacation: s.is_vacation,
        is_banned: s.is_banned,
        icon_url: s.icon_url,
        visual_seed: s.visual_seed,
        visual_version: s.visual_version,
    }
}
