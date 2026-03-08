use axum::extract::FromRequestParts;
use axum::http::{header::AUTHORIZATION, request::Parts, HeaderMap};
use axum::middleware::Next;
use axum::response::Response;
use axum::RequestExt;

use platform_auth::{AuthConfig, Claims};

use crate::response::unauthorized;

/// Cached auth configuration loaded once from environment.
fn auth_config() -> AuthConfig {
    AuthConfig::from_env()
}

/// Extractor that validates a Bearer JWT and provides the user id (`sub` claim).
///
/// Handlers destructure this as `BearerToken(user_id): BearerToken` and use
/// the inner `String` as a player key.  The validated [`Claims`] are inserted
/// into request extensions so that [`AuthUser`] can retrieve them downstream.
pub struct BearerToken(pub String);

#[axum::async_trait]
impl<S> FromRequestParts<S> for BearerToken
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let (_token, claims) = validate_bearer_jwt(&parts.headers)?;
        let user_id = claims.sub.clone();
        parts.extensions.insert(claims);
        Ok(Self(user_id))
    }
}

/// Axum middleware that rejects requests without a valid Bearer JWT.
///
/// Used via `route_layer(middleware::from_fn(require_bearer_auth))`.
pub async fn require_bearer_auth(
    mut request: axum::http::Request<axum::body::Body>,
    next: Next<axum::body::Body>,
) -> Response {
    if request.extract_parts::<BearerToken>().await.is_err() {
        return unauthorized("Unauthorized");
    }

    next.run(request).await
}

// ---------------------------------------------------------------------------
// AuthUser extractor — pulls validated claims from extensions
// ---------------------------------------------------------------------------

/// Extractor that provides the authenticated user's claims.
///
/// This must be used *after* [`BearerToken`] or [`require_bearer_auth`] has
/// run, because those are responsible for inserting [`Claims`] into extensions.
pub struct AuthUser(pub platform_auth::AuthUser);

#[axum::async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let claims = parts
            .extensions
            .get::<Claims>()
            .ok_or_else(|| unauthorized("Unauthorized"))?;

        Ok(AuthUser(platform_auth::AuthUser {
            user_id: claims.sub.clone(),
            username: claims.username.clone(),
            role: claims.role.clone(),
            universe_id: claims.universe_id,
        }))
    }
}

// ---------------------------------------------------------------------------
// Role guard helper
// ---------------------------------------------------------------------------

/// Returns a 401 response if the authenticated user does not hold at least
/// the given role.
///
/// ```ignore
/// let user: AuthUser = /* extracted */;
/// if let Err(resp) = require_role(&user.0, platform_auth::UserRole::Admin) {
///     return resp;
/// }
/// ```
pub fn require_role(
    user: &platform_auth::AuthUser,
    minimum: platform_auth::UserRole,
) -> Result<(), Response> {
    platform_auth::require_role(user, minimum).map_err(|e| unauthorized(&e.to_string()))
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Extract the Bearer token from headers and validate it as a JWT.
///
/// Returns the raw token string and the decoded [`Claims`].
fn validate_bearer_jwt(headers: &HeaderMap) -> Result<(String, Claims), Response> {
    let authorization = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| unauthorized("Unauthorized"))?;

    let token = platform_auth::extract_bearer_token(authorization)
        .ok_or_else(|| unauthorized("Unauthorized"))?;

    let config = auth_config();
    let claims =
        platform_auth::validate_token(&config, token).map_err(|e| unauthorized(&e.to_string()))?;

    Ok((token.to_string(), claims))
}
