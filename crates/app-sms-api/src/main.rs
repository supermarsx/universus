use std::net::SocketAddr;

use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use uuid::Uuid;

const SERVICE_NAME: &str = "app-sms-api";
const DEFAULT_PORT: u16 = 3003;

#[derive(Serialize)]
struct ServiceStatus {
    status: &'static str,
    service: &'static str,
}

#[derive(Serialize)]
struct SendAcceptedResponse {
    status: &'static str,
    service: &'static str,
    request_id: String,
    payload: serde_json::Value,
}

async fn health() -> Json<ServiceStatus> {
    Json(ServiceStatus {
        status: "ok",
        service: SERVICE_NAME,
    })
}

async fn ready() -> Json<ServiceStatus> {
    Json(ServiceStatus {
        status: "ok",
        service: SERVICE_NAME,
    })
}

async fn send_sms_handler(Json(payload): Json<serde_json::Value>) -> Json<SendAcceptedResponse> {
    Json(SendAcceptedResponse {
        status: "accepted",
        service: SERVICE_NAME,
        request_id: Uuid::new_v4().to_string(),
        payload,
    })
}

fn listen_port(default_port: u16) -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(default_port)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app = Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/api/send", post(send_sms_handler));

    let addr = SocketAddr::from(([0, 0, 0, 0], listen_port(DEFAULT_PORT)));
    tracing::info!(service = SERVICE_NAME, %addr, "startup");

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .expect("server failed");
}
