use std::path::Path;

use app_admin_api::privacy;
use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use platform_db::{
    AuthSessionCreateInput, Database, PrivacyRequestCreateInput, PrivacyRequestType,
};
use serde_json::{json, Value};
use tokio_postgres::{Client, NoTls};
use tower::ServiceExt;

async fn apply_schema(client: &Client) {
    client
        .batch_execute("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
        .await
        .unwrap();
    let steps_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../database/sql/steps");
    let mut steps = std::fs::read_dir(steps_dir)
        .unwrap()
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            let name = path.file_name()?.to_str()?.to_string();
            let version = name.split_once('_')?.0.parse::<u32>().ok()?;
            (version <= 56).then_some((version, name, path))
        })
        .collect::<Vec<_>>();
    steps.sort_by(|left, right| (left.0, &left.1).cmp(&(right.0, &right.1)));
    assert_eq!(steps.last().map(|step| step.0), Some(56));
    for (version, name, path) in steps {
        let sql = std::fs::read_to_string(path).unwrap();
        client
            .batch_execute(&sql)
            .await
            .unwrap_or_else(|error| panic!("migration {version} {name}: {error:?}"));
    }
}

async fn token(database: &Database, user_id: i32, fixture: u8) -> String {
    let issue = database
        .create_auth_session(AuthSessionCreateInput {
            account_id: user_id.to_string(),
            session_id: format!("privacy-admin-session-{fixture:0<40}"),
            family_id: format!("privacy-admin-family-{fixture:0<40}"),
            refresh_token_digest: vec![fixture; 32],
            refresh_expiry_seconds: 86_400,
            max_active_sessions: 5,
            device_label: Some("privacy admin integration".to_string()),
            ip_digest: None,
            user_agent_digest: None,
            account_throttle_digest: vec![fixture.saturating_add(20); 32],
        })
        .await
        .unwrap();
    platform_auth::generate_session_token_with_email(
        &platform_auth::AuthConfig {
            jwt_secret: "privacy-admin-integration-secret".to_string(),
            jwt_expiry_seconds: 3600,
            ..platform_auth::AuthConfig::default()
        },
        &issue.principal.user_id,
        &issue.principal.username,
        Some(&issue.principal.email),
        &issue.principal.role,
        Some(issue.principal.universe_id),
        &issue.session_id,
        issue.principal.auth_epoch,
    )
    .unwrap()
}

fn privacy_request(universe_id: i64, user_id: i32, key: &str) -> PrivacyRequestCreateInput {
    PrivacyRequestCreateInput {
        universe_id,
        user_id,
        request_type: PrivacyRequestType::Erasure,
        idempotency_key: key.to_string(),
        request_source: "privacy_admin_postgres".to_string(),
        requester_ip_digest: None,
        encrypted_payload: None,
        erasure_cooling_off_seconds: Some(0),
    }
}

async fn call(
    app: axum::Router,
    method: Method,
    uri: &str,
    token: Option<&str>,
    payload: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let body = if let Some(payload) = payload {
        builder = builder.header("content-type", "application/json");
        Body::from(payload.to_string())
    } else {
        Body::empty()
    };
    let response = app.oneshot(builder.body(body).unwrap()).await.unwrap();
    let status = response.status();
    assert_eq!(response.headers()["cache-control"], "no-store, max-age=0");
    let bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

/// Owns and resets `UNIVERSUS_TEST_DATABASE_URL`; use only a disposable DB.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires disposable PostgreSQL in UNIVERSUS_TEST_DATABASE_URL"]
async fn privacy_admin_requires_live_tenant_admin_and_preserves_dual_control() {
    let url = std::env::var("UNIVERSUS_TEST_DATABASE_URL").unwrap();
    std::env::set_var("JWT_SECRET", "privacy-admin-integration-secret");
    std::env::set_var("AUTH_ALLOW_LEGACY_HS256", "true");
    let (client, connection) = tokio_postgres::connect(&url, NoTls).await.unwrap();
    tokio::spawn(async move { connection.await.unwrap() });
    apply_schema(&client).await;
    client
        .execute(
            "INSERT INTO universes (id, name, speed, registration_open)
             VALUES (2, 'Admin tenant two', 1, TRUE) ON CONFLICT DO NOTHING",
            &[],
        )
        .await
        .unwrap();
    let rows = client
        .query(
            "INSERT INTO users (username, email, password_hash, universe_id, is_admin)
             VALUES
                ('PrivacyAdminOne', 'privacy-admin-one@example.test', '!test!', 1, TRUE),
                ('PrivacyAdminTwo', 'privacy-admin-two@example.test', '!test!', 1, TRUE),
                ('PrivacyPlayer', 'privacy-player@example.test', '!test!', 1, FALSE),
                ('PrivacySubject', 'privacy-subject@example.test', '!test!', 1, FALSE),
                ('PrivacyOtherTenant', 'privacy-other@example.test', '!test!', 2, FALSE)
             RETURNING id, username",
            &[],
        )
        .await
        .unwrap();
    let id = |name: &str| {
        rows.iter()
            .find(|row| row.get::<_, String>("username") == name)
            .unwrap()
            .get::<_, i32>("id")
    };
    let admin_one = id("PrivacyAdminOne");
    let admin_two = id("PrivacyAdminTwo");
    let player = id("PrivacyPlayer");
    let subject = id("PrivacySubject");
    let other = id("PrivacyOtherTenant");
    let database = Database::from_database_url(&url).unwrap();
    let request = database
        .create_privacy_request(privacy_request(1, subject, "admin-erasure-one"))
        .await
        .unwrap();
    let other_request = database
        .create_privacy_request(privacy_request(2, other, "admin-erasure-other"))
        .await
        .unwrap();
    let admin_one_token = token(&database, admin_one, 1).await;
    let admin_two_token = token(&database, admin_two, 2).await;
    let player_token = token(&database, player, 3).await;
    let app = privacy::router(database.clone());

    assert_eq!(
        call(
            app.clone(),
            Method::GET,
            "/api/admin/privacy/requests",
            None,
            None
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            app.clone(),
            Method::GET,
            "/api/admin/privacy/requests",
            Some(&player_token),
            None,
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (_, list) = call(
        app.clone(),
        Method::GET,
        "/api/admin/privacy/requests?requestType=erasure&limit=10",
        Some(&admin_one_token),
        None,
    )
    .await;
    assert_eq!(list["data"]["requests"].as_array().unwrap().len(), 1);
    assert_eq!(list["data"]["requests"][0]["id"], request.id);
    assert_ne!(list["data"]["requests"][0]["id"], other_request.id);
    assert_eq!(
        call(
            app.clone(),
            Method::GET,
            &format!("/api/admin/privacy/requests/{}", other_request.id),
            Some(&admin_one_token),
            None,
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );

    let approve = |version| {
        json!({
            "decision": "approve",
            "reasonCode": "verified_subject_request",
            "expectedVersion": version
        })
    };
    let (status, first) = call(
        app.clone(),
        Method::POST,
        &format!("/api/admin/privacy/requests/{}/decisions", request.id),
        Some(&admin_one_token),
        Some(approve(request.version)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["data"]["status"], "in_review");
    let current_version = first["data"]["version"].as_i64().unwrap();
    assert_eq!(
        call(
            app.clone(),
            Method::POST,
            &format!("/api/admin/privacy/requests/{}/decisions", request.id),
            Some(&admin_two_token),
            Some(approve(request.version)),
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let (status, second) = call(
        app.clone(),
        Method::POST,
        &format!("/api/admin/privacy/requests/{}/decisions", request.id),
        Some(&admin_two_token),
        Some(approve(current_version)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second["data"]["status"], "queued");
    let decision_count: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM privacy_admin_decisions WHERE request_id = $1",
            &[&request.id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(decision_count, 2);

    client
        .execute(
            "UPDATE users SET is_admin = FALSE WHERE id = $1",
            &[&admin_one],
        )
        .await
        .unwrap();
    assert_eq!(
        call(
            app,
            Method::GET,
            "/api/admin/privacy/requests",
            Some(&admin_one_token),
            None,
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
}
