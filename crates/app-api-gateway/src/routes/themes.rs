use axum::extract::Path;
use axum::response::Response;
use axum::routing::{get, put};
use axum::{Extension, Json, Router};
use serde::{Deserialize, Serialize};

use crate::auth_guard::BearerToken;
use crate::response::{bad_request, success};
use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ThemeOverview {
    id: i64,
    key: &'static str,
    name: &'static str,
    category: &'static str,
    is_available: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateThemePreferencesRequest {
    theme_key: Option<String>,
    reduce_motion: Option<bool>,
    high_contrast: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateCustomCssRequest {
    css: String,
}

pub fn router() -> Router {
    Router::new()
        .route("/api/themes/current", get(current_theme_handler))
        .route("/api/themes", get(list_themes_handler))
        .route("/api/themes/:id", get(theme_by_id_handler))
        .route("/api/themes/user/preferences", get(get_preferences_handler))
        .route("/api/themes/user/preferences", put(update_preferences_handler))
        .route("/api/themes/user/custom-css", get(get_custom_css_handler))
        .route("/api/themes/user/custom-css", put(update_custom_css_handler))
}

async fn current_theme_handler() -> Response {
    success(serde_json::json!({
        "theme": {
            "id": 1,
            "key": "default",
            "name": "Default Command",
            "category": "standard"
        },
        "assets": [],
        "cssVariables": {
            "--accent": "#f59e0b"
        },
        "customCSS": ""
    }))
}

async fn list_themes_handler() -> Response {
    success(vec![
        ThemeOverview {
            id: 1,
            key: "default",
            name: "Default Command",
            category: "standard",
            is_available: true,
        },
        ThemeOverview {
            id: 2,
            key: "solstice",
            name: "Solstice Event",
            category: "seasonal",
            is_available: true,
        },
    ])
}

async fn theme_by_id_handler(Path(id): Path<i64>) -> Response {
    success(serde_json::json!({
        "theme": {
            "id": id,
            "key": format!("theme-{id}"),
            "name": format!("Theme {id}"),
            "category": "standard"
        },
        "assets": [],
        "configurations": []
    }))
}

async fn get_preferences_handler(
    BearerToken(token): BearerToken,
    Extension(app_state): Extension<AppState>,
) -> Response {
    let preferences = app_state.theme_preferences(&token);
    success(serde_json::json!({
        "themeKey": preferences.theme_key,
        "reduceMotion": preferences.reduce_motion,
        "highContrast": preferences.high_contrast
    }))
}

async fn update_preferences_handler(
    BearerToken(token): BearerToken,
    Extension(app_state): Extension<AppState>,
    Json(payload): Json<UpdateThemePreferencesRequest>,
) -> Response {
    if let Some(theme_key) = payload.theme_key.as_ref() {
        if theme_key.trim().is_empty() {
            return bad_request("Theme key cannot be empty");
        }
    }

    let preferences = app_state.update_theme_preferences(
        &token,
        payload.theme_key,
        payload.reduce_motion,
        payload.high_contrast,
    );

    success(serde_json::json!({
        "themeKey": preferences.theme_key,
        "reduceMotion": preferences.reduce_motion,
        "highContrast": preferences.high_contrast,
        "message": "Preferences updated successfully"
    }))
}

async fn get_custom_css_handler(
    BearerToken(token): BearerToken,
    Extension(app_state): Extension<AppState>,
) -> Response {
    success(serde_json::json!({
        "customCSS": app_state.user_custom_css(&token)
    }))
}

async fn update_custom_css_handler(
    BearerToken(token): BearerToken,
    Extension(app_state): Extension<AppState>,
    Json(payload): Json<UpdateCustomCssRequest>,
) -> Response {
    if payload.css.len() > 20_000 {
        return bad_request("Custom CSS too large");
    }
    let updated = app_state.update_user_custom_css(&token, payload.css);
    success(serde_json::json!({
        "customCSS": updated
    }))
}
