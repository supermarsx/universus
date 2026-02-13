use std::net::SocketAddr;

use axum::extract::OriginalUri;
use axum::response::Html;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

pub const SERVICE_NAME: &str = "app-web-frontend";
pub const DEFAULT_PORT: u16 = 3005;

const TEMPLATE_ROUTES: [(&str, &str); 23] = [
    ("/", "Home"),
    ("/overview", "Overview"),
    ("/buildings", "Buildings"),
    ("/research", "Research"),
    ("/shipyard", "Shipyard"),
    ("/fleet", "Fleet"),
    ("/galaxy", "Galaxy"),
    ("/leaderboard", "Leaderboard"),
    ("/messages", "Messages"),
    ("/shop", "Shop"),
    ("/notifications", "Notifications"),
    ("/admin", "Admin"),
    ("/admin/dashboard", "Admin Dashboard"),
    ("/admin/users", "Admin Users"),
    ("/admin/monitoring", "Admin Monitoring"),
    ("/admin/settings", "Admin Settings"),
    ("/chat", "Chat"),
    ("/account/settings", "Account Settings"),
    ("/alliance", "Alliance"),
    ("/alliance/dashboard", "Alliance Dashboard"),
    ("/alliance/wars", "Alliance Wars"),
    ("/alliance/diplomacy", "Alliance Diplomacy"),
    ("/alliance/manage", "Alliance Management"),
];

#[derive(Serialize)]
struct ServiceStatus {
    status: &'static str,
    service: &'static str,
}

pub fn build_router() -> Router {
    let mut router = Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready));

    for (path, title) in TEMPLATE_ROUTES {
        router = router.route(path, get(move |uri: OriginalUri| template_page(title, uri)));
    }

    router
}

pub fn listen_port(default_port: u16) -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(default_port)
}

pub async fn serve() {
    tracing_subscriber::fmt::init();

    let addr = SocketAddr::from(([0, 0, 0, 0], listen_port(DEFAULT_PORT)));
    tracing::info!(service = SERVICE_NAME, %addr, "startup");

    axum::Server::bind(&addr)
        .serve(build_router().into_make_service())
        .await
        .expect("server failed");
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

async fn template_page(title: &'static str, OriginalUri(uri): OriginalUri) -> Html<String> {
    let route_path = uri.path();

    Html(format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{title} - Universus</title></head><body><main><h1>{title}</h1><p>Placeholder template page for <code>{route_path}</code>.</p></main></body></html>"
    ))
}
