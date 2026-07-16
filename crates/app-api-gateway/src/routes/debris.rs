use axum::extract::Path;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::auth_guard::AuthUser;
use crate::authorization::effective_numeric_user_id;
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerateDebrisRequest {
    galaxy: Option<i32>,
    system: Option<i32>,
    position: Option<i32>,
    seed: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClaimDebrisRequest {
    collector_id: Option<i64>,
}

pub fn router() -> Router {
    Router::new()
        .route("/api/debris", get(list_debris_handler))
        .route("/api/debris/system/stats", get(system_stats_handler))
        .route("/api/debris/claims/my", get(my_claims_handler))
        .route("/api/debris/generate", post(generate_debris_handler))
        .route("/api/debris/search", post(search_debris_handler))
        .route("/api/debris/:id/claim", post(claim_debris_handler))
        .route("/api/debris/:id", get(debris_by_id_handler))
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

async fn debris_by_id_handler(Path(id): Path<i64>) -> Response {
    success(sample_debris_from_id(id))
}

async fn generate_debris_handler(Json(payload): Json<GenerateDebrisRequest>) -> Response {
    let id = payload.seed.unwrap_or(1_001).abs().max(1);
    let generated = sample_debris(
        id,
        payload.galaxy.unwrap_or(1),
        payload.system.unwrap_or(120),
        payload.position.unwrap_or(7),
    );
    success(json!({
        "generated": true,
        "seed": payload.seed.unwrap_or(id),
        "field": generated
    }))
}

async fn system_stats_handler() -> Response {
    success(json!({
        "trackedFields": 2,
        "totalMetal": 50_000i64,
        "totalCrystal": 24_000i64,
        "totalDeuterium": 8_000i64,
        "claimableFields": 2
    }))
}

async fn claim_debris_handler(
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
    Json(payload): Json<ClaimDebrisRequest>,
) -> Response {
    let collector_id = match effective_numeric_user_id(&user, payload.collector_id) {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    success(json!({
        "claimId": id.saturating_mul(10).saturating_add(1),
        "debrisId": id,
        "collectorId": collector_id,
        "claimed": true,
        "resources": {
            "metal": 5_000i64,
            "crystal": 2_000i64,
            "deuterium": 750i64
        }
    }))
}

async fn my_claims_handler(AuthUser(user): AuthUser) -> Response {
    let collector_id = match effective_numeric_user_id(&user, None) {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    success(vec![
        json!({
            "claimId": 101,
            "debrisId": 11,
            "collectorId": collector_id,
            "claimedAtUnix": 1_700_000_001i64
        }),
        json!({
            "claimId": 102,
            "debrisId": 12,
            "collectorId": collector_id,
            "claimedAtUnix": 1_700_000_002i64
        }),
    ])
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

fn sample_debris_from_id(id: i64) -> DebrisField {
    let normalized = id.abs().max(1);
    let galaxy = ((normalized % 9) + 1) as i32;
    let system = 100 + (normalized % 400) as i32;
    let position = ((normalized % 15) + 1) as i32;
    sample_debris(normalized, galaxy, system, position)
}
