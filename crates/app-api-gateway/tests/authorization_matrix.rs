use app_api_gateway::accounts::AccountRepository;
use app_api_gateway::authorization::{ActorPolicy, RouteAuthorization, ROUTE_AUTHORIZATION};
use app_api_gateway::routes::build_router_with_dependencies;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

#[derive(Clone, Copy, Debug)]
enum TestActor {
    Anonymous,
    Player,
    OtherPlayer,
    Moderator,
    Admin,
    SuperAdmin,
    Service,
}

impl TestActor {
    const ALL: [Self; 7] = [
        Self::Anonymous,
        Self::Player,
        Self::OtherPlayer,
        Self::Moderator,
        Self::Admin,
        Self::SuperAdmin,
        Self::Service,
    ];

    fn subject(self) -> Option<&'static str> {
        match self {
            Self::Anonymous => None,
            Self::Player | Self::Moderator => Some("101"),
            Self::OtherPlayer => Some("202"),
            Self::Admin => Some("admin-1"),
            Self::SuperAdmin => Some("superadmin-1"),
            Self::Service => Some("service-1"),
        }
    }

    fn role(self) -> Option<&'static str> {
        match self {
            Self::Anonymous => None,
            Self::Player | Self::OtherPlayer => Some("player"),
            Self::Moderator => Some("moderator"),
            Self::Admin => Some("admin"),
            Self::SuperAdmin => Some("superadmin"),
            Self::Service => Some("service"),
        }
    }
}

fn app() -> axum::Router {
    build_router_with_dependencies("app-api-gateway", None, AccountRepository::in_memory())
}

fn token(actor: TestActor, service_scope: Option<&str>) -> Option<String> {
    let (Some(subject), Some(role)) = (actor.subject(), actor.role()) else {
        return None;
    };
    let config = platform_auth::AuthConfig::from_env();
    if matches!(actor, TestActor::Service) {
        return platform_auth::generate_service_token(
            &config,
            subject,
            "universus",
            &[service_scope.unwrap_or("test.unscoped")],
            3_600,
        )
        .ok();
    }
    platform_auth::generate_token(
        &config,
        subject,
        &format!("{role}-{subject}"),
        role,
        Some(1),
    )
    .ok()
}

fn concrete_path(template: &str) -> String {
    template
        .split('/')
        .map(|segment| match segment {
            ":user_id" => "101",
            ":achievement_id" | ":badge_id" | ":reward_id" | ":notification_id" => "1",
            ":fleet_id" => "f-1001",
            ":planet_id" => "1",
            ":moon_id" => "101",
            ":message_id" => "m-001",
            ":target_identifier" => "blocked-user",
            ":tech_id" => "energyTechnology",
            ":category" => "general",
            ":galaxy" | ":system" | ":position" | ":id" => "1",
            ":key" => "universe.speed",
            other => other,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn actor_is_allowed(policy: ActorPolicy, actor: TestActor) -> bool {
    match policy {
        ActorPolicy::Public => true,
        ActorPolicy::Player => matches!(
            actor,
            TestActor::Player
                | TestActor::OtherPlayer
                | TestActor::Moderator
                | TestActor::Admin
                | TestActor::SuperAdmin
        ),
        ActorPolicy::Admin => matches!(actor, TestActor::Admin | TestActor::SuperAdmin),
        ActorPolicy::SuperAdmin => matches!(actor, TestActor::SuperAdmin),
        ActorPolicy::Service { .. } => matches!(actor, TestActor::Service),
        ActorPolicy::AdminOrService { .. } => matches!(
            actor,
            TestActor::Admin | TestActor::SuperAdmin | TestActor::Service
        ),
        ActorPolicy::SelfOrAdminPath { .. } => matches!(
            actor,
            TestActor::Player | TestActor::Moderator | TestActor::Admin | TestActor::SuperAdmin
        ),
    }
}

async fn call(app: &axum::Router, rule: &RouteAuthorization, actor: TestActor) -> StatusCode {
    let mut builder = Request::builder()
        .method(rule.method)
        .uri(concrete_path(rule.path))
        .header("content-type", "application/json");
    let service_scope = match rule.policy {
        ActorPolicy::Service { scope } | ActorPolicy::AdminOrService { scope } => Some(scope),
        _ => None,
    };
    if let Some(token) = token(actor, service_scope) {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    app.clone()
        .oneshot(builder.body(Body::from("{}")).expect("request"))
        .await
        .expect("gateway response")
        .status()
}

#[tokio::test]
async fn every_route_enforces_the_complete_actor_matrix() {
    let app = app();
    let mut assertions = 0usize;
    for rule in ROUTE_AUTHORIZATION {
        for actor in TestActor::ALL {
            let status = call(&app, rule, actor).await;
            let allowed = actor_is_allowed(rule.policy, actor);
            if allowed {
                if !matches!(rule.path, "/api/auth/me" | "/api/account/profile") {
                    assert_ne!(
                        status,
                        StatusCode::UNAUTHORIZED,
                        "allowed actor {actor:?} was unauthorized for {} {} ({:?})",
                        rule.method,
                        rule.path,
                        rule.policy
                    );
                }
                assert_ne!(
                    status,
                    StatusCode::FORBIDDEN,
                    "allowed actor {actor:?} was forbidden for {} {} ({:?})",
                    rule.method,
                    rule.path,
                    rule.policy
                );
                assert_ne!(
                    status,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "allowed route was missing policy for actor {actor:?} on {} {} ({:?})",
                    rule.method,
                    rule.path,
                    rule.policy
                );
            } else if matches!(actor, TestActor::Anonymous) {
                assert_eq!(
                    status,
                    StatusCode::UNAUTHORIZED,
                    "anonymous policy drift for {} {} ({:?})",
                    rule.method,
                    rule.path,
                    rule.policy
                );
            } else {
                assert_eq!(
                    status,
                    StatusCode::FORBIDDEN,
                    "role policy drift for actor {actor:?} on {} {} ({:?})",
                    rule.method,
                    rule.path,
                    rule.policy
                );
            }
            assertions += 1;
        }
    }

    assert_eq!(assertions, ROUTE_AUTHORIZATION.len() * TestActor::ALL.len());
}

async fn direct_request(method: &str, uri: &str, actor: TestActor, body: &str) -> StatusCode {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    let service_scope = if uri.contains("/api/achievements/user/") {
        Some("achievements.write")
    } else if uri == "/api/notifications" && method == "POST" {
        Some("notifications.write")
    } else if uri.contains("/api/shards/messages/") {
        Some("shards.messages.write")
    } else if uri == "/api/shards/servers/register" {
        Some("shards.servers.register")
    } else {
        None
    };
    if let Some(token) = token(actor, service_scope) {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    app()
        .oneshot(builder.body(Body::from(body.to_string())).expect("request"))
        .await
        .expect("gateway response")
        .status()
}

async fn request_with_token(method: &str, uri: &str, token: &str, body: &str) -> StatusCode {
    app()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(body.to_string()))
                .expect("request"),
        )
        .await
        .expect("gateway response")
        .status()
}

#[tokio::test]
async fn signed_subject_owns_achievement_paths_and_admins_may_inspect() {
    assert_eq!(
        direct_request(
            "GET",
            "/api/achievements/user/101/achievements",
            TestActor::Player,
            "",
        )
        .await,
        StatusCode::OK
    );
    assert_eq!(
        direct_request(
            "GET",
            "/api/achievements/user/101/achievements",
            TestActor::OtherPlayer,
            "",
        )
        .await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        direct_request(
            "GET",
            "/api/achievements/user/202/achievements",
            TestActor::OtherPlayer,
            "",
        )
        .await,
        StatusCode::OK
    );
    assert_eq!(
        direct_request(
            "GET",
            "/api/achievements/user/202/achievements",
            TestActor::Moderator,
            "",
        )
        .await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        direct_request(
            "GET",
            "/api/achievements/user/202/achievements",
            TestActor::Admin,
            "",
        )
        .await,
        StatusCode::OK
    );
}

#[tokio::test]
async fn query_and_body_user_selectors_cannot_escape_the_signed_subject() {
    for uri in [
        "/api/notifications?userId=202",
        "/api/notifications/unread-count?userId=202",
        "/api/notifications/preferences?userId=202",
        "/api/notifications/read-all?userId=202",
    ] {
        let method = if uri.contains("read-all") {
            "POST"
        } else {
            "GET"
        };
        assert_eq!(
            direct_request(method, uri, TestActor::Player, "{}").await,
            StatusCode::FORBIDDEN,
            "selector escaped on {uri}"
        );
        assert_ne!(
            direct_request(method, uri, TestActor::Admin, "{}").await,
            StatusCode::FORBIDDEN,
            "admin inspection unexpectedly denied on {uri}"
        );
    }

    assert_eq!(
        direct_request(
            "PUT",
            "/api/notifications/preferences/combat",
            TestActor::Player,
            r#"{"userId":202,"enabled":true}"#,
        )
        .await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        direct_request(
            "POST",
            "/api/debris/11/claim",
            TestActor::Player,
            r#"{"collectorId":202}"#,
        )
        .await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        direct_request(
            "POST",
            "/api/debris/11/claim",
            TestActor::Player,
            r#"{"collectorId":101}"#,
        )
        .await,
        StatusCode::OK
    );
}

#[tokio::test]
async fn grants_and_internal_writes_accept_only_admin_superadmin_or_service() {
    for actor in [
        TestActor::Anonymous,
        TestActor::Player,
        TestActor::OtherPlayer,
        TestActor::Moderator,
    ] {
        let expected = if matches!(actor, TestActor::Anonymous) {
            StatusCode::UNAUTHORIZED
        } else {
            StatusCode::FORBIDDEN
        };
        assert_eq!(
            direct_request(
                "POST",
                "/api/achievements/user/101/achievements/1",
                actor,
                "{}",
            )
            .await,
            expected
        );
    }

    for actor in [TestActor::Admin, TestActor::SuperAdmin, TestActor::Service] {
        let status = direct_request(
            "POST",
            "/api/achievements/user/101/achievements/1",
            actor,
            "{}",
        )
        .await;
        assert_ne!(status, StatusCode::UNAUTHORIZED);
        assert_ne!(status, StatusCode::FORBIDDEN);
    }
}

#[tokio::test]
async fn service_scope_does_not_cross_internal_writers_or_inherit_player_access() {
    let config = platform_auth::AuthConfig::from_env();
    let token = platform_auth::generate_service_token(
        &config,
        "notifications-worker",
        "universus",
        &["notifications.write"],
        3_600,
    )
    .expect("scoped service token");

    let own_writer = request_with_token(
        "POST",
        "/api/notifications",
        &token,
        r#"{"userId":101,"category":"system","title":"test","message":"test"}"#,
    )
    .await;
    assert_ne!(own_writer, StatusCode::UNAUTHORIZED);
    assert_ne!(own_writer, StatusCode::FORBIDDEN);

    assert_eq!(
        request_with_token(
            "POST",
            "/api/achievements/user/101/achievements/1",
            &token,
            "{}",
        )
        .await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request_with_token("GET", "/api/users/me", &token, "").await,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn refresh_token_is_never_accepted_by_gateway_api_authorization() {
    let refresh = platform_auth::generate_refresh_token(
        &platform_auth::AuthConfig::from_env(),
        "gateway-user",
    )
    .expect("refresh token");
    assert_eq!(
        request_with_token("GET", "/api/users/me", &refresh, "").await,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn users_me_is_derived_from_signed_claims() {
    let response = app()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/users/me")
                .header(
                    "authorization",
                    format!(
                        "Bearer {}",
                        token(TestActor::Admin, None).expect("admin token")
                    ),
                )
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = hyper::body::to_bytes(response.into_body())
        .await
        .expect("body");
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(body["role"], "admin");
    assert_eq!(body["isAdmin"], true);
    assert_eq!(body["username"], "admin-admin-1");
}
