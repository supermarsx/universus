use axum::extract::{rejection::JsonRejection, Path};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use platform_db::{Database, GameplayPlanetRow};
use serde::{Deserialize, Serialize};

use super::gameplay::{
    building_quote, map_write_error, parse_building, CatalogError, GatewayGameplayError,
};
use crate::auth_guard::AuthUser;
use crate::response::{bad_request, conflict, not_found, service_unavailable, success};

const PLANET_VISUAL_VERSION: &str = "game-planet-visuals@0.1.0";

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
    production_breakdown: ProductionBreakdown,
    storage_cap: ResourceTriplet,
    energy: EnergyBreakdown,
    production_factor: f64,
    fusion_online: bool,
}

#[derive(Debug, Serialize)]
struct ResourceTriplet {
    metal: i64,
    crystal: i64,
    deuterium: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductionBreakdown {
    deuterium_gross_per_hour: i64,
    fusion_fuel_per_hour: i64,
}

#[derive(Debug, Serialize)]
struct EnergyBreakdown {
    supply: i64,
    demand: i64,
    net: i64,
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
    #[serde(alias = "building_type")]
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

async fn list_planets_handler(
    AuthUser(user): AuthUser,
    Extension(database): Extension<Option<Database>>,
) -> Response {
    let Some(database) = database else {
        return repository_unavailable();
    };
    match database.gameplay_planets_for_user(&user.user_id).await {
        Ok(planets) => success(planets.into_iter().map(planet_payload).collect::<Vec<_>>()),
        Err(_) => repository_unavailable(),
    }
}

async fn get_planet_handler(
    Path(planet_id): Path<String>,
    AuthUser(user): AuthUser,
    Extension(database): Extension<Option<Database>>,
) -> Response {
    let Some(database) = database else {
        return repository_unavailable();
    };
    match database
        .gameplay_planet_for_user(&user.user_id, &planet_id)
        .await
    {
        Ok(Some(planet)) => success(planet_payload(planet)),
        Ok(None) => not_found("Planet not found"),
        Err(_) => repository_unavailable(),
    }
}

async fn get_planet_resources_handler(
    Path(planet_id): Path<String>,
    AuthUser(user): AuthUser,
    Extension(database): Extension<Option<Database>>,
) -> Response {
    let Some(database) = database else {
        return repository_unavailable();
    };
    let projection = match database
        .gameplay_resource_projection_for_user(&user.user_id, &planet_id)
        .await
    {
        Ok(Some(projection)) => projection,
        Ok(None) => return not_found("Planet not found"),
        Err(_) => return repository_unavailable(),
    };
    success(PlanetResources {
        planet_id,
        production_per_hour: ResourceTriplet {
            metal: projection.metal_per_hour,
            crystal: projection.crystal_per_hour,
            deuterium: projection.deuterium_per_hour,
        },
        production_breakdown: ProductionBreakdown {
            deuterium_gross_per_hour: projection.deuterium_gross_per_hour,
            fusion_fuel_per_hour: projection.fusion_fuel_per_hour,
        },
        storage_cap: ResourceTriplet {
            metal: projection.metal_storage,
            crystal: projection.crystal_storage,
            deuterium: projection.deuterium_storage,
        },
        energy: EnergyBreakdown {
            supply: projection.energy_supply,
            demand: projection.energy_demand,
            net: projection.energy_net,
        },
        production_factor: projection.production_factor,
        fusion_online: projection.fusion_online,
    })
}

async fn rename_planet_handler(
    Path(planet_id): Path<String>,
    AuthUser(user): AuthUser,
    Extension(database): Extension<Option<Database>>,
    payload: Result<Json<RenamePlanetRequest>, JsonRejection>,
) -> Response {
    let Json(input) = match payload {
        Ok(value) => value,
        Err(_) => return bad_request("Invalid rename payload"),
    };
    let Some(database) = database else {
        return repository_unavailable();
    };
    match database
        .gameplay_rename_planet(&user.user_id, &planet_id, &input.name)
        .await
    {
        Ok((old_name, new_name)) => success(RenamePlanetPayload {
            planet_id,
            old_name,
            new_name,
        }),
        Err(error) => gameplay_error(map_write_error(error), "Planet not found"),
    }
}

async fn build_planet_handler(
    Path(planet_id): Path<String>,
    AuthUser(user): AuthUser,
    Extension(database): Extension<Option<Database>>,
    payload: Result<Json<BuildPlanetRequest>, JsonRejection>,
) -> Response {
    let Json(input) = match payload {
        Ok(value) => value,
        Err(_) => return bad_request("Invalid build payload"),
    };
    if input.building_type.trim().is_empty() {
        return bad_request("Building type is required");
    }
    if parse_building(&input.building_type).is_none() {
        return bad_request("Building type not found");
    }
    let Some(database) = database else {
        return repository_unavailable();
    };
    let planet = match database
        .gameplay_planet_for_user(&user.user_id, &planet_id)
        .await
    {
        Ok(Some(planet)) => planet,
        Ok(None) => return not_found("Planet not found"),
        Err(_) => return repository_unavailable(),
    };
    let research = match database.gameplay_research_for_user(&user.user_id).await {
        Ok(Some(research)) => research,
        Ok(None) => return service_unavailable("Gameplay state is unavailable"),
        Err(_) => return repository_unavailable(),
    };
    let speed = match database.gameplay_universe_speed(planet.universe_id).await {
        Ok(Some(speed)) => speed,
        _ => return repository_unavailable(),
    };
    let quote = match building_quote(
        &user.user_id,
        &planet,
        &research,
        &input.building_type,
        speed,
    ) {
        Ok(quote) => quote,
        Err(error) => return catalog_error(error),
    };
    match database.gameplay_enqueue_building(&quote.input).await {
        Ok(item) => success(BuildPlanetResponse {
            queue_id: item.id,
            planet_id: item.planet_id,
            building_type: quote.api_id.to_string(),
            level_target: item.target_level.unwrap_or_default(),
            finishes_in_seconds: item.finishes_in_seconds,
            queued: true,
        }),
        Err(error) => gameplay_error(map_write_error(error), "Planet not found"),
    }
}

fn planet_payload(planet: GameplayPlanetRow) -> PlanetPayload {
    let visual_seed = visual_seed(&planet);
    PlanetPayload {
        id: planet.id,
        name: planet.name,
        galaxy: planet.galaxy,
        system: planet.system,
        position: planet.position,
        metal: planet.metal,
        crystal: planet.crystal,
        deuterium: planet.deuterium,
        icon_url: format!("/assets/planet-cache/{visual_seed}/planet-icon.png"),
        banner_url: format!("/assets/planet-cache/{visual_seed}/overview-banner.png"),
        visual_seed,
        visual_version: PLANET_VISUAL_VERSION.to_string(),
    }
}

fn visual_seed(planet: &GameplayPlanetRow) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in format!(
        "{}:{}:{}:{}:{}",
        planet.universe_id, planet.id, planet.galaxy, planet.system, planet.position
    )
    .bytes()
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn catalog_error(error: CatalogError) -> Response {
    bad_request(&error.to_string())
}

fn gameplay_error(error: GatewayGameplayError, not_found_message: &str) -> Response {
    match error {
        GatewayGameplayError::BadRequest(message) => bad_request(&message),
        GatewayGameplayError::NotFound => not_found(not_found_message),
        GatewayGameplayError::Conflict(message) => conflict(&message),
        GatewayGameplayError::Unavailable => repository_unavailable(),
    }
}

fn repository_unavailable() -> Response {
    service_unavailable("Gameplay repository is unavailable")
}
