use axum::response::Response;
use axum::routing::get;
use axum::{Extension, Router};
use platform_db::Database;
use serde::Serialize;

use crate::accounts::{AccountRepository, RepositoryError};
use crate::auth_guard::AuthUser;
use crate::response::{internal_error, service_unavailable, success, unauthorized};

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
    Extension(database): Extension<Option<Database>>,
) -> Response {
    match accounts.find_by_id(&user.user_id).await {
        Ok(Some(account)) if !account.is_banned => {
            let (rank, alliance_tag) = match database {
                Some(database) => {
                    match database.gameplay_profile_meta_for_user(&user.user_id).await {
                        Ok(Some(meta)) => (meta.rank, meta.alliance_tag.unwrap_or_default()),
                        Ok(None) => return unauthorized("Authenticated account is unavailable"),
                        Err(_) => return repository_unavailable(),
                    }
                }
                None => (0, String::new()),
            };
            success(AccountProfile {
                id: account.id,
                username: account.username,
                email: account.email,
                rank,
                alliance_tag,
            })
        }
        Ok(_) => unauthorized("Authenticated account is unavailable"),
        Err(RepositoryError::Unavailable(_) | RepositoryError::Storage(_)) => {
            internal_error("Account service is unavailable")
        }
        Err(RepositoryError::Duplicate) => internal_error("Account service is unavailable"),
    }
}

async fn account_resources_handler(
    AuthUser(user): AuthUser,
    Extension(database): Extension<Option<Database>>,
) -> Response {
    let Some(database) = database else {
        return repository_unavailable();
    };
    match database.gameplay_account_resources(&user.user_id).await {
        Ok(Some(resources)) => success(AccountResources {
            metal: resources.metal,
            crystal: resources.crystal,
            deuterium: resources.deuterium,
            dark_matter: resources.dark_matter,
        }),
        Ok(None) => unauthorized("Authenticated account is unavailable"),
        Err(_) => repository_unavailable(),
    }
}

fn repository_unavailable() -> Response {
    service_unavailable("Gameplay repository is unavailable")
}
