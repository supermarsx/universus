use app_web_frontend::{build_router, build_router_with_state, AppState, SERVICE_NAME};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use hyper::body::to_bytes;
use serde_json::Value;
use tower::ServiceExt;

fn auth_token(user_id: &str, username: &str, role: &str) -> String {
    let config = platform_auth::AuthConfig {
        jwt_secret: "default-secret".to_string(),
        ..platform_auth::AuthConfig::default()
    };
    platform_auth::generate_token(&config, user_id, username, role, None).expect("generate token")
}

fn dev_token() -> String {
    auth_token("user-1", "player1", "user")
}

fn admin_token() -> String {
    auth_token("admin-1", "admin", "admin")
}

fn superadmin_token() -> String {
    auth_token("superadmin-1", "root", "superadmin")
}

async fn response_body_text(response: axum::response::Response) -> String {
    let body = to_bytes(response.into_body()).await.unwrap();
    String::from_utf8(body.to_vec()).unwrap()
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body()).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn get_response(path: &str) -> axum::response::Response {
    get_response_with_token(path, None).await
}

async fn get_response_with_token(path: &str, token: Option<&str>) -> axum::response::Response {
    let mut request = Request::builder().uri(path).method("GET");
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }

    build_router()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

#[tokio::test]
async fn health_returns_ok_with_service_name() {
    let response = get_response("/health").await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["status"], "ok");
    assert_eq!(body["service"], SERVICE_NAME);
}

#[tokio::test]
async fn ready_returns_ok_with_service_name() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let upstream =
        axum::Router::new().route("/ready", axum::routing::get(|| async { StatusCode::OK }));
    let server = axum::Server::from_tcp(listener)
        .unwrap()
        .serve(upstream.into_make_service());
    let handle = tokio::spawn(async move {
        let _ = server.await;
    });
    let mut state = AppState::new(platform_auth::AuthConfig::default());
    state.api_gateway_url = format!("http://{address}");
    let response = build_router_with_state(state)
        .oneshot(Request::get("/ready").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["status"], "ok");
    assert_eq!(body["service"], SERVICE_NAME);
    assert_eq!(body["dependency"], "app-api-gateway");
    handle.abort();
}

#[tokio::test]
async fn overview_route_serves_progressive_command_center_with_metadata() {
    let token = dev_token();
    let response = get_response_with_token("/overview", Some(&token)).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_body_text(response).await;
    assert!(body.contains("<h1>Overview</h1>"));
    assert!(body.contains("name=\"route-title\" content=\"Overview\""));
    assert!(body.contains("name=\"route-path\" content=\"/overview\""));
    assert!(body.contains("data-view=\"overview\""));
    assert!(body.contains("/api/account/resources"));
}

#[tokio::test]
async fn privacy_route_serves_real_self_service_contract() {
    let token = dev_token();
    let response = get_response_with_token("/account/privacy", Some(&token)).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_body_text(response).await;
    assert!(body.contains("<h1>Privacy and Data Management</h1>"));
    assert!(body.contains("data-view=\"privacy\""));
    assert!(body.contains("/api/privacy/requests?limit=50"));
    assert!(body.contains("/api/privacy/communications"));
    assert!(body.contains("RESTRICT MY ACCOUNT"));
    assert!(body.contains("ERASE MY ACCOUNT"));
    assert!(body.contains("CANCEL REQUEST"));
    assert!(body.contains("Required for account operation"));
    assert!(body.contains("aria-live=\"polite\""));
    assert!(!body.contains("<section class=\"panel contract-gap\""));
    assert!(!body.contains("href=\"#\">Download"));
}

#[tokio::test]
async fn admin_users_route_serves_access_controlled_contract_state() {
    let token = admin_token();
    let response = get_response_with_token("/admin/users", Some(&token)).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_body_text(response).await;
    assert!(body.contains("<h1>Admin Users</h1>"));
    assert!(body.contains("/admin/users"));
}

#[tokio::test]
async fn alliance_manage_route_reports_unsupported_mutation_contract() {
    let token = dev_token();
    let response = get_response_with_token("/alliance/manage", Some(&token)).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_body_text(response).await;
    assert!(body.contains("<h1>Alliance Management</h1>"));
    assert!(body.contains("/alliance/manage"));
    assert!(body.contains("Contract status"));
    assert!(body.contains("no membership, role, or diplomacy mutation contract"));
}

#[tokio::test]
async fn index_html_alias_maps_to_home() {
    let response = get_response("/index.html").await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_body_text(response).await;
    assert!(body.contains("<h1>Home</h1>"));
    assert!(body.contains("name=\"route-path\" content=\"/index.html\""));
}

#[tokio::test]
async fn overview_html_alias_maps_to_overview() {
    let token = dev_token();
    let response = get_response_with_token("/overview.html", Some(&token)).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_body_text(response).await;
    assert!(body.contains("<h1>Overview</h1>"));
    assert!(body.contains("name=\"route-path\" content=\"/overview.html\""));
}

#[tokio::test]
async fn admin_bots_html_alias_maps_to_admin_bot_management() {
    let token = admin_token();
    let response = get_response_with_token("/admin/bots.html", Some(&token)).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_body_text(response).await;
    assert!(body.contains("<h1>Admin Bot Management</h1>"));
    assert!(body.contains("name=\"route-title\" content=\"Admin Bot Management\""));
}

#[tokio::test]
async fn chat_html_alias_maps_to_chat() {
    let token = dev_token();
    let response = get_response_with_token("/chat.html", Some(&token)).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_body_text(response).await;
    assert!(body.contains("<h1>Chat</h1>"));
    assert!(body.contains("name=\"route-path\" content=\"/chat.html\""));
}

#[tokio::test]
async fn unknown_route_returns_not_found() {
    let response = get_response("/nope").await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn protected_route_without_token_returns_401() {
    let response = get_response("/account/settings").await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn overview_without_token_returns_401() {
    let response = get_response("/overview").await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn admin_route_with_dev_token_returns_403() {
    let token = dev_token();
    let response = get_response_with_token("/admin/users", Some(&token)).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn protected_route_with_dev_token_returns_200() {
    let token = dev_token();
    let response = get_response_with_token("/alliance/manage", Some(&token)).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn admin_route_with_admin_token_returns_200() {
    let token = admin_token();
    let response = get_response_with_token("/admin/users", Some(&token)).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn admin_route_with_superadmin_token_returns_200() {
    let token = superadmin_token();
    let response = get_response_with_token("/admin/users", Some(&token)).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn all_template_routes_have_expected_auth_gating_and_render() {
    let public_routes = ["/", "/index.html"];
    let authenticated_routes = [
        "/overview",
        "/overview.html",
        "/buildings",
        "/buildings.html",
        "/research",
        "/research.html",
        "/shipyard",
        "/shipyard.html",
        "/fleet",
        "/fleet.html",
        "/galaxy",
        "/galaxy.html",
        "/leaderboard",
        "/leaderboard.html",
        "/messages",
        "/messages.html",
        "/shop",
        "/shop.html",
        "/notifications",
        "/notifications.html",
        "/matrix-shop",
        "/matrix-shop.html",
        "/chat",
        "/chat.html",
        "/account/settings",
        "/account/security",
        "/account/2fa",
        "/account/email",
        "/account/password",
        "/account/privacy",
        "/account/transfer",
        "/alliance",
        "/alliance/dashboard",
        "/alliance/wars",
        "/alliance/diplomacy",
        "/alliance/manage",
    ];
    let admin_routes = [
        "/admin",
        "/admin.html",
        "/admin/dashboard",
        "/admin/users",
        "/admin/monitoring",
        "/admin/settings",
        "/admin/events",
        "/admin/analytics",
        "/admin/audit",
        "/admin/sms-service",
        "/admin/bots",
        "/admin/bots.html",
    ];

    for route in public_routes {
        let response = get_response(route).await;
        assert_eq!(response.status(), StatusCode::OK, "public route {}", route);
    }

    for route in authenticated_routes {
        let unauth = get_response(route).await;
        assert_eq!(
            unauth.status(),
            StatusCode::UNAUTHORIZED,
            "auth route unauth {}",
            route
        );
        let token = dev_token();
        let auth = get_response_with_token(route, Some(&token)).await;
        assert_eq!(auth.status(), StatusCode::OK, "auth route {}", route);
    }

    for route in admin_routes {
        let unauth = get_response(route).await;
        assert_eq!(
            unauth.status(),
            StatusCode::UNAUTHORIZED,
            "admin route unauth {}",
            route
        );
        let dev_token = dev_token();
        let non_admin = get_response_with_token(route, Some(&dev_token)).await;
        assert_eq!(
            non_admin.status(),
            StatusCode::FORBIDDEN,
            "admin route non-admin {}",
            route
        );
        let admin_token = admin_token();
        let admin = get_response_with_token(route, Some(&admin_token)).await;
        assert_eq!(admin.status(), StatusCode::OK, "admin route {}", route);
    }
}

#[test]
fn compose_wires_frontend_to_container_gateway() {
    let compose_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("docker-compose.yml");
    let compose = std::fs::read_to_string(&compose_path)
        .unwrap_or_else(|error| panic!("read {}: {error}", compose_path.display()));
    let frontend = compose
        .split("  rust-web-frontend:")
        .nth(1)
        .and_then(|tail| tail.split("\n  rust-admin-api:").next())
        .expect("rust-web-frontend compose service");

    assert!(
        frontend.contains("API_GATEWAY_URL: http://rust-api-gateway:3000"),
        "frontend container must use service DNS instead of loopback"
    );
}

#[test]
fn service_image_copies_runtime_planet_assets() {
    let dockerfile_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("Dockerfile.service");
    let dockerfile = std::fs::read_to_string(&dockerfile_path)
        .unwrap_or_else(|error| panic!("read {}: {error}", dockerfile_path.display()));

    assert!(
        dockerfile.contains("COPY assets /app/assets"),
        "runtime service image must include the asset tree served by app-web-frontend"
    );
}

#[test]
fn service_image_uses_a_fixed_exec_form_entrypoint() {
    let dockerfile_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("Dockerfile.service");
    let dockerfile = std::fs::read_to_string(&dockerfile_path)
        .unwrap_or_else(|error| panic!("read {}: {error}", dockerfile_path.display()));
    let entrypoint = dockerfile
        .lines()
        .find(|line| line.trim_start().starts_with("ENTRYPOINT"))
        .expect("shared service image entrypoint");

    assert_eq!(
        entrypoint.trim(),
        r#"ENTRYPOINT ["/usr/local/bin/service"]"#,
        "runtime startup must not depend on build-only ARG expansion or a shell"
    );
    assert!(
        dockerfile.contains(r#"ln -s "/usr/local/bin/${BIN_NAME}" /usr/local/bin/service"#),
        "the fixed entrypoint must resolve to the selected service binary"
    );
    assert!(
        !dockerfile.contains(r#"ENTRYPOINT ["/bin/sh", "-c""#),
        "service images must preserve argument boundaries with exec-form startup"
    );
}
