use axum::response::Response;
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;

use crate::response::{bad_request, success};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DestroyMoonRequest {
    source_moon_id: Option<i64>,
    target_moon_id: Option<i64>,
    num_deathstars: Option<i32>,
    speed_percent: Option<f64>,
}

pub fn router() -> Router {
    Router::new().route("/api/rips/destroyMoon", post(destroy_moon_handler))
}

async fn destroy_moon_handler(Json(payload): Json<DestroyMoonRequest>) -> Response {
    let source_moon_id = payload.source_moon_id.unwrap_or(0);
    let target_moon_id = payload.target_moon_id.unwrap_or(0);
    let num_deathstars = payload.num_deathstars.unwrap_or(0);
    let speed_percent = payload
        .speed_percent
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(100.0);

    if source_moon_id <= 0 || target_moon_id <= 0 || num_deathstars < 1 {
        return bad_request("Invalid destroy moon request");
    }

    success(serde_json::json!({
        "missionId": "rip-destroy-001",
        "sourceMoonId": source_moon_id,
        "targetMoonId": target_moon_id,
        "numDeathstars": num_deathstars,
        "speedPercent": speed_percent,
        "accepted": true,
        "etaSeconds": 5400
    }))
}
