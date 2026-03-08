use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use platform_errors::AppError;
use serde::Serialize;

#[derive(Serialize)]
pub struct SuccessResponse<T> {
    success: bool,
    data: T,
}

#[derive(Serialize)]
struct ErrorResponse {
    success: bool,
    error: String,
}

pub fn success<T: Serialize>(data: T) -> Response {
    (
        StatusCode::OK,
        Json(SuccessResponse {
            success: true,
            data,
        }),
    )
        .into_response()
}

pub fn bad_request(message: &str) -> Response {
    app_error_response(AppError::bad_request(message))
}

pub fn unauthorized(message: &str) -> Response {
    app_error_response(AppError::unauthorized(message))
}

pub fn not_found(message: &str) -> Response {
    app_error_response(AppError::not_found(message))
}

fn app_error_response(error: AppError) -> Response {
    let (status, message) = match error {
        AppError::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
        AppError::Unauthorized(message) => (StatusCode::UNAUTHORIZED, message),
        AppError::NotFound(message) => (StatusCode::NOT_FOUND, message),
        AppError::Internal(message) => (StatusCode::INTERNAL_SERVER_ERROR, message),
        AppError::Conflict(message) => (StatusCode::CONFLICT, message),
        AppError::RateLimited(message) => (StatusCode::TOO_MANY_REQUESTS, message),
        AppError::ServiceUnavailable(message) => (StatusCode::SERVICE_UNAVAILABLE, message),
        AppError::Forbidden(message) => (StatusCode::FORBIDDEN, message),
        AppError::UnprocessableEntity(message) => (StatusCode::UNPROCESSABLE_ENTITY, message),
    };

    (
        status,
        Json(ErrorResponse {
            success: false,
            error: message,
        }),
    )
        .into_response()
}
