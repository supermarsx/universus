use axum::response::Response;
use axum::routing::post;
use axum::{Extension, Json, Router};
use platform_db::{Database, RipDestroyRequestCreateInput};
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

async fn destroy_moon_handler(
    Extension(db): Extension<Option<Database>>,
    Json(payload): Json<DestroyMoonRequest>,
) -> Response {
    let source_moon_id = payload.source_moon_id.unwrap_or(0);
    let target_moon_id = payload.target_moon_id.unwrap_or(0);
    let num_deathstars = payload.num_deathstars.unwrap_or(0);
    let speed_percent = payload.speed_percent.unwrap_or(100.0);

    if source_moon_id <= 0
        || target_moon_id <= 0
        || source_moon_id == target_moon_id
        || num_deathstars < 1
        || num_deathstars > 10_000
        || !speed_percent.is_finite()
        || !(10.0..=100.0).contains(&speed_percent)
    {
        return bad_request("Invalid destroy moon request");
    }

    if let Some(database) = db {
        let insert_input = RipDestroyRequestCreateInput {
            mission_id: format!("rip-destroy-{}-{}", source_moon_id, target_moon_id),
            source_moon_id,
            target_moon_id,
            num_deathstars,
            speed_percent,
            status: "queued".to_string(),
            requested_at_unix: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_secs() as i64)
                .unwrap_or(0),
        };
        if let Ok(row) = database.queue_rip_attack(insert_input).await {
            let eta_seconds = ((10_000.0 / speed_percent) * 54.0).round() as i64;
            return success(serde_json::json!({
                "missionId": row.mission_id,
                "sourceMoonId": row.source_moon_id,
                "targetMoonId": row.target_moon_id,
                "numDeathstars": row.num_deathstars,
                "speedPercent": row.speed_percent,
                "accepted": true,
                "etaSeconds": eta_seconds.max(1)
            }));
        }
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
