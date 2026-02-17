use axum::extract::{rejection::JsonRejection, Query};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use platform_db::Database;
use serde::Deserialize;

use crate::auth_guard::BearerToken;
use crate::response::{bad_request, success};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AnalyticsEventRequest {
    event_type: String,
    session_id: Option<String>,
    properties: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct UsageQuery {
    days: Option<i32>,
}

pub fn router() -> Router {
    Router::new()
        .route("/api/analytics/events", post(track_event_handler))
        .route("/api/analytics/usage", get(usage_handler))
}

async fn track_event_handler(
    Extension(db): Extension<Option<Database>>,
    Extension(app_state): Extension<AppState>,
    payload: Result<Json<AnalyticsEventRequest>, JsonRejection>,
) -> Response {
    let Json(input) = match payload {
        Ok(value) => value,
        Err(_) => return bad_request("Invalid analytics payload"),
    };

    if input.event_type.trim().is_empty() {
        return bad_request("eventType is required");
    }

    if let Some(database) = db {
        let track_result = database
            .track_analytics_event(
                input.event_type.trim(),
                input.session_id.as_deref(),
                input.properties.clone(),
                None,
            )
            .await;
        if track_result.is_err() {
            app_state.track_analytics_event(input.event_type.trim());
        }
    } else {
        app_state.track_analytics_event(input.event_type.trim());
    }

    success(serde_json::json!({
        "recorded": true,
        "eventType": input.event_type,
        "sessionId": input.session_id,
        "properties": input.properties
    }))
}

async fn usage_handler(
    BearerToken(_token): BearerToken,
    Extension(db): Extension<Option<Database>>,
    Extension(app_state): Extension<AppState>,
    Query(query): Query<UsageQuery>,
) -> Response {
    let days = query.days.unwrap_or(7);
    if let Some(database) = db {
        if let Ok(usage) = database.analytics_usage(days).await {
            return success(serde_json::json!({
                "days": days,
                "totalEvents": usage.total_events,
                "activeUsers": usage.active_users,
                "eventsByType": usage
                    .by_type
                    .into_iter()
                    .map(|entry| serde_json::json!({ "eventType": entry.event_type, "count": entry.count }))
                    .collect::<Vec<_>>()
            }));
        }
    }

    let usage = app_state.analytics_usage(days);
    success(serde_json::json!({
        "days": days,
        "totalEvents": usage.total_events,
        "activeUsers": usage.active_users,
        "eventsByType": usage
            .by_type
            .into_iter()
            .map(|(event_type, count)| serde_json::json!({ "eventType": event_type, "count": count }))
            .collect::<Vec<_>>()
    }))
}
