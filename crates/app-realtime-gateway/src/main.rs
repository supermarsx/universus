use std::net::SocketAddr;

use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

const SERVICE_NAME: &str = "app-realtime-gateway";
const DEFAULT_PORT: u16 = 3004;

#[derive(Serialize)]
struct ServiceStatus {
    status: &'static str,
    service: &'static str,
}

#[derive(Serialize)]
struct WebSocketInfo {
    service: &'static str,
    websocket: bool,
    endpoint: &'static str,
    formats: Vec<&'static str>,
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

async fn ws_info() -> Json<WebSocketInfo> {
    Json(WebSocketInfo {
        service: SERVICE_NAME,
        websocket: true,
        endpoint: "/ws",
        formats: vec!["json"],
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
        .route("/ws-info", get(ws_info));

    let addr = SocketAddr::from(([0, 0, 0, 0], listen_port(DEFAULT_PORT)));
    tracing::info!(service = SERVICE_NAME, %addr, "startup");

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .expect("server failed");
}
