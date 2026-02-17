use std::net::SocketAddr;

use axum::extract::OriginalUri;
use axum::http::{header::AUTHORIZATION, HeaderMap, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

pub const SERVICE_NAME: &str = "app-web-frontend";
pub const DEFAULT_PORT: u16 = 3005;

const TEMPLATE_ROUTES: [(&str, &str); 50] = [
    ("/", "Home"),
    ("/index.html", "Home"),
    ("/overview", "Overview"),
    ("/overview.html", "Overview"),
    ("/buildings", "Buildings"),
    ("/buildings.html", "Buildings"),
    ("/research", "Research"),
    ("/research.html", "Research"),
    ("/shipyard", "Shipyard"),
    ("/shipyard.html", "Shipyard"),
    ("/fleet", "Fleet"),
    ("/fleet.html", "Fleet"),
    ("/galaxy", "Galaxy"),
    ("/galaxy.html", "Galaxy"),
    ("/leaderboard", "Leaderboard"),
    ("/leaderboard.html", "Leaderboard"),
    ("/messages", "Messages"),
    ("/messages.html", "Messages"),
    ("/shop", "Shop"),
    ("/shop.html", "Shop"),
    ("/notifications", "Notifications"),
    ("/notifications.html", "Notifications"),
    ("/matrix-shop", "Matrix Shop"),
    ("/matrix-shop.html", "Matrix Shop"),
    ("/admin", "Admin"),
    ("/admin.html", "Admin"),
    ("/admin/dashboard", "Admin Dashboard"),
    ("/admin/users", "Admin Users"),
    ("/admin/monitoring", "Admin Monitoring"),
    ("/admin/settings", "Admin Settings"),
    ("/admin/events", "Admin Events"),
    ("/admin/analytics", "Admin Analytics"),
    ("/admin/audit", "Admin Audit Logs"),
    ("/admin/sms-service", "Admin SMS Service"),
    ("/admin/bots", "Admin Bot Management"),
    ("/admin/bots.html", "Admin Bot Management"),
    ("/chat", "Chat"),
    ("/chat.html", "Chat"),
    ("/account/settings", "Account Settings"),
    ("/account/security", "Security Dashboard"),
    ("/account/2fa", "2FA Setup"),
    ("/account/email", "Email Verification"),
    ("/account/password", "Password Recovery"),
    ("/account/privacy", "Privacy and Data Management"),
    ("/account/transfer", "Account Transfer"),
    ("/alliance", "Alliance Dashboard"),
    ("/alliance/dashboard", "Alliance Dashboard"),
    ("/alliance/wars", "Alliance Wars"),
    ("/alliance/diplomacy", "Alliance Diplomacy"),
    ("/alliance/manage", "Alliance Management"),
];

const AUTH_TOKEN: &str = "dev-token";
const ADMIN_TOKEN: &str = "admin-token";

#[derive(Serialize)]
struct ServiceStatus {
    status: &'static str,
    service: &'static str,
}

pub fn build_router() -> Router {
    let mut public_routes = Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready));
    let mut authenticated_routes = Router::new();
    let mut admin_routes = Router::new();

    for (path, title) in TEMPLATE_ROUTES {
        let route = get(move |uri: OriginalUri| template_page(title, uri));
        if path.starts_with("/admin") {
            admin_routes = admin_routes.route(path, route);
            continue;
        }
        if path == "/" || path == "/index.html" {
            public_routes = public_routes.route(path, route);
            continue;
        }
        authenticated_routes = authenticated_routes.route(path, route);
    }

    public_routes
        .merge(authenticated_routes.route_layer(middleware::from_fn(require_authenticated)))
        .merge(admin_routes.route_layer(middleware::from_fn(require_admin)))
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
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><meta name=\"route-title\" content=\"{title}\"><meta name=\"route-path\" content=\"{route_path}\"><title>{title} - Universus</title></head><body><main><h1>{title}</h1><p>Placeholder template page for <code>{route_path}</code>.</p></main></body></html>"
    ))
}

async fn require_authenticated(
    request: axum::http::Request<axum::body::Body>,
    next: Next<axum::body::Body>,
) -> axum::response::Response {
    match bearer_token(request.headers()) {
        Some(token) if token == AUTH_TOKEN || token == ADMIN_TOKEN => next.run(request).await,
        _ => StatusCode::UNAUTHORIZED.into_response(),
    }
}

async fn require_admin(
    request: axum::http::Request<axum::body::Body>,
    next: Next<axum::body::Body>,
) -> axum::response::Response {
    match bearer_token(request.headers()) {
        Some(token) if token == ADMIN_TOKEN => next.run(request).await,
        Some(_) => StatusCode::FORBIDDEN.into_response(),
        None => StatusCode::UNAUTHORIZED.into_response(),
    }
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}
