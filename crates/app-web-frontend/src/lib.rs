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

use axum::body::Bytes;
use axum::extract::{Extension, OriginalUri, Path};
use axum::http::{
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, COOKIE, HOST, ORIGIN, SET_COOKIE},
    HeaderMap, Method, StatusCode,
};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{any, get};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tower_http::services::ServeDir;

use platform_auth::{AuthConfig, Claims, UserRole};

mod ui;

use ui::CLIENT_JS;

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
    /// Fixed upstream used by the same-origin `/game-api/*` bridge.
    /// Keeping the URL server-side avoids exposing an arbitrary proxy target.
    pub api_gateway_url: String,
    pub http_client: reqwest::Client,
    pub secure_cookies: bool,
    pub assets_dir: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CookieSecurityError {
    InvalidBoolean { name: &'static str },
    InsecureProductionCookie,
}

impl std::fmt::Display for CookieSecurityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidBoolean { name } => write!(
                formatter,
                "{name} must be one of true/false, 1/0, yes/no, or on/off"
            ),
            Self::InsecureProductionCookie => write!(
                formatter,
                "COOKIE_SECURE=false is forbidden in production-like environments; use TLS or set UNIVERSUS_ALLOW_INSECURE_LOCAL_HTTP_COOKIE=true only for an isolated local HTTP test"
            ),
        }
    }
}

impl std::error::Error for CookieSecurityError {}

impl AppState {
    pub fn new(auth_config: AuthConfig) -> Self {
        Self::try_new(auth_config).expect("invalid browser cookie security configuration")
    }

    pub fn try_new(auth_config: AuthConfig) -> Result<Self, CookieSecurityError> {
        let secure_cookies = cookie_security_from_environment()?;
        Ok(Self {
            auth_config,
            service_name: SERVICE_NAME.to_string(),
            start_time: chrono_now_iso(),
            api_gateway_url: std::env::var("API_GATEWAY_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:3000".to_string())
                .trim_end_matches('/')
                .to_string(),
            http_client: reqwest::Client::new(),
            secure_cookies,
            assets_dir: std::env::var("ASSETS_DIR").unwrap_or_else(|_| "assets".to_string()),
        })
    }

    pub fn from_env() -> Self {
        Self::try_from_env().expect("invalid browser cookie security configuration")
    }

    pub fn try_from_env() -> Result<Self, CookieSecurityError> {
        Self::try_new(AuthConfig::from_env())
    }
}

fn cookie_security_from_environment() -> Result<bool, CookieSecurityError> {
    let environment = ["UNIVERSUS_ENV", "APP_ENV", "ENVIRONMENT", "RUST_ENV"]
        .into_iter()
        .find_map(|name| std::env::var(name).ok())
        .unwrap_or_else(|| "development".to_string());
    let secure = std::env::var("COOKIE_SECURE").ok();
    let local_override = std::env::var("UNIVERSUS_ALLOW_INSECURE_LOCAL_HTTP_COOKIE").ok();
    resolve_cookie_security(&environment, secure.as_deref(), local_override.as_deref())
}

fn resolve_cookie_security(
    environment: &str,
    secure: Option<&str>,
    local_override: Option<&str>,
) -> Result<bool, CookieSecurityError> {
    let configured_secure = parse_boolean("COOKIE_SECURE", secure)?;
    let override_enabled =
        parse_boolean("UNIVERSUS_ALLOW_INSECURE_LOCAL_HTTP_COOKIE", local_override)?
            .unwrap_or(false);
    let production_like = matches!(
        environment.trim().to_ascii_lowercase().as_str(),
        "production" | "prod" | "staging" | "stage"
    );

    if production_like {
        match configured_secure {
            None | Some(true) => Ok(true),
            Some(false) if override_enabled => Ok(false),
            Some(false) => Err(CookieSecurityError::InsecureProductionCookie),
        }
    } else {
        Ok(configured_secure.unwrap_or(false))
    }
}

fn parse_boolean(
    name: &'static str,
    value: Option<&str>,
) -> Result<Option<bool>, CookieSecurityError> {
    let Some(value) = value else {
        return Ok(None);
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(Some(true)),
        "0" | "false" | "no" | "off" => Ok(Some(false)),
        _ => Err(CookieSecurityError::InvalidBoolean { name }),
    }
}

// ---------------------------------------------------------------------------
// API types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub status: String,
    pub service: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dependency: Option<String>,
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
    let assets_dir = state.assets_dir.clone();
    let shared = Arc::new(state);

    let mut public_routes = Router::new()
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
        .route("/game-api/*path", any(gateway_proxy_handler))
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
        .nest_service("/assets", ServeDir::new(assets_dir))
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
    let state = AppState::try_from_env().expect("invalid browser cookie security configuration");
    state
        .auth_config
        .validate_runtime()
        .expect("invalid authentication runtime configuration");
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

async fn ready_handler(Extension(state): Extension<Arc<AppState>>) -> impl IntoResponse {
    let upstream = format!("{}/ready", state.api_gateway_url);
    let available = state
        .http_client
        .get(upstream)
        .timeout(std::time::Duration::from_millis(750))
        .send()
        .await
        .map(|response| response.status().is_success())
        .unwrap_or(false);
    let status = if available {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (
        status,
        Json(ServiceStatus {
            status: if available { "ok" } else { "unavailable" }.to_string(),
            service: SERVICE_NAME.to_string(),
            dependency: Some("app-api-gateway".to_string()),
        }),
    )
}

// ---------------------------------------------------------------------------
// Same-origin API bridge
// ---------------------------------------------------------------------------

/// Relays the gateway's existing `/api/*` contract through the frontend
/// origin. This lets the server-rendered UI work in browsers without a broad
/// CORS policy and keeps the upstream host out of client-side configuration.
async fn gateway_proxy_handler(
    Path(path): Path<String>,
    OriginalUri(uri): OriginalUri,
    Extension(state): Extension<Arc<AppState>>,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if path != "health" && path != "ready" && !path.starts_with("api/") {
        return proxy_error(StatusCode::NOT_FOUND, "Gateway path is not available");
    }
    if path.split('/').any(|segment| segment == "..") {
        return proxy_error(StatusCode::BAD_REQUEST, "Invalid gateway path");
    }
    if !matches!(
        method,
        Method::GET | Method::HEAD | Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    ) {
        return proxy_error(StatusCode::METHOD_NOT_ALLOWED, "Method is not supported");
    }
    if !mutation_origin_allowed(&method, &headers, state.secure_cookies) {
        return proxy_error(
            StatusCode::FORBIDDEN,
            "Cross-origin API mutations are not allowed",
        );
    }

    let mut upstream_url = format!("{}/{}", state.api_gateway_url, path);
    if let Some(query) = uri.query() {
        upstream_url.push('?');
        upstream_url.push_str(query);
    }

    let upstream_method = match reqwest::Method::from_bytes(method.as_str().as_bytes()) {
        Ok(method) => method,
        Err(_) => return proxy_error(StatusCode::METHOD_NOT_ALLOWED, "Invalid method"),
    };
    let mut request = state.http_client.request(upstream_method, upstream_url);
    if let Some(value) = headers.get(ACCEPT).and_then(|value| value.to_str().ok()) {
        request = request.header(reqwest::header::ACCEPT, value);
    }
    if let Some(value) = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
    {
        request = request.header(reqwest::header::CONTENT_TYPE, value);
    }
    let bearer = bearer_value(&headers).map(str::to_string);
    if let Some(value) = bearer.as_deref() {
        request = request.bearer_auth(value);
    }
    if !body.is_empty() {
        request = request.body(body);
    }

    let upstream = match request.send().await {
        Ok(response) => response,
        Err(error) => {
            tracing::warn!(%error, "api gateway proxy request failed");
            let mut response = proxy_error(
                StatusCode::BAD_GATEWAY,
                "The game API is temporarily unavailable",
            );
            if path == "api/auth/logout" {
                if let Ok(value) = expired_session_cookie(state.secure_cookies).parse() {
                    response.headers_mut().insert(SET_COOKIE, value);
                }
            }
            return response;
        }
    };
    let status = upstream.status();
    let content_type = upstream
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .cloned();
    let bytes = match upstream.bytes().await {
        Ok(bytes) => bytes,
        Err(error) => {
            tracing::warn!(%error, "api gateway proxy response failed");
            return proxy_error(
                StatusCode::BAD_GATEWAY,
                "The game API returned an unreadable response",
            );
        }
    };

    let session_cookie =
        session_cookie_for_response(&path, status, bytes.as_ref(), state.secure_cookies);
    let mut response = (status, bytes.to_vec()).into_response();
    if let Some(content_type) = content_type {
        if let Ok(value) = content_type.to_str().unwrap_or_default().parse() {
            response.headers_mut().insert(CONTENT_TYPE, value);
        }
    }
    if let Some(cookie) = session_cookie {
        if let Ok(value) = cookie.parse() {
            response.headers_mut().insert(SET_COOKIE, value);
        }
    }
    response
}

fn mutation_origin_allowed(method: &Method, headers: &HeaderMap, secure_cookies: bool) -> bool {
    if matches!(*method, Method::GET | Method::HEAD) {
        return true;
    }

    let fetch_site = headers
        .get("sec-fetch-site")
        .and_then(|value| value.to_str().ok());
    if fetch_site.is_some_and(|value| value.eq_ignore_ascii_case("cross-site")) {
        return false;
    }

    let Some(origin) = headers.get(ORIGIN).and_then(|value| value.to_str().ok()) else {
        // Cookie-backed mutations and anonymous auth submissions must prove
        // browser same-origin. Non-browser clients may omit Origin only when
        // they use an explicit Authorization header and no session cookie.
        return fetch_site.is_some_and(|value| value.eq_ignore_ascii_case("same-origin"))
            || (headers.contains_key(AUTHORIZATION) && !headers.contains_key(COOKIE));
    };
    let Some(host) = headers.get(HOST).and_then(|value| value.to_str().ok()) else {
        return false;
    };
    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| matches!(*value, "http" | "https"))
        .unwrap_or(if secure_cookies { "https" } else { "http" });
    let Ok(origin) = reqwest::Url::parse(origin) else {
        return false;
    };
    let Ok(expected) = reqwest::Url::parse(&format!("{scheme}://{host}/")) else {
        return false;
    };

    origin.scheme() == expected.scheme()
        && origin
            .host_str()
            .zip(expected.host_str())
            .is_some_and(|(actual, expected)| actual.eq_ignore_ascii_case(expected))
        && origin.port_or_known_default() == expected.port_or_known_default()
}

fn session_cookie_for_response(
    path: &str,
    status: reqwest::StatusCode,
    body: &[u8],
    secure: bool,
) -> Option<String> {
    let secure_attribute = if secure { "; Secure" } else { "" };
    if path == "api/auth/logout" {
        return Some(expired_session_cookie(secure));
    }
    if !status.is_success() {
        return None;
    }
    if !matches!(path, "api/auth/login" | "api/auth/register") {
        return None;
    }

    let payload: serde_json::Value = serde_json::from_slice(body).ok()?;
    let data = payload.get("data")?;
    let token = data.get("token")?.as_str()?;
    if token.is_empty()
        || token
            .bytes()
            .any(|byte| matches!(byte, b';' | b'\r' | b'\n' | b' '))
    {
        return None;
    }
    let max_age = data
        .get("expiresInSeconds")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(604_800)
        .clamp(60, 31_536_000);
    Some(format!(
        "universus_token={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}{secure_attribute}"
    ))
}

fn expired_session_cookie(secure: bool) -> String {
    let secure_attribute = if secure { "; Secure" } else { "" };
    format!("universus_token=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{secure_attribute}")
}

fn proxy_error(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(serde_json::json!({
            "success": false,
            "error": message,
        })),
    )
        .into_response()
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
        .map(|claims| role_is_admin(&claims.0.role))
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
        r#"<form id="login-form" data-auth-form="login" method="post" action="/game-api/api/auth/login">
  <div class="form-group">
    <label for="email">Email</label>
    <input type="email" id="email" name="email" required autocomplete="username" />
  </div>
  <div class="form-group">
    <label for="password">Password</label>
    <input type="password" id="password" name="password" required autocomplete="current-password" />
  </div>
  <button type="submit">Login</button>
  <p class="form-feedback" role="status" aria-live="polite"></p>
  <p><a href="/forgot-password">Forgot your password?</a></p>
  <p>Don't have an account? <a href="/register">Register</a></p>
</form>"#,
    ))
}

async fn register_page() -> Html<String> {
    Html(render_public_page(
        "Register",
        r#"<form id="register-form" data-auth-form="register" method="post" action="/game-api/api/auth/register">
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
  <p class="form-feedback" role="status" aria-live="polite"></p>
  <p>Already have an account? <a href="/login">Login</a></p>
</form>"#,
    ))
}

async fn forgot_password_page() -> Html<String> {
    Html(render_public_page(
        "Forgot Password",
        r#"<div class="notice-card" role="note">
  <h2>Account recovery</h2>
  <p>The current game API does not expose a password-reset contract yet. Contact a universe administrator to recover access; this page will not pretend to submit a request that cannot be fulfilled.</p>
</div>
<form id="forgot-form">
  <div class="form-group">
    <label for="email">Email Address</label>
    <input type="email" id="email" name="email" autocomplete="email" disabled />
  </div>
  <button type="submit" disabled>Send Reset Link</button>
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
        r##"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <meta name="route-title" content="{title}">
  <meta name="route-path" content="{route_path}">
  <meta name="nav-section" content="{section}">
  <meta name="api-prefix" content="/game-api">
  <title>{title} - Universus</title>
  <style>{CSS}</style>
</head>
<body data-page="{title}">
  <a class="skip-link" href="#content">Skip to content</a>
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
  <script>{client_js}</script>
</body>
</html>"##,
        title = title,
        route_path = route_path,
        section = section,
        username = username,
        role = role,
        nav_html = nav_html,
        body = page_body_for(title),
        client_js = CLIENT_JS,
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
        Ok(claims) if role_is_admin(&claims.role) => {
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
    let token = bearer_value(headers).ok_or(StatusCode::UNAUTHORIZED)?;

    platform_auth::validate_token(config, token).map_err(|_| StatusCode::UNAUTHORIZED)
}

/// Reads a bearer token from an API-style Authorization header or the
/// same-site browser cookie created by the login flow.
fn bearer_value(headers: &HeaderMap) -> Option<&str> {
    if let Some(token) = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(platform_auth::extract_bearer_token)
    {
        return Some(token);
    }

    headers
        .get(COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|cookie| {
                let (name, value) = cookie.trim().split_once('=')?;
                (name == "universus_token" && !value.is_empty()).then_some(value)
            })
        })
}

// ---------------------------------------------------------------------------
// HTML rendering helpers
// ---------------------------------------------------------------------------

const CSS: &str = r#"
*:where(:not(dialog)){box-sizing:border-box;margin:0}
:root{color-scheme:dark;--bg:#050912;--surface:#0d1524;--surface-2:#111e32;--line:#21314a;--text:#d8e2f1;--muted:#8391a8;--accent:#58c7ff;--accent-2:#7c6fff;--good:#55d6a7;--warn:#ffbd5b;--danger:#ff6b7a;--radius:12px;font-family:Inter,ui-sans-serif,system-ui,-apple-system,"Segoe UI",sans-serif}
body{background:radial-gradient(circle at 70% -10%,#102c4d 0,transparent 34rem),var(--bg);color:var(--text);min-height:100vh;display:flex;flex-direction:column;line-height:1.5}
a{color:var(--accent);text-decoration:none}a:hover{text-decoration:underline}
button,.button,input,select,textarea{font:inherit}
button,.button{display:inline-flex;align-items:center;justify-content:center;gap:.45rem;background:linear-gradient(135deg,#1677ae,#4a5bd5);color:#fff;border:1px solid #5bb9ee55;padding:.62rem .9rem;cursor:pointer;border-radius:8px;font-weight:700}
button:hover,.button:hover{filter:brightness(1.13);text-decoration:none}button:disabled,.button:disabled,input:disabled{cursor:not-allowed;opacity:.48}
button.secondary,.button.secondary{background:#15233a;border-color:var(--line);color:#c7d5e9}
input,select,textarea{width:100%;background:#09111f;border:1px solid #30415d;color:var(--text);padding:.62rem .7rem;border-radius:7px}
input:focus,select:focus,textarea:focus,button:focus-visible,a:focus-visible{outline:2px solid var(--accent);outline-offset:2px}
header{position:sticky;top:0;z-index:10;background:#07101bea;backdrop-filter:blur(12px);padding:.75rem 1.25rem;display:flex;justify-content:space-between;align-items:center;border-bottom:1px solid var(--line)}
.header-brand a{color:#fff;font-size:1.25rem;font-weight:900;letter-spacing:.12em;text-transform:uppercase}
.header-user{display:flex;gap:.85rem;align-items:center;flex-wrap:wrap}.header-user .username{color:#b8dfff;font-weight:700}.header-user .role{color:var(--muted);font-size:.78rem}
.skip-link{position:fixed;top:-5rem;left:1rem;z-index:100;background:#fff;color:#000;padding:.5rem}.skip-link:focus{top:1rem}
.layout{display:flex;flex:1;min-width:0}
nav{width:220px;flex:0 0 220px;background:#08101df2;padding:1rem .75rem;border-right:1px solid var(--line);overflow-y:auto}
nav ul{list-style:none;padding:0}nav li{margin-bottom:2px}nav a{display:block;padding:.45rem .7rem;color:#9eabc0;border-radius:7px;font-size:.88rem}nav a:hover{background:#17243a;color:#fff;text-decoration:none}nav a.active{background:linear-gradient(90deg,#174c71,#293c75);color:#fff;box-shadow:inset 3px 0 var(--accent)}
nav .nav-section{color:#5e708d;font-size:.68rem;font-weight:800;letter-spacing:.16em;text-transform:uppercase;padding:1rem .7rem .35rem}
main{flex:1;min-width:0;padding:clamp(1rem,3vw,2.25rem);max-width:1500px;margin-inline:auto;width:100%}main>h1{margin-bottom:1.25rem;color:#f1f6ff;font-size:clamp(1.45rem,3vw,2.1rem);letter-spacing:-.035em}
footer{background:#070d17;padding:.65rem 1rem;text-align:center;color:#526077;font-size:.74rem;border-top:1px solid var(--line)}
h2{font-size:1.08rem;color:#f0f5ff}h3{font-size:.95rem;color:#e8f1ff}small{color:var(--muted)}hr{border:0;border-top:1px solid var(--line);margin:1.25rem 0}
.eyebrow{display:block;color:#68bfe8;font-size:.68rem;font-weight:800;letter-spacing:.14em;text-transform:uppercase;margin-bottom:.25rem}
.panel,.notice-card{background:linear-gradient(145deg,#0e1828,#0a1220);border:1px solid var(--line);border-radius:var(--radius);padding:1rem;box-shadow:0 16px 40px #0004}.panel+.panel{margin-top:1rem}
.panel-heading{display:flex;align-items:center;justify-content:space-between;gap:1rem;margin-bottom:.75rem}
.dashboard-grid,.messages-layout{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:1rem}.dashboard-grid.wide-first{grid-template-columns:minmax(0,2fr) minmax(260px,1fr)}
.metric-grid{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:.75rem;margin:1rem 0}.metric-grid.compact-metrics{grid-template-columns:repeat(3,minmax(0,1fr))}.metric-card{background:#0c1727;border:1px solid var(--line);border-radius:10px;padding:.8rem}.metric-card span{display:block;color:var(--muted);font-size:.75rem;text-transform:uppercase}.metric-card strong{display:block;margin-top:.25rem;font-size:1.18rem;color:#fff}
.hero-panel{position:relative;overflow:hidden;min-height:210px;padding:1.25rem;border:1px solid #385478;border-radius:14px;background:radial-gradient(circle at 78% 35%,#2d8bb077,transparent 15rem),linear-gradient(135deg,#0c1c31,#121331);display:flex;flex-direction:column;justify-content:flex-end}.hero-panel h2{font-size:1.7rem}.hero-panel progress{width:min(460px,100%)}
.planet-hero{padding:0}.planet-banner{width:100%;height:260px;object-fit:cover;display:block}.planet-banner.asset-missing{visibility:hidden}.planet-hero:has(.asset-missing){background:radial-gradient(circle at 72% 40%,#4ac4dd 0 8%,#173d68 9% 17%,transparent 18%),linear-gradient(125deg,#0a1629,#151338)}.hero-overlay{position:absolute;inset:auto 0 0;padding:3.5rem 1.25rem 1.25rem;background:linear-gradient(transparent,#050912e8)}
.landing-hero{max-width:900px;margin:5vh auto 2rem;padding:clamp(1rem,5vw,3rem);text-align:center}.landing-hero h2{font-size:clamp(2rem,7vw,4.2rem);line-height:1.02;margin:.5rem 0 1rem}.landing-hero p{color:var(--muted);font-size:1.1rem;max-width:680px;margin:auto}.landing-hero .button-row{justify-content:center;margin-top:1.5rem}.feature-grid{display:grid;grid-template-columns:repeat(3,1fr);gap:.75rem}.feature-grid article{background:#0d1725;border:1px solid var(--line);padding:1rem;border-radius:10px}.feature-grid strong,.feature-grid span{display:block}.feature-grid strong{font-size:1.4rem;color:var(--accent)}.feature-grid span{color:var(--muted)}
.card-grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(190px,1fr));gap:.75rem;margin-top:.8rem}.tech-card,.shop-card{background:#0a1423;border:1px solid var(--line);border-radius:10px;padding:.85rem;display:grid;gap:.55rem}.tech-card:hover,.shop-card:hover{border-color:#3c678d}.matrix-only{box-shadow:inset 0 0 25px #703fff1f}
.level-chip,.rank-badge,.count-badge,.status-chip,.rarity{display:inline-flex;width:max-content;border:1px solid #39516f;border-radius:999px;padding:.15rem .45rem;color:#a9dfff;background:#10233a;font-size:.7rem;font-weight:800}.count-badge{font-size:.85rem}.status-chip.ally{color:#a8f5d9;border-color:#27745c}.status-chip.war{color:#ffc0c7;border-color:#843745}
.planet-list,.movement-list,.relation-list,.action-list{display:grid;gap:.4rem;margin-top:.65rem}.planet-row,.movement-row,.message-row{width:100%;display:grid;grid-template-columns:auto 1fr auto;align-items:center;text-align:left;gap:.7rem;background:#091321;border:1px solid transparent;color:var(--text);padding:.65rem;border-radius:8px}.planet-row:hover,.movement-row:hover,.message-row:hover{border-color:#315275;background:#0f1c2e;text-decoration:none}.planet-row small,.movement-row small,.message-row small{display:block}.planet-dot{width:1.1rem;height:1.1rem;border-radius:50%;background:radial-gradient(circle at 35% 30%,#91efff,#2c7caa 35%,#142a50 70%);box-shadow:0 0 12px #4acfff88}.mission-icon{display:grid;place-items:center;width:2rem;height:2rem;border-radius:7px;background:#23355b;color:#fff}.message-row.unread{border-left:3px solid var(--accent)}.unread-dot{width:.45rem;height:.45rem;background:var(--accent);border-radius:50%}.message-row:not(.unread) .unread-dot{opacity:0}
.detail-list{display:grid;margin-top:.7rem}.detail-list>div,.mini-cost>div,.relation-row{display:flex;justify-content:space-between;gap:1rem;padding:.45rem 0;border-bottom:1px solid #1b2b41}.detail-list dt,.mini-cost dt{color:var(--muted)}.detail-list dd,.mini-cost dd{text-align:right}.action-list a{display:flex;justify-content:space-between;padding:.65rem;border:1px solid var(--line);border-radius:8px}
.queue-item{display:grid;gap:.3rem;padding:.7rem 0;border-bottom:1px solid var(--line)}progress{accent-color:var(--accent);height:.55rem}
.stacked-form,.inline-form{display:grid;gap:.75rem;margin-top:.8rem}.stacked-form label,.inline-form label,.toolbar label,.shop-card label{display:grid;gap:.25rem;color:var(--muted);font-size:.78rem}.inline-form{grid-template-columns:repeat(2,minmax(140px,1fr));align-items:end}.inline-form .form-feedback{grid-column:1/-1}.form-group{margin-bottom:.8rem}.form-group label{display:block;margin-bottom:.25rem;color:var(--muted)}.form-feedback,.inline-result{display:flex;gap:.4rem;flex-wrap:wrap;color:var(--good);min-height:1.5rem}.form-feedback.is-error{color:var(--danger)}.contract-note{color:var(--muted);font-size:.78rem;margin-top:.8rem}.button-row{display:flex;gap:.5rem;flex-wrap:wrap}
.toolbar{display:flex;align-items:end;gap:.6rem;flex-wrap:wrap;background:#0c1625;border:1px solid var(--line);border-radius:10px;padding:.75rem}.toolbar label{width:110px}.toolbar span{color:var(--muted);margin-left:auto}
.tabs{display:flex;gap:.35rem;margin-bottom:.75rem}.tab{background:#101c2e;border-color:var(--line)}.tab.active{background:#21577b;border-color:#56b7e7}
.galaxy-shell{display:grid;gap:1rem}.galaxy-slot-grid{display:grid;grid-template-columns:repeat(5,minmax(120px,1fr));gap:.65rem}.galaxy-slot{position:relative;min-height:180px;border:1px solid var(--line);background:linear-gradient(#0d1727,#080e18);padding:.65rem;border-radius:10px;display:flex;flex-direction:column;align-items:center;gap:.25rem;text-align:center}.galaxy-slot.empty{opacity:.65}.slot-number{position:absolute;left:.55rem;top:.4rem;color:#5f7089;font-size:.72rem}.planet-orbit{width:58px;height:58px;border-radius:50%;margin:.55rem;background:radial-gradient(circle at 35% 28%,#a9f6ff,#3a87b1 32%,#162b53 64%,#05080f 70%);box-shadow:0 0 18px #4ecfff55}.planet-orbit.vacant{background:transparent;border:1px dashed #31425c;box-shadow:none}.slot-status{font-size:.67rem;color:var(--warn);text-transform:uppercase}.number{text-align:right}
.notification-row{display:flex;justify-content:space-between;align-items:center;gap:1rem;padding:.85rem;border-bottom:1px solid var(--line)}.notification-row.unread{background:#10223966}.message-detail{display:grid;gap:.5rem}.contract-gap{max-width:720px}.contract-gap .button{margin-top:1rem}
.loading-state,.empty-state,.error-state{display:flex;align-items:center;justify-content:center;gap:.6rem;min-height:120px;color:var(--muted);padding:1rem;text-align:center}.empty-state.compact{min-height:auto}.error-state{flex-direction:column;color:#ffadb6}.spinner{width:1rem;height:1rem;border:2px solid #31445f;border-top-color:var(--accent);border-radius:50%;animation:spin .8s linear infinite}@keyframes spin{to{transform:rotate(360deg)}}
.public-page{width:min(500px,calc(100% - 2rem));margin:8vh auto;padding:1.4rem;background:#0d1725;border-radius:14px;border:1px solid var(--line);box-shadow:0 25px 80px #0008}.public-page h1{text-align:center;margin-bottom:1.25rem}.public-page p{margin-top:.8rem}.error-page{text-align:center;padding:12vh 1.5rem}.error-page h1{font-size:2rem;margin-bottom:.75rem}
.sr-only{position:absolute!important;width:1px!important;height:1px!important;padding:0!important;margin:-1px!important;overflow:hidden!important;clip:rect(0,0,0,0)!important;white-space:nowrap!important;border:0!important}
@media(max-width:1000px){.galaxy-slot-grid{grid-template-columns:repeat(3,minmax(110px,1fr))}.metric-grid{grid-template-columns:repeat(2,1fr)}}
@media(max-width:760px){header{position:static}.header-user .role{display:none}.layout{display:block}nav{width:100%;max-height:none;border-right:0;border-bottom:1px solid var(--line)}nav ul{display:flex;gap:.25rem;overflow-x:auto}nav .nav-section{display:none}nav li{flex:0 0 auto}.dashboard-grid,.dashboard-grid.wide-first,.messages-layout{grid-template-columns:1fr}.galaxy-slot-grid{grid-template-columns:repeat(2,minmax(110px,1fr))}.feature-grid{grid-template-columns:1fr}.planet-banner{height:210px}}
@media(prefers-reduced-motion:reduce){*,*::before,*::after{scroll-behavior:auto!important;animation-duration:.01ms!important;animation-iteration-count:1!important}}
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
  <script>{client_js}</script>
</body>
</html>"#,
        title = title,
        body_html = body_html,
        client_js = CLIENT_JS,
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
    let is_admin = role_is_admin(role);

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

fn role_is_admin(role: &str) -> bool {
    role.parse::<UserRole>()
        .map(|role| platform_auth::has_permission(&role, &UserRole::Admin))
        .unwrap_or(false)
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
        "Home" => r#"<section class="landing-hero"><span class="eyebrow">Persistent strategy universe</span><h2>Build an empire across a living galaxy.</h2><p>Coordinate research, industry, fleets, diplomacy, and trade from one command surface.</p><div class="button-row"><a class="button" href="/register">Create commander</a><a class="button secondary" href="/login">Resume campaign</a></div></section><section class="feature-grid"><article><strong>15</strong><span>orbital positions per system</span></article><article><strong>Live</strong><span>fleet and resource telemetry</span></article><article><strong>CPU</strong><span>deterministic planetary visuals</span></article></section>"#.to_string(),
        "Overview" => r#"<div id="terrain-level-banner" data-view="overview"><div id="resources" class="loading-state"><span class="spinner" aria-hidden="true"></span>Synchronizing resources…</div><div id="planet-info" class="sr-only">Planet profile pending</div></div>"#.to_string(),
        "Buildings" => r#"<div id="building-list" data-view="buildings"><div id="build-queue" class="loading-state"><span class="spinner" aria-hidden="true"></span>Preparing construction controls…</div></div>"#.to_string(),
        "Research" => r#"<div id="research-tree" data-view="research"><div class="loading-state"><span class="spinner" aria-hidden="true"></span>Resolving technology matrix…</div></div>"#.to_string(),
        "Shipyard" => r#"<div id="ship-list" data-view="shipyard"><div id="defense-list" class="sr-only">Defense production uses the shared shipyard catalogue.</div><div id="shipyard-queue" class="loading-state"><span class="spinner" aria-hidden="true"></span>Opening orbital shipyard…</div></div>"#.to_string(),
        "Fleet" => r#"<div id="fleet-movements" data-view="fleet"><div id="fleet-dispatch" class="loading-state"><span class="spinner" aria-hidden="true"></span>Connecting to fleet telemetry…</div></div>"#.to_string(),
        "Galaxy" => r#"<div id="galaxy-view" class="galaxy-shell" data-view="galaxy"><form id="galaxy-controls" class="toolbar"><button type="button" id="galaxy-prev" class="secondary" aria-label="Previous system">←</button><label>Galaxy <input type="number" name="galaxy" id="galaxy-num" min="1" max="9" value="1" required></label><label>System <input type="number" name="system" id="system-num" min="1" max="499" value="120" required></label><button type="submit" id="galaxy-go">Scan</button><button type="button" id="galaxy-next" class="secondary" aria-label="Next system">→</button><span id="galaxy-status" role="status" aria-live="polite"></span></form><div id="galaxy-slot-thumbnails" class="galaxy-slot-grid" aria-label="Galaxy slot thumbnails"><div class="loading-state"><span class="spinner" aria-hidden="true"></span>Scanning system…</div></div></div>"#.to_string(),
        "Leaderboard" => r#"<section data-view="leaderboard"><div id="leaderboard-tabs" class="tabs" role="tablist" aria-label="Leaderboard scope"><button class="tab active" data-scope="players" role="tab" aria-selected="true">Commanders</button><button class="tab" data-scope="alliances" role="tab" aria-selected="false">Alliances</button></div><div id="leaderboard-table" class="panel"><div class="loading-state"><span class="spinner" aria-hidden="true"></span>Loading standings…</div></div></section>"#.to_string(),
        "Messages" => r#"<section data-view="messages" class="messages-layout"><div class="panel"><div id="message-folders" class="panel-heading"><div><span class="eyebrow">Secure channel</span><h2>Inbox</h2></div></div><div id="message-list"><div class="loading-state"><span class="spinner" aria-hidden="true"></span>Receiving messages…</div></div></div><div class="panel"><div id="message-detail" class="empty-state compact">Select a message to read its contents.</div><hr><h2>Compose</h2><form id="compose-form" class="stacked-form"><label>Recipient<input name="to" required></label><label>Subject<input name="subject" required></label><label>Message<textarea name="body" rows="5" required></textarea></label><button type="submit">Queue message</button><p class="form-feedback" role="status" aria-live="polite"></p></form></div></section>"#.to_string(),
        "Shop" => r#"<div id="shop-items" data-view="shop"><div id="dark-matter-balance" class="loading-state"><span class="spinner" aria-hidden="true"></span>Loading verified catalogue…</div></div>"#.to_string(),
        "Matrix Shop" => r#"<div id="shop-items" data-view="matrix-shop"><div id="dark-matter-balance" class="loading-state"><span class="spinner" aria-hidden="true"></span>Decoding Matrix catalogue…</div></div>"#.to_string(),
        "Notifications" => r#"<section data-view="notifications"><div class="panel-heading"><div><span class="eyebrow">Command alerts</span><h2>Notification center</h2></div><button type="button" id="mark-all-read" class="secondary">Mark all read</button></div><p id="notification-feedback" role="status"></p><div id="notification-list"><div class="loading-state"><span class="spinner" aria-hidden="true"></span>Checking alerts…</div></div></section>"#.to_string(),
        "Alliance Dashboard" | "Alliance Wars" | "Alliance Diplomacy" => r#"<div id="alliance-info" data-view="alliance"><div id="alliance-members" class="loading-state"><span class="spinner" aria-hidden="true"></span>Opening alliance command net…</div><div id="war-list" class="sr-only">War status is represented by diplomatic relations.</div><div id="diplomacy-status" class="sr-only">Diplomacy pending</div></div>"#.to_string(),
        "Alliance Management" => contract_gap("Alliance administration", "The gateway currently publishes alliance, roster, and diplomacy reads but no membership, role, or diplomacy mutation contract.", "/alliance/dashboard", "Return to alliance dashboard"),
        "Account Settings" => r#"<div id="account-form" data-view="account"><div class="loading-state"><span class="spinner" aria-hidden="true"></span>Loading account profile…</div></div>"#.to_string(),
        "Security Dashboard" => contract_gap("Session security", "Session listing and revocation endpoints are not yet present in the game gateway.", "/account/settings", "Review account profile"),
        "2FA Setup" => contract_gap("Two-factor authentication", "A 2FA enrollment and recovery-code contract is required before this control can safely be activated.", "/account/security", "Return to security"),
        "Email Verification" => contract_gap("Email verification", "The current account API exposes profile data but does not expose verification delivery or confirmation endpoints.", "/account/settings", "Return to account"),
        "Password Recovery" => contract_gap("Password change", "The gateway does not yet publish an authenticated password-change contract.", "/account/security", "Return to security"),
        "Privacy and Data Management" => contract_gap("Privacy controls", "Account export and deletion operations require explicit gateway contracts and confirmation workflows.", "/account/settings", "Return to account"),
        "Account Transfer" => contract_gap("Account transfer", "Universe transfer eligibility and confirmation endpoints are not yet available.", "/account/settings", "Return to account"),
        "Chat" => contract_gap("Realtime chat", "Chat rooms and message delivery are owned by the realtime gateway and are not yet exposed through the web frontend bridge.", "/messages", "Open asynchronous messages"),
        "Admin Dashboard" => r#"<section id="admin-stats" class="panel"><span class="eyebrow">Operations</span><h2>Administration gateway required</h2><p>Admin data lives in the separate admin API. This frontend intentionally does not display invented metrics while that authenticated bridge is still absent.</p><table><thead><tr><th>Surface</th><th>Status</th></tr></thead><tbody id="admin-metrics"><tr><td>Web route access control</td><td><span class="status-chip ally">Active</span></td></tr><tr><td>Admin data bridge</td><td><span class="status-chip war">Not connected</span></td></tr></tbody></table></section>"#.to_string(),
        "Admin Users" => r#"<section id="user-search" class="panel"><h2>User operations</h2><p>User search and mutations require the separate admin API; no synthetic user records are shown.</p><table><thead><tr><th>ID</th><th>Username</th><th>Role</th><th>Status</th><th>Actions</th></tr></thead><tbody id="user-table"><tr><td colspan="5" class="empty-state compact">Admin API bridge not connected.</td></tr></tbody></table></section>"#.to_string(),
        "Admin Monitoring" => r#"<section id="system-health" class="panel"><h2>Service monitoring</h2><div id="service-status" class="empty-state">Monitoring is owned by the admin API and observability service; this route is access-controlled but not yet data-connected.</div></section>"#.to_string(),
        _ if title.starts_with("Admin") => contract_gap(title, "This access-controlled route is waiting for a typed bridge to the separate admin API.", "/admin/dashboard", "Return to admin dashboard"),
        _ => contract_gap(title, "No supported server contract is currently mapped to this route.", "/overview", "Return to command center"),
    }
}

fn contract_gap(title: &str, message: &str, href: &str, action: &str) -> String {
    format!(
        r#"<section class="notice-card contract-gap" role="note"><span class="eyebrow">Contract status</span><h2>{title}</h2><p>{message}</p><a class="button secondary" href="{href}">{action}</a></section>"#
    )
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
        let mut state = AppState::new(test_auth_config());
        state.assets_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("assets")
            .to_string_lossy()
            .into_owned();
        state
    }

    fn test_router() -> Router {
        build_router_with_state(test_state())
    }

    async fn ready_gateway(status: StatusCode) -> (AppState, tokio::task::JoinHandle<()>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let upstream =
            Router::new().route("/ready", axum::routing::get(move || async move { status }));
        let server = axum::Server::from_tcp(listener)
            .unwrap()
            .serve(upstream.into_make_service());
        let handle = tokio::spawn(async move {
            let _ = server.await;
        });
        let mut state = test_state();
        state.api_gateway_url = format!("http://{address}");
        (state, handle)
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
        let (state, server) = ready_gateway(StatusCode::OK).await;
        let app = build_router_with_state(state);
        let resp = app
            .oneshot(Request::get("/ready").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(resp.into_body()).await.unwrap();
        let parsed: ServiceStatus = serde_json::from_slice(&body).unwrap();
        assert_eq!(parsed.status, "ok");
        assert_eq!(parsed.dependency.as_deref(), Some("app-api-gateway"));
        server.abort();
    }

    #[tokio::test]
    async fn ready_returns_service_unavailable_when_gateway_is_offline() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let unavailable_url = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);
        let mut state = test_state();
        state.api_gateway_url = unavailable_url;
        let app = build_router_with_state(state);
        let resp = app
            .oneshot(Request::get("/ready").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = hyper::body::to_bytes(resp.into_body()).await.unwrap();
        let parsed: ServiceStatus = serde_json::from_slice(&body).unwrap();
        assert_eq!(parsed.status, "unavailable");
        assert_eq!(parsed.dependency.as_deref(), Some("app-api-gateway"));
    }

    #[tokio::test]
    async fn existing_planet_asset_is_served_with_png_content_type() {
        let app = test_router();
        let resp = app
            .oneshot(
                Request::get(
                    "/assets/planet-rust-prototype/new-terra-rust-480p-overview-banner.png",
                )
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers()
                .get(CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("image/png")
        );
        let body = hyper::body::to_bytes(resp.into_body()).await.unwrap();
        assert_eq!(&body[..8], b"\x89PNG\r\n\x1a\n");
    }

    #[tokio::test]
    async fn asset_route_rejects_parent_directory_traversal() {
        let app = test_router();
        let resp = app
            .oneshot(
                Request::get("/assets/%2e%2e/Cargo.toml")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_ne!(resp.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(resp.into_body()).await.unwrap();
        assert!(!String::from_utf8_lossy(&body).contains("[workspace]"));
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
        assert!(html.contains("terrain-level-banner"));
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
        assert!(html.contains("galaxy-slot-thumbnails"));
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
            dependency: None,
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
        assert!(body.contains("terrain-level-banner"));
    }

    #[test]
    fn page_body_for_galaxy() {
        let body = page_body_for("Galaxy");
        assert!(body.contains("galaxy-view"));
        assert!(body.contains("galaxy-controls"));
        assert!(body.contains("galaxy-slot-thumbnails"));
        assert!(body.contains("galaxy-slot-thumbnail"));
    }

    #[test]
    fn page_body_for_unknown() {
        let body = page_body_for("Something Random");
        assert!(body.contains("Something Random"));
        assert!(body.contains("Contract status"));
        assert!(!body.contains("Content for"));
    }

    #[test]
    fn auth_session_cookie_is_http_only_and_same_site() {
        let body = serde_json::json!({
            "success": true,
            "data": {
                "token": "signed.jwt.token",
                "expiresInSeconds": 7200
            }
        })
        .to_string();
        let cookie = session_cookie_for_response(
            "api/auth/login",
            reqwest::StatusCode::OK,
            body.as_bytes(),
            true,
        )
        .expect("login response creates session cookie");

        assert!(cookie.contains("universus_token=signed.jwt.token"));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Lax"));
        assert!(cookie.contains("Max-Age=7200"));
        assert!(cookie.contains("Secure"));
    }

    #[test]
    fn production_cookie_security_defaults_to_secure() {
        assert_eq!(resolve_cookie_security("production", None, None), Ok(true));
        assert_eq!(
            resolve_cookie_security("staging", Some("true"), None),
            Ok(true)
        );
    }

    #[test]
    fn production_rejects_insecure_cookie_without_explicit_local_override() {
        assert_eq!(
            resolve_cookie_security("production", Some("false"), None),
            Err(CookieSecurityError::InsecureProductionCookie)
        );
        assert_eq!(
            resolve_cookie_security("production", Some("false"), Some("true")),
            Ok(false)
        );
    }

    #[test]
    fn development_keeps_http_cookies_usable_and_rejects_typos() {
        assert_eq!(
            resolve_cookie_security("development", None, None),
            Ok(false)
        );
        assert_eq!(
            resolve_cookie_security("test", Some("false"), None),
            Ok(false)
        );
        assert_eq!(
            resolve_cookie_security("development", Some("sometimes"), None),
            Err(CookieSecurityError::InvalidBoolean {
                name: "COOKIE_SECURE"
            })
        );
    }

    #[test]
    fn logout_always_expires_browser_session_cookie() {
        let cookie = session_cookie_for_response(
            "api/auth/logout",
            reqwest::StatusCode::BAD_GATEWAY,
            b"{}",
            false,
        )
        .expect("logout always expires cookie");

        assert!(cookie.starts_with("universus_token=;"));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Lax"));
        assert!(cookie.contains("Max-Age=0"));
    }

    #[tokio::test]
    async fn cookie_token_allows_protected_page_navigation() {
        let app = test_router();
        let token = user_token();
        let resp = app
            .oneshot(
                Request::get("/overview")
                    .header("Cookie", format!("universus_token={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn logout_route_expires_cookie_even_when_gateway_is_offline() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let unavailable_url = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);

        let mut state = test_state();
        state.api_gateway_url = unavailable_url;
        let app = build_router_with_state(state);
        let resp = app
            .oneshot(
                Request::post("/game-api/api/auth/logout")
                    .header("Host", "universus.test")
                    .header("Origin", "http://universus.test")
                    .header("Sec-Fetch-Site", "same-origin")
                    .header("Cookie", "universus_token=signed.jwt.token")
                    .header("Content-Type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);
        let cookie = resp
            .headers()
            .get(SET_COOKIE)
            .and_then(|value| value.to_str().ok())
            .expect("logout clears cookie even during upstream outage");
        assert!(cookie.contains("universus_token=;"));
        assert!(cookie.contains("Max-Age=0"));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Lax"));
    }

    #[tokio::test]
    async fn gateway_bridge_rejects_cross_origin_auth_and_game_mutations() {
        let app = test_router();
        for path in [
            "/game-api/api/auth/login",
            "/game-api/api/auth/register",
            "/game-api/api/auth/logout",
            "/game-api/api/fleet/send",
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::post(path)
                        .header("Host", "universus.test")
                        .header("Origin", "https://attacker.example")
                        .header("Sec-Fetch-Site", "cross-site")
                        .header("Content-Type", "application/json")
                        .body(Body::from("{}"))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN, "{path}");
            assert!(response.headers().get(SET_COOKIE).is_none(), "{path}");
        }
    }

    #[tokio::test]
    async fn gateway_bridge_rejects_cookie_mutation_without_origin_proof() {
        let response = test_router()
            .oneshot(
                Request::post("/game-api/api/auth/logout")
                    .header("Host", "universus.test")
                    .header("Cookie", "universus_token=signed.jwt.token")
                    .header("Content-Type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(response.headers().get(SET_COOKIE).is_none());
    }

    #[tokio::test]
    async fn gateway_bridge_keeps_cross_origin_safe_reads_unblocked() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let unavailable_url = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);

        let mut state = test_state();
        state.api_gateway_url = unavailable_url;
        let response = build_router_with_state(state)
            .oneshot(
                Request::get("/game-api/health")
                    .header("Host", "universus.test")
                    .header("Origin", "https://attacker.example")
                    .header("Sec-Fetch-Site", "cross-site")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    }

    #[tokio::test]
    async fn gateway_bridge_rejects_non_api_paths_without_network_access() {
        let app = test_router();
        let resp = app
            .oneshot(
                Request::get("/game-api/internal/config")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn core_game_views_are_progressive_and_contract_wired() {
        for (title, view) in [
            ("Overview", "overview"),
            ("Buildings", "buildings"),
            ("Research", "research"),
            ("Shipyard", "shipyard"),
            ("Fleet", "fleet"),
            ("Galaxy", "galaxy"),
            ("Leaderboard", "leaderboard"),
            ("Messages", "messages"),
            ("Shop", "shop"),
            ("Notifications", "notifications"),
            ("Alliance Dashboard", "alliance"),
            ("Account Settings", "account"),
        ] {
            let body = page_body_for(title);
            assert!(
                body.contains(&format!("data-view=\"{view}\"")),
                "{title} must declare its progressive view"
            );
            assert!(!body.contains("Content for"));
        }

        for endpoint in [
            "/api/planets",
            "/api/account/resources",
            "/api/research/queue",
            "/api/research/start",
            "/api/shipyard/build",
            "/api/fleet/send",
            "/api/galaxy/",
            "/api/leaderboard/",
            "/api/messages/send",
            "/api/shop/purchase-preview",
            "/api/notifications/read-all",
            "/api/alliance/diplomacy",
        ] {
            assert!(
                CLIENT_JS.contains(endpoint),
                "missing UI contract {endpoint}"
            );
        }
        assert!(!CLIENT_JS.contains("localStorage"));
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
