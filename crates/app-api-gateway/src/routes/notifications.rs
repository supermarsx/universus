use std::sync::{Mutex, OnceLock};

use axum::extract::{Path, Query};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use game_notifications::{NewNotification, NotificationStore};
use serde::{Deserialize, Serialize};

use crate::auth_guard::BearerToken;
use crate::response::{bad_request, success};

fn store() -> &'static Mutex<NotificationStore> {
    static STORE: OnceLock<Mutex<NotificationStore>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(NotificationStore::default()))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListQuery {
    unread_only: Option<bool>,
    limit: Option<usize>,
    user_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateNotificationRequest {
    user_id: Option<i64>,
    title: String,
    message: String,
    category: String,
    priority: Option<u8>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UnreadCount {
    unread_count: usize,
}

pub fn protected_router() -> Router {
    Router::new()
        .route("/api/notifications", get(list_notifications_handler))
        .route("/api/notifications", post(create_notification_handler))
        .route("/api/notifications/unread-count", get(unread_count_handler))
        .route(
            "/api/notifications/:notification_id/read",
            post(mark_read_handler),
        )
        .route("/api/notifications/read-all", post(mark_all_read_handler))
}

async fn list_notifications_handler(
    BearerToken(_token): BearerToken,
    Query(query): Query<ListQuery>,
) -> Response {
    let user_id = query.user_id.unwrap_or(1);
    if user_id <= 0 {
        return bad_request("Invalid user id");
    }
    let unread_only = query.unread_only.unwrap_or(false);
    let limit = query.limit.unwrap_or(50);
    let mut state = store().lock().expect("notifications store poisoned");
    success(state.list_user_notifications(user_id, unread_only, limit))
}

async fn unread_count_handler(BearerToken(_token): BearerToken, Query(query): Query<ListQuery>) -> Response {
    let user_id = query.user_id.unwrap_or(1);
    if user_id <= 0 {
        return bad_request("Invalid user id");
    }
    let mut state = store().lock().expect("notifications store poisoned");
    success(UnreadCount {
        unread_count: state.unread_count(user_id),
    })
}

async fn create_notification_handler(
    BearerToken(_token): BearerToken,
    Json(input): Json<CreateNotificationRequest>,
) -> Response {
    if input.title.trim().is_empty() || input.message.trim().is_empty() || input.category.trim().is_empty() {
        return bad_request("Title, message and category are required");
    }
    let user_id = input.user_id.unwrap_or(1);
    if user_id <= 0 {
        return bad_request("Invalid user id");
    }
    let mut state = store().lock().expect("notifications store poisoned");
    let created = state.create_notification(
        user_id,
        NewNotification {
            title: input.title.trim().to_string(),
            message: input.message.trim().to_string(),
            category: input.category.trim().to_string(),
            priority: input.priority.unwrap_or(1),
        },
    );
    success(created)
}

async fn mark_read_handler(
    BearerToken(_token): BearerToken,
    Query(query): Query<ListQuery>,
    Path(notification_id): Path<i64>,
) -> Response {
    let user_id = query.user_id.unwrap_or(1);
    if user_id <= 0 || notification_id <= 0 {
        return bad_request("Invalid user id or notification id");
    }
    let mut state = store().lock().expect("notifications store poisoned");
    if !state.mark_read(user_id, notification_id) {
        return bad_request("Notification not found");
    }
    success(serde_json::json!({ "success": true }))
}

async fn mark_all_read_handler(BearerToken(_token): BearerToken, Query(query): Query<ListQuery>) -> Response {
    let user_id = query.user_id.unwrap_or(1);
    if user_id <= 0 {
        return bad_request("Invalid user id");
    }
    let mut state = store().lock().expect("notifications store poisoned");
    let updated = state.mark_all_read(user_id);
    success(serde_json::json!({ "updated": updated }))
}
