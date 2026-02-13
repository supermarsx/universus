use axum::extract::Path;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::response::success;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DebrisField {
    id: i64,
    galaxy: i32,
    system: i32,
    position: i32,
    metal: i64,
    crystal: i64,
    deuterium: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DebrisSearchRequest {
    galaxy: Option<i32>,
    system: Option<i32>,
}

pub fn router() -> Router {
    Router::new()
        .route("/api/debris", get(list_debris_handler))
        .route("/api/debris/search", post(search_debris_handler))
        .route(
            "/api/debris/location/:galaxy/:system/:position",
            get(location_debris_handler),
        )
}

async fn list_debris_handler() -> Response {
    success(vec![
        sample_debris(11, 1, 120, 7),
        sample_debris(12, 1, 121, 5),
    ])
}

async fn search_debris_handler(Json(payload): Json<DebrisSearchRequest>) -> Response {
    let mut data = vec![sample_debris(11, 1, 120, 7), sample_debris(12, 1, 121, 5)];
    if let Some(galaxy) = payload.galaxy {
        data.retain(|field| field.galaxy == galaxy);
    }
    if let Some(system) = payload.system {
        data.retain(|field| field.system == system);
    }
    success(data)
}

async fn location_debris_handler(
    Path((galaxy, system, position)): Path<(i32, i32, i32)>,
) -> Response {
    success(vec![sample_debris(21, galaxy, system, position)])
}

fn sample_debris(id: i64, galaxy: i32, system: i32, position: i32) -> DebrisField {
    DebrisField {
        id,
        galaxy,
        system,
        position,
        metal: 25_000,
        crystal: 12_000,
        deuterium: 4_000,
    }
}
