use std::path::{Path, PathBuf};

fn repo_file(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(name)
}

fn service_block(compose: &str, service: &str) -> String {
    let marker = format!("  {service}:");
    let mut block = String::new();
    let mut in_service = false;
    for line in compose.lines() {
        if line == marker {
            in_service = true;
            continue;
        }
        if in_service && (line.trim().is_empty() || line.starts_with("    ")) {
            block.push_str(line);
            block.push('\n');
        } else if in_service {
            break;
        }
    }
    assert!(in_service, "missing compose service {service}");
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
        ("rust-email-worker", "app-email-worker"),
        ("rust-sms-api", "app-sms-api"),
        ("rust-privacy-worker", "app-privacy-worker"),
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
        "AUTH_TOKEN_AUDIENCES: app-api-gateway,app-web-frontend,app-admin-api,app-bot-api,app-realtime-gateway,app-privacy-worker",
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
        "rust-email-worker",
        "rust-sms-api",
        "rust-privacy-worker",
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
fn compose_runs_durable_communications_without_legacy_queue_dependencies() {
    let compose_path = repo_file("docker-compose.yml");
    let compose = std::fs::read_to_string(&compose_path)
        .unwrap_or_else(|error| panic!("read {}: {error}", compose_path.display()));
    let email = service_block(&compose, "rust-email-worker");
    let sms = service_block(&compose, "rust-sms-api");

    for (service, block, audience) in [
        ("rust-email-worker", &email, "app-email-worker"),
        ("rust-sms-api", &sms, "app-sms-api"),
    ] {
        assert!(
            block.contains("<<: *database-dependencies")
                && block.contains("<<: *auth-verifier-environment")
                && block.contains(&format!("AUTH_EXPECTED_AUDIENCE: {audience}"))
                && block.contains("DATABASE_URL: \"${DATABASE_URL_INTERNAL:?"),
            "{service} must be migration-gated, PostgreSQL-backed, and audience-bound"
        );
        for obsolete in [
            "REDIS_URL:",
            "RABBITMQ_URL:",
            "EMAIL_QUEUE_NAME:",
            "EMAIL_DLQ_NAME:",
            "REALTIME_GATEWAY_URL:",
            "PLATFORM_EVENTS_SERVICE_TOKEN:",
        ] {
            assert!(
                !block.contains(obsolete),
                "{service} still contains obsolete communication setting {obsolete}"
            );
        }
    }

    for required in [
        "COMMUNICATION_EVIDENCE_HMAC_KEY_BASE64: ${COMMUNICATION_EVIDENCE_HMAC_KEY_BASE64:?",
        "COMMUNICATION_SERVICE_TOKEN_FILE: /run/secrets/email_worker_communication_token",
        "EMAIL_PROVIDER_URL: ${EMAIL_PROVIDER_URL:?",
        "EMAIL_PROVIDER_BEARER_TOKEN: ${EMAIL_PROVIDER_BEARER_TOKEN:?",
        "EMAIL_PROVIDER_TIMEOUT_SECONDS: ${EMAIL_PROVIDER_TIMEOUT_SECONDS:-15}",
        "EMAIL_WORKER_UNIVERSE_ID: ${EMAIL_WORKER_UNIVERSE_ID:?",
        "EMAIL_WORKER_LEASE_SECONDS: ${EMAIL_WORKER_LEASE_SECONDS:-90}",
        "EMAIL_WORKER_HEALTH_PORT: ${EMAIL_WORKER_HEALTH_PORT:-3002}",
        "EMAIL_READINESS_MAX_STALENESS_SECONDS: ${EMAIL_READINESS_MAX_STALENESS_SECONDS:-30}",
        "source: email_worker_communication_token",
        "$${EMAIL_WORKER_HEALTH_PORT}",
        "GET /health HTTP/1.1",
        "stop_signal: SIGINT",
        "stop_grace_period: 130s",
    ] {
        assert!(
            email.contains(required),
            "email deployment missing {required}"
        );
    }

    for required in [
        "COMMUNICATION_EVIDENCE_HMAC_KEY_BASE64: ${COMMUNICATION_EVIDENCE_HMAC_KEY_BASE64:?",
        "COMMUNICATION_SERVICE_TOKEN_FILE: /run/secrets/sms_worker_communication_token",
        "SMS_PROVIDER_URL: ${SMS_PROVIDER_URL:?",
        "SMS_PROVIDER_BEARER_TOKEN: ${SMS_PROVIDER_BEARER_TOKEN:?",
        "SMS_PROVIDER_TIMEOUT_SECONDS: ${SMS_PROVIDER_TIMEOUT_SECONDS:-15}",
        "SMS_WORKER_UNIVERSE_IDS: ${SMS_WORKER_UNIVERSE_IDS:?",
        "SMS_DISPATCH_LEASE_SECONDS: ${SMS_DISPATCH_LEASE_SECONDS:-90}",
        "SMS_READINESS_MAX_STALENESS_SECONDS: ${SMS_READINESS_MAX_STALENESS_SECONDS:-30}",
        "source: sms_worker_communication_token",
        "${SMS_PORT:-4303}:3003",
        "GET /health HTTP/1.1",
        "stop_signal: SIGINT",
        "stop_grace_period: 130s",
    ] {
        assert!(sms.contains(required), "SMS deployment missing {required}");
    }

    assert!(
        compose.contains("email_worker_communication_token:\n    file: ${EMAIL_WORKER_COMMUNICATION_TOKEN_FILE:?")
            && compose.contains("sms_worker_communication_token:\n    file: ${SMS_WORKER_COMMUNICATION_TOKEN_FILE:?"),
        "communication JWTs must be mounted as distinct Compose secrets"
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

#[test]
fn example_environment_documents_fail_closed_communication_runtime() {
    let example_path = repo_file(".env.example");
    let example = std::fs::read_to_string(&example_path)
        .unwrap_or_else(|error| panic!("read {}: {error}", example_path.display()));

    for required in [
        "COMMUNICATION_EVIDENCE_HMAC_KEY_BASE64=",
        "EMAIL_WORKER_COMMUNICATION_TOKEN_FILE=",
        "SMS_WORKER_COMMUNICATION_TOKEN_FILE=",
        "EMAIL_PROVIDER_URL=https://",
        "EMAIL_PROVIDER_BEARER_TOKEN=",
        "EMAIL_PROVIDER_TIMEOUT_SECONDS=",
        "EMAIL_WORKER_UNIVERSE_ID=",
        "EMAIL_WORKER_LEASE_SECONDS=",
        "EMAIL_WORKER_HEALTH_PORT=",
        "EMAIL_READINESS_MAX_STALENESS_SECONDS=",
        "SMS_PROVIDER_URL=https://",
        "SMS_PROVIDER_BEARER_TOKEN=",
        "SMS_PROVIDER_TIMEOUT_SECONDS=",
        "SMS_WORKER_UNIVERSE_IDS=",
        "SMS_DISPATCH_LEASE_SECONDS=",
        "SMS_READINESS_MAX_STALENESS_SECONDS=",
    ] {
        assert!(
            example.lines().any(|line| line.starts_with(required)),
            "missing communication example setting {required}"
        );
    }
    assert!(
        example.contains("openssl rand -base64 32")
            && example.contains("Never put\n# the JWT itself in this environment file")
            && example
                .contains("Callers of the SMS HTTP API need their own audience-bound service JWTs")
            && example.contains("not reuse the background token")
            && !example.contains("EMAIL_WORKER_REALTIME_PUBLISH_TOKEN="),
        "communication examples must document generated secrets and distinct scoped credentials"
    );
}
