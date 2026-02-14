use std::sync::{Mutex, OnceLock};

use axum::extract::{Path, Query};
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;
use game_achievements::{default_catalog, AchievementStore, Catalog};
use serde::Deserialize;

use crate::auth_guard::BearerToken;
use crate::response::{bad_request, success};

fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(default_catalog)
}

fn store() -> &'static Mutex<AchievementStore> {
    static STORE: OnceLock<Mutex<AchievementStore>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(AchievementStore::default()))
}

#[derive(Debug, Deserialize)]
struct HallOfFameQuery {
    limit: Option<usize>,
}

pub fn router() -> Router {
    Router::new()
        .route("/api/achievements", get(list_achievements_handler))
        .route("/api/achievements/badges", get(list_badges_handler))
        .route("/api/achievements/rewards", get(list_rewards_handler))
        .route("/api/achievements/ladders", get(list_ladders_handler))
        .route("/api/achievements/hall-of-fame", get(hall_of_fame_handler))
}

pub fn protected_router() -> Router {
    Router::new()
        .route(
            "/api/achievements/user/:user_id/achievements",
            get(list_user_achievements_handler),
        )
        .route("/api/achievements/user/:user_id/badges", get(list_user_badges_handler))
        .route(
            "/api/achievements/user/:user_id/rewards",
            get(list_user_rewards_handler),
        )
        .route(
            "/api/achievements/user/:user_id/achievements/:achievement_id",
            post(award_achievement_handler),
        )
        .route(
            "/api/achievements/user/:user_id/badges/:badge_id",
            post(award_badge_handler),
        )
        .route(
            "/api/achievements/user/:user_id/rewards/:reward_id",
            post(award_reward_handler),
        )
}

async fn list_achievements_handler() -> Response {
    success(catalog().achievements.clone())
}

async fn list_badges_handler() -> Response {
    success(catalog().badges.clone())
}

async fn list_rewards_handler() -> Response {
    success(catalog().rewards.clone())
}

async fn list_ladders_handler() -> Response {
    success(catalog().ladders.clone())
}

async fn hall_of_fame_handler(Query(query): Query<HallOfFameQuery>) -> Response {
    let limit = query.limit.unwrap_or(100).max(1);
    let mut entries = catalog().hall_of_fame.clone();
    entries.truncate(limit);
    success(entries)
}

async fn list_user_achievements_handler(
    BearerToken(_token): BearerToken,
    Path(user_id): Path<i64>,
) -> Response {
    if user_id <= 0 {
        return bad_request("Invalid user id");
    }
    let state = store().lock().expect("achievements store poisoned");
    success(state.list_user_achievements(catalog(), user_id))
}

async fn list_user_badges_handler(BearerToken(_token): BearerToken, Path(user_id): Path<i64>) -> Response {
    if user_id <= 0 {
        return bad_request("Invalid user id");
    }
    let state = store().lock().expect("achievements store poisoned");
    success(state.list_user_badges(catalog(), user_id))
}

async fn list_user_rewards_handler(
    BearerToken(_token): BearerToken,
    Path(user_id): Path<i64>,
) -> Response {
    if user_id <= 0 {
        return bad_request("Invalid user id");
    }
    let state = store().lock().expect("achievements store poisoned");
    success(state.list_user_rewards(catalog(), user_id))
}

async fn award_achievement_handler(
    BearerToken(_token): BearerToken,
    Path((user_id, achievement_id)): Path<(i64, i64)>,
) -> Response {
    if user_id <= 0 || achievement_id <= 0 {
        return bad_request("Invalid user or achievement id");
    }
    let mut state = store().lock().expect("achievements store poisoned");
    if !state.award_achievement(catalog(), user_id, achievement_id) {
        return bad_request("Achievement not found");
    }
    success(serde_json::json!({ "success": true }))
}

async fn award_badge_handler(
    BearerToken(_token): BearerToken,
    Path((user_id, badge_id)): Path<(i64, i64)>,
) -> Response {
    if user_id <= 0 || badge_id <= 0 {
        return bad_request("Invalid user or badge id");
    }
    let mut state = store().lock().expect("achievements store poisoned");
    if !state.award_badge(catalog(), user_id, badge_id) {
        return bad_request("Badge not found");
    }
    success(serde_json::json!({ "success": true }))
}

async fn award_reward_handler(
    BearerToken(_token): BearerToken,
    Path((user_id, reward_id)): Path<(i64, i64)>,
) -> Response {
    if user_id <= 0 || reward_id <= 0 {
        return bad_request("Invalid user or reward id");
    }
    let mut state = store().lock().expect("achievements store poisoned");
    if !state.award_reward(catalog(), user_id, reward_id) {
        return bad_request("Reward not found");
    }
    success(serde_json::json!({ "success": true }))
}
