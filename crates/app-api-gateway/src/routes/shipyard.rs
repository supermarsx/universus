use axum::extract::{rejection::JsonRejection, Path};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use platform_db::{Database, GameplayPlanetRow, GameplayResearchRow};
use serde::{Deserialize, Serialize};

use super::gameplay::{
    map_write_error, parse_ship, ship_api_id, ship_options, ship_quote, CatalogError,
    GatewayGameplayError,
};
use crate::auth_guard::AuthUser;
use crate::response::{bad_request, conflict, not_found, service_unavailable, success};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BuildQueueItem {
    order_id: String,
    ship_type: String,
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
    #[serde(alias = "shipType")]
    ship_type: String,
    #[serde(alias = "quantity")]
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
    energy_required: i64,
    total_build_time_seconds: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ShipyardBuildRequest {
    #[serde(alias = "planet_id")]
    planet_id: FlexiblePlanetId,
    #[serde(alias = "ship_type")]
    ship_type: String,
    #[serde(alias = "count")]
    quantity: i64,
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
struct ShipyardBuildResponse {
    order_id: String,
    planet_id: String,
    ship_type: String,
    quantity: i64,
    completes_in_seconds: i64,
    queued: bool,
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

pub fn protected_router() -> Router {
    Router::new().route("/api/shipyard/build", post(shipyard_build_handler))
}

async fn queue_handler(
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
        Ok(Some(_)) => {}
        Ok(None) => return not_found("Planet not found"),
        Err(_) => return repository_unavailable(),
    }
    match database
        .gameplay_shipyard_queue_for_user(&user.user_id)
        .await
    {
        Ok(queue) => success(
            queue
                .into_iter()
                .filter(|item| item.planet_id == planet_id)
                .map(|item| BuildQueueItem {
                    order_id: item.id,
                    ship_type: parse_ship(&item.item_type)
                        .map(ship_api_id)
                        .unwrap_or("unknown")
                        .to_string(),
                    count: item.quantity.unwrap_or_default(),
                    completes_in_seconds: item.finishes_in_seconds,
                })
                .collect::<Vec<_>>(),
        ),
        Err(_) => repository_unavailable(),
    }
}

async fn build_options_handler(
    Path(planet_id): Path<String>,
    AuthUser(user): AuthUser,
    Extension(database): Extension<Option<Database>>,
) -> Response {
    let Some(database) = database else {
        return repository_unavailable();
    };
    let (planet, research, speed) = match load_state(&database, &user.user_id, &planet_id).await {
        Ok(state) => state,
        Err(response) => return response,
    };
    match ship_options(&planet, &research, speed) {
        Ok(options) => success(
            options
                .into_iter()
                .map(|option| BuildOption {
                    ship_type: ship_api_id(option.ship_type),
                    metal: option.metal,
                    crystal: option.crystal,
                    deuterium: option.deuterium,
                    build_time_seconds: option.build_time_seconds,
                })
                .collect::<Vec<_>>(),
        ),
        Err(error) => catalog_error(error),
    }
}

async fn build_preview_handler(
    Path(planet_id): Path<String>,
    AuthUser(user): AuthUser,
    Extension(database): Extension<Option<Database>>,
    payload: Result<Json<BuildPreviewRequest>, JsonRejection>,
) -> Response {
    let Json(input) = match payload {
        Ok(value) => value,
        Err(_) => return bad_request("Invalid build preview payload"),
    };
    if input.count <= 0 {
        return bad_request("Count must be greater than zero");
    }
    if parse_ship(&input.ship_type).is_none() {
        return bad_request("Ship type not found");
    }
    let Some(database) = database else {
        return repository_unavailable();
    };
    let (planet, research, speed) = match load_state(&database, &user.user_id, &planet_id).await {
        Ok(state) => state,
        Err(response) => return response,
    };
    match ship_quote(
        &user.user_id,
        &planet,
        &research,
        &input.ship_type,
        input.count,
        speed,
    ) {
        Ok(quote) => success(BuildPreview {
            planet_id,
            ship_type: quote.api_id.to_string(),
            count: input.count,
            total_metal: quote.input.metal_cost,
            total_crystal: quote.input.crystal_cost,
            total_deuterium: quote.input.deuterium_cost,
            energy_required: quote.input.energy_required,
            total_build_time_seconds: quote.input.duration_seconds,
        }),
        Err(error) => catalog_error(error),
    }
}

async fn shipyard_build_handler(
    AuthUser(user): AuthUser,
    Extension(database): Extension<Option<Database>>,
    payload: Result<Json<ShipyardBuildRequest>, JsonRejection>,
) -> Response {
    let Json(input) = match payload {
        Ok(value) => value,
        Err(_) => return bad_request("Invalid shipyard build payload"),
    };
    let planet_id = input.planet_id.into_planet_id();
    if planet_id.trim().is_empty() {
        return bad_request("Planet id is required");
    }
    if input.quantity <= 0 {
        return bad_request("Quantity must be greater than zero");
    }
    if input.ship_type.trim().is_empty() {
        return bad_request("Ship type is required");
    }
    if parse_ship(&input.ship_type).is_none() {
        return bad_request("Ship type not found");
    }
    let Some(database) = database else {
        return repository_unavailable();
    };
    let (planet, research, speed) = match load_state(&database, &user.user_id, &planet_id).await {
        Ok(state) => state,
        Err(response) => return response,
    };
    let quote = match ship_quote(
        &user.user_id,
        &planet,
        &research,
        &input.ship_type,
        input.quantity,
        speed,
    ) {
        Ok(quote) => quote,
        Err(error) => return catalog_error(error),
    };
    match database.gameplay_enqueue_ships(&quote.input).await {
        Ok(item) => success(ShipyardBuildResponse {
            order_id: item.id,
            planet_id: item.planet_id,
            ship_type: quote.api_id.to_string(),
            quantity: item.quantity.unwrap_or_default(),
            completes_in_seconds: item.finishes_in_seconds,
            queued: true,
        }),
        Err(error) => gameplay_error(map_write_error(error), "Planet not found"),
    }
}

async fn load_state(
    database: &Database,
    user_id: &str,
    planet_id: &str,
) -> Result<(GameplayPlanetRow, GameplayResearchRow, i32), Response> {
    let planet = database
        .gameplay_planet_for_user(user_id, planet_id)
        .await
        .map_err(|_| repository_unavailable())?
        .ok_or_else(|| not_found("Planet not found"))?;
    let research = database
        .gameplay_research_for_user(user_id)
        .await
        .map_err(|_| repository_unavailable())?
        .ok_or_else(|| service_unavailable("Gameplay state is unavailable"))?;
    let speed = database
        .gameplay_universe_speed(planet.universe_id)
        .await
        .map_err(|_| repository_unavailable())?
        .ok_or_else(repository_unavailable)?;
    Ok((planet, research, speed))
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
