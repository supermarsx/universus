use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Extension, Json, Router};
use serde::Serialize;

use crate::auth_guard::BearerToken;
use crate::state::AppState;

#[derive(Debug, Serialize)]
struct UserIdentity {
    id: i64,
    username: &'static str,
    email: &'static str,
    role: &'static str,
    is_admin: bool,
    dark_matter: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MeResponse {
    id: i64,
    username: &'static str,
    email: &'static str,
    role: &'static str,
    is_admin: bool,
    dark_matter: i64,
    is_admin_user: bool,
    dark_matter_balance: i64,
    user: UserIdentity,
    research: UserResearchLevels,
    data: UserIdentity,
}

#[derive(Debug, Serialize)]
struct UserResearchLevels {
    energy_technology: i64,
    weapons_technology: i64,
    shielding_technology: i64,
    combustion_drive: i64,
}

#[derive(Debug, Serialize)]
struct LeaderboardEntry {
    id: i64,
    username: &'static str,
    total_score: i64,
    economy_score: i64,
    research_score: i64,
    military_score: i64,
    total_score_value: i64,
    economy_score_value: i64,
    research_score_value: i64,
    military_score_value: i64,
}

pub fn router() -> Router {
    Router::new()
        .route("/api/users/me", get(me_handler))
        .route("/api/users/leaderboard", get(leaderboard_handler))
}

async fn me_handler(
    BearerToken(token): BearerToken,
    Extension(app_state): Extension<AppState>,
) -> Response {
    let resources = app_state.account_resources(&token);
    let user = UserIdentity {
        id: 1,
        username: "Commander",
        email: "commander@example.com",
        role: "player",
        is_admin: false,
        dark_matter: resources.dark_matter,
    };

    Json(MeResponse {
        id: user.id,
        username: user.username,
        email: user.email,
        role: user.role,
        is_admin: user.is_admin,
        dark_matter: user.dark_matter,
        is_admin_user: user.is_admin,
        dark_matter_balance: user.dark_matter,
        user: UserIdentity { ..user },
        research: UserResearchLevels {
            energy_technology: 12,
            weapons_technology: 9,
            shielding_technology: 8,
            combustion_drive: 10,
        },
        data: user,
    })
    .into_response()
}

async fn leaderboard_handler() -> Response {
    Json(vec![
        leaderboard_entry(1, "AdmiralNova", 8_400_000, 3_150_000, 1_600_000, 3_650_000),
        leaderboard_entry(2, "Commander", 6_200_000, 2_400_000, 1_300_000, 2_500_000),
    ])
    .into_response()
}

fn leaderboard_entry(
    id: i64,
    username: &'static str,
    total_score: i64,
    economy_score: i64,
    research_score: i64,
    military_score: i64,
) -> LeaderboardEntry {
    LeaderboardEntry {
        id,
        username,
        total_score,
        economy_score,
        research_score,
        military_score,
        total_score_value: total_score,
        economy_score_value: economy_score,
        research_score_value: research_score,
        military_score_value: military_score,
    }
}
