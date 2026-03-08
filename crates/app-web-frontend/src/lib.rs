#![forbid(unsafe_code)]

//! # app-web-frontend
//!
//! Server-side rendered web frontend for Universus.
//!
//! Serves HTML pages for every game screen, account/admin panel, and public
//! landing pages.  Authentication is validated via JWT tokens issued by
//! `platform-auth`.  Admin routes additionally require the `admin` role claim.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{Extension, OriginalUri};
use axum::http::{header::AUTHORIZATION, HeaderMap, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use platform_auth::{AuthConfig, Claims};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub const SERVICE_NAME: &str = "app-web-frontend";
pub const DEFAULT_PORT: u16 = 3005;

// ---------------------------------------------------------------------------
// Route definitions
// ---------------------------------------------------------------------------

/// All template routes with their access level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AccessLevel {
    Public,
    Authenticated,
    Admin,
}

struct RouteEntry {
    path: &'static str,
    title: &'static str,
    access: AccessLevel,
    nav_section: Option<&'static str>,
}

const ROUTES: &[RouteEntry] = &[
    // Public pages
    RouteEntry {
        path: "/",
        title: "Home",
        access: AccessLevel::Public,
        nav_section: None,
    },
    RouteEntry {
        path: "/index.html",
        title: "Home",
        access: AccessLevel::Public,
        nav_section: None,
    },
    RouteEntry {
        path: "/login",
        title: "Login",
        access: AccessLevel::Public,
        nav_section: None,
    },
    RouteEntry {
        path: "/register",
        title: "Register",
        access: AccessLevel::Public,
        nav_section: None,
    },
    RouteEntry {
        path: "/forgot-password",
        title: "Forgot Password",
        access: AccessLevel::Public,
        nav_section: None,
    },
    // Main game views (authenticated)
    RouteEntry {
        path: "/overview",
        title: "Overview",
        access: AccessLevel::Authenticated,
        nav_section: Some("game"),
    },
    RouteEntry {
        path: "/overview.html",
        title: "Overview",
        access: AccessLevel::Authenticated,
        nav_section: Some("game"),
    },
    RouteEntry {
        path: "/buildings",
        title: "Buildings",
        access: AccessLevel::Authenticated,
        nav_section: Some("game"),
    },
    RouteEntry {
        path: "/buildings.html",
        title: "Buildings",
        access: AccessLevel::Authenticated,
        nav_section: Some("game"),
    },
    RouteEntry {
        path: "/research",
        title: "Research",
        access: AccessLevel::Authenticated,
        nav_section: Some("game"),
    },
    RouteEntry {
        path: "/research.html",
        title: "Research",
        access: AccessLevel::Authenticated,
        nav_section: Some("game"),
    },
    RouteEntry {
        path: "/shipyard",
        title: "Shipyard",
        access: AccessLevel::Authenticated,
        nav_section: Some("game"),
    },
    RouteEntry {
        path: "/shipyard.html",
        title: "Shipyard",
        access: AccessLevel::Authenticated,
        nav_section: Some("game"),
    },
    RouteEntry {
        path: "/fleet",
        title: "Fleet",
        access: AccessLevel::Authenticated,
        nav_section: Some("game"),
    },
    RouteEntry {
        path: "/fleet.html",
        title: "Fleet",
        access: AccessLevel::Authenticated,
        nav_section: Some("game"),
    },
    RouteEntry {
        path: "/galaxy",
        title: "Galaxy",
        access: AccessLevel::Authenticated,
        nav_section: Some("game"),
    },
    RouteEntry {
        path: "/galaxy.html",
        title: "Galaxy",
        access: AccessLevel::Authenticated,
        nav_section: Some("game"),
    },
    RouteEntry {
        path: "/leaderboard",
        title: "Leaderboard",
        access: AccessLevel::Authenticated,
        nav_section: Some("game"),
    },
    RouteEntry {
        path: "/leaderboard.html",
        title: "Leaderboard",
        access: AccessLevel::Authenticated,
        nav_section: Some("game"),
    },
    RouteEntry {
        path: "/messages",
        title: "Messages",
        access: AccessLevel::Authenticated,
        nav_section: Some("game"),
    },
    RouteEntry {
        path: "/messages.html",
        title: "Messages",
        access: AccessLevel::Authenticated,
        nav_section: Some("game"),
    },
    RouteEntry {
        path: "/shop",
        title: "Shop",
        access: AccessLevel::Authenticated,
        nav_section: Some("shop"),
    },
    RouteEntry {
        path: "/shop.html",
        title: "Shop",
        access: AccessLevel::Authenticated,
        nav_section: Some("shop"),
    },
    RouteEntry {
        path: "/notifications",
        title: "Notifications",
        access: AccessLevel::Authenticated,
        nav_section: Some("game"),
    },
    RouteEntry {
        path: "/notifications.html",
        title: "Notifications",
        access: AccessLevel::Authenticated,
        nav_section: Some("game"),
    },
    RouteEntry {
        path: "/matrix-shop",
        title: "Matrix Shop",
        access: AccessLevel::Authenticated,
        nav_section: Some("shop"),
    },
    RouteEntry {
        path: "/matrix-shop.html",
        title: "Matrix Shop",
        access: AccessLevel::Authenticated,
        nav_section: Some("shop"),
    },
    RouteEntry {
        path: "/chat",
        title: "Chat",
        access: AccessLevel::Authenticated,
        nav_section: Some("social"),
    },
    RouteEntry {
        path: "/chat.html",
        title: "Chat",
        access: AccessLevel::Authenticated,
        nav_section: Some("social"),
    },
    // Account pages
    RouteEntry {
        path: "/account/settings",
        title: "Account Settings",
        access: AccessLevel::Authenticated,
        nav_section: Some("account"),
    },
    RouteEntry {
        path: "/account/security",
        title: "Security Dashboard",
        access: AccessLevel::Authenticated,
        nav_section: Some("account"),
    },
    RouteEntry {
        path: "/account/2fa",
        title: "2FA Setup",
        access: AccessLevel::Authenticated,
        nav_section: Some("account"),
    },
    RouteEntry {
        path: "/account/email",
        title: "Email Verification",
        access: AccessLevel::Authenticated,
        nav_section: Some("account"),
    },
    RouteEntry {
        path: "/account/password",
        title: "Password Recovery",
        access: AccessLevel::Authenticated,
        nav_section: Some("account"),
    },
    RouteEntry {
        path: "/account/privacy",
        title: "Privacy and Data Management",
        access: AccessLevel::Authenticated,
        nav_section: Some("account"),
    },
    RouteEntry {
        path: "/account/transfer",
        title: "Account Transfer",
        access: AccessLevel::Authenticated,
        nav_section: Some("account"),
    },
    // Alliance pages
    RouteEntry {
        path: "/alliance",
        title: "Alliance Dashboard",
        access: AccessLevel::Authenticated,
        nav_section: Some("alliance"),
    },
    RouteEntry {
        path: "/alliance/dashboard",
        title: "Alliance Dashboard",
        access: AccessLevel::Authenticated,
        nav_section: Some("alliance"),
    },
    RouteEntry {
        path: "/alliance/wars",
        title: "Alliance Wars",
        access: AccessLevel::Authenticated,
        nav_section: Some("alliance"),
    },
    RouteEntry {
        path: "/alliance/diplomacy",
        title: "Alliance Diplomacy",
        access: AccessLevel::Authenticated,
        nav_section: Some("alliance"),
    },
    RouteEntry {
        path: "/alliance/manage",
        title: "Alliance Management",
        access: AccessLevel::Authenticated,
        nav_section: Some("alliance"),
    },
    // Admin pages
    RouteEntry {
        path: "/admin",
        title: "Admin",
        access: AccessLevel::Admin,
        nav_section: Some("admin"),
    },
    RouteEntry {
        path: "/admin.html",
        title: "Admin",
        access: AccessLevel::Admin,
        nav_section: Some("admin"),
    },
    RouteEntry {
        path: "/admin/dashboard",
        title: "Admin Dashboard",
        access: AccessLevel::Admin,
        nav_section: Some("admin"),
    },
    RouteEntry {
        path: "/admin/users",
        title: "Admin Users",
        access: AccessLevel::Admin,
        nav_section: Some("admin"),
    },
    RouteEntry {
        path: "/admin/monitoring",
        title: "Admin Monitoring",
        access: AccessLevel::Admin,
        nav_section: Some("admin"),
    },
    RouteEntry {
        path: "/admin/settings",
        title: "Admin Settings",
        access: AccessLevel::Admin,
        nav_section: Some("admin"),
    },
    RouteEntry {
        path: "/admin/events",
        title: "Admin Events",
        access: AccessLevel::Admin,
        nav_section: Some("admin"),
    },
    RouteEntry {
        path: "/admin/analytics",
        title: "Admin Analytics",
        access: AccessLevel::Admin,
        nav_section: Some("admin"),
    },
    RouteEntry {
        path: "/admin/audit",
        title: "Admin Audit Logs",
        access: AccessLevel::Admin,
        nav_section: Some("admin"),
    },
    RouteEntry {
        path: "/admin/sms-service",
        title: "Admin SMS Service",
        access: AccessLevel::Admin,
        nav_section: Some("admin"),
    },
    RouteEntry {
        path: "/admin/bots",
        title: "Admin Bot Management",
        access: AccessLevel::Admin,
        nav_section: Some("admin"),
    },
    RouteEntry {
        path: "/admin/bots.html",
        title: "Admin Bot Management",
        access: AccessLevel::Admin,
        nav_section: Some("admin"),
    },
];

// ---------------------------------------------------------------------------
// Shared application state
// ---------------------------------------------------------------------------

/// Application state shared across all handlers via Extension.
#[derive(Debug, Clone)]
pub struct AppState {
    pub auth_config: AuthConfig,
    pub service_name: String,
    pub start_time: String,
}

impl AppState {
    pub fn new(auth_config: AuthConfig) -> Self {
        Self {
            auth_config,
            service_name: SERVICE_NAME.to_string(),
            start_time: chrono_now_iso(),
        }
    }

    pub fn from_env() -> Self {
        Self::new(AuthConfig::from_env())
    }
}

// ---------------------------------------------------------------------------
// API types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub status: String,
    pub service: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceHealth {
    pub status: String,
    pub service: String,
    pub uptime_since: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigationItem {
    pub path: String,
    pub title: String,
    pub section: String,
    pub active: bool,
}

// ---------------------------------------------------------------------------
// Router construction
// ---------------------------------------------------------------------------

/// Builds the full application router with JWT authentication.
pub fn build_router() -> Router {
    build_router_with_state(AppState::from_env())
}

/// Builds the router using the provided application state.
///
/// This is the testable entry point — tests pass a custom [`AppState`] with
/// a known JWT secret so they can generate valid tokens.
pub fn build_router_with_state(state: AppState) -> Router {
    let shared = Arc::new(state);

    let mut public_routes = Router::new()
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
        .route("/login", get(login_page))
        .route("/register", get(register_page))
        .route("/forgot-password", get(forgot_password_page));

    let mut authenticated_routes = Router::new().route("/api/nav", get(navigation_handler));
    let mut admin_routes = Router::new();

    for entry in ROUTES {
        let title = entry.title;
        let section = entry.nav_section.unwrap_or("general");
        let route = get(move |uri: OriginalUri, claims: Option<Extension<Claims>>| {
            template_page(title, section, uri, claims)
        });

        match entry.access {
            AccessLevel::Public => {
                // /login, /register, /forgot-password already registered above.
                if entry.path == "/" || entry.path == "/index.html" {
                    public_routes = public_routes.route(entry.path, route);
                }
            }
            AccessLevel::Authenticated => {
                authenticated_routes = authenticated_routes.route(entry.path, route);
            }
            AccessLevel::Admin => {
                admin_routes = admin_routes.route(entry.path, route);
            }
        }
    }

    let shared_admin = Arc::clone(&shared);
    let shared_auth = Arc::clone(&shared);

    public_routes
        .merge(
            authenticated_routes
                .layer(Extension(Arc::clone(&shared_auth)))
                .route_layer(middleware::from_fn(move |req, next| {
                    let cfg = Arc::clone(&shared_auth);
                    require_authenticated(cfg, req, next)
                })),
        )
        .merge(
            admin_routes
                .layer(Extension(Arc::clone(&shared_admin)))
                .route_layer(middleware::from_fn(move |req, next| {
                    let cfg = Arc::clone(&shared_admin);
                    require_admin(cfg, req, next)
                })),
        )
        .route("/404", get(not_found_page))
        .fallback(get(fallback_handler))
        .layer(Extension(shared))
}

// ---------------------------------------------------------------------------
// Public port helper
// ---------------------------------------------------------------------------

pub fn listen_port(default_port: u16) -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(default_port)
}

// ---------------------------------------------------------------------------
// Server entry point
// ---------------------------------------------------------------------------

pub async fn serve() {
    tracing_subscriber::fmt::init();

    let addr = SocketAddr::from(([0, 0, 0, 0], listen_port(DEFAULT_PORT)));
    let state = AppState::from_env();
    tracing::info!(service = SERVICE_NAME, %addr, "startup");

    axum::Server::bind(&addr)
        .serve(build_router_with_state(state).into_make_service())
        .await
        .expect("server failed");
}

// ---------------------------------------------------------------------------
// Health / readiness handlers
// ---------------------------------------------------------------------------

async fn health_handler(Extension(state): Extension<Arc<AppState>>) -> Json<ServiceHealth> {
    Json(ServiceHealth {
        status: "ok".to_string(),
        service: state.service_name.clone(),
        uptime_since: state.start_time.clone(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

async fn ready_handler() -> Json<ServiceStatus> {
    Json(ServiceStatus {
        status: "ok".to_string(),
        service: SERVICE_NAME.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Navigation handler
// ---------------------------------------------------------------------------

async fn navigation_handler(
    uri: OriginalUri,
    claims: Option<Extension<Claims>>,
) -> Json<Vec<NavigationItem>> {
    let current_path = uri.path().to_string();
    let is_admin = claims
        .as_ref()
        .map(|c| c.0.role == "admin")
        .unwrap_or(false);

    let mut items = Vec::new();
    for entry in ROUTES {
        // Skip .html duplicates in the nav list
        if entry.path.ends_with(".html") {
            continue;
        }
        if entry.access == AccessLevel::Public {
            continue;
        }
        if entry.access == AccessLevel::Admin && !is_admin {
            continue;
        }
        let section = entry.nav_section.unwrap_or("general");
        items.push(NavigationItem {
            path: entry.path.to_string(),
            title: entry.title.to_string(),
            section: section.to_string(),
            active: current_path == entry.path,
        });
    }
    Json(items)
}

// ---------------------------------------------------------------------------
// Public page handlers
// ---------------------------------------------------------------------------

async fn login_page() -> Html<String> {
    Html(render_public_page(
        "Login",
        r#"<form id="login-form" method="post" action="/api/auth/login">
  <div class="form-group">
    <label for="username">Username</label>
    <input type="text" id="username" name="username" required autocomplete="username" />
  </div>
  <div class="form-group">
    <label for="password">Password</label>
    <input type="password" id="password" name="password" required autocomplete="current-password" />
  </div>
  <button type="submit">Login</button>
  <p><a href="/forgot-password">Forgot your password?</a></p>
  <p>Don't have an account? <a href="/register">Register</a></p>
</form>"#,
    ))
}

async fn register_page() -> Html<String> {
    Html(render_public_page(
        "Register",
        r#"<form id="register-form" method="post" action="/api/auth/register">
  <div class="form-group">
    <label for="username">Username</label>
    <input type="text" id="username" name="username" required minlength="3" maxlength="32" />
  </div>
  <div class="form-group">
    <label for="email">Email</label>
    <input type="email" id="email" name="email" required />
  </div>
  <div class="form-group">
    <label for="password">Password</label>
    <input type="password" id="password" name="password" required minlength="8" autocomplete="new-password" />
  </div>
  <div class="form-group">
    <label for="password-confirm">Confirm Password</label>
    <input type="password" id="password-confirm" name="password_confirm" required minlength="8" autocomplete="new-password" />
  </div>
  <button type="submit">Create Account</button>
  <p>Already have an account? <a href="/login">Login</a></p>
</form>"#,
    ))
}

async fn forgot_password_page() -> Html<String> {
    Html(render_public_page(
        "Forgot Password",
        r#"<form id="forgot-form" method="post" action="/api/auth/forgot-password">
  <div class="form-group">
    <label for="email">Email Address</label>
    <input type="email" id="email" name="email" required />
  </div>
  <button type="submit">Send Reset Link</button>
  <p><a href="/login">Back to login</a></p>
</form>"#,
    ))
}

// ---------------------------------------------------------------------------
// Authenticated template page
// ---------------------------------------------------------------------------

async fn template_page(
    title: &'static str,
    section: &'static str,
    OriginalUri(uri): OriginalUri,
    claims: Option<Extension<Claims>>,
) -> Html<String> {
    let route_path = uri.path();
    let username = claims
        .as_ref()
        .map(|c| c.0.username.as_str())
        .unwrap_or("Guest");
    let role = claims
        .as_ref()
        .map(|c| c.0.role.as_str())
        .unwrap_or("anonymous");

    let nav_html = build_nav_html(route_path, role);

    Html(format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <meta name="route-title" content="{title}">
  <meta name="route-path" content="{route_path}">
  <meta name="nav-section" content="{section}">
  <title>{title} - Universus</title>
  <style>{CSS}</style>
</head>
<body>
  <header>
    <div class="header-brand"><a href="/overview">Universus</a></div>
    <div class="header-user">
      <span class="username">{username}</span>
      <span class="role">[{role}]</span>
      <a href="/account/settings">Settings</a>
      <a href="/login" id="logout-link">Logout</a>
    </div>
  </header>
  <div class="layout">
    <nav>{nav_html}</nav>
    <main>
      <h1>{title}</h1>
      <div id="content" data-route="{route_path}">
        {body}
      </div>
    </main>
  </div>
  <footer>Universus &copy; 2026</footer>
</body>
</html>"#,
        title = title,
        route_path = route_path,
        section = section,
        username = username,
        role = role,
        nav_html = nav_html,
        body = page_body_for(title),
    ))
}

// ---------------------------------------------------------------------------
// Error pages
// ---------------------------------------------------------------------------

async fn not_found_page() -> (StatusCode, Html<String>) {
    (
        StatusCode::NOT_FOUND,
        Html(render_error_page(
            "404 — Not Found",
            "The page you requested does not exist.",
        )),
    )
}

async fn fallback_handler(OriginalUri(uri): OriginalUri) -> (StatusCode, Html<String>) {
    let path = uri.path().to_string();
    (
        StatusCode::NOT_FOUND,
        Html(render_error_page(
            "404 — Not Found",
            &format!("No route matches <code>{}</code>.", path),
        )),
    )
}

fn unauthorized_page(msg: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Html(render_error_page("401 — Unauthorized", msg)),
    )
        .into_response()
}

fn forbidden_page(msg: &str) -> Response {
    (
        StatusCode::FORBIDDEN,
        Html(render_error_page("403 — Forbidden", msg)),
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// JWT auth middleware
// ---------------------------------------------------------------------------

async fn require_authenticated(
    state: Arc<AppState>,
    mut request: axum::http::Request<axum::body::Body>,
    next: Next<axum::body::Body>,
) -> Response {
    match extract_and_validate_jwt(request.headers(), &state.auth_config) {
        Ok(claims) => {
            request.extensions_mut().insert(claims);
            next.run(request).await
        }
        Err(_) => unauthorized_page(
            "You must be logged in to view this page. <a href=\"/login\">Login</a>",
        ),
    }
}

async fn require_admin(
    state: Arc<AppState>,
    mut request: axum::http::Request<axum::body::Body>,
    next: Next<axum::body::Body>,
) -> Response {
    match extract_and_validate_jwt(request.headers(), &state.auth_config) {
        Ok(claims) if claims.role == "admin" => {
            request.extensions_mut().insert(claims);
            next.run(request).await
        }
        Ok(_) => forbidden_page("You do not have permission to access the admin panel."),
        Err(_) => {
            unauthorized_page("You must be logged in as an admin. <a href=\"/login\">Login</a>")
        }
    }
}

fn extract_and_validate_jwt(
    headers: &HeaderMap,
    config: &AuthConfig,
) -> Result<Claims, StatusCode> {
    let auth_header = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let token = platform_auth::extract_bearer_token(auth_header).ok_or(StatusCode::UNAUTHORIZED)?;

    platform_auth::validate_token(config, token).map_err(|_| StatusCode::UNAUTHORIZED)
}

// ---------------------------------------------------------------------------
// HTML rendering helpers
// ---------------------------------------------------------------------------

const CSS: &str = r#"
*{margin:0;padding:0;box-sizing:border-box}
body{font-family:monospace;background:#0a0e17;color:#c0c8d8;min-height:100vh;display:flex;flex-direction:column}
a{color:#4ea8de;text-decoration:none}a:hover{text-decoration:underline}
header{background:#111827;padding:8px 16px;display:flex;justify-content:space-between;align-items:center;border-bottom:1px solid #1e293b}
.header-brand a{color:#e0e7ff;font-size:1.2em;font-weight:bold}
.header-user{display:flex;gap:12px;align-items:center}
.header-user .username{color:#a5b4fc}.header-user .role{color:#64748b;font-size:0.85em}
.layout{display:flex;flex:1}
nav{width:200px;background:#111827;padding:12px;border-right:1px solid #1e293b;overflow-y:auto}
nav ul{list-style:none}
nav li{margin-bottom:2px}
nav a{display:block;padding:4px 8px;color:#94a3b8;border-radius:4px}
nav a:hover{background:#1e293b;color:#e0e7ff;text-decoration:none}
nav a.active{background:#1e40af;color:#fff}
nav .nav-section{color:#475569;font-size:0.8em;text-transform:uppercase;padding:8px 8px 4px;margin-top:8px}
main{flex:1;padding:24px;max-width:1200px}
main h1{margin-bottom:16px;color:#e0e7ff}
footer{background:#111827;padding:8px 16px;text-align:center;color:#475569;font-size:0.8em;border-top:1px solid #1e293b}
.form-group{margin-bottom:12px}
.form-group label{display:block;margin-bottom:4px;color:#94a3b8}
.form-group input{background:#1e293b;border:1px solid #334155;color:#e0e7ff;padding:8px;width:100%;max-width:360px;border-radius:4px}
button{background:#1e40af;color:#fff;border:none;padding:8px 16px;cursor:pointer;border-radius:4px}
button:hover{background:#2563eb}
.error-page{text-align:center;padding:80px 24px}
.error-page h1{font-size:2em;margin-bottom:12px}
.public-page{max-width:480px;margin:60px auto;padding:24px;background:#111827;border-radius:8px;border:1px solid #1e293b}
.public-page h1{text-align:center;margin-bottom:24px}
table{width:100%;border-collapse:collapse;margin-top:12px}
th,td{text-align:left;padding:6px 10px;border-bottom:1px solid #1e293b}
th{color:#94a3b8;font-weight:normal;text-transform:uppercase;font-size:0.8em}
"#;

fn render_public_page(title: &str, body_html: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <title>{title} - Universus</title>
  <style>{CSS}</style>
</head>
<body>
  <header>
    <div class="header-brand"><a href="/">Universus</a></div>
    <div class="header-user">
      <a href="/login">Login</a>
      <a href="/register">Register</a>
    </div>
  </header>
  <div class="public-page">
    <h1>{title}</h1>
    {body_html}
  </div>
  <footer>Universus &copy; 2026</footer>
</body>
</html>"#,
        title = title,
        body_html = body_html,
    )
}

fn render_error_page(title: &str, message: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <title>{title} - Universus</title>
  <style>{CSS}</style>
</head>
<body>
  <header>
    <div class="header-brand"><a href="/">Universus</a></div>
  </header>
  <div class="error-page">
    <h1>{title}</h1>
    <p>{message}</p>
    <p style="margin-top:16px"><a href="/">Return to home page</a></p>
  </div>
  <footer>Universus &copy; 2026</footer>
</body>
</html>"#,
        title = title,
        message = message,
    )
}

fn build_nav_html(current_path: &str, role: &str) -> String {
    let is_admin = role == "admin";

    // Group routes by section.
    let mut sections: Vec<(&str, Vec<(&str, &str)>)> = Vec::new();
    let section_order = ["game", "alliance", "social", "shop", "account", "admin"];

    for section_name in &section_order {
        let mut items = Vec::new();
        for entry in ROUTES {
            if entry.path.ends_with(".html") {
                continue;
            }
            let es = entry.nav_section.unwrap_or("general");
            if es != *section_name {
                continue;
            }
            if entry.access == AccessLevel::Public {
                continue;
            }
            if entry.access == AccessLevel::Admin && !is_admin {
                continue;
            }
            items.push((entry.path, entry.title));
        }
        if !items.is_empty() {
            sections.push((section_name, items));
        }
    }

    let mut html = String::from("<ul>");
    for (section_name, items) in &sections {
        html.push_str(&format!(
            "<li class=\"nav-section\">{}</li>",
            capitalize(section_name)
        ));
        for (path, title) in items {
            let active = if *path == current_path { " active" } else { "" };
            html.push_str(&format!(
                "<li><a href=\"{path}\" class=\"nav-link{active}\">{title}</a></li>"
            ));
        }
    }
    html.push_str("</ul>");
    html
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

/// Generate contextual body HTML for each page type.
fn page_body_for(title: &str) -> String {
    match title {
        "Overview" => r#"<div id="resources"><p>Loading resource overview...</p></div>
<div id="planet-info"><p>Loading planet information...</p></div>"#.to_string(),
        "Buildings" => r#"<div id="building-list"><p>Loading buildings...</p></div>
<div id="build-queue"><p>Loading build queue...</p></div>"#.to_string(),
        "Research" => r#"<div id="research-tree"><p>Loading research tree...</p></div>"#.to_string(),
        "Shipyard" => r#"<div id="ship-list"><p>Loading ships...</p></div>
<div id="defense-list"><p>Loading defenses...</p></div>
<div id="shipyard-queue"><p>Loading shipyard queue...</p></div>"#.to_string(),
        "Fleet" => r#"<div id="fleet-movements"><p>Loading fleet movements...</p></div>
<div id="fleet-dispatch"><p>Loading dispatch form...</p></div>"#.to_string(),
        "Galaxy" => r#"<div id="galaxy-view"><p>Loading galaxy view...</p></div>
<div id="galaxy-controls">
  <label>Galaxy: <input type="number" id="galaxy-num" min="1" max="9" value="1" /></label>
  <label>System: <input type="number" id="system-num" min="1" max="499" value="1" /></label>
  <button id="galaxy-go">Go</button>
</div>"#.to_string(),
        "Leaderboard" => r#"<div id="leaderboard-tabs">
  <button class="tab active" data-tab="overall">Overall</button>
  <button class="tab" data-tab="fleet">Fleet</button>
  <button class="tab" data-tab="research">Research</button>
  <button class="tab" data-tab="buildings">Buildings</button>
</div>
<div id="leaderboard-table"><p>Loading rankings...</p></div>"#.to_string(),
        "Messages" => r#"<div id="message-folders">
  <button class="tab active" data-tab="inbox">Inbox</button>
  <button class="tab" data-tab="sent">Sent</button>
  <button class="tab" data-tab="combat">Combat Reports</button>
  <button class="tab" data-tab="espionage">Espionage Reports</button>
</div>
<div id="message-list"><p>Loading messages...</p></div>"#.to_string(),
        "Shop" | "Matrix Shop" => r#"<div id="shop-items"><p>Loading shop...</p></div>
<div id="dark-matter-balance"><p>Loading balance...</p></div>"#.to_string(),
        "Notifications" => r#"<div id="notification-list"><p>Loading notifications...</p></div>"#.to_string(),
        "Chat" => r#"<div id="chat-rooms"><p>Loading chat rooms...</p></div>
<div id="chat-messages"></div>
<div id="chat-input">
  <input type="text" id="msg-input" placeholder="Type a message..." />
  <button id="msg-send">Send</button>
</div>"#.to_string(),
        "Alliance Dashboard" => r#"<div id="alliance-info"><p>Loading alliance info...</p></div>
<div id="alliance-members"><p>Loading members...</p></div>"#.to_string(),
        "Alliance Wars" => r#"<div id="war-list"><p>Loading active wars...</p></div>"#.to_string(),
        "Alliance Diplomacy" => r#"<div id="diplomacy-status"><p>Loading diplomacy...</p></div>"#.to_string(),
        "Alliance Management" => r#"<div id="alliance-settings"><p>Loading settings...</p></div>"#.to_string(),
        "Account Settings" => r#"<form id="account-form">
  <div class="form-group"><label for="display-name">Display Name</label><input type="text" id="display-name" name="display_name" /></div>
  <div class="form-group"><label for="email">Email</label><input type="email" id="email" name="email" /></div>
  <button type="submit">Save Changes</button>
</form>"#.to_string(),
        "Security Dashboard" => r#"<div id="security-status"><p>Loading security status...</p></div>
<div id="active-sessions"><p>Loading sessions...</p></div>"#.to_string(),
        "Admin Dashboard" => r#"<div id="admin-stats"><p>Loading admin stats...</p></div>
<table><thead><tr><th>Metric</th><th>Value</th></tr></thead><tbody id="admin-metrics"></tbody></table>"#.to_string(),
        "Admin Users" => r#"<div id="user-search"><input type="text" id="user-query" placeholder="Search users..." /><button id="user-search-btn">Search</button></div>
<table><thead><tr><th>ID</th><th>Username</th><th>Role</th><th>Status</th><th>Actions</th></tr></thead><tbody id="user-table"></tbody></table>"#.to_string(),
        "Admin Monitoring" => r#"<div id="system-health"><p>Loading system health...</p></div>
<div id="service-status"><p>Loading service status...</p></div>"#.to_string(),
        _ => format!("<p>Content for <code>{}</code> is loading...</p>", title),
    }
}

// ---------------------------------------------------------------------------
// Timestamp helper (no chrono dependency — simple ISO string)
// ---------------------------------------------------------------------------

fn chrono_now_iso() -> String {
    // We avoid pulling in the `chrono` crate; for uptime tracking a simple
    // epoch-second string is sufficient.  In production the frontend would
    // read real wall-clock from the OS.
    "2026-03-08T00:00:00Z".to_string()
}

// =========================================================================
// Tests
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    fn test_auth_config() -> AuthConfig {
        AuthConfig {
            jwt_secret: "test-frontend-secret".to_string(),
            jwt_expiry_seconds: 86_400,
            ..AuthConfig::default()
        }
    }

    fn test_state() -> AppState {
        AppState::new(test_auth_config())
    }

    fn test_router() -> Router {
        build_router_with_state(test_state())
    }

    fn generate_token(user_id: &str, username: &str, role: &str) -> String {
        let config = test_auth_config();
        platform_auth::generate_token(&config, user_id, username, role, None).unwrap()
    }

    fn user_token() -> String {
        generate_token("user-1", "player1", "user")
    }

    fn admin_token() -> String {
        generate_token("admin-1", "admin_user", "admin")
    }

    // --- Health / Ready ---

    #[tokio::test]
    async fn health_returns_ok() {
        let app = test_router();
        let resp = app
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(resp.into_body()).await.unwrap();
        let parsed: ServiceHealth = serde_json::from_slice(&body).unwrap();
        assert_eq!(parsed.status, "ok");
        assert_eq!(parsed.service, SERVICE_NAME);
    }

    #[tokio::test]
    async fn ready_returns_ok() {
        let app = test_router();
        let resp = app
            .oneshot(Request::get("/ready").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    // --- Public pages ---

    #[tokio::test]
    async fn home_page_is_public() {
        let app = test_router();
        let resp = app
            .oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(resp.into_body()).await.unwrap();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("Home"));
        assert!(html.contains("Universus"));
    }

    #[tokio::test]
    async fn login_page_is_public() {
        let app = test_router();
        let resp = app
            .oneshot(Request::get("/login").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(resp.into_body()).await.unwrap();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("login-form"));
        assert!(html.contains("username"));
    }

    #[tokio::test]
    async fn register_page_is_public() {
        let app = test_router();
        let resp = app
            .oneshot(Request::get("/register").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(resp.into_body()).await.unwrap();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("register-form"));
        assert!(html.contains("password_confirm"));
    }

    #[tokio::test]
    async fn forgot_password_page_is_public() {
        let app = test_router();
        let resp = app
            .oneshot(
                Request::get("/forgot-password")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    // --- Auth-required routes ---

    #[tokio::test]
    async fn overview_requires_auth() {
        let app = test_router();
        let resp = app
            .oneshot(Request::get("/overview").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn overview_with_valid_token() {
        let app = test_router();
        let token = user_token();
        let resp = app
            .oneshot(
                Request::get("/overview")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(resp.into_body()).await.unwrap();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("Overview"));
        assert!(html.contains("player1"));
        assert!(html.contains("resources"));
    }

    #[tokio::test]
    async fn buildings_with_valid_token() {
        let app = test_router();
        let token = user_token();
        let resp = app
            .oneshot(
                Request::get("/buildings")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(resp.into_body()).await.unwrap();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("Buildings"));
        assert!(html.contains("build-queue"));
    }

    #[tokio::test]
    async fn galaxy_with_valid_token() {
        let app = test_router();
        let token = user_token();
        let resp = app
            .oneshot(
                Request::get("/galaxy")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(resp.into_body()).await.unwrap();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("galaxy-view"));
        assert!(html.contains("galaxy-controls"));
    }

    #[tokio::test]
    async fn fleet_with_valid_token() {
        let app = test_router();
        let token = user_token();
        let resp = app
            .oneshot(
                Request::get("/fleet")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(resp.into_body()).await.unwrap();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("fleet-movements"));
    }

    #[tokio::test]
    async fn invalid_token_rejected() {
        let app = test_router();
        let resp = app
            .oneshot(
                Request::get("/overview")
                    .header("Authorization", "Bearer invalid-garbage-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn missing_bearer_prefix_rejected() {
        let app = test_router();
        let token = user_token();
        let resp = app
            .oneshot(
                Request::get("/overview")
                    .header("Authorization", token) // no "Bearer " prefix
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    // --- Admin routes ---

    #[tokio::test]
    async fn admin_requires_auth() {
        let app = test_router();
        let resp = app
            .oneshot(Request::get("/admin").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn admin_rejects_normal_user() {
        let app = test_router();
        let token = user_token();
        let resp = app
            .oneshot(
                Request::get("/admin")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn admin_allows_admin_user() {
        let app = test_router();
        let token = admin_token();
        let resp = app
            .oneshot(
                Request::get("/admin")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(resp.into_body()).await.unwrap();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("Admin"));
    }

    #[tokio::test]
    async fn admin_dashboard_accessible() {
        let app = test_router();
        let token = admin_token();
        let resp = app
            .oneshot(
                Request::get("/admin/dashboard")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(resp.into_body()).await.unwrap();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("Admin Dashboard"));
        assert!(html.contains("admin-stats"));
    }

    #[tokio::test]
    async fn admin_users_page() {
        let app = test_router();
        let token = admin_token();
        let resp = app
            .oneshot(
                Request::get("/admin/users")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(resp.into_body()).await.unwrap();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("user-search"));
    }

    // --- Navigation API ---

    #[tokio::test]
    async fn nav_api_requires_auth() {
        let app = test_router();
        let resp = app
            .oneshot(Request::get("/api/nav").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn nav_api_returns_items_for_user() {
        let app = test_router();
        let token = user_token();
        let resp = app
            .oneshot(
                Request::get("/api/nav")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(resp.into_body()).await.unwrap();
        let items: Vec<NavigationItem> = serde_json::from_slice(&body).unwrap();
        assert!(!items.is_empty());
        // Normal user should NOT see admin items
        assert!(items.iter().all(|i| i.section != "admin"));
        // Should see game items
        assert!(items.iter().any(|i| i.section == "game"));
    }

    #[tokio::test]
    async fn nav_api_shows_admin_items_for_admin() {
        let app = test_router();
        let token = admin_token();
        let resp = app
            .oneshot(
                Request::get("/api/nav")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(resp.into_body()).await.unwrap();
        let items: Vec<NavigationItem> = serde_json::from_slice(&body).unwrap();
        assert!(items.iter().any(|i| i.section == "admin"));
    }

    // --- Fallback / 404 ---

    #[tokio::test]
    async fn unknown_route_returns_404() {
        let app = test_router();
        let resp = app
            .oneshot(
                Request::get("/this-does-not-exist")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let body = hyper::body::to_bytes(resp.into_body()).await.unwrap();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("404"));
    }

    #[tokio::test]
    async fn explicit_404_route() {
        let app = test_router();
        let resp = app
            .oneshot(Request::get("/404").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    // --- HTML template structure ---

    #[tokio::test]
    async fn authenticated_page_has_nav_and_header() {
        let app = test_router();
        let token = user_token();
        let resp = app
            .oneshot(
                Request::get("/messages")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(resp.into_body()).await.unwrap();
        let html = String::from_utf8_lossy(&body);
        // Has proper structure
        assert!(html.contains("<header>"));
        assert!(html.contains("<nav>"));
        assert!(html.contains("<main>"));
        assert!(html.contains("<footer>"));
        // Has navigation links
        assert!(html.contains("nav-link"));
        // Has user info
        assert!(html.contains("player1"));
        // Has page-specific content
        assert!(html.contains("message-folders"));
    }

    #[tokio::test]
    async fn html_page_has_correct_meta_tags() {
        let app = test_router();
        let token = user_token();
        let resp = app
            .oneshot(
                Request::get("/research")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = hyper::body::to_bytes(resp.into_body()).await.unwrap();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("route-title"));
        assert!(html.contains("route-path"));
        assert!(html.contains("nav-section"));
        assert!(html.contains("<title>Research - Universus</title>"));
    }

    // --- Account pages ---

    #[tokio::test]
    async fn account_settings_page() {
        let app = test_router();
        let token = user_token();
        let resp = app
            .oneshot(
                Request::get("/account/settings")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(resp.into_body()).await.unwrap();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("account-form"));
    }

    // --- Alliance pages ---

    #[tokio::test]
    async fn alliance_dashboard_page() {
        let app = test_router();
        let token = user_token();
        let resp = app
            .oneshot(
                Request::get("/alliance/dashboard")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(resp.into_body()).await.unwrap();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("alliance-info"));
        assert!(html.contains("alliance-members"));
    }

    // --- .html route duplicates ---

    #[tokio::test]
    async fn html_extension_route_works() {
        let app = test_router();
        let token = user_token();
        let resp = app
            .oneshot(
                Request::get("/buildings.html")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(resp.into_body()).await.unwrap();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("Buildings"));
    }

    // --- Listen port helper ---

    #[test]
    fn default_port() {
        // When no PORT env var is set (or invalid), use the default.
        assert_eq!(listen_port(3005), 3005);
    }

    // --- Utility tests ---

    #[test]
    fn capitalize_works() {
        assert_eq!(capitalize("game"), "Game");
        assert_eq!(capitalize("admin"), "Admin");
        assert_eq!(capitalize(""), "");
    }

    #[test]
    fn service_status_serializes() {
        let s = ServiceStatus {
            status: "ok".to_string(),
            service: "test".to_string(),
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"status\":\"ok\""));
    }

    #[test]
    fn navigation_item_serializes() {
        let item = NavigationItem {
            path: "/overview".to_string(),
            title: "Overview".to_string(),
            section: "game".to_string(),
            active: true,
        };
        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("\"active\":true"));
    }

    #[test]
    fn render_error_page_contains_message() {
        let html = render_error_page("Test Error", "Something went wrong");
        assert!(html.contains("Test Error"));
        assert!(html.contains("Something went wrong"));
        assert!(html.contains("Return to home page"));
    }

    #[test]
    fn render_public_page_contains_body() {
        let html = render_public_page("Login", "<form>test</form>");
        assert!(html.contains("Login"));
        assert!(html.contains("<form>test</form>"));
        assert!(html.contains("Universus"));
    }

    #[test]
    fn page_body_for_overview() {
        let body = page_body_for("Overview");
        assert!(body.contains("resources"));
        assert!(body.contains("planet-info"));
    }

    #[test]
    fn page_body_for_unknown() {
        let body = page_body_for("Something Random");
        assert!(body.contains("Something Random"));
        assert!(body.contains("loading"));
    }

    #[test]
    fn build_nav_html_for_user() {
        let nav = build_nav_html("/overview", "user");
        assert!(nav.contains("Overview"));
        assert!(nav.contains("Buildings"));
        // Should NOT contain admin links
        assert!(!nav.contains("Admin Dashboard"));
    }

    #[test]
    fn build_nav_html_for_admin() {
        let nav = build_nav_html("/admin/dashboard", "admin");
        assert!(nav.contains("Admin Dashboard"));
        assert!(nav.contains("Overview"));
    }

    #[test]
    fn route_entries_have_valid_paths() {
        for entry in ROUTES {
            assert!(
                entry.path.starts_with('/'),
                "path must start with /: {}",
                entry.path
            );
            assert!(
                !entry.title.is_empty(),
                "title must not be empty for {}",
                entry.path
            );
        }
    }

    #[test]
    fn no_duplicate_route_paths() {
        let mut seen = std::collections::HashSet::new();
        for entry in ROUTES {
            assert!(
                seen.insert(entry.path),
                "duplicate route path: {}",
                entry.path
            );
        }
    }
}
