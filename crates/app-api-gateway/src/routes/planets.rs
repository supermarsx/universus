use axum::extract::{Path, rejection::JsonRejection};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::response::{bad_request, success};

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

pub fn router() -> Router {
    Router::new()
        .route("/api/planets", get(list_planets_handler))
        .route("/api/planets/:planet_id", get(get_planet_handler))
        .route(
            "/api/planets/:planet_id/resources",
            get(get_planet_resources_handler),
        )
        .route("/api/planets/:planet_id/rename", post(rename_planet_handler))
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
    let Some(planet) = default_planets().into_iter().find(|planet| planet.id == planet_id) else {
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

    let Some(planet) = default_planets().into_iter().find(|planet| planet.id == planet_id) else {
        return bad_request("Planet not found");
    };

    success(RenamePlanetPayload {
        planet_id,
        old_name: planet.name,
        new_name: input.name.trim().to_string(),
    })
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
        },
    ]
}
