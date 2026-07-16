use std::net::SocketAddr;
use std::sync::{Arc, OnceLock};

use axum::extract::rejection::JsonRejection;
use axum::extract::{ConnectInfo, Path};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Extension, Json, Router};
use platform_db::{
    normalize_account_email, AccountCreateInput, AccountRow, AuthPrincipal, AuthRefreshRotateInput,
    AuthSessionCreateInput, AuthSessionError,
};
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;

use crate::accounts::{AccountRepository, RepositoryError, SessionSecurityConfig};
use crate::auth_guard::{AuthUser as AuthenticatedUser, AuthenticatedClaims, BearerToken};
use crate::response::{
    bad_request, conflict, internal_error, service_unavailable, success, unauthorized,
};

const INVALID_CREDENTIALS: &str = "Invalid email or password";
const KDF_CONCURRENCY_LIMIT: usize = 4;

#[derive(Debug, Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
    #[serde(default)]
    device_label: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RegisterRequest {
    username: String,
    email: String,
    password: String,
    #[serde(default)]
    device_label: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RefreshRequest {
    refresh_token: String,
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

impl From<&AuthPrincipal> for AuthUser {
    fn from(principal: &AuthPrincipal) -> Self {
        Self {
            id: principal.user_id.clone(),
            username: principal.username.clone(),
            email: principal.email.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthPayload {
    token: String,
    refresh_token: String,
    user: AuthUser,
    expires_in_seconds: i64,
    refresh_expires_in_seconds: i64,
    session_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LogoutPayload {
    revoked: bool,
    session_revocation_supported: bool,
    reason: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionPayload {
    session_id: String,
    device_label: Option<String>,
    created_at_unix: i64,
    last_used_at_unix: i64,
    expires_at_unix: i64,
    revoked_at_unix: Option<i64>,
    current: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionRevocationPayload {
    revoked: bool,
}

#[derive(Debug, Serialize)]
struct ErrorPayload {
    success: bool,
    error: &'static str,
}

pub fn router() -> Router {
    Router::new()
        .route("/api/auth/login", post(login_handler))
        .route("/api/auth/register", post(register_handler))
        .route("/api/auth/logout", post(logout_handler))
        .route("/api/auth/me", get(me_handler))
        .route("/api/auth/refresh", post(refresh_handler))
        .route("/api/auth/sessions", get(list_sessions_handler))
        .route("/api/auth/sessions", delete(revoke_all_sessions_handler))
        .route(
            "/api/auth/sessions/:session_id",
            delete(revoke_session_handler),
        )
}

async fn login_handler(
    Extension(accounts): Extension<AccountRepository>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
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
    let device_label = match normalize_device_label(input.device_label) {
        Ok(label) => label,
        Err(message) => return bad_request(message),
    };
    let security = match SessionSecurityConfig::from_environment() {
        Ok(config) => config,
        Err(error) => {
            tracing::error!(%error, "invalid session security configuration");
            return service_unavailable("Account service is unavailable");
        }
    };
    let account_digest = security.digest_account(&email);
    let ip_digest = security.digest_ip(
        connect_info
            .as_ref()
            .map(|ConnectInfo(address)| address.ip().to_string())
            .as_deref(),
    );
    let user_agent_digest = security.digest_user_agent(
        headers
            .get(header::USER_AGENT)
            .and_then(|value| value.to_str().ok()),
    );
    match accounts
        .auth_login_throttle_status(&account_digest, ip_digest.as_deref())
        .await
    {
        Ok(status) if status.blocked => return rate_limited(status.retry_after_seconds),
        Ok(_) => {}
        Err(error) => return session_repository_error(error),
    }

    let account = match accounts.find_by_email(&email).await {
        Ok(account) => account,
        Err(error) => return repository_error(error),
    };
    let password_matches =
        match verify_with_non_enumerating_fallback(account.as_ref(), &input.password).await {
            Ok(matches) => matches,
            Err(KdfError::Saturated) => return kdf_busy(),
            Err(KdfError::Failed) => return internal_error("Unable to verify account credentials"),
        };
    let Some(account) = account else {
        record_login_failure(&accounts, &security, &account_digest, ip_digest.as_deref()).await;
        return unauthorized(INVALID_CREDENTIALS);
    };
    if !password_matches || account.is_banned {
        record_login_failure(&accounts, &security, &account_digest, ip_digest.as_deref()).await;
        return unauthorized(INVALID_CREDENTIALS);
    }

    create_session_payload(
        &accounts,
        &account,
        &security,
        device_label,
        account_digest,
        ip_digest,
        user_agent_digest,
    )
    .await
}

async fn register_handler(
    Extension(accounts): Extension<AccountRepository>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
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
    let device_label = match normalize_device_label(input.device_label) {
        Ok(label) => label,
        Err(message) => return bad_request(message),
    };
    let security = match SessionSecurityConfig::from_environment() {
        Ok(config) => config,
        Err(error) => {
            tracing::error!(%error, "invalid session security configuration");
            return service_unavailable("Account service is unavailable");
        }
    };
    let peer_ip = connect_info
        .as_ref()
        .map(|ConnectInfo(address)| address.ip().to_string());
    let registration_ip_digest = security.digest_registration_ip(peer_ip.as_deref());
    match accounts
        .admit_auth_registration(
            &registration_ip_digest,
            security.registration_throttle_policy,
        )
        .await
    {
        Ok(status) if status.blocked => {
            return registration_rate_limited(status.retry_after_seconds)
        }
        Ok(_) => {}
        Err(error) => return session_repository_error(error),
    }

    let password_hash = match hash_password_off_thread(input.password.clone()).await {
        Ok(hash) => hash,
        Err(KdfError::Saturated) => return kdf_busy(),
        Err(KdfError::Failed) => return internal_error("Unable to secure account credentials"),
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

    let account_digest = security.digest_account(&account.email);
    let ip_digest = security.digest_ip(peer_ip.as_deref());
    let user_agent_digest = security.digest_user_agent(
        headers
            .get(header::USER_AGENT)
            .and_then(|value| value.to_str().ok()),
    );
    create_session_payload(
        &accounts,
        &account,
        &security,
        device_label,
        account_digest,
        ip_digest,
        user_agent_digest,
    )
    .await
}

async fn logout_handler(
    Extension(accounts): Extension<AccountRepository>,
    BearerToken(_): BearerToken,
    AuthenticatedClaims(claims): AuthenticatedClaims,
) -> Response {
    let Some(session_id) = claims.sid.as_deref() else {
        return success(LogoutPayload {
            revoked: false,
            session_revocation_supported: false,
            reason: "legacy_access_token_discard_required",
        });
    };
    match accounts.revoke_auth_session(&claims.sub, session_id).await {
        Ok(revoked) => success(LogoutPayload {
            revoked,
            session_revocation_supported: true,
            reason: if revoked {
                "session_revoked"
            } else {
                "session_already_inactive"
            },
        }),
        Err(error) => session_repository_error(error),
    }
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

async fn refresh_handler(
    Extension(accounts): Extension<AccountRepository>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    payload: Result<Json<RefreshRequest>, JsonRejection>,
) -> Response {
    let Json(input) = match payload {
        Ok(value) => value,
        Err(_) => return bad_request("Invalid refresh payload"),
    };
    if !(32..=256).contains(&input.refresh_token.len())
        || input.refresh_token.trim() != input.refresh_token
    {
        return unauthorized("Invalid refresh token");
    }
    let security = match SessionSecurityConfig::from_environment() {
        Ok(config) => config,
        Err(error) => {
            tracing::error!(%error, "invalid session security configuration");
            return service_unavailable("Account service is unavailable");
        }
    };
    let replacement_token = platform_auth::generate_opaque_refresh_token();
    let issue = accounts
        .rotate_auth_refresh_token(AuthRefreshRotateInput {
            consumed_token_digest: platform_auth::refresh_token_digest(&input.refresh_token)
                .to_vec(),
            replacement_token_digest: platform_auth::refresh_token_digest(&replacement_token)
                .to_vec(),
            ip_digest: security.digest_ip(
                connect_info
                    .as_ref()
                    .map(|ConnectInfo(address)| address.ip().to_string())
                    .as_deref(),
            ),
            user_agent_digest: security.digest_user_agent(
                headers
                    .get(header::USER_AGENT)
                    .and_then(|value| value.to_str().ok()),
            ),
        })
        .await;
    match issue {
        Ok(issue) => issue_auth_payload(issue, replacement_token, &security),
        Err(AuthSessionError::InvalidToken | AuthSessionError::ReplayDetected) => {
            unauthorized("Invalid refresh token")
        }
        Err(error) => session_repository_error(error),
    }
}

async fn list_sessions_handler(
    Extension(accounts): Extension<AccountRepository>,
    BearerToken(_): BearerToken,
    AuthenticatedClaims(claims): AuthenticatedClaims,
) -> Response {
    match accounts.list_auth_sessions(&claims.sub).await {
        Ok(sessions) => success(
            sessions
                .into_iter()
                .map(|session| SessionPayload {
                    current: claims.sid.as_deref() == Some(session.session_id.as_str()),
                    session_id: session.session_id,
                    device_label: session.device_label,
                    created_at_unix: session.created_at_unix,
                    last_used_at_unix: session.last_used_at_unix,
                    expires_at_unix: session.expires_at_unix,
                    revoked_at_unix: session.revoked_at_unix,
                })
                .collect::<Vec<_>>(),
        ),
        Err(error) => session_repository_error(error),
    }
}

async fn revoke_session_handler(
    Extension(accounts): Extension<AccountRepository>,
    Path(session_id): Path<String>,
    BearerToken(_): BearerToken,
    AuthenticatedClaims(claims): AuthenticatedClaims,
) -> Response {
    if !(32..=128).contains(&session_id.len()) || session_id.trim() != session_id {
        return bad_request("Invalid session id");
    }
    match accounts.revoke_auth_session(&claims.sub, &session_id).await {
        Ok(revoked) => success(SessionRevocationPayload { revoked }),
        Err(error) => session_repository_error(error),
    }
}

async fn revoke_all_sessions_handler(
    Extension(accounts): Extension<AccountRepository>,
    BearerToken(_): BearerToken,
    AuthenticatedClaims(claims): AuthenticatedClaims,
) -> Response {
    match accounts.revoke_all_auth_sessions(&claims.sub).await {
        Ok(revoked) => success(SessionRevocationPayload {
            revoked: revoked > 0,
        }),
        Err(error) => session_repository_error(error),
    }
}

async fn create_session_payload(
    accounts: &AccountRepository,
    account: &AccountRow,
    security: &SessionSecurityConfig,
    device_label: Option<String>,
    account_digest: Vec<u8>,
    ip_digest: Option<Vec<u8>>,
    user_agent_digest: Option<Vec<u8>>,
) -> Response {
    let session_id = platform_auth::generate_secure_id();
    let family_id = platform_auth::generate_secure_id();
    let refresh_token = platform_auth::generate_opaque_refresh_token();
    let issue = accounts
        .create_auth_session(AuthSessionCreateInput {
            account_id: account.id.clone(),
            session_id,
            family_id,
            refresh_token_digest: platform_auth::refresh_token_digest(&refresh_token).to_vec(),
            refresh_expiry_seconds: security.refresh_expiry_seconds,
            max_active_sessions: security.max_sessions_per_user,
            device_label,
            ip_digest,
            user_agent_digest,
            account_throttle_digest: account_digest,
        })
        .await;
    match issue {
        Ok(issue) => issue_auth_payload(issue, refresh_token, security),
        Err(AuthSessionError::AccountDisabled) => unauthorized(INVALID_CREDENTIALS),
        Err(error) => session_repository_error(error),
    }
}

fn issue_auth_payload(
    issue: platform_db::AuthSessionIssue,
    refresh_token: String,
    security: &SessionSecurityConfig,
) -> Response {
    let config = platform_auth::AuthConfig::from_env();
    let refresh_expires_in_seconds = issue
        .expires_at_unix
        .saturating_sub(current_unix_timestamp())
        .clamp(0, security.refresh_expiry_seconds);
    let token = match platform_auth::generate_session_token_with_email(
        &config,
        &issue.principal.user_id,
        &issue.principal.username,
        Some(&issue.principal.email),
        &issue.principal.role,
        Some(issue.principal.universe_id),
        &issue.session_id,
        issue.principal.auth_epoch,
    ) {
        Ok(token) => token,
        Err(_) => return internal_error("Unable to issue authentication token"),
    };

    success(AuthPayload {
        token,
        refresh_token,
        user: AuthUser::from(&issue.principal),
        expires_in_seconds: config.jwt_expiry_seconds,
        refresh_expires_in_seconds,
        session_id: issue.session_id,
    })
}

fn current_unix_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

async fn record_login_failure(
    accounts: &AccountRepository,
    security: &SessionSecurityConfig,
    account_digest: &[u8],
    ip_digest: Option<&[u8]>,
) {
    if let Err(error) = accounts
        .record_auth_login_failure(account_digest, ip_digest, security.throttle_policy)
        .await
    {
        tracing::warn!(?error, "unable to persist login throttle failure");
    }
}

fn normalize_device_label(label: Option<String>) -> Result<Option<String>, &'static str> {
    let Some(label) = label else {
        return Ok(None);
    };
    let label = label.trim();
    if label.is_empty() || label.chars().count() > 128 {
        return Err("Device label must be 1-128 characters");
    }
    Ok(Some(label.to_string()))
}

fn rate_limited(retry_after_seconds: i64) -> Response {
    rate_limited_with_message(
        retry_after_seconds,
        "Too many login attempts. Try again later.",
    )
}

fn registration_rate_limited(retry_after_seconds: i64) -> Response {
    rate_limited_with_message(
        retry_after_seconds,
        "Too many registration attempts. Try again later.",
    )
}

fn rate_limited_with_message(retry_after_seconds: i64, message: &'static str) -> Response {
    let retry_after = retry_after_seconds.max(1).to_string();
    let mut response = (
        StatusCode::TOO_MANY_REQUESTS,
        Json(ErrorPayload {
            success: false,
            error: message,
        }),
    )
        .into_response();
    if let Ok(value) = HeaderValue::from_str(&retry_after) {
        response.headers_mut().insert(header::RETRY_AFTER, value);
    }
    response
}

fn kdf_busy() -> Response {
    let mut response = service_unavailable("Authentication service is busy; retry shortly");
    response
        .headers_mut()
        .insert(header::RETRY_AFTER, HeaderValue::from_static("1"));
    response
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

fn session_repository_error(error: AuthSessionError) -> Response {
    tracing::error!(?error, "authentication session repository operation failed");
    match error {
        AuthSessionError::InvalidToken | AuthSessionError::ReplayDetected => {
            unauthorized("Invalid refresh token")
        }
        AuthSessionError::AccountDisabled => unauthorized(INVALID_CREDENTIALS),
        AuthSessionError::InvalidInput => bad_request("Invalid authentication session request"),
        AuthSessionError::Database(_) => service_unavailable("Account service is unavailable"),
    }
}

async fn verify_with_non_enumerating_fallback(
    account: Option<&AccountRow>,
    password: &str,
) -> Result<bool, KdfError> {
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

async fn hash_password_off_thread(password: String) -> Result<String, KdfError> {
    run_kdf(move || platform_auth::hash_password(&password))
        .await?
        .map_err(|_| KdfError::Failed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KdfError {
    Saturated,
    Failed,
}

async fn run_kdf<T, F>(work: F) -> Result<T, KdfError>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    static KDF_LIMITER: OnceLock<Arc<Semaphore>> = OnceLock::new();
    let limiter = KDF_LIMITER
        .get_or_init(|| Arc::new(Semaphore::new(KDF_CONCURRENCY_LIMIT)))
        .clone();
    let permit = limiter
        .try_acquire_owned()
        .map_err(|_| KdfError::Saturated)?;
    tokio::task::spawn_blocking(move || {
        // Keep admission ownership inside the blocking task so request
        // cancellation cannot release capacity while Argon2 is still running.
        let _permit = permit;
        work()
    })
    .await
    .map_err(|_| KdfError::Failed)
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
    async fn kdf_boundary_caps_work_and_rejects_saturation_without_waiters() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::{Condvar, Mutex};

        let started = Arc::new(AtomicUsize::new(0));
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let tasks: Vec<_> = (0..KDF_CONCURRENCY_LIMIT)
            .map(|_| {
                let started = Arc::clone(&started);
                let gate = Arc::clone(&gate);
                tokio::spawn(async move {
                    run_kdf(move || {
                        started.fetch_add(1, Ordering::SeqCst);
                        let (lock, wake) = &*gate;
                        let mut released = lock.lock().unwrap();
                        while !*released {
                            released = wake.wait(released).unwrap();
                        }
                    })
                    .await
                })
            })
            .collect();

        while started.load(Ordering::SeqCst) < KDF_CONCURRENCY_LIMIT {
            tokio::task::yield_now().await;
        }
        assert_eq!(run_kdf(|| ()).await, Err(KdfError::Saturated));

        let (lock, wake) = &*gate;
        *lock.lock().unwrap() = true;
        wake.notify_all();

        for task in tasks {
            task.await.unwrap().unwrap();
        }
    }
}
