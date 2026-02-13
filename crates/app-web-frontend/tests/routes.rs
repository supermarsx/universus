use app_web_frontend::{build_router, SERVICE_NAME};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use hyper::body::to_bytes;
use serde_json::Value;
use tower::ServiceExt;

async fn response_body_text(response: axum::response::Response) -> String {
    let body = to_bytes(response.into_body()).await.unwrap();
    String::from_utf8(body.to_vec()).unwrap()
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body()).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn health_returns_ok_with_service_name() {
    let app = build_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["status"], "ok");
    assert_eq!(body["service"], SERVICE_NAME);
}

#[tokio::test]
async fn ready_returns_ok_with_service_name() {
    let app = build_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/ready")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["status"], "ok");
    assert_eq!(body["service"], SERVICE_NAME);
}

#[tokio::test]
async fn overview_route_serves_placeholder_html() {
    let app = build_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/overview")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_body_text(response).await;
    assert!(body.contains("<h1>Overview</h1>"));
    assert!(body.contains("/overview"));
}

#[tokio::test]
async fn admin_users_route_serves_placeholder_html() {
    let app = build_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/users")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_body_text(response).await;
    assert!(body.contains("<h1>Admin Users</h1>"));
    assert!(body.contains("/admin/users"));
}

#[tokio::test]
async fn alliance_manage_route_serves_placeholder_html() {
    let app = build_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/alliance/manage")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_body_text(response).await;
    assert!(body.contains("<h1>Alliance Management</h1>"));
    assert!(body.contains("/alliance/manage"));
}

#[tokio::test]
async fn unknown_route_returns_not_found() {
    let app = build_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/nope")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
