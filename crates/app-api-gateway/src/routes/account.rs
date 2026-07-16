use axum::response::Response;
use axum::routing::get;
use axum::{Extension, Router};
use serde::Serialize;

use crate::accounts::{AccountRepository, RepositoryError};
use crate::auth_guard::AuthUser;
use crate::auth_guard::BearerToken;
use crate::response::{internal_error, success, unauthorized};
use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountProfile {
    id: String,
    username: String,
    email: String,
    rank: i64,
    alliance_tag: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountResources {
    metal: i64,
    crystal: i64,
    deuterium: i64,
    dark_matter: i64,
}

pub fn router() -> Router {
    Router::new()
        .route("/api/account/profile", get(account_profile_handler))
        .route("/api/account/resources", get(account_resources_handler))
}

async fn account_profile_handler(
    Extension(accounts): Extension<AccountRepository>,
    AuthUser(user): AuthUser,
) -> Response {
    match accounts.find_by_id(&user.user_id).await {
        Ok(Some(account)) if !account.is_banned => success(AccountProfile {
            id: account.id,
            username: account.username,
            email: account.email,
            rank: 42,
            alliance_tag: "RUST".to_string(),
        }),
        Ok(_) => unauthorized("Authenticated account is unavailable"),
        Err(RepositoryError::Unavailable(_) | RepositoryError::Storage(_)) => {
            internal_error("Account service is unavailable")
        }
        Err(RepositoryError::Duplicate) => internal_error("Account service is unavailable"),
    }
}

async fn account_resources_handler(
    BearerToken(token): BearerToken,
    Extension(app_state): Extension<AppState>,
) -> Response {
    let resources = app_state.account_resources(&token);
    success(AccountResources {
        metal: resources.metal,
        crystal: resources.crystal,
        deuterium: resources.deuterium,
        dark_matter: resources.dark_matter,
    })
}
