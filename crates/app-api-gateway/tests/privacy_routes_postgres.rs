use std::{net::SocketAddr, path::Path};

use app_api_gateway::accounts::AccountRepository;
use app_api_gateway::routes::build_router_with_dependencies;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{header, Request, StatusCode};
use hyper::body::to_bytes;
use platform_db::{
    AccountCreateInput, AuthSessionCreateInput, ConsentStatus, ConsentUpdate, Database,
    PreparedExportArtifact,
};
use serde_json::{json, Value};
use tokio_postgres::{Client, NoTls};
use tower::ServiceExt;

fn account_input(username: &str, email: &str) -> AccountCreateInput {
    AccountCreateInput {
        username: username.to_string(),
        email: email.to_string(),
        password_hash: "$argon2id$v=19$m=19456,t=2,p=1$c2FsdA$aGFzaA".to_string(),
    }
}

async fn session_token(database: &Database, subject: &str, fixture: u8) -> String {
    let session_id = format!("privacy-integration-session-{fixture:0<32}");
    let issue = database
        .create_auth_session(AuthSessionCreateInput {
            account_id: subject.to_string(),
            session_id: session_id.clone(),
            family_id: format!("privacy-integration-family-{fixture:0<32}"),
            refresh_token_digest: vec![fixture; 32],
            refresh_expiry_seconds: 604_800,
            max_active_sessions: 5,
            device_label: Some("Privacy route integration test".to_string()),
            ip_digest: Some(vec![fixture.saturating_add(10); 32]),
            user_agent_digest: Some(vec![fixture.saturating_add(20); 32]),
            account_throttle_digest: vec![fixture.saturating_add(30); 32],
        })
        .await
        .expect("create durable privacy test session");
    let config = platform_auth::AuthConfig {
        jwt_secret: "default-secret".to_string(),
        jwt_expiry_seconds: 86_400,
        ..platform_auth::AuthConfig::default()
    };
    platform_auth::generate_session_token_with_email(
        &config,
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

fn request(method: &str, path: &str, token: &str, body: Option<Value>) -> Request<Body> {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"));
    if body.is_some() {
        request = request.header(header::CONTENT_TYPE, "application/json");
    }
    let mut request = request
        .header("x-forwarded-for", "192.0.2.55, 198.51.100.10")
        .body(
            body.map(|value| Body::from(value.to_string()))
                .unwrap_or_else(Body::empty),
        )
        .unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([203, 0, 113, 55], 42_000))));
    request
}

async fn json_body(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body()).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn assert_privacy_no_store(response: &axum::response::Response) {
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store, max-age=0"
    );
    assert_eq!(response.headers().get(header::PRAGMA).unwrap(), "no-cache");
}

async fn apply_canonical_privacy_schema(client: &Client) {
    client
        .batch_execute("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
        .await
        .expect("reset disposable schema");
    let steps_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../database/sql/steps");
    let mut steps = std::fs::read_dir(&steps_dir)
        .expect("read canonical migration directory")
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            let file_name = path.file_name()?.to_str()?.to_string();
            let version = file_name
                .split_once('_')
                .and_then(|(version, _)| version.parse::<u32>().ok())?;
            Some((version, file_name, path))
        })
        .collect::<Vec<_>>();
    steps.sort_by(|left, right| (left.0, &left.1).cmp(&(right.0, &right.1)));
    assert_eq!(steps.first().map(|step| step.0), Some(1));
    assert!(steps.last().is_some_and(|step| step.0 >= 53));
    for duplicate in steps.windows(2) {
        assert_ne!(
            duplicate[0].0, duplicate[1].0,
            "duplicate migration version"
        );
    }
    for (version, file_name, path) in steps {
        let sql = std::fs::read_to_string(path).expect("read canonical migration SQL");
        client
            .batch_execute(&sql)
            .await
            .unwrap_or_else(|error| panic!("apply migration {version} {file_name}: {error:?}"));
    }
}

fn find_communication<'a>(matrix: &'a Value, channel: &str, category: &str) -> &'a Value {
    matrix["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["channel"] == channel && entry["category"] == category)
        .unwrap()
}

/// Owns and resets `UNIVERSUS_TEST_DATABASE_URL`; the database must be disposable.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires disposable PostgreSQL in UNIVERSUS_TEST_DATABASE_URL"]
async fn signed_privacy_routes_are_owner_scoped_durable_and_consent_gated() {
    let prior_jwt_secret = std::env::var("JWT_SECRET").ok();
    let prior_legacy_hmac = std::env::var("AUTH_ALLOW_LEGACY_HS256").ok();
    let prior_export_key_id = std::env::var("PRIVACY_EXPORT_KEY_ID").ok();
    let prior_export_key = std::env::var("PRIVACY_EXPORT_KEY_BASE64").ok();
    let prior_worker_url = std::env::var("PRIVACY_WORKER_INTERNAL_URL").ok();
    std::env::set_var("JWT_SECRET", "default-secret");
    std::env::set_var("AUTH_ALLOW_LEGACY_HS256", "true");

    let database_url = std::env::var("UNIVERSUS_TEST_DATABASE_URL")
        .expect("UNIVERSUS_TEST_DATABASE_URL must name a disposable PostgreSQL database");
    let (client, connection) = tokio_postgres::connect(&database_url, NoTls)
        .await
        .expect("connect disposable PostgreSQL");
    tokio::spawn(async move {
        connection.await.expect("PostgreSQL test connection");
    });
    apply_canonical_privacy_schema(&client).await;

    let database = Database::from_database_url(&database_url).unwrap();
    let subject = database
        .register_account_with_starting_state(account_input(
            "PrivacyRouteSubject",
            "privacy-route-subject@example.test",
        ))
        .await
        .unwrap();
    let same_tenant_other = database
        .register_account_with_starting_state(account_input(
            "PrivacyRouteOther",
            "privacy-route-other@example.test",
        ))
        .await
        .unwrap();
    client
        .execute(
            "INSERT INTO universes (id, name, speed, registration_open)
             VALUES (2, 'Privacy API Tenant Two', 1, TRUE)
             ON CONFLICT (id) DO NOTHING",
            &[],
        )
        .await
        .unwrap();
    let tenant_two_user = client
        .query_one(
            "INSERT INTO users (username, email, password_hash, universe_id, is_admin)
             VALUES ('PrivacyRouteTenantTwo', 'privacy-route-tenant-two@example.test',
                     '!test!', 2, FALSE)
             RETURNING id",
            &[],
        )
        .await
        .unwrap()
        .get::<_, i32>("id");

    let subject_token = session_token(&database, &subject.id, 1).await;
    let same_tenant_token = session_token(&database, &same_tenant_other.id, 2).await;
    let tenant_two_token = session_token(&database, &tenant_two_user.to_string(), 3).await;

    // Production-like construction captures missing pepper as unavailable.
    // The environment returns to development before JWT validation, proving
    // the 503 comes from privacy configuration rather than auth mode.
    let prior_environment = std::env::var("UNIVERSUS_ENV").ok();
    let prior_pepper = std::env::var("PRIVACY_REQUEST_IP_PEPPER").ok();
    std::env::set_var("UNIVERSUS_ENV", "production");
    std::env::remove_var("PRIVACY_REQUEST_IP_PEPPER");
    let unavailable = build_router_with_dependencies(
        "privacy-api-test",
        Some(database.clone()),
        AccountRepository::from_environment(Some(database.clone())),
    );
    std::env::set_var("UNIVERSUS_ENV", "development");
    let response = unavailable
        .oneshot(request(
            "GET",
            "/api/privacy/requests",
            &subject_token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_privacy_no_store(&response);
    assert_eq!(
        json_body(response).await["code"],
        "privacy_repository_unavailable"
    );

    std::env::set_var(
        "PRIVACY_REQUEST_IP_PEPPER",
        "privacy-api-postgres-test-pepper-32-bytes-minimum",
    );
    std::env::set_var("PRIVACY_POLICY_VERSION", "privacy-v1");
    std::env::set_var("PRIVACY_EXPORT_KEY_ID", "v1:privacy-route-test");
    std::env::set_var(
        "PRIVACY_EXPORT_KEY_BASE64",
        "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=",
    );
    let delivery_mock = axum::Router::new()
        .route(
            "/api/privacy/exports/:request_id/delivery",
            axum::routing::post(|| async {
                axum::Json(json!({
                    "token": "privacy-one-time-grant",
                    "expiresAtUnix": 4_102_444_800_i64
                }))
            }),
        )
        .route(
            "/api/privacy/exports/:request_id/download",
            axum::routing::post(|headers: axum::http::HeaderMap| async move {
                assert_eq!(
                    headers
                        .get("x-privacy-delivery-token")
                        .and_then(|value| value.to_str().ok()),
                    Some("privacy-one-time-grant")
                );
                assert!(headers.get(header::AUTHORIZATION).is_some());
                (
                    [
                        (header::CACHE_CONTROL, "no-store, max-age=0"),
                        (header::PRAGMA, "no-cache"),
                        (
                            header::CONTENT_DISPOSITION,
                            "attachment; filename=\"universus-data-export-1.json\"",
                        ),
                        (header::CONTENT_TYPE, "application/json"),
                    ],
                    "{\"schemaVersion\":1}",
                )
            }),
        );
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let delivery_address = listener.local_addr().unwrap();
    let delivery_server = axum::Server::from_tcp(listener)
        .unwrap()
        .serve(delivery_mock.into_make_service());
    let delivery_server = tokio::spawn(async move {
        let _ = delivery_server.await;
    });
    std::env::set_var(
        "PRIVACY_WORKER_INTERNAL_URL",
        format!("http://{delivery_address}"),
    );
    let app = build_router_with_dependencies(
        "privacy-api-test",
        Some(database.clone()),
        AccountRepository::from_environment(Some(database.clone())),
    );

    let export_body = json!({
        "requestType": "export",
        "idempotencyKey": "privacy-api-export-0001"
    });
    let export = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/privacy/requests",
            &subject_token,
            Some(export_body.clone()),
        ))
        .await
        .unwrap();
    assert_eq!(export.status(), StatusCode::OK);
    assert_privacy_no_store(&export);
    let export = json_body(export).await;
    let export_id = export["data"]["id"].as_i64().unwrap() as i32;
    assert_eq!(export["data"]["status"], "queued");

    let repeated = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/privacy/requests",
            &subject_token,
            Some(export_body),
        ))
        .await
        .unwrap();
    assert_eq!(repeated.status(), StatusCode::OK);
    assert_eq!(json_body(repeated).await["data"]["id"], export_id);

    let conflict = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/privacy/requests",
            &subject_token,
            Some(json!({
                "requestType": "correction",
                "idempotencyKey": "privacy-api-export-0001",
                "confirmation": "APPLY MY CORRECTIONS",
                "changes": {"email": "conflict@example.test"}
            })),
        ))
        .await
        .unwrap();
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
    assert_privacy_no_store(&conflict);
    assert_eq!(json_body(conflict).await["code"], "privacy_conflict");

    let evidence = client
        .query_one(
            "SELECT requester_ip_digest, request_source FROM gdpr_requests WHERE id = $1",
            &[&export_id],
        )
        .await
        .unwrap();
    assert_eq!(evidence.get::<_, Vec<u8>>("requester_ip_digest").len(), 32);
    assert_eq!(
        evidence.get::<_, String>("request_source"),
        "user_self_service"
    );

    let claimed = database
        .claim_privacy_jobs("privacy-api-export-worker", Some(1), 1, 30)
        .await
        .unwrap();
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].request_id, export_id);
    database
        .complete_privacy_export_job(
            claimed[0].id,
            "privacy-api-export-worker",
            PreparedExportArtifact {
                ciphertext: vec![1, 2, 3, 4],
                encryption_key_id: "v1:privacy-api-test".to_string(),
                encryption_nonce: [2; 12],
                plaintext_sha256: [3; 32],
                plaintext_size: 128,
                expires_in_seconds: 3600,
            },
        )
        .await
        .unwrap();
    let detail = app
        .clone()
        .oneshot(request(
            "GET",
            &format!("/api/privacy/requests/{export_id}"),
            &subject_token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(detail.status(), StatusCode::OK);
    let detail = json_body(detail).await;
    assert_eq!(detail["data"]["request"]["export"]["ready"], true);
    assert_eq!(
        detail["data"]["request"]["export"]["deliveryAvailable"],
        true
    );
    assert_eq!(
        detail["data"]["request"]["export"]["deliveryStatus"],
        "ready"
    );
    assert!(detail["data"]["timeline"].as_array().unwrap().len() >= 2);

    let grant = app
        .clone()
        .oneshot(request(
            "POST",
            &format!("/api/privacy/requests/{export_id}/delivery"),
            &subject_token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(grant.status(), StatusCode::OK);
    assert_privacy_no_store(&grant);
    assert_eq!(json_body(grant).await["token"], "privacy-one-time-grant");
    let mut download_request = request(
        "POST",
        &format!("/api/privacy/requests/{export_id}/download"),
        &subject_token,
        None,
    );
    download_request.headers_mut().insert(
        "x-privacy-delivery-token",
        "privacy-one-time-grant".parse().unwrap(),
    );
    let download = app.clone().oneshot(download_request).await.unwrap();
    assert_eq!(download.status(), StatusCode::OK);
    assert_privacy_no_store(&download);
    assert_eq!(
        download.headers().get(header::CONTENT_DISPOSITION).unwrap(),
        "attachment; filename=\"universus-data-export-1.json\""
    );
    assert_eq!(json_body(download).await["schemaVersion"], 1);

    for other_token in [&same_tenant_token, &tenant_two_token] {
        let hidden = app
            .clone()
            .oneshot(request(
                "GET",
                &format!("/api/privacy/requests/{export_id}"),
                other_token,
                None,
            ))
            .await
            .unwrap();
        assert_eq!(hidden.status(), StatusCode::NOT_FOUND);
        assert_eq!(json_body(hidden).await["code"], "privacy_not_found");
    }

    let correction = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/privacy/requests",
            &subject_token,
            Some(json!({
                "requestType": "correction",
                "idempotencyKey": "privacy-api-correction-0001",
                "confirmation": "APPLY MY CORRECTIONS",
                "changes": {
                    "username": "PrivacyCorrected",
                    "email": "privacy-corrected@example.test",
                    "phoneNumber": null
                }
            })),
        ))
        .await
        .unwrap();
    assert_eq!(correction.status(), StatusCode::OK);
    let correction = json_body(correction).await;
    assert_eq!(correction["data"]["status"], "in_review");
    let correction_id = correction["data"]["id"].as_i64().unwrap() as i32;
    let encrypted = client
        .query_one(
            "SELECT request_payload_ciphertext, payload_key_id, payload_nonce,
                    payload_sha256
             FROM gdpr_requests WHERE id = $1",
            &[&correction_id],
        )
        .await
        .unwrap();
    let ciphertext = encrypted.get::<_, Vec<u8>>("request_payload_ciphertext");
    assert!(ciphertext.len() > 16);
    assert!(!ciphertext
        .windows("privacy-corrected@example.test".len())
        .any(|window| window == b"privacy-corrected@example.test"));
    assert_eq!(
        encrypted.get::<_, String>("payload_key_id"),
        "v1:privacy-route-test"
    );
    assert_eq!(encrypted.get::<_, Vec<u8>>("payload_nonce").len(), 12);
    assert_eq!(encrypted.get::<_, Vec<u8>>("payload_sha256").len(), 32);

    let weak_restriction = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/privacy/requests",
            &subject_token,
            Some(json!({
                "requestType": "restriction",
                "idempotencyKey": "privacy-api-restriction-weak",
                "confirmation": "yes"
            })),
        ))
        .await
        .unwrap();
    assert_eq!(weak_restriction.status(), StatusCode::BAD_REQUEST);

    let restriction = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/privacy/requests",
            &subject_token,
            Some(json!({
                "requestType": "restriction",
                "idempotencyKey": "privacy-api-restriction-0001",
                "confirmation": "RESTRICT MY ACCOUNT"
            })),
        ))
        .await
        .unwrap();
    assert_eq!(restriction.status(), StatusCode::OK);
    let restriction_id = json_body(restriction).await["data"]["id"].as_i64().unwrap() as i32;

    let erasure = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/privacy/requests",
            &subject_token,
            Some(json!({
                "requestType": "erasure",
                "idempotencyKey": "privacy-api-erasure-0001",
                "confirmation": "ERASE MY ACCOUNT"
            })),
        ))
        .await
        .unwrap();
    assert_eq!(erasure.status(), StatusCode::OK);
    let erasure = json_body(erasure).await;
    let erasure_id = erasure["data"]["id"].as_i64().unwrap() as i32;
    assert_eq!(erasure["data"]["status"], "cooling_off");
    assert!(erasure["data"]["coolingOffUntilUnix"].as_i64().is_some());

    let cancelled = app
        .clone()
        .oneshot(request(
            "POST",
            &format!("/api/privacy/requests/{erasure_id}/cancel"),
            &subject_token,
            Some(json!({
                "expectedVersion": erasure["data"]["version"],
                "confirmation": "CANCEL REQUEST"
            })),
        ))
        .await
        .unwrap();
    assert_eq!(cancelled.status(), StatusCode::OK);
    assert_eq!(json_body(cancelled).await["data"]["status"], "cancelled");

    let list = app
        .clone()
        .oneshot(request(
            "GET",
            "/api/privacy/requests?limit=20",
            &subject_token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    assert_eq!(json_body(list).await["data"].as_array().unwrap().len(), 4);

    let consents = app
        .clone()
        .oneshot(request(
            "GET",
            "/api/privacy/consents",
            &subject_token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(consents.status(), StatusCode::OK);
    assert_eq!(
        json_body(consents).await["data"]["currentPolicyVersion"],
        "privacy-v1"
    );

    client
        .execute(
            "INSERT INTO privacy_communication_preferences (
                universe_id, user_id, channel, category, enabled
             ) VALUES (1, $1, 'email', 'security', FALSE)",
            &[&subject.id.parse::<i32>().unwrap()],
        )
        .await
        .expect("seed a legacy disabled essential preference");
    let matrix = app
        .clone()
        .oneshot(request(
            "GET",
            "/api/privacy/communications",
            &subject_token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(matrix.status(), StatusCode::OK);
    let matrix = json_body(matrix).await;
    assert_eq!(matrix["data"].as_array().unwrap().len(), 20);
    let security = find_communication(&matrix, "email", "security");
    assert_eq!(security["essential"], true);
    assert_eq!(security["enabled"], true);
    assert_eq!(security["effectiveAllowed"], true);
    assert_eq!(security["explicitlyConfigured"], true);
    let marketing = find_communication(&matrix, "email", "marketing");
    assert_eq!(marketing["enabled"], false);
    assert_eq!(marketing["effectiveAllowed"], false);

    for enabled in [false, true] {
        let essential_update = app
            .clone()
            .oneshot(request(
                "PUT",
                "/api/privacy/communications/email/security",
                &subject_token,
                Some(json!({"enabled": enabled, "expectedVersion": 1})),
            ))
            .await
            .unwrap();
        assert_eq!(essential_update.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            json_body(essential_update).await["code"],
            "privacy_essential_communication_read_only"
        );
    }

    let preference = app
        .clone()
        .oneshot(request(
            "PUT",
            "/api/privacy/communications/email/marketing",
            &subject_token,
            Some(json!({"enabled": true, "expectedVersion": 0})),
        ))
        .await
        .unwrap();
    assert_eq!(preference.status(), StatusCode::OK);
    let preference = json_body(preference).await;
    assert_eq!(preference["data"]["enabled"], true);
    assert_eq!(preference["data"]["effectiveAllowed"], false);

    let consent = app
        .clone()
        .oneshot(request(
            "PUT",
            "/api/privacy/consents/email",
            &subject_token,
            Some(json!({
                "status": "granted",
                "policyVersion": "privacy-v1",
                "expectedVersion": 0,
                "confirmed": true
            })),
        ))
        .await
        .unwrap();
    assert_eq!(consent.status(), StatusCode::OK);
    let consent = json_body(consent).await;
    assert_eq!(consent["data"]["version"], 1);

    let replay = app
        .clone()
        .oneshot(request(
            "PUT",
            "/api/privacy/consents/email",
            &subject_token,
            Some(json!({
                "status": "granted",
                "policyVersion": "privacy-v1",
                "expectedVersion": 1,
                "confirmed": true
            })),
        ))
        .await
        .unwrap();
    assert_eq!(replay.status(), StatusCode::OK);
    assert_eq!(json_body(replay).await["data"]["version"], 1);
    assert_eq!(
        client
            .query_one(
                "SELECT COUNT(*)::BIGINT FROM privacy_consent_events
                 WHERE universe_id = 1 AND user_id = $1
                   AND purpose = 'marketing' AND channel = 'email'",
                &[&subject.id.parse::<i32>().unwrap()],
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );

    let matrix = app
        .clone()
        .oneshot(request(
            "GET",
            "/api/privacy/communications",
            &subject_token,
            None,
        ))
        .await
        .unwrap();
    let matrix = json_body(matrix).await;
    assert_eq!(
        find_communication(&matrix, "email", "marketing")["effectiveAllowed"],
        true
    );

    let withdrawn = app
        .clone()
        .oneshot(request(
            "PUT",
            "/api/privacy/consents/email",
            &subject_token,
            Some(json!({
                "status": "withdrawn",
                "policyVersion": "privacy-v1",
                "expectedVersion": 1,
                "confirmed": false
            })),
        ))
        .await
        .unwrap();
    assert_eq!(withdrawn.status(), StatusCode::OK);
    assert_eq!(json_body(withdrawn).await["data"]["version"], 2);

    let stale = app
        .clone()
        .oneshot(request(
            "PUT",
            "/api/privacy/consents/email",
            &subject_token,
            Some(json!({
                "status": "granted",
                "policyVersion": "privacy-v1",
                "expectedVersion": 1,
                "confirmed": true
            })),
        ))
        .await
        .unwrap();
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    assert_eq!(json_body(stale).await["code"], "privacy_version_conflict");

    let precedence_subject = database
        .register_account_with_starting_state(account_input(
            "PrivacyRoutePrecedence",
            "privacy-route-precedence@example.test",
        ))
        .await
        .unwrap();
    let precedence_token = session_token(&database, &precedence_subject.id, 4).await;
    for channel in ["email", "push"] {
        let response = app
            .clone()
            .oneshot(request(
                "PUT",
                &format!("/api/privacy/communications/{channel}/marketing"),
                &precedence_token,
                Some(json!({"enabled": true, "expectedVersion": 0})),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
    let all_granted = app
        .clone()
        .oneshot(request(
            "PUT",
            "/api/privacy/consents/all",
            &precedence_token,
            Some(json!({
                "status": "granted", "policyVersion": "privacy-v1",
                "expectedVersion": 0, "confirmed": true
            })),
        ))
        .await
        .unwrap();
    assert_eq!(all_granted.status(), StatusCode::OK);
    let email_denied = app
        .clone()
        .oneshot(request(
            "PUT",
            "/api/privacy/consents/email",
            &precedence_token,
            Some(json!({
                "status": "denied", "policyVersion": "privacy-v1",
                "expectedVersion": 0, "confirmed": false
            })),
        ))
        .await
        .unwrap();
    assert_eq!(email_denied.status(), StatusCode::OK);
    let matrix = app
        .clone()
        .oneshot(request(
            "GET",
            "/api/privacy/communications",
            &precedence_token,
            None,
        ))
        .await
        .unwrap();
    let matrix = json_body(matrix).await;
    let email = find_communication(&matrix, "email", "marketing");
    let push = find_communication(&matrix, "push", "marketing");
    assert_eq!(email["marketingConsentCurrent"], false);
    assert_eq!(email["effectiveAllowed"], false);
    assert_eq!(push["marketingConsentCurrent"], true);
    assert_eq!(push["effectiveAllowed"], true);

    for (channel, status, expected_version, confirmed) in [
        ("email", "granted", 1, true),
        ("all", "withdrawn", 1, false),
    ] {
        let response = app
            .clone()
            .oneshot(request(
                "PUT",
                &format!("/api/privacy/consents/{channel}"),
                &precedence_token,
                Some(json!({
                    "status": status, "policyVersion": "privacy-v1",
                    "expectedVersion": expected_version, "confirmed": confirmed
                })),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
    let matrix = app
        .clone()
        .oneshot(request(
            "GET",
            "/api/privacy/communications",
            &precedence_token,
            None,
        ))
        .await
        .unwrap();
    let matrix = json_body(matrix).await;
    assert_eq!(
        find_communication(&matrix, "email", "marketing")["effectiveAllowed"],
        true
    );
    assert_eq!(
        find_communication(&matrix, "push", "marketing")["effectiveAllowed"],
        false
    );

    let precedence_user_id = precedence_subject.id.parse::<i32>().unwrap();
    let expired_at = client
        .query_one(
            "SELECT EXTRACT(EPOCH FROM now() - interval '1 minute')::BIGINT",
            &[],
        )
        .await
        .unwrap()
        .get::<_, i64>(0);
    for (channel, expires_at_unix) in [("all", None), ("email", Some(expired_at))] {
        database
            .set_privacy_consent(ConsentUpdate {
                universe_id: 1,
                user_id: precedence_user_id,
                purpose: "marketing".to_string(),
                channel: channel.to_string(),
                status: ConsentStatus::Granted,
                lawful_basis: "consent".to_string(),
                policy_version: "privacy-v1".to_string(),
                proof_digest: Some([12; 32]),
                expires_at_unix,
                changed_by_user_id: precedence_user_id,
                actor_type: "user".to_string(),
            })
            .await
            .unwrap();
    }
    let matrix = app
        .clone()
        .oneshot(request(
            "GET",
            "/api/privacy/communications",
            &precedence_token,
            None,
        ))
        .await
        .unwrap();
    let matrix = json_body(matrix).await;
    assert_eq!(
        find_communication(&matrix, "email", "marketing")["marketingConsentCurrent"],
        false
    );
    assert_eq!(
        find_communication(&matrix, "push", "marketing")["marketingConsentCurrent"],
        true
    );

    // Rebuilding the full router simulates a process restart; every request,
    // consent and preference remains in PostgreSQL while the session is live.
    let restarted = build_router_with_dependencies(
        "privacy-api-test-restarted",
        Some(database.clone()),
        AccountRepository::from_environment(Some(database.clone())),
    );
    let durable = restarted
        .oneshot(request(
            "GET",
            "/api/privacy/requests?limit=20",
            &subject_token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(durable.status(), StatusCode::OK);
    assert_eq!(
        json_body(durable).await["data"].as_array().unwrap().len(),
        4
    );

    let restriction_claim = database
        .claim_privacy_jobs("privacy-api-restriction-worker", Some(1), 1, 30)
        .await
        .unwrap();
    assert_eq!(restriction_claim.len(), 1);
    assert_eq!(restriction_claim[0].request_id, restriction_id);
    database
        .complete_privacy_restriction_job(restriction_claim[0].id, "privacy-api-restriction-worker")
        .await
        .unwrap();
    let restricted = app
        .clone()
        .oneshot(request(
            "GET",
            "/api/privacy/communications",
            &subject_token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(restricted.status(), StatusCode::UNAUTHORIZED);
    let restricted = database
        .communication_preferences_for_owner(1, subject.id.parse::<i32>().unwrap())
        .await
        .unwrap();
    let marketing = restricted
        .iter()
        .find(|entry| entry.channel == "email" && entry.category == "marketing")
        .unwrap();
    assert!(marketing.suppressed_by_restriction);
    assert!(!marketing.effective_allowed);
    assert!(
        restricted
            .iter()
            .find(|entry| entry.channel == "email" && entry.category == "security")
            .unwrap()
            .effective_allowed
    );

    match prior_environment {
        Some(value) => std::env::set_var("UNIVERSUS_ENV", value),
        None => std::env::remove_var("UNIVERSUS_ENV"),
    }
    match prior_pepper {
        Some(value) => std::env::set_var("PRIVACY_REQUEST_IP_PEPPER", value),
        None => std::env::remove_var("PRIVACY_REQUEST_IP_PEPPER"),
    }
    match prior_jwt_secret {
        Some(value) => std::env::set_var("JWT_SECRET", value),
        None => std::env::remove_var("JWT_SECRET"),
    }
    match prior_legacy_hmac {
        Some(value) => std::env::set_var("AUTH_ALLOW_LEGACY_HS256", value),
        None => std::env::remove_var("AUTH_ALLOW_LEGACY_HS256"),
    }
    match prior_export_key_id {
        Some(value) => std::env::set_var("PRIVACY_EXPORT_KEY_ID", value),
        None => std::env::remove_var("PRIVACY_EXPORT_KEY_ID"),
    }
    match prior_export_key {
        Some(value) => std::env::set_var("PRIVACY_EXPORT_KEY_BASE64", value),
        None => std::env::remove_var("PRIVACY_EXPORT_KEY_BASE64"),
    }
    match prior_worker_url {
        Some(value) => std::env::set_var("PRIVACY_WORKER_INTERNAL_URL", value),
        None => std::env::remove_var("PRIVACY_WORKER_INTERNAL_URL"),
    }
    delivery_server.abort();
}
