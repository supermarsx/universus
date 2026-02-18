use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use hyper::body::to_bytes;
use tower::ServiceExt;

use app_web_frontend::build_router;

async fn html_body(response: axum::response::Response) -> String {
    let bytes = to_bytes(response.into_body())
        .await
        .expect("read body bytes");
    String::from_utf8(bytes.to_vec()).expect("utf8 body")
}

#[tokio::test]
async fn integration_template_routes_render_and_require_tokens() {
    let app = build_router();
    let home = app
        .clone()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(home.status(), StatusCode::OK);
    let home_html = html_body(home).await;
    assert!(home_html.contains("<h1>Home</h1>"));

    let buildings = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/buildings")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(buildings.status(), StatusCode::UNAUTHORIZED);

    let buildings_auth = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/buildings")
                .header(header::AUTHORIZATION, "Bearer dev-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(buildings_auth.status(), StatusCode::OK);
    assert!(html_body(buildings_auth)
        .await
        .contains("<h1>Buildings</h1>"));

    let admin_no_token = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(admin_no_token.status(), StatusCode::UNAUTHORIZED);

    let admin_with_token = app
        .oneshot(
            Request::builder()
                .uri("/admin")
                .header(header::AUTHORIZATION, "Bearer admin-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(admin_with_token.status(), StatusCode::OK);
    assert!(html_body(admin_with_token).await.contains("Admin"));
}
