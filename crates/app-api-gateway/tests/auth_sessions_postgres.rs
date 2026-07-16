use std::net::SocketAddr;
use std::path::Path;

use app_api_gateway::accounts::AccountRepository;
use app_api_gateway::routes::build_router_with_dependencies;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{header, Request, StatusCode};
use hyper::body::to_bytes;
use platform_db::Database;
use serde_json::{json, Value};
use tokio_postgres::{Client, NoTls};
use tower::ServiceExt;

async fn apply_all_canonical_migrations(client: &Client) {
    let steps_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../database/sql/steps");
    let mut steps = std::fs::read_dir(&steps_dir)
        .expect("read canonical migration directory")
        .map(|entry| {
            let path = entry.expect("read migration entry").path();
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .expect("UTF-8 migration filename")
                .to_string();
            let version = file_name
                .split_once('_')
                .and_then(|(version, _)| version.parse::<u32>().ok());
            (version, file_name, path)
        })
        .filter_map(|(version, file_name, path)| version.map(|version| (version, file_name, path)))
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

fn request(
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
    ip: [u8; 4],
) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(path);
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    if body.is_some() {
        builder = builder.header(header::CONTENT_TYPE, "application/json");
    }
    builder = builder.header(header::USER_AGENT, "Universus auth integration test");
    let mut request = builder
        .body(
            body.map(|value| Body::from(value.to_string()))
                .unwrap_or_else(Body::empty),
        )
        .unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo(SocketAddr::from((ip, 42_000))));
    request
}

async fn json_body(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body()).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn login(
    app: &axum::Router,
    email: &str,
    password: &str,
    ip: [u8; 4],
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/auth/login",
            None,
            Some(json!({
                "email": email,
                "password": password,
                "deviceLabel": "Integration browser"
            })),
            ip,
        ))
        .await
        .unwrap();
    let status = response.status();
    (status, json_body(response).await)
}

async fn register(
    app: &axum::Router,
    username: &str,
    email: &str,
    password: &str,
    ip: [u8; 4],
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/auth/register",
            None,
            Some(json!({
                "username": username,
                "email": email,
                "password": password,
                "deviceLabel": "Registration integration browser"
            })),
            ip,
        ))
        .await
        .unwrap();
    let status = response.status();
    (status, json_body(response).await)
}

async fn refresh(app: &axum::Router, token: &str, ip: [u8; 4]) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/auth/refresh",
            None,
            Some(json!({"refreshToken": token})),
            ip,
        ))
        .await
        .unwrap();
    let status = response.status();
    (status, json_body(response).await)
}

fn auth_data(body: &Value, field: &str) -> String {
    body["data"][field]
        .as_str()
        .unwrap_or_else(|| panic!("missing auth field {field}: {body}"))
        .to_string()
}

/// Owns and resets `UNIVERSUS_TEST_DATABASE_URL`; the database must be disposable.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires disposable PostgreSQL in UNIVERSUS_TEST_DATABASE_URL"]
async fn durable_session_rotation_replay_invalidation_and_throttling_journey() {
    std::env::set_var("UNIVERSUS_ENV", "test");
    std::env::set_var("JWT_SECRET", "integration-only-jwt-secret");
    std::env::set_var("AUTH_ALLOW_LEGACY_HS256", "true");
    std::env::set_var(
        "AUTH_SESSION_DIGEST_KEY",
        "integration-only-session-digest-key-at-least-32-bytes",
    );
    std::env::set_var("AUTH_LOGIN_ACCOUNT_FAILURE_LIMIT", "2");
    std::env::set_var("AUTH_LOGIN_IP_FAILURE_LIMIT", "2");
    std::env::set_var("AUTH_LOGIN_WINDOW_SECONDS", "900");
    std::env::set_var("AUTH_LOGIN_BLOCK_SECONDS", "900");
    std::env::set_var("AUTH_REGISTRATION_IP_WINDOW_SECONDS", "3600");
    std::env::set_var("AUTH_REGISTRATION_IP_ATTEMPT_LIMIT", "2");
    std::env::set_var("AUTH_REGISTRATION_IP_BLOCK_SECONDS", "3600");

    let database_url = std::env::var("UNIVERSUS_TEST_DATABASE_URL")
        .expect("UNIVERSUS_TEST_DATABASE_URL must name a disposable PostgreSQL database");
    let (client, connection) = tokio_postgres::connect(&database_url, NoTls)
        .await
        .expect("connect disposable PostgreSQL");
    tokio::spawn(async move {
        connection.await.expect("PostgreSQL test connection");
    });
    client
        .batch_execute("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
        .await
        .unwrap();
    apply_all_canonical_migrations(&client).await;
    // The forward migration must remain rerunnable for partially provisioned
    // environments and must never require editing an older checksum.
    client
        .batch_execute(include_str!(
            "../../../database/sql/steps/53_auth_sessions.sql"
        ))
        .await
        .expect("auth migration is idempotent");

    let password_hash = platform_auth::hash_password("CorrectHorse1").unwrap();
    let user_id: i32 = client
        .query_one(
            "INSERT INTO users (username, email, password_hash, universe_id)
             VALUES ('AuthCommander', 'auth@example.test', $1, 1)
             RETURNING id",
            &[&password_hash],
        )
        .await
        .expect("seed user directly without gameplay/alliance fixtures")
        .get("id");

    let database = Database::from_database_url(&database_url).unwrap();
    database.auth_repository_ready().await.unwrap();
    let app = build_router_with_dependencies(
        "auth-test",
        Some(database.clone()),
        AccountRepository::from_environment(Some(database.clone())),
    );

    let (status, first_login) = login(
        &app,
        " AUTH@example.test ",
        "CorrectHorse1",
        [192, 0, 2, 10],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first_login}");
    let first_access = auth_data(&first_login, "token");
    let first_refresh = auth_data(&first_login, "refreshToken");
    let first_session = auth_data(&first_login, "sessionId");
    assert!(!first_access.contains(&first_refresh));

    let me = app
        .clone()
        .oneshot(request(
            "GET",
            "/api/auth/me",
            Some(&first_access),
            None,
            [192, 0, 2, 10],
        ))
        .await
        .unwrap();
    assert_eq!(me.status(), StatusCode::OK);

    let (status, rotated) = refresh(&app, &first_refresh, [192, 0, 2, 10]).await;
    assert_eq!(status, StatusCode::OK, "{rotated}");
    let rotated_access = auth_data(&rotated, "token");
    let rotated_refresh = auth_data(&rotated, "refreshToken");
    assert_ne!(first_refresh, rotated_refresh);
    assert_eq!(auth_data(&rotated, "sessionId"), first_session);

    let (status, replay) = refresh(&app, &first_refresh, [192, 0, 2, 10]).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{replay}");
    let rejected = app
        .clone()
        .oneshot(request(
            "GET",
            "/api/auth/me",
            Some(&rotated_access),
            None,
            [192, 0, 2, 10],
        ))
        .await
        .unwrap();
    assert_eq!(rejected.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        refresh(&app, &rotated_refresh, [192, 0, 2, 10]).await.0,
        StatusCode::UNAUTHORIZED
    );

    // A fresh pool/router proves sessions survive gateway restarts.
    let (status, persistent_login) =
        login(&app, "auth@example.test", "CorrectHorse1", [192, 0, 2, 11]).await;
    assert_eq!(status, StatusCode::OK, "{persistent_login}");
    let persistent_access = auth_data(&persistent_login, "token");
    let persistent_session = auth_data(&persistent_login, "sessionId");
    let restarted_database = Database::from_database_url(&database_url).unwrap();
    let restarted = build_router_with_dependencies(
        "auth-restarted-test",
        Some(restarted_database.clone()),
        AccountRepository::from_environment(Some(restarted_database)),
    );
    let sessions = restarted
        .clone()
        .oneshot(request(
            "GET",
            "/api/auth/sessions",
            Some(&persistent_access),
            None,
            [192, 0, 2, 11],
        ))
        .await
        .unwrap();
    assert_eq!(sessions.status(), StatusCode::OK);
    let sessions = json_body(sessions).await;
    assert!(sessions["data"].as_array().unwrap().iter().any(|session| {
        session["sessionId"] == persistent_session && session["current"] == true
    }));

    let logout = restarted
        .clone()
        .oneshot(request(
            "POST",
            "/api/auth/logout",
            Some(&persistent_access),
            None,
            [192, 0, 2, 11],
        ))
        .await
        .unwrap();
    assert_eq!(logout.status(), StatusCode::OK);
    assert_eq!(json_body(logout).await["data"]["revoked"], true);
    let after_logout = restarted
        .clone()
        .oneshot(request(
            "GET",
            "/api/auth/me",
            Some(&persistent_access),
            None,
            [192, 0, 2, 11],
        ))
        .await
        .unwrap();
    assert_eq!(after_logout.status(), StatusCode::UNAUTHORIZED);

    // Session management is actor-derived: one session can revoke another
    // owned session, and revoke-all immediately kills the caller as well.
    let (status, manager_login) =
        login(&app, "auth@example.test", "CorrectHorse1", [192, 0, 2, 30]).await;
    assert_eq!(status, StatusCode::OK, "{manager_login}");
    let manager_access = auth_data(&manager_login, "token");
    let (status, secondary_login) =
        login(&app, "auth@example.test", "CorrectHorse1", [192, 0, 2, 31]).await;
    assert_eq!(status, StatusCode::OK, "{secondary_login}");
    let secondary_access = auth_data(&secondary_login, "token");
    let secondary_session = auth_data(&secondary_login, "sessionId");
    let revoke_secondary = app
        .clone()
        .oneshot(request(
            "DELETE",
            &format!("/api/auth/sessions/{secondary_session}"),
            Some(&manager_access),
            None,
            [192, 0, 2, 30],
        ))
        .await
        .unwrap();
    assert_eq!(revoke_secondary.status(), StatusCode::OK);
    assert_eq!(json_body(revoke_secondary).await["data"]["revoked"], true);
    let secondary_rejected = app
        .clone()
        .oneshot(request(
            "GET",
            "/api/auth/me",
            Some(&secondary_access),
            None,
            [192, 0, 2, 31],
        ))
        .await
        .unwrap();
    assert_eq!(secondary_rejected.status(), StatusCode::UNAUTHORIZED);
    let manager_still_live = app
        .clone()
        .oneshot(request(
            "GET",
            "/api/auth/me",
            Some(&manager_access),
            None,
            [192, 0, 2, 30],
        ))
        .await
        .unwrap();
    assert_eq!(manager_still_live.status(), StatusCode::OK);
    let revoke_all = app
        .clone()
        .oneshot(request(
            "DELETE",
            "/api/auth/sessions",
            Some(&manager_access),
            None,
            [192, 0, 2, 30],
        ))
        .await
        .unwrap();
    assert_eq!(revoke_all.status(), StatusCode::OK);
    assert_eq!(json_body(revoke_all).await["data"]["revoked"], true);
    let manager_rejected = app
        .clone()
        .oneshot(request(
            "GET",
            "/api/auth/me",
            Some(&manager_access),
            None,
            [192, 0, 2, 30],
        ))
        .await
        .unwrap();
    assert_eq!(manager_rejected.status(), StatusCode::UNAUTHORIZED);

    // Concurrent use has one rotation winner; the losing replay then revokes
    // the complete family, including the winner's freshly issued access JWT.
    let (status, concurrent_login) =
        login(&app, "auth@example.test", "CorrectHorse1", [192, 0, 2, 12]).await;
    assert_eq!(status, StatusCode::OK, "{concurrent_login}");
    let concurrent_refresh = auth_data(&concurrent_login, "refreshToken");
    let left = refresh(&app, &concurrent_refresh, [192, 0, 2, 12]);
    let right = refresh(&app, &concurrent_refresh, [192, 0, 2, 12]);
    let (left, right) = tokio::join!(left, right);
    let statuses = [left.0, right.0];
    assert_eq!(
        statuses
            .iter()
            .filter(|status| **status == StatusCode::OK)
            .count(),
        1
    );
    assert_eq!(
        statuses
            .iter()
            .filter(|status| **status == StatusCode::UNAUTHORIZED)
            .count(),
        1
    );
    let winner = if left.0 == StatusCode::OK {
        left.1
    } else {
        right.1
    };
    let winner_access = auth_data(&winner, "token");
    let winner_rejected = app
        .clone()
        .oneshot(request(
            "GET",
            "/api/auth/me",
            Some(&winner_access),
            None,
            [192, 0, 2, 12],
        ))
        .await
        .unwrap();
    assert_eq!(winner_rejected.status(), StatusCode::UNAUTHORIZED);

    // Ban and privacy changes bump the epoch and revoke every live session.
    let (status, ban_login) =
        login(&app, "auth@example.test", "CorrectHorse1", [192, 0, 2, 13]).await;
    assert_eq!(status, StatusCode::OK, "{ban_login}");
    let ban_access = auth_data(&ban_login, "token");
    client
        .execute(
            "UPDATE users SET is_banned = TRUE WHERE id = $1",
            &[&user_id],
        )
        .await
        .unwrap();
    let banned = app
        .clone()
        .oneshot(request(
            "GET",
            "/api/auth/me",
            Some(&ban_access),
            None,
            [192, 0, 2, 13],
        ))
        .await
        .unwrap();
    assert_eq!(banned.status(), StatusCode::UNAUTHORIZED);
    client
        .execute(
            "UPDATE users SET is_banned = FALSE WHERE id = $1",
            &[&user_id],
        )
        .await
        .unwrap();
    let (status, privacy_login) =
        login(&app, "auth@example.test", "CorrectHorse1", [192, 0, 2, 14]).await;
    assert_eq!(status, StatusCode::OK, "{privacy_login}");
    let privacy_access = auth_data(&privacy_login, "token");
    client
        .execute(
            "UPDATE users SET privacy_restriction_active = TRUE,
                              privacy_restricted_at = now()
             WHERE id = $1",
            &[&user_id],
        )
        .await
        .unwrap();
    let restricted = app
        .clone()
        .oneshot(request(
            "GET",
            "/api/auth/me",
            Some(&privacy_access),
            None,
            [192, 0, 2, 14],
        ))
        .await
        .unwrap();
    assert_eq!(restricted.status(), StatusCode::UNAUTHORIZED);
    client
        .execute(
            "UPDATE users SET privacy_restriction_active = FALSE,
                              privacy_restricted_at = NULL
             WHERE id = $1",
            &[&user_id],
        )
        .await
        .unwrap();

    // A successful login clears only its account failure window. Failures from
    // a shared IP survive unrelated valid authentication and remain bounded.
    assert_eq!(
        login(
            &app,
            "ghost-one@example.test",
            "WrongPassword1",
            [192, 0, 2, 40]
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        login(&app, "auth@example.test", "CorrectHorse1", [192, 0, 2, 40])
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        login(
            &app,
            "ghost-two@example.test",
            "WrongPassword1",
            [192, 0, 2, 40]
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        login(
            &app,
            "ghost-three@example.test",
            "WrongPassword1",
            [192, 0, 2, 40]
        )
        .await
        .0,
        StatusCode::TOO_MANY_REQUESTS,
        "valid authentication must not clear a shared-IP failure bucket"
    );

    // The normalized account window is reset by successful login while each
    // request uses an isolated IP bucket.
    assert_eq!(
        login(&app, "auth@example.test", "WrongPassword1", [192, 0, 2, 20])
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        login(&app, "auth@example.test", "CorrectHorse1", [192, 0, 2, 22])
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        login(&app, "auth@example.test", "WrongPassword1", [192, 0, 2, 23])
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        login(
            &app,
            "other@example.test",
            "WrongPassword1",
            [192, 0, 2, 21]
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED,
        "a different normalized account and IP must remain isolated"
    );
    assert_eq!(
        login(&app, "auth@example.test", "WrongPassword1", [192, 0, 2, 24])
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        login(&app, "auth@example.test", "WrongPassword1", [192, 0, 2, 25])
            .await
            .0,
        StatusCode::TOO_MANY_REQUESTS
    );

    // Registration has a separate durable IP admission bucket that is checked
    // before password hashing. It neither consumes nor resets login-IP state.
    assert_eq!(
        register(
            &app,
            "RegisterOne",
            "register-one@example.test",
            "RegistrationPass1",
            [192, 0, 2, 60],
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        register(
            &app,
            "RegisterTwo",
            "register-two@example.test",
            "RegistrationPass1",
            [192, 0, 2, 60],
        )
        .await
        .0,
        StatusCode::OK
    );
    let (registration_status, registration_rejected) = register(
        &app,
        "RegisterThree",
        "register-three@example.test",
        "RegistrationPass1",
        [192, 0, 2, 60],
    )
    .await;
    assert_eq!(registration_status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        registration_rejected["error"],
        "Too many registration attempts. Try again later."
    );
    assert_eq!(
        login(
            &app,
            "register-two@example.test",
            "RegistrationPass1",
            [192, 0, 2, 60],
        )
        .await
        .0,
        StatusCode::OK,
        "registration-IP throttling must not spill into login-IP throttling"
    );
    let concurrent_registrations = tokio::join!(
        register(
            &app,
            "ConcurrentRegOne",
            "concurrent-reg-one@example.test",
            "RegistrationPass1",
            [192, 0, 2, 61],
        ),
        register(
            &app,
            "ConcurrentRegTwo",
            "concurrent-reg-two@example.test",
            "RegistrationPass1",
            [192, 0, 2, 61],
        ),
        register(
            &app,
            "ConcurrentRegThree",
            "concurrent-reg-three@example.test",
            "RegistrationPass1",
            [192, 0, 2, 61],
        ),
    );
    let registration_statuses = [
        concurrent_registrations.0 .0,
        concurrent_registrations.1 .0,
        concurrent_registrations.2 .0,
    ];
    assert_eq!(
        registration_statuses
            .iter()
            .filter(|status| **status == StatusCode::OK)
            .count(),
        2
    );
    assert_eq!(
        registration_statuses
            .iter()
            .filter(|status| **status == StatusCode::TOO_MANY_REQUESTS)
            .count(),
        1
    );

    let evidence = client
        .query_one(
            "SELECT
                (SELECT COUNT(*) FROM auth_sessions) > 0 AS has_sessions,
                (SELECT bool_and(octet_length(token_digest) = 32)
                 FROM auth_refresh_tokens) AS digest_only_tokens,
                (SELECT bool_and(octet_length(subject_digest) = 32)
                 FROM auth_login_throttles) AS digest_only_throttles,
                NOT EXISTS (
                    SELECT user_id FROM auth_sessions
                    WHERE revoked_at IS NULL AND expires_at > now()
                    GROUP BY user_id HAVING COUNT(*) > 5
                ) AS active_session_cap_enforced",
            &[],
        )
        .await
        .unwrap();
    assert!(evidence.get::<_, bool>("has_sessions"));
    assert!(evidence.get::<_, bool>("digest_only_tokens"));
    assert!(evidence.get::<_, bool>("digest_only_throttles"));
    assert!(evidence.get::<_, bool>("active_session_cap_enforced"));

    let epoch: i64 = client
        .query_one("SELECT auth_epoch FROM users WHERE id = $1", &[&user_id])
        .await
        .unwrap()
        .get("auth_epoch");
    assert!(epoch > 0);
    assert!(client
        .execute(
            "UPDATE users SET auth_epoch = $2 WHERE id = $1",
            &[&user_id, &epoch.saturating_sub(1)],
        )
        .await
        .is_err());

    client
        .execute("DELETE FROM users WHERE id = $1", &[&user_id])
        .await
        .expect("account erasure cascades through refresh lineage");
    let remaining: i64 = client
        .query_one(
            "SELECT (SELECT COUNT(*) FROM auth_sessions WHERE user_id = $1)
                  + (SELECT COUNT(*)
                     FROM auth_refresh_tokens AS token
                     LEFT JOIN auth_sessions AS session
                       ON session.session_id = token.session_id
                     WHERE session.session_id IS NULL) AS remaining",
            &[&user_id],
        )
        .await
        .unwrap()
        .get("remaining");
    assert_eq!(remaining, 0);
}
