use axum::extract::{rejection::JsonRejection, Path};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use serde::{Deserialize, Serialize};

use crate::auth_guard::BearerToken;
use crate::response::{bad_request, success};
use crate::state::AppState;

const PLANET_VISUAL_VERSION: &str = "game-planet-visuals@0.1.0";
const NEW_TERRA_VISUAL_SEED: u64 = 0x5EED_1208_0001;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PlanetPayload {
    id: String,
    name: String,
    galaxy: i32,
    system: i32,
    position: i32,
    metal: i64,
    crystal: i64,
    deuterium: i64,
    icon_url: String,
    banner_url: String,
    visual_seed: u64,
    visual_version: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlanetResources {
    planet_id: String,
    production_per_hour: ResourceTriplet,
    storage_cap: ResourceTriplet,
}

#[derive(Debug, Serialize)]
struct ResourceTriplet {
    metal: i64,
    crystal: i64,
    deuterium: i64,
}

#[derive(Debug, Deserialize)]
struct RenamePlanetRequest {
    name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RenamePlanetPayload {
    planet_id: String,
    old_name: String,
    new_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BuildPlanetRequest {
    #[serde(alias = "buildingType")]
    building_type: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BuildPlanetResponse {
    queue_id: String,
    planet_id: String,
    building_type: String,
    level_target: i32,
    finishes_in_seconds: i64,
    queued: bool,
}

pub fn router() -> Router {
    Router::new()
        .route("/api/planets", get(list_planets_handler))
        .route("/api/planets/:planet_id", get(get_planet_handler))
        .route(
            "/api/planets/:planet_id/resources",
            get(get_planet_resources_handler),
        )
        .route(
            "/api/planets/:planet_id/rename",
            post(rename_planet_handler),
        )
}

pub fn protected_router() -> Router {
    Router::new().route("/api/planets/:planet_id/build", post(build_planet_handler))
}

async fn list_planets_handler() -> Response {
    success(default_planets())
}

async fn get_planet_handler(Path(planet_id): Path<String>) -> Response {
    let planets = default_planets();
    if let Some(planet) = planets.into_iter().find(|planet| planet.id == planet_id) {
        success(planet)
    } else {
        bad_request("Planet not found")
    }
}

async fn get_planet_resources_handler(Path(planet_id): Path<String>) -> Response {
    let Some(planet) = default_planets()
        .into_iter()
        .find(|planet| planet.id == planet_id)
    else {
        return bad_request("Planet not found");
    };

    success(PlanetResources {
        planet_id: planet.id,
        production_per_hour: ResourceTriplet {
            metal: 2100,
            crystal: 1300,
            deuterium: 620,
        },
        storage_cap: ResourceTriplet {
            metal: 120_000,
            crystal: 80_000,
            deuterium: 55_000,
        },
    })
}

async fn rename_planet_handler(
    Path(planet_id): Path<String>,
    payload: Result<Json<RenamePlanetRequest>, JsonRejection>,
) -> Response {
    let Json(input) = match payload {
        Ok(value) => value,
        Err(_) => return bad_request("Invalid rename payload"),
    };
    if input.name.trim().is_empty() {
        return bad_request("Planet name is required");
    }

    let Some(planet) = default_planets()
        .into_iter()
        .find(|planet| planet.id == planet_id)
    else {
        return bad_request("Planet not found");
    };

    success(RenamePlanetPayload {
        planet_id,
        old_name: planet.name,
        new_name: input.name.trim().to_string(),
    })
}

async fn build_planet_handler(
    Path(planet_id): Path<String>,
    BearerToken(token): BearerToken,
    Extension(app_state): Extension<AppState>,
    payload: Result<Json<BuildPlanetRequest>, JsonRejection>,
) -> Response {
    let Json(input) = match payload {
        Ok(value) => value,
        Err(_) => return bad_request("Invalid build payload"),
    };
    if input.building_type.trim().is_empty() {
        return bad_request("Building type is required");
    }

    match app_state.enqueue_building_upgrade(&token, &planet_id, &input.building_type) {
        Ok(item) => success(BuildPlanetResponse {
            queue_id: item.queue_id,
            planet_id: item.planet_id,
            building_type: item.building_type,
            level_target: item.level_target,
            finishes_in_seconds: item.finishes_in_seconds,
            queued: true,
        }),
        Err(message) => bad_request(message),
    }
}

fn default_planets() -> Vec<PlanetPayload> {
    vec![
        PlanetPayload {
            id: "p-001".to_string(),
            name: "New Terra".to_string(),
            galaxy: 1,
            system: 120,
            position: 8,
            metal: 12_000,
            crystal: 8_500,
            deuterium: 2_300,
            icon_url: "/assets/planet-rust-prototype/new-terra-rust-480p-icon.png".to_string(),
            banner_url: "/assets/planet-rust-prototype/new-terra-rust-480p-overview-banner.png"
                .to_string(),
            visual_seed: NEW_TERRA_VISUAL_SEED,
            visual_version: PLANET_VISUAL_VERSION.to_string(),
        },
        PlanetPayload {
            id: "p-002".to_string(),
            name: "Helios".to_string(),
            galaxy: 1,
            system: 121,
            position: 4,
            metal: 9_400,
            crystal: 7_100,
            deuterium: 1_800,
            icon_url: fixture_icon_url(0x5EED_1214_0002),
            banner_url: fixture_banner_url(0x5EED_1214_0002),
            visual_seed: 0x5EED_1214_0002,
            visual_version: PLANET_VISUAL_VERSION.to_string(),
        },
    ]
}

fn fixture_icon_url(seed: u64) -> String {
    format!("/assets/planet-cache/fixtures/{seed}/planet-icon.png")
}

fn fixture_banner_url(seed: u64) -> String {
    format!("/assets/planet-cache/fixtures/{seed}/overview-banner.png")
}
