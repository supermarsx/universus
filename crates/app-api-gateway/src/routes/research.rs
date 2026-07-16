use axum::extract::{rejection::JsonRejection, Path};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use platform_db::{Database, GameplayPlanetRow, GameplayResearchRow};
use serde::{Deserialize, Serialize};

use super::gameplay::{
    map_write_error, parse_research, research_api_id, research_name, research_quote, CatalogError,
    GatewayGameplayError, RESEARCH,
};
use crate::auth_guard::AuthUser;
use crate::response::{bad_request, conflict, not_found, service_unavailable, success};

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
    queue_id: String,
    planet_id: String,
    tech_id: String,
    level_target: i32,
    finishes_in_seconds: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResearchCost {
    planet_id: String,
    tech_id: String,
    next_level: i32,
    metal: i64,
    crystal: i64,
    deuterium: i64,
    energy_required: i64,
    time_seconds: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResearchStartRequest {
    #[serde(alias = "technology_type", alias = "techId")]
    technology_type: String,
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

async fn list_research_handler(
    AuthUser(user): AuthUser,
    Extension(database): Extension<Option<Database>>,
) -> Response {
    let Some(database) = database else {
        return repository_unavailable();
    };
    match database.gameplay_research_for_user(&user.user_id).await {
        Ok(Some(research)) => success(
            RESEARCH
                .into_iter()
                .map(|technology| ResearchLevel {
                    tech_id: research_api_id(technology),
                    name: research_name(technology),
                    level: super::gameplay::research_level(&research, technology),
                })
                .collect::<Vec<_>>(),
        ),
        Ok(None) => service_unavailable("Gameplay state is unavailable"),
        Err(_) => repository_unavailable(),
    }
}

async fn research_queue_handler(
    AuthUser(user): AuthUser,
    Extension(database): Extension<Option<Database>>,
) -> Response {
    let Some(database) = database else {
        return repository_unavailable();
    };
    match database
        .gameplay_research_queue_for_user(&user.user_id)
        .await
    {
        Ok(queue) => success(
            queue
                .into_iter()
                .map(|item| ResearchQueueItem {
                    queue_id: item.id,
                    planet_id: item.planet_id,
                    tech_id: parse_research(&item.item_type)
                        .map(research_api_id)
                        .unwrap_or("unknown")
                        .to_string(),
                    level_target: item.target_level.unwrap_or_default(),
                    finishes_in_seconds: item.finishes_in_seconds,
                })
                .collect::<Vec<_>>(),
        ),
        Err(_) => repository_unavailable(),
    }
}

async fn research_cost_handler(
    Path(tech_id): Path<String>,
    AuthUser(user): AuthUser,
    Extension(database): Extension<Option<Database>>,
) -> Response {
    if parse_research(&tech_id).is_none() {
        return bad_request("Research technology not found");
    }
    let Some(database) = database else {
        return repository_unavailable();
    };
    let (planet, research) = match load_best_research_planet(&database, &user.user_id).await {
        Ok(state) => state,
        Err(response) => return response,
    };
    let speed = match universe_speed(&database, planet.universe_id).await {
        Ok(speed) => speed,
        Err(response) => return response,
    };
    match research_quote(&user.user_id, &planet, &research, &tech_id, speed) {
        Ok(quote) => success(ResearchCost {
            planet_id: planet.id,
            tech_id: quote.api_id.to_string(),
            next_level: quote.input.target_level.unwrap_or_default(),
            metal: quote.input.metal_cost,
            crystal: quote.input.crystal_cost,
            deuterium: quote.input.deuterium_cost,
            energy_required: quote.input.energy_required,
            time_seconds: quote.input.duration_seconds,
        }),
        Err(error) => catalog_error(error),
    }
}

async fn research_start_handler(
    AuthUser(user): AuthUser,
    Extension(database): Extension<Option<Database>>,
    payload: Result<Json<ResearchStartRequest>, JsonRejection>,
) -> Response {
    let Json(input) = match payload {
        Ok(value) => value,
        Err(_) => return bad_request("Invalid research start payload"),
    };
    if input.technology_type.trim().is_empty() {
        return bad_request("Technology type is required");
    }
    if parse_research(&input.technology_type).is_none() {
        return bad_request("Research technology not found");
    }
    let Some(database) = database else {
        return repository_unavailable();
    };
    // Research is account-global, so both quote and enqueue deliberately use
    // the same server-owned highest-lab planet. A client-supplied planet id is
    // ignored for backwards compatibility and can never select a faster lab
    // for display than the repository later charges against.
    let (planet, research) = match load_best_research_planet(&database, &user.user_id).await {
        Ok(state) => state,
        Err(response) => return response,
    };
    let speed = match universe_speed(&database, planet.universe_id).await {
        Ok(speed) => speed,
        Err(response) => return response,
    };
    let quote = match research_quote(
        &user.user_id,
        &planet,
        &research,
        &input.technology_type,
        speed,
    ) {
        Ok(quote) => quote,
        Err(error) => return catalog_error(error),
    };
    match database.gameplay_enqueue_research(&quote.input).await {
        Ok(item) => success(ResearchStartResponse {
            queue_id: item.id,
            planet_id: item.planet_id,
            technology_type: quote.api_id.to_string(),
            level_target: item.target_level.unwrap_or_default(),
            finishes_in_seconds: item.finishes_in_seconds,
            queued: true,
        }),
        Err(error) => gameplay_error(map_write_error(error), "Planet not found"),
    }
}

async fn load_best_research_planet(
    database: &Database,
    user_id: &str,
) -> Result<(GameplayPlanetRow, GameplayResearchRow), Response> {
    let mut planets = database
        .gameplay_planets_for_user(user_id)
        .await
        .map_err(|_| repository_unavailable())?;
    let research = database
        .gameplay_research_for_user(user_id)
        .await
        .map_err(|_| repository_unavailable())?
        .ok_or_else(|| service_unavailable("Gameplay state is unavailable"))?;
    planets.sort_by_key(|planet| {
        std::cmp::Reverse(
            planet
                .buildings
                .get("research_lab")
                .copied()
                .unwrap_or_default(),
        )
    });
    let planet = planets
        .into_iter()
        .next()
        .ok_or_else(|| not_found("A planet is required"))?;
    Ok((planet, research))
}

async fn universe_speed(database: &Database, universe_id: i64) -> Result<i32, Response> {
    database
        .gameplay_universe_speed(universe_id)
        .await
        .map_err(|_| repository_unavailable())?
        .ok_or_else(repository_unavailable)
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
