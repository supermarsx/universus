use axum::extract::Path;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::response::{bad_request, success};

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct MessageSummary {
    id: &'static str,
    from: &'static str,
    subject: &'static str,
    unread: bool,
    sent_at: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MessageDetail {
    id: &'static str,
    from: &'static str,
    subject: &'static str,
    body: &'static str,
    unread: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UnreadCount {
    unread_count: i64,
}

#[derive(Debug, Deserialize)]
struct SendMessageRequest {
    to: String,
    subject: String,
    body: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SendMessageResult {
    message_id: String,
    queued: bool,
}

pub fn router() -> Router {
    Router::new()
        .route("/api/messages", get(list_messages_handler))
        .route("/api/messages/unread-count", get(unread_count_handler))
        .route("/api/messages/:message_id", get(get_message_handler))
        .route("/api/messages/send", post(send_message_handler))
}

async fn list_messages_handler() -> Response {
    success(seed_messages())
}

async fn unread_count_handler() -> Response {
    let unread = seed_messages().into_iter().filter(|msg| msg.unread).count() as i64;
    success(UnreadCount {
        unread_count: unread,
    })
}

async fn get_message_handler(Path(message_id): Path<String>) -> Response {
    let detail = match message_id.as_str() {
        "m-001" => MessageDetail {
            id: "m-001",
            from: "High Command",
            subject: "Sector Scan Complete",
            body: "Recon confirms no hostile fleets in [1:120:8].",
            unread: true,
        },
        "m-002" => MessageDetail {
            id: "m-002",
            from: "Alliance Council",
            subject: "Operation Dawn",
            body: "Attack window begins at server tick +6.",
            unread: false,
        },
        _ => return bad_request("Message not found"),
    };
    success(detail)
}

async fn send_message_handler(Json(input): Json<SendMessageRequest>) -> Response {
    if input.to.trim().is_empty() || input.subject.trim().is_empty() || input.body.trim().is_empty()
    {
        return bad_request("To, subject and body are required");
    }

    success(SendMessageResult {
        message_id: format!("m-out-{}", input.to.trim().to_ascii_lowercase()),
        queued: true,
    })
}

fn seed_messages() -> Vec<MessageSummary> {
    vec![
        MessageSummary {
            id: "m-001",
            from: "High Command",
            subject: "Sector Scan Complete",
            unread: true,
            sent_at: "2026-01-15T10:00:00Z",
        },
        MessageSummary {
            id: "m-002",
            from: "Alliance Council",
            subject: "Operation Dawn",
            unread: false,
            sent_at: "2026-01-14T18:30:00Z",
        },
    ]
}
