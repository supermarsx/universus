use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use axum::extract::{Path, Query};
use axum::response::Response;
use axum::routing::{get, post};
use axum::Extension;
use axum::{Json, Router};
use game_notifications::{NewNotification, Notification, NotificationStore};
use platform_db::{
    Database, NotificationCreateInput, NotificationPreferenceUpsert, NotificationRow,
};
use serde::{Deserialize, Serialize};

use crate::auth_guard::{AuthUser, BearerToken};
use crate::authorization::{effective_numeric_user_id, numeric_subject};
use crate::response::{bad_request, success};

fn store() -> &'static Mutex<NotificationStore> {
    static STORE: OnceLock<Mutex<NotificationStore>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(NotificationStore::default()))
}

#[derive(Clone, Copy)]
struct PreferenceState {
    enabled: bool,
    min_priority: u8,
}

fn preference_store() -> &'static Mutex<HashMap<(i64, String), PreferenceState>> {
    static PREFS: OnceLock<Mutex<HashMap<(i64, String), PreferenceState>>> = OnceLock::new();
    PREFS.get_or_init(|| Mutex::new(HashMap::new()))
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreferenceUpdateRequest {
    user_id: Option<i64>,
    enabled: bool,
    min_priority: Option<u8>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PreferencePayload {
    user_id: i64,
    category: String,
    enabled: bool,
    min_priority: u8,
}

pub fn protected_router() -> Router {
    Router::new()
        .route("/api/notifications", get(list_notifications_handler))
        .route("/api/notifications", post(create_notification_handler))
        .route("/api/notifications/unread-count", get(unread_count_handler))
        .route(
            "/api/notifications/preferences",
            get(list_preferences_handler),
        )
        .route(
            "/api/notifications/preferences/:category",
            axum::routing::put(update_preference_handler),
        )
        .route(
            "/api/notifications/:notification_id/read",
            post(mark_read_handler),
        )
        .route("/api/notifications/read-all", post(mark_all_read_handler))
}

async fn list_notifications_handler(
    AuthUser(user): AuthUser,
    Extension(db): Extension<Option<Database>>,
    Query(query): Query<ListQuery>,
) -> Response {
    let user_id = match effective_numeric_user_id(&user, query.user_id) {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    if user_id <= 0 {
        return bad_request("Invalid user id");
    }
    let unread_only = query.unread_only.unwrap_or(false);
    let limit = query.limit.unwrap_or(50);

    if let Some(database) = db {
        if let Ok(rows) = database
            .list_notifications(user_id, unread_only, limit as i64)
            .await
        {
            return success(rows.into_iter().map(to_notification).collect::<Vec<_>>());
        }
    }

    let mut state = store().lock().expect("notifications store poisoned");
    success(state.list_user_notifications(user_id, unread_only, limit))
}

async fn unread_count_handler(
    AuthUser(user): AuthUser,
    Extension(db): Extension<Option<Database>>,
    Query(query): Query<ListQuery>,
) -> Response {
    let user_id = match effective_numeric_user_id(&user, query.user_id) {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    if user_id <= 0 {
        return bad_request("Invalid user id");
    }

    if let Some(database) = db {
        if let Ok(unread_count) = database.notification_unread_count(user_id).await {
            return success(UnreadCount {
                unread_count: unread_count.max(0) as usize,
            });
        }
    }

    let mut state = store().lock().expect("notifications store poisoned");
    success(UnreadCount {
        unread_count: state.unread_count(user_id),
    })
}

async fn create_notification_handler(
    BearerToken(subject): BearerToken,
    Extension(db): Extension<Option<Database>>,
    Json(input): Json<CreateNotificationRequest>,
) -> Response {
    if input.title.trim().is_empty()
        || input.message.trim().is_empty()
        || input.category.trim().is_empty()
    {
        return bad_request("Title, message and category are required");
    }
    let user_id = input.user_id.unwrap_or_else(|| numeric_subject(&subject));
    if user_id <= 0 {
        return bad_request("Invalid user id");
    }
    let priority = input.priority.unwrap_or(1);
    let category = input.category.trim().to_string();

    if !is_preference_allowed(db.as_ref(), user_id, &category, priority).await {
        return bad_request("Notification blocked by user preferences");
    }

    if let Some(database) = db {
        if let Ok(row) = database
            .create_notification(NotificationCreateInput {
                user_id,
                title: input.title.trim().to_string(),
                message: input.message.trim().to_string(),
                category: category.clone(),
                priority: priority as i16,
            })
            .await
        {
            let created = to_notification(row);
            publish_realtime_notification(user_id, "created", &created).await;
            return success(created);
        }
    }

    let created = {
        let mut state = store().lock().expect("notifications store poisoned");
        state.create_notification(
            user_id,
            NewNotification {
                title: input.title.trim().to_string(),
                message: input.message.trim().to_string(),
                category,
                priority,
            },
        )
    };
    publish_realtime_notification(user_id, "created", &created).await;
    success(created)
}

async fn mark_read_handler(
    AuthUser(user): AuthUser,
    Extension(db): Extension<Option<Database>>,
    Query(query): Query<ListQuery>,
    Path(notification_id): Path<i64>,
) -> Response {
    let user_id = match effective_numeric_user_id(&user, query.user_id) {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    if user_id <= 0 || notification_id <= 0 {
        return bad_request("Invalid user id or notification id");
    }

    if let Some(database) = db {
        if let Ok(updated) = database
            .mark_notification_read(user_id, notification_id)
            .await
        {
            if !updated {
                return bad_request("Notification not found");
            }
            publish_realtime_notification(
                user_id,
                "read",
                &serde_json::json!({ "notificationId": notification_id }),
            )
            .await;
            return success(serde_json::json!({ "success": true }));
        }
    }

    let updated = {
        let mut state = store().lock().expect("notifications store poisoned");
        state.mark_read(user_id, notification_id)
    };
    if !updated {
        return bad_request("Notification not found");
    }
    publish_realtime_notification(
        user_id,
        "read",
        &serde_json::json!({ "notificationId": notification_id }),
    )
    .await;
    success(serde_json::json!({ "success": true }))
}

async fn mark_all_read_handler(
    AuthUser(user): AuthUser,
    Extension(db): Extension<Option<Database>>,
    Query(query): Query<ListQuery>,
) -> Response {
    let user_id = match effective_numeric_user_id(&user, query.user_id) {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    if user_id <= 0 {
        return bad_request("Invalid user id");
    }

    if let Some(database) = db {
        if let Ok(updated) = database.mark_all_notifications_read(user_id).await {
            publish_realtime_notification(
                user_id,
                "read_all",
                &serde_json::json!({ "updated": updated }),
            )
            .await;
            return success(serde_json::json!({ "updated": updated }));
        }
    }

    let updated = {
        let mut state = store().lock().expect("notifications store poisoned");
        state.mark_all_read(user_id)
    };
    publish_realtime_notification(
        user_id,
        "read_all",
        &serde_json::json!({ "updated": updated }),
    )
    .await;
    success(serde_json::json!({ "updated": updated }))
}

fn to_notification(row: NotificationRow) -> Notification {
    Notification {
        id: row.id,
        user_id: row.user_id,
        title: row.title,
        message: row.message,
        category: row.category,
        priority: row.priority.clamp(0, u8::MAX as i16) as u8,
        is_read: row.is_read,
        created_at: format!("unix:{}", row.created_at_unix),
        read_at: row.read_at_unix.map(|value| format!("unix:{value}")),
    }
}

async fn list_preferences_handler(
    AuthUser(user): AuthUser,
    Extension(db): Extension<Option<Database>>,
    Query(query): Query<ListQuery>,
) -> Response {
    let user_id = match effective_numeric_user_id(&user, query.user_id) {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    if user_id <= 0 {
        return bad_request("Invalid user id");
    }

    if let Some(database) = db {
        if let Ok(rows) = database.list_notification_preferences(user_id).await {
            let payload = rows
                .into_iter()
                .map(|row| PreferencePayload {
                    user_id: row.user_id,
                    category: row.category,
                    enabled: row.enabled,
                    min_priority: row.min_priority.clamp(0, u8::MAX as i16) as u8,
                })
                .collect::<Vec<_>>();
            return success(payload);
        }
    }

    let prefs = preference_store()
        .lock()
        .expect("notification preferences store poisoned");
    let payload = prefs
        .iter()
        .filter(|((pref_user_id, _), _)| *pref_user_id == user_id)
        .map(|((pref_user_id, category), value)| PreferencePayload {
            user_id: *pref_user_id,
            category: category.clone(),
            enabled: value.enabled,
            min_priority: value.min_priority,
        })
        .collect::<Vec<_>>();
    success(payload)
}

async fn update_preference_handler(
    AuthUser(user): AuthUser,
    Extension(db): Extension<Option<Database>>,
    Path(category): Path<String>,
    Json(input): Json<PreferenceUpdateRequest>,
) -> Response {
    let user_id = match effective_numeric_user_id(&user, input.user_id) {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    if user_id <= 0 {
        return bad_request("Invalid user id");
    }
    let category = category.trim().to_string();
    if category.is_empty() {
        return bad_request("Category is required");
    }
    let min_priority = input.min_priority.unwrap_or(1);

    if let Some(database) = db {
        if let Ok(row) = database
            .upsert_notification_preference(NotificationPreferenceUpsert {
                user_id,
                category: category.clone(),
                enabled: input.enabled,
                min_priority: min_priority as i16,
            })
            .await
        {
            return success(PreferencePayload {
                user_id: row.user_id,
                category: row.category,
                enabled: row.enabled,
                min_priority: row.min_priority.clamp(0, u8::MAX as i16) as u8,
            });
        }
    }

    let mut prefs = preference_store()
        .lock()
        .expect("notification preferences store poisoned");
    prefs.insert(
        (user_id, category.clone()),
        PreferenceState {
            enabled: input.enabled,
            min_priority,
        },
    );
    success(PreferencePayload {
        user_id,
        category,
        enabled: input.enabled,
        min_priority,
    })
}

async fn is_preference_allowed(
    db: Option<&Database>,
    user_id: i64,
    category: &str,
    priority: u8,
) -> bool {
    if let Some(database) = db {
        if let Ok(Some(pref)) = database.notification_preference(user_id, category).await {
            return pref.enabled && priority as i16 >= pref.min_priority;
        }
    }

    let prefs = preference_store()
        .lock()
        .expect("notification preferences store poisoned");
    if let Some(pref) = prefs.get(&(user_id, category.to_string())) {
        return pref.enabled && priority >= pref.min_priority;
    }
    true
}

async fn publish_realtime_notification<T: Serialize>(user_id: i64, event_type: &str, payload: &T) {
    let base = std::env::var("REALTIME_GATEWAY_URL")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let Some(base_url) = base else {
        return;
    };
    let envelope = platform_events::build_event(event_type, payload);
    let channel = platform_events::user_notification_channel(user_id);
    if let Err(error) = platform_events::publish_http(&base_url, &channel, &envelope).await {
        tracing::warn!(
            %error,
            user_id,
            %channel,
            event_type,
            "realtime notification delivery failed"
        );
    }
}
