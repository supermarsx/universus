use std::path::{Path, PathBuf};

fn repo_file(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(name)
}

fn service_block(compose: &str, service: &str) -> String {
    let marker = format!("  {service}:");
    let tail = compose
        .split_once(&marker)
        .unwrap_or_else(|| panic!("missing compose service {service}"))
        .1;
    let mut block = String::new();
    for line in tail.lines() {
        if line.trim().is_empty() || line.starts_with("    ") {
            block.push_str(line);
            block.push('\n');
        } else {
            break;
        }
    }
    block
}

#[test]
fn compose_separates_the_only_signer_from_audience_bound_verifiers() {
    let compose_path = repo_file("docker-compose.yml");
    let compose = std::fs::read_to_string(&compose_path)
        .unwrap_or_else(|error| panic!("read {}: {error}", compose_path.display()));

    assert!(
        compose.contains("x-runtime-environment: &runtime-environment")
            && compose.contains("UNIVERSUS_ENV: production"),
        "compose must activate production auth validation"
    );
    assert!(
        compose.contains("x-auth-verifier-environment: &auth-verifier-environment")
            && compose.contains("AUTH_ALLOW_LEGACY_HS256: \"false\"")
            && compose.contains("AUTH_JWT_VERIFICATION_KEYS: ${AUTH_JWT_VERIFICATION_KEYS:?"),
        "production verifiers must use the fail-closed public-key contract"
    );
    assert!(
        !compose
            .lines()
            .any(|line| line.trim_start().starts_with("JWT_SECRET:")),
        "production compose must not distribute a symmetric signing secret"
    );

    let verifier_services = [
        ("rust-api-gateway", "app-api-gateway"),
        ("rust-web-frontend", "app-web-frontend"),
        ("rust-admin-api", "app-admin-api"),
        ("rust-bot-api", "app-bot-api"),
        ("rust-realtime-gateway", "app-realtime-gateway"),
    ];
    for (service, audience) in verifier_services {
        let block = service_block(&compose, service);
        assert!(
            block.contains("<<: *auth-verifier-environment"),
            "{service} must inherit public verification keys"
        );
        assert!(
            block.contains(&format!("AUTH_EXPECTED_AUDIENCE: {audience}")),
            "{service} must reject tokens for other services"
        );
    }

    let gateway = service_block(&compose, "rust-api-gateway");
    for required in [
        "AUTH_TOKEN_ISSUER: \"true\"",
        "AUTH_TOKEN_AUDIENCES: app-api-gateway,app-web-frontend,app-admin-api,app-bot-api,app-realtime-gateway",
        "AUTH_JWT_SIGNING_KEY_ID: ${AUTH_JWT_SIGNING_KEY_ID:?",
        "AUTH_JWT_PRIVATE_KEY_BASE64: ${AUTH_JWT_PRIVATE_KEY_BASE64:?",
    ] {
        assert!(gateway.contains(required), "gateway issuer missing {required}");
    }

    for verifier in [
        "rust-web-frontend",
        "rust-admin-api",
        "rust-bot-api",
        "rust-realtime-gateway",
    ] {
        let block = service_block(&compose, verifier);
        assert!(
            !block.contains("AUTH_TOKEN_ISSUER")
                && !block.contains("AUTH_JWT_SIGNING_KEY_ID")
                && !block.contains("AUTH_JWT_PRIVATE_KEY_BASE64"),
            "{verifier} must never receive private signing capability"
        );
    }

    assert_eq!(
        compose
            .lines()
            .filter(|line| {
                line.trim_start()
                    .starts_with("AUTH_JWT_PRIVATE_KEY_BASE64:")
            })
            .count(),
        1,
        "exactly one service may receive private signing material"
    );
}

#[test]
fn compose_provisions_distinct_least_privilege_worker_credentials() {
    let compose_path = repo_file("docker-compose.yml");
    let compose = std::fs::read_to_string(&compose_path)
        .unwrap_or_else(|error| panic!("read {}: {error}", compose_path.display()));

    let bot = service_block(&compose, "rust-bot-worker");
    assert!(
        bot.contains("BOT_WORKER_SERVICE_TOKEN: ${BOT_WORKER_SERVICE_TOKEN:?")
            && bot.contains("PLATFORM_EVENTS_SERVICE_TOKEN: ${BOT_WORKER_REALTIME_PUBLISH_TOKEN:?"),
        "bot processing and realtime publishing need separate audience-bound credentials"
    );

    for (service, token_name) in [
        ("rust-api-gateway", "API_GATEWAY_REALTIME_PUBLISH_TOKEN"),
        ("rust-email-worker", "EMAIL_WORKER_REALTIME_PUBLISH_TOKEN"),
        (
            "rust-analytics-worker",
            "ANALYTICS_WORKER_REALTIME_PUBLISH_TOKEN",
        ),
        ("rust-app-core-engine", "CORE_ENGINE_REALTIME_PUBLISH_TOKEN"),
        (
            "rust-notifications-worker",
            "NOTIFICATIONS_WORKER_REALTIME_PUBLISH_TOKEN",
        ),
        ("rust-chat-worker", "CHAT_WORKER_REALTIME_PUBLISH_TOKEN"),
        (
            "rust-scheduler-worker",
            "SCHEDULER_WORKER_REALTIME_PUBLISH_TOKEN",
        ),
        (
            "rust-sharding-worker",
            "SHARDING_WORKER_REALTIME_PUBLISH_TOKEN",
        ),
    ] {
        let block = service_block(&compose, service);
        assert!(
            block.contains(&format!("PLATFORM_EVENTS_SERVICE_TOKEN: ${{{token_name}:?")),
            "{service} must use its own realtime.publish credential"
        );
    }

    let realtime = service_block(&compose, "rust-realtime-gateway");
    assert!(
        realtime.contains("REDIS_URL: redis://redis:6379")
            && realtime.contains("redis:\n        condition: service_healthy")
            && realtime.contains("REALTIME_ALLOWED_ORIGINS: ${REALTIME_ALLOWED_ORIGINS:?"),
        "realtime production fanout and browser origins must remain explicit"
    );

    let frontend = service_block(&compose, "rust-web-frontend");
    assert!(
        frontend.contains("COOKIE_SECURE: ${COOKIE_SECURE:-true}")
            && frontend.contains(
                "UNIVERSUS_ALLOW_INSECURE_LOCAL_HTTP_COOKIE: ${UNIVERSUS_ALLOW_INSECURE_LOCAL_HTTP_COOKIE:-false}"
            ),
        "production cookies must remain Secure by default"
    );
}

#[test]
fn example_environment_documents_asymmetric_keys_and_scoped_tokens() {
    let example_path = repo_file(".env.example");
    let example = std::fs::read_to_string(&example_path)
        .unwrap_or_else(|error| panic!("read {}: {error}", example_path.display()));

    assert!(
        !example
            .lines()
            .any(|line| line.trim_start().starts_with("JWT_SECRET=")),
        "the production example must not assign a shared JWT secret"
    );
    for required in [
        "AUTH_JWT_ISSUER=",
        "AUTH_JWT_SIGNING_KEY_ID=",
        "AUTH_JWT_PRIVATE_KEY_BASE64=",
        "AUTH_JWT_VERIFICATION_KEYS=",
        "API_GATEWAY_REALTIME_PUBLISH_TOKEN=",
        "BOT_WORKER_SERVICE_TOKEN=",
        "BOT_WORKER_REALTIME_PUBLISH_TOKEN=",
        "EMAIL_WORKER_REALTIME_PUBLISH_TOKEN=",
        "ANALYTICS_WORKER_REALTIME_PUBLISH_TOKEN=",
        "CORE_ENGINE_REALTIME_PUBLISH_TOKEN=",
        "NOTIFICATIONS_WORKER_REALTIME_PUBLISH_TOKEN=",
        "CHAT_WORKER_REALTIME_PUBLISH_TOKEN=",
        "SCHEDULER_WORKER_REALTIME_PUBLISH_TOKEN=",
        "SHARDING_WORKER_REALTIME_PUBLISH_TOKEN=",
    ] {
        assert!(
            example.lines().any(|line| line.starts_with(required)),
            "missing production example {required}"
        );
    }
    assert!(
        example.contains("expose it only to rust-api-gateway")
            && example.contains("never reuse one token"),
        "private-key separation and per-worker credentials must be prominent"
    );
    assert!(
        example.lines().any(|line| line == "COOKIE_SECURE=true")
            && example
                .lines()
                .any(|line| line == "# UNIVERSUS_ALLOW_INSECURE_LOCAL_HTTP_COOKIE=true"),
        "production cookie safety and its local-only override must remain documented"
    );
}
