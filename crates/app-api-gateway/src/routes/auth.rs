use std::sync::{Arc, OnceLock};

use axum::extract::rejection::JsonRejection;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use platform_db::{normalize_account_email, AccountCreateInput, AccountRow};
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;

use crate::accounts::{AccountRepository, RepositoryError};
use crate::auth_guard::{AuthUser as AuthenticatedUser, BearerToken};
use crate::response::{
    bad_request, conflict, internal_error, service_unavailable, success, unauthorized,
};

const INVALID_CREDENTIALS: &str = "Invalid email or password";
const KDF_CONCURRENCY_LIMIT: usize = 4;

#[derive(Debug, Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct RegisterRequest {
    username: String,
    email: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct AuthUser {
    id: String,
    username: String,
    email: String,
}

impl From<&AccountRow> for AuthUser {
    fn from(account: &AccountRow) -> Self {
        Self {
            id: account.id.clone(),
            username: account.username.clone(),
            email: account.email.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthPayload {
    token: String,
    user: AuthUser,
    expires_in_seconds: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LogoutPayload {
    revoked: bool,
    session_revocation_supported: bool,
    reason: &'static str,
}

pub fn router() -> Router {
    Router::new()
        .route("/api/auth/login", post(login_handler))
        .route("/api/auth/register", post(register_handler))
        .route("/api/auth/logout", post(logout_handler))
        .route("/api/auth/me", get(me_handler))
}

async fn login_handler(
    Extension(accounts): Extension<AccountRepository>,
    payload: Result<Json<LoginRequest>, JsonRejection>,
) -> Response {
    let Json(input) = match payload {
        Ok(value) => value,
        Err(_) => return bad_request("Invalid login payload"),
    };
    let email = normalize_account_email(&input.email);
    if !valid_email(&email) || input.password.is_empty() || input.password.len() > 1024 {
        return bad_request("A valid email and password are required");
    }

    let account = match accounts.find_by_email(&email).await {
        Ok(account) => account,
        Err(error) => return repository_error(error),
    };
    let password_matches =
        match verify_with_non_enumerating_fallback(account.as_ref(), &input.password).await {
            Ok(matches) => matches,
            Err(()) => return internal_error("Unable to verify account credentials"),
        };
    let Some(account) = account else {
        return unauthorized(INVALID_CREDENTIALS);
    };
    if !password_matches || account.is_banned {
        return unauthorized(INVALID_CREDENTIALS);
    }
    if let Err(error) = accounts.record_login(&account.id).await {
        return repository_error(error);
    }

    auth_success(&account)
}

async fn register_handler(
    Extension(accounts): Extension<AccountRepository>,
    payload: Result<Json<RegisterRequest>, JsonRejection>,
) -> Response {
    let Json(input) = match payload {
        Ok(value) => value,
        Err(_) => return bad_request("Invalid register payload"),
    };
    let username = input.username.trim();
    let email = normalize_account_email(&input.email);
    if !valid_username(username) {
        return bad_request("Username must be 3-32 characters using letters, numbers, '_' or '-'");
    }
    if !valid_email(&email) {
        return bad_request("A valid email address is required");
    }
    if input.password.len() > 1024 {
        return bad_request("Password is too long");
    }
    if let Err(error) = platform_auth::validate_password_strength(&input.password) {
        return bad_request(&error.to_string());
    }

    let password_hash = match hash_password_off_thread(input.password.clone()).await {
        Ok(hash) => hash,
        Err(_) => return internal_error("Unable to secure account credentials"),
    };
    let account = match accounts
        .create(AccountCreateInput {
            username: username.to_string(),
            email,
            password_hash,
        })
        .await
    {
        Ok(account) => account,
        Err(RepositoryError::Duplicate) => {
            return conflict("Username or email is already registered")
        }
        Err(error) => return repository_error(error),
    };

    auth_success(&account)
}

async fn logout_handler(BearerToken(_): BearerToken) -> Response {
    // JWT access tokens are stateless and remain valid until expiry. The web
    // frontend expires its HttpOnly browser cookie locally, but this endpoint
    // must not claim server-side revocation until a durable session store exists.
    success(LogoutPayload {
        revoked: false,
        session_revocation_supported: false,
        reason: "stateless_access_token_discard_required",
    })
}

async fn me_handler(
    Extension(accounts): Extension<AccountRepository>,
    BearerToken(_): BearerToken,
    AuthenticatedUser(user): AuthenticatedUser,
) -> Response {
    match accounts.find_by_id(&user.user_id).await {
        Ok(Some(account)) if !account.is_banned => success(AuthUser::from(&account)),
        Ok(_) => unauthorized("Authenticated account is unavailable"),
        Err(error) => repository_error(error),
    }
}

fn auth_success(account: &AccountRow) -> Response {
    let config = platform_auth::AuthConfig::from_env();
    let token = match platform_auth::generate_token_with_email(
        &config,
        &account.id,
        &account.username,
        Some(&account.email),
        &account.role,
        account.universe_id,
    ) {
        Ok(token) => token,
        Err(_) => return internal_error("Unable to issue authentication token"),
    };

    success(AuthPayload {
        token,
        user: AuthUser::from(account),
        expires_in_seconds: config.jwt_expiry_seconds,
    })
}

fn repository_error(error: RepositoryError) -> Response {
    tracing::error!(?error, "account repository operation failed");
    match error {
        RepositoryError::Duplicate => conflict("Username or email is already registered"),
        RepositoryError::Unavailable(_) | RepositoryError::Storage(_) => {
            service_unavailable("Account service is unavailable")
        }
    }
}

async fn verify_with_non_enumerating_fallback(
    account: Option<&AccountRow>,
    password: &str,
) -> Result<bool, ()> {
    static DUMMY_HASH: OnceLock<String> = OnceLock::new();
    let account_exists = account.is_some();
    let stored_hash = account.map(|account| account.password_hash.clone());
    let password = password.to_string();

    run_kdf(move || {
        let dummy = DUMMY_HASH.get_or_init(|| {
            platform_auth::hash_password("NotARealAccountPassword1").unwrap_or_default()
        });
        let verified = platform_auth::verify_password(
            &password,
            stored_hash.as_deref().unwrap_or(dummy.as_str()),
        );
        account_exists && verified
    })
    .await
}

async fn hash_password_off_thread(password: String) -> Result<String, ()> {
    run_kdf(move || platform_auth::hash_password(&password))
        .await?
        .map_err(|_| ())
}

async fn run_kdf<T, F>(work: F) -> Result<T, ()>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    static KDF_LIMITER: OnceLock<Arc<Semaphore>> = OnceLock::new();
    let limiter = KDF_LIMITER
        .get_or_init(|| Arc::new(Semaphore::new(KDF_CONCURRENCY_LIMIT)))
        .clone();
    let _permit = limiter.acquire_owned().await.map_err(|_| ())?;
    tokio::task::spawn_blocking(work).await.map_err(|_| ())
}

fn valid_username(username: &str) -> bool {
    (3..=32).contains(&username.len())
        && username
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn valid_email(email: &str) -> bool {
    if email.is_empty() || email.len() > 254 || email.bytes().any(|byte| byte.is_ascii_whitespace())
    {
        return false;
    }
    let mut address_parts = email.split('@');
    let Some(local) = address_parts.next() else {
        return false;
    };
    let Some(domain) = address_parts.next() else {
        return false;
    };
    if address_parts.next().is_some() {
        return false;
    }

    !local.is_empty()
        && local.len() <= 64
        && !local.starts_with('.')
        && !local.ends_with('.')
        && !local.contains("..")
        && local.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'+' | b'-' | b'\'')
        })
        && !domain.is_empty()
        && domain.len() <= 253
        && domain.split('.').count() >= 2
        && domain.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_username_and_email_boundaries() {
        assert!(valid_username("Commander_1"));
        assert!(!valid_username("ab"));
        assert!(!valid_username("bad name"));
        assert!(valid_email("commander@example.com"));
        assert!(!valid_email("commander@example"));
        assert!(!valid_email("@example.com"));
        assert!(!valid_email("a@b@c.com"));
        assert!(!valid_email("commander()@example.com"));
        assert!(!valid_email("commander@example..com"));
        assert!(!valid_email("commander@-example.com"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn kdf_boundary_caps_concurrent_blocking_work() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::time::Duration;

        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let tasks = (0..12).map(|_| {
            let active = Arc::clone(&active);
            let peak = Arc::clone(&peak);
            tokio::spawn(async move {
                run_kdf(move || {
                    let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(current, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(20));
                    active.fetch_sub(1, Ordering::SeqCst);
                })
                .await
            })
        });

        for task in tasks {
            task.await.unwrap().unwrap();
        }
        assert!(peak.load(Ordering::SeqCst) <= KDF_CONCURRENCY_LIMIT);
    }
}
