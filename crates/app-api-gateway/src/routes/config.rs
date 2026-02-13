use axum::extract::{Path, Query};
use axum::response::Response;
use axum::routing::{get, post, put};
use axum::{Extension, Json, Router};
use serde::{Deserialize, Serialize};

use crate::auth_guard::BearerToken;
use crate::response::{bad_request, not_found, success};
use crate::state::{AppState, ConfigParameterSnapshot};

#[derive(Debug, Deserialize)]
struct ParametersQuery {
    category: Option<String>,
    search: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HistoryQuery {
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateParameterRequest {
    value: serde_json::Value,
    reason: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfigCategory {
    category: String,
    parameter_count: usize,
    modified_count: usize,
}

pub fn router() -> Router {
    Router::new()
        .route("/api/config/categories", get(list_categories_handler))
        .route(
            "/api/config/categories/:category",
            get(category_parameters_handler),
        )
        .route("/api/config/parameters", get(parameters_handler))
        .route("/api/config/parameters/:key", get(parameter_handler))
        .route("/api/config/parameters/:key", put(update_parameter_handler))
        .route("/api/config/game-config", get(game_config_handler))
        .route("/api/config/history", get(config_history_handler))
        .route(
            "/api/config/game-config/refresh",
            post(game_config_refresh_handler),
        )
}

async fn list_categories_handler(
    BearerToken(token): BearerToken,
    Extension(app_state): Extension<AppState>,
) -> Response {
    let _ = token;
    let parameters = app_state.config_parameters(None);

    let mut grouped = std::collections::BTreeMap::<String, ConfigCategory>::new();
    for parameter in parameters {
        let entry = grouped
            .entry(parameter.category.clone())
            .or_insert(ConfigCategory {
                category: parameter.category.clone(),
                parameter_count: 0,
                modified_count: 0,
            });
        entry.parameter_count += 1;
        if parameter.value != parameter.default_value {
            entry.modified_count += 1;
        }
    }

    success(grouped.into_values().collect::<Vec<_>>())
}

async fn category_parameters_handler(
    BearerToken(token): BearerToken,
    Extension(app_state): Extension<AppState>,
    Path(category): Path<String>,
) -> Response {
    let _ = token;
    success(parameter_payloads(
        app_state.config_parameters(Some(category.as_str())),
    ))
}

async fn parameters_handler(
    BearerToken(token): BearerToken,
    Extension(app_state): Extension<AppState>,
    Query(query): Query<ParametersQuery>,
) -> Response {
    let _ = token;
    let mut parameters = app_state.config_parameters(query.category.as_deref());

    if let Some(search) = query.search {
        let needle = search.to_lowercase();
        parameters.retain(|parameter| {
            parameter.key.to_lowercase().contains(&needle)
                || parameter.description.to_lowercase().contains(&needle)
        });
    }

    success(parameter_payloads(parameters))
}

async fn parameter_handler(
    BearerToken(token): BearerToken,
    Extension(app_state): Extension<AppState>,
    Path(key): Path<String>,
) -> Response {
    let _ = token;
    match app_state.config_parameter(&key) {
        Some(parameter) => success(parameter_payload(parameter)),
        None => not_found("Parameter not found"),
    }
}

async fn update_parameter_handler(
    BearerToken(token): BearerToken,
    Extension(app_state): Extension<AppState>,
    Path(key): Path<String>,
    Json(payload): Json<UpdateParameterRequest>,
) -> Response {
    let _ = token;
    let value = if let Some(value) = payload.value.as_str() {
        value.to_string()
    } else {
        payload.value.to_string()
    };
    let reason = payload
        .reason
        .unwrap_or_else(|| "Manual configuration update".to_string());

    match app_state.update_config_parameter(&key, value, reason) {
        Ok(parameter) => success(parameter_payload(parameter)),
        Err(message) if message == "Parameter not found" => not_found(message),
        Err(message) => bad_request(message),
    }
}

async fn game_config_handler(
    BearerToken(token): BearerToken,
    Extension(app_state): Extension<AppState>,
) -> Response {
    let _ = token;
    let parameters = app_state.config_parameters(None);
    success(serde_json::json!({
        "snapshotVersion": "rust-v1",
        "parameters": parameter_payloads(parameters)
    }))
}

async fn game_config_refresh_handler(
    BearerToken(token): BearerToken,
    Extension(_app_state): Extension<AppState>,
) -> Response {
    let _ = token;
    success(serde_json::json!({
        "refreshed": true,
        "snapshotVersion": "rust-v1"
    }))
}

async fn config_history_handler(
    BearerToken(token): BearerToken,
    Extension(app_state): Extension<AppState>,
    Query(query): Query<HistoryQuery>,
) -> Response {
    let _ = token;
    let limit = query.limit.unwrap_or(100);
    let history = app_state.config_history(limit);
    success(
        history
            .into_iter()
            .map(|entry| {
                serde_json::json!({
                    "changeId": entry.change_id,
                    "parameterKey": entry.parameter_key,
                    "oldValue": entry.old_value,
                    "newValue": entry.new_value,
                    "reason": entry.reason
                })
            })
            .collect::<Vec<_>>(),
    )
}

fn parameter_payloads(parameters: Vec<ConfigParameterSnapshot>) -> Vec<serde_json::Value> {
    parameters.into_iter().map(parameter_payload).collect()
}

fn parameter_payload(parameter: ConfigParameterSnapshot) -> serde_json::Value {
    serde_json::json!({
        "key": parameter.key,
        "category": parameter.category,
        "value": parameter.value,
        "defaultValue": parameter.default_value,
        "dataType": parameter.data_type,
        "description": parameter.description
    })
}
