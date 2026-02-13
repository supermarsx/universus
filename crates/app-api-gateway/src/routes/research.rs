use axum::extract::{rejection::JsonRejection, Path};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use serde::{Deserialize, Serialize};

use crate::auth_guard::BearerToken;
use crate::response::{bad_request, success};
use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResearchLevel {
    tech_id: &'static str,
    name: &'static str,
    level: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResearchQueueItem {
    tech_id: &'static str,
    level_target: i32,
    finishes_in_seconds: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResearchCost {
    tech_id: String,
    next_level: i32,
    metal: i64,
    crystal: i64,
    deuterium: i64,
    time_seconds: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResearchStartRequest {
    #[serde(alias = "planetId")]
    planet_id: FlexiblePlanetId,
    #[serde(alias = "technologyType", alias = "techId")]
    technology_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum FlexiblePlanetId {
    String(String),
    Number(i64),
}

impl FlexiblePlanetId {
    fn into_planet_id(self) -> String {
        match self {
            Self::String(value) => value,
            Self::Number(value) => value.to_string(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResearchStartResponse {
    queue_id: String,
    planet_id: String,
    technology_type: String,
    level_target: i32,
    finishes_in_seconds: i64,
    queued: bool,
}

pub fn router() -> Router {
    Router::new()
        .route("/api/research", get(list_research_handler))
        .route("/api/research/queue", get(research_queue_handler))
        .route("/api/research/:tech_id/cost", post(research_cost_handler))
}

pub fn protected_router() -> Router {
    Router::new().route("/api/research/start", post(research_start_handler))
}

async fn list_research_handler() -> Response {
    success(vec![
        ResearchLevel {
            tech_id: "energy_tech",
            name: "Energy Technology",
            level: 11,
        },
        ResearchLevel {
            tech_id: "weapons_tech",
            name: "Weapons Technology",
            level: 9,
        },
    ])
}

async fn research_queue_handler() -> Response {
    success(vec![ResearchQueueItem {
        tech_id: "hyperspace_drive",
        level_target: 7,
        finishes_in_seconds: 14_400,
    }])
}

async fn research_cost_handler(Path(tech_id): Path<String>) -> Response {
    let (level, metal, crystal, deuterium, time_seconds) = match tech_id.as_str() {
        "energy_tech" => (12, 240_000, 120_000, 50_000, 5_400),
        "weapons_tech" => (10, 310_000, 155_000, 60_000, 7_200),
        "hyperspace_drive" => (7, 520_000, 390_000, 210_000, 14_400),
        _ => return bad_request("Research technology not found"),
    };

    success(ResearchCost {
        tech_id,
        next_level: level,
        metal,
        crystal,
        deuterium,
        time_seconds,
    })
}

async fn research_start_handler(
    BearerToken(token): BearerToken,
    Extension(app_state): Extension<AppState>,
    payload: Result<Json<ResearchStartRequest>, JsonRejection>,
) -> Response {
    let Json(input) = match payload {
        Ok(value) => value,
        Err(_) => return bad_request("Invalid research start payload"),
    };
    if input.technology_type.trim().is_empty() {
        return bad_request("Technology type is required");
    }

    let planet_id = input.planet_id.into_planet_id();
    if planet_id.trim().is_empty() {
        return bad_request("Planet id is required");
    }

    match app_state.enqueue_research(&token, &planet_id, &input.technology_type) {
        Ok(item) => success(ResearchStartResponse {
            queue_id: item.queue_id,
            planet_id,
            technology_type: item.tech_id,
            level_target: item.level_target,
            finishes_in_seconds: item.finishes_in_seconds,
            queued: true,
        }),
        Err(message) => bad_request(message),
    }
}
