use app_web_frontend::{build_router, SERVICE_NAME};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use hyper::body::to_bytes;
use serde_json::Value;
use tower::ServiceExt;

const DEV_TOKEN: &str = "dev-token";
const ADMIN_TOKEN: &str = "admin-token";

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
    let response = get_response("/ready").await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["status"], "ok");
    assert_eq!(body["service"], SERVICE_NAME);
}

#[tokio::test]
async fn overview_route_serves_placeholder_html_with_metadata() {
    let response = get_response("/overview").await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_body_text(response).await;
    assert!(body.contains("<h1>Overview</h1>"));
    assert!(body.contains("name=\"route-title\" content=\"Overview\""));
    assert!(body.contains("name=\"route-path\" content=\"/overview\""));
}

#[tokio::test]
async fn admin_users_route_serves_placeholder_html() {
    let response = get_response_with_token("/admin/users", Some(ADMIN_TOKEN)).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_body_text(response).await;
    assert!(body.contains("<h1>Admin Users</h1>"));
    assert!(body.contains("/admin/users"));
}

#[tokio::test]
async fn alliance_manage_route_serves_placeholder_html() {
    let response = get_response_with_token("/alliance/manage", Some(DEV_TOKEN)).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_body_text(response).await;
    assert!(body.contains("<h1>Alliance Management</h1>"));
    assert!(body.contains("/alliance/manage"));
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
    let response = get_response("/overview.html").await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_body_text(response).await;
    assert!(body.contains("<h1>Overview</h1>"));
    assert!(body.contains("name=\"route-path\" content=\"/overview.html\""));
}

#[tokio::test]
async fn admin_bots_html_alias_maps_to_admin_bot_management() {
    let response = get_response_with_token("/admin/bots.html", Some(ADMIN_TOKEN)).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_body_text(response).await;
    assert!(body.contains("<h1>Admin Bot Management</h1>"));
    assert!(body.contains("name=\"route-title\" content=\"Admin Bot Management\""));
}

#[tokio::test]
async fn chat_html_alias_maps_to_chat() {
    let response = get_response_with_token("/chat.html", Some(DEV_TOKEN)).await;

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
async fn admin_route_with_dev_token_returns_403() {
    let response = get_response_with_token("/admin/users", Some(DEV_TOKEN)).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn protected_route_with_dev_token_returns_200() {
    let response = get_response_with_token("/alliance/manage", Some(DEV_TOKEN)).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn admin_route_with_admin_token_returns_200() {
    let response = get_response_with_token("/admin/users", Some(ADMIN_TOKEN)).await;
    assert_eq!(response.status(), StatusCode::OK);
}
