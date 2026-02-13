use axum::extract::{Path, rejection::JsonRejection};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::response::{bad_request, success};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BuildQueueItem {
    order_id: &'static str,
    ship_type: &'static str,
    count: i64,
    completes_in_seconds: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BuildOption {
    ship_type: &'static str,
    metal: i64,
    crystal: i64,
    deuterium: i64,
    build_time_seconds: i64,
}

#[derive(Debug, Deserialize)]
struct BuildPreviewRequest {
    ship_type: String,
    count: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BuildPreview {
    planet_id: String,
    ship_type: String,
    count: i64,
    total_metal: i64,
    total_crystal: i64,
    total_deuterium: i64,
    total_build_time_seconds: i64,
}

pub fn router() -> Router {
    Router::new()
        .route("/api/shipyard/:planet_id/queue", get(queue_handler))
        .route(
            "/api/shipyard/:planet_id/build-options",
            get(build_options_handler),
        )
        .route(
            "/api/shipyard/:planet_id/build-preview",
            post(build_preview_handler),
        )
}

async fn queue_handler(Path(_planet_id): Path<String>) -> Response {
    success(vec![
        BuildQueueItem {
            order_id: "o-201",
            ship_type: "lightFighter",
            count: 25,
            completes_in_seconds: 1800,
        },
        BuildQueueItem {
            order_id: "o-202",
            ship_type: "smallCargo",
            count: 10,
            completes_in_seconds: 3200,
        },
    ])
}

async fn build_options_handler(Path(_planet_id): Path<String>) -> Response {
    success(vec![
        BuildOption {
            ship_type: "lightFighter",
            metal: 3_000,
            crystal: 1_000,
            deuterium: 0,
            build_time_seconds: 45,
        },
        BuildOption {
            ship_type: "smallCargo",
            metal: 2_000,
            crystal: 2_000,
            deuterium: 0,
            build_time_seconds: 60,
        },
    ])
}

async fn build_preview_handler(
    Path(planet_id): Path<String>,
    payload: Result<Json<BuildPreviewRequest>, JsonRejection>,
) -> Response {
    let Json(input) = match payload {
        Ok(value) => value,
        Err(_) => return bad_request("Invalid build preview payload"),
    };
    if input.count <= 0 {
        return bad_request("Count must be greater than zero");
    }

    let (metal, crystal, deuterium, build_time) = match input.ship_type.as_str() {
        "lightFighter" => (3_000, 1_000, 0, 45),
        "smallCargo" => (2_000, 2_000, 0, 60),
        _ => return bad_request("Ship type not found"),
    };

    success(BuildPreview {
        planet_id,
        ship_type: input.ship_type,
        count: input.count,
        total_metal: metal * input.count,
        total_crystal: crystal * input.count,
        total_deuterium: deuterium * input.count,
        total_build_time_seconds: build_time * input.count,
    })
}
