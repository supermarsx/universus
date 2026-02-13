use axum::extract::Path;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use serde::Serialize;

use crate::response::success;

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
    occupant: &'static str,
    status: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SystemView {
    galaxy: i32,
    system: i32,
    slots: Vec<SystemSlot>,
}

pub fn router() -> Router {
    Router::new()
        .route("/api/galaxy", get(galaxy_overview_handler))
        .route("/api/galaxy/:galaxy/:system", get(system_view_handler))
        .route("/api/galaxy/:galaxy/:system/:position", get(position_view_handler))
}

async fn galaxy_overview_handler() -> Response {
    success(vec![
        GalaxyOverview {
            galaxy: 1,
            systems: 499,
            active_players: 187,
        },
        GalaxyOverview {
            galaxy: 2,
            systems: 499,
            active_players: 142,
        },
    ])
}

async fn system_view_handler(Path((galaxy, system)): Path<(i32, i32)>) -> Response {
    success(SystemView {
        galaxy,
        system,
        slots: vec![
            SystemSlot {
                position: 4,
                occupant: "Helios",
                status: "active",
            },
            SystemSlot {
                position: 8,
                occupant: "New Terra",
                status: "active",
            },
        ],
    })
}

async fn position_view_handler(Path((galaxy, system, position)): Path<(i32, i32, i32)>) -> Response {
    success(SystemSlot {
        position,
        occupant: if galaxy == 1 && system == 120 && position == 8 {
            "New Terra"
        } else {
            "Unoccupied"
        },
        status: if galaxy == 1 && system == 120 && position == 8 {
            "active"
        } else {
            "empty"
        },
    })
}
