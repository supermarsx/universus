use axum::extract::FromRequestParts;
use axum::http::{header::AUTHORIZATION, request::Parts, HeaderMap};
use axum::middleware::Next;
use axum::response::Response;
use axum::RequestExt;

use crate::response::unauthorized;

pub struct BearerToken(pub String);

#[axum::async_trait]
impl<S> FromRequestParts<S> for BearerToken
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let token = validate_bearer_token(&parts.headers)?;
        Ok(Self(token))
    }
}

pub async fn require_bearer_auth(
    mut request: axum::http::Request<axum::body::Body>,
    next: Next<axum::body::Body>,
) -> Response {
    if request.extract_parts::<BearerToken>().await.is_err() {
        return unauthorized("Unauthorized");
    }

    next.run(request).await
}

fn validate_bearer_token(headers: &HeaderMap) -> Result<String, Response> {
    let authorization = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| unauthorized("Unauthorized"))?;

    let token = authorization
        .strip_prefix("Bearer ")
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| unauthorized("Unauthorized"))?;

    let expected = expected_token();
    if token == expected {
        Ok(token.to_string())
    } else {
        Err(unauthorized("Unauthorized"))
    }
}

fn expected_token() -> String {
    std::env::var("API_GATEWAY_TOKEN")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "dev-token".to_string())
}
