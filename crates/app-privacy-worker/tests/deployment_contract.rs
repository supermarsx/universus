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
fn compose_keeps_privacy_processing_fail_closed_and_migration_gated() {
    let compose_path = repo_file("docker-compose.yml");
    let compose = std::fs::read_to_string(&compose_path)
        .unwrap_or_else(|error| panic!("read {}: {error}", compose_path.display()));
    let worker = service_block(&compose, "rust-privacy-worker");
    let gateway = service_block(&compose, "rust-api-gateway");
    let admin = service_block(&compose, "rust-admin-api");

    for required in [
        "BIN_NAME: app-privacy-worker",
        "<<: *database-dependencies",
        "rust-realtime-gateway:\n        condition: service_started",
        "<<: *auth-verifier-environment",
        "AUTH_EXPECTED_AUDIENCE: app-privacy-worker",
        "DATABASE_URL: \"${DATABASE_URL_INTERNAL:?",
        "PLATFORM_EVENTS_SERVICE_TOKEN: ${PRIVACY_WORKER_REALTIME_PUBLISH_TOKEN:?",
        "PRIVACY_WORKER_ID: ${PRIVACY_WORKER_ID:?",
        "PRIVACY_EXPORT_ACTIVE_KEY_ID: ${PRIVACY_EXPORT_ACTIVE_KEY_ID:?",
        "PRIVACY_EXPORT_KEYRING_JSON: ${PRIVACY_EXPORT_KEYRING_JSON:?",
        "COMMUNICATION_EVIDENCE_HMAC_KEY_BASE64: ${COMMUNICATION_EVIDENCE_HMAC_KEY_BASE64:?",
        "PRIVACY_WORKER_RUN_ONCE: \"false\"",
        "PRIVACY_EXPORT_DELIVERY_TOKEN_TTL_SECS: ${PRIVACY_EXPORT_DELIVERY_TOKEN_TTL_SECS:-900}",
        "PRIVACY_RETENTION_INTERVAL_SECS: ${PRIVACY_RETENTION_INTERVAL_SECS:-3600}",
        "PRIVACY_OUTBOX_RETENTION_DAYS: ${PRIVACY_OUTBOX_RETENTION_DAYS:-30}",
        "PRIVACY_WORKER_READINESS_STALE_SECS: ${PRIVACY_WORKER_READINESS_STALE_SECS:-30}",
        "test: [\"CMD\", \"/usr/local/bin/app-privacy-worker\", \"healthcheck\"]",
        "stop_grace_period: 80s",
    ] {
        assert!(
            worker.contains(required),
            "privacy worker deployment contract is missing {required}"
        );
    }
    assert!(
        !worker.contains("PRIVACY_EXPORT_KEY_ID:")
            && !worker.contains("PRIVACY_EXPORT_KEY_BASE64:")
            && !worker.contains("AUTH_JWT_PRIVATE_KEY_BASE64:"),
        "privacy worker must use the rotation keyring and remain verifier-only"
    );

    for required in [
        "rust-privacy-worker:\n        condition: service_healthy",
        "<<: *auth-verifier-environment",
        "AUTH_EXPECTED_AUDIENCE: app-api-gateway",
        "AUTH_TOKEN_AUDIENCES: app-api-gateway,app-web-frontend,app-admin-api,app-bot-api,app-realtime-gateway,app-privacy-worker",
        "AUTH_SESSION_DIGEST_KEY: ${AUTH_SESSION_DIGEST_KEY:?",
        "DATABASE_URL: \"${DATABASE_URL_INTERNAL:?",
        "PRIVACY_POLICY_VERSION: ${PRIVACY_POLICY_VERSION:-privacy-v1}",
        "PRIVACY_REQUEST_IP_PEPPER: ${PRIVACY_REQUEST_IP_PEPPER:?",
        "PRIVACY_EXPORT_ACTIVE_KEY_ID: ${PRIVACY_EXPORT_ACTIVE_KEY_ID:?",
        "PRIVACY_EXPORT_KEYRING_JSON: ${PRIVACY_EXPORT_KEYRING_JSON:?",
        "PRIVACY_WORKER_INTERNAL_URL: http://rust-privacy-worker:3010",
    ] {
        assert!(
            gateway.contains(required),
            "privacy gateway deployment contract is missing {required}"
        );
    }

    for required in [
        "<<: *database-dependencies",
        "<<: *auth-verifier-environment",
        "AUTH_EXPECTED_AUDIENCE: app-admin-api",
        "PORT: 3001",
        "DATABASE_URL: \"${DATABASE_URL_INTERNAL:?",
        "${ADMIN_PORT:-4302}:3001",
    ] {
        assert!(
            admin.contains(required),
            "privacy admin deployment contract is missing {required}"
        );
    }

    assert!(
        compose.contains("database:\n    condition: service_healthy")
            && compose.contains("database-migrate:\n    condition: service_completed_successfully"),
        "the shared database dependency must require health and successful migrations"
    );
}

#[test]
fn example_environment_requires_explicit_non_fixture_secrets() {
    let example_path = repo_file(".env.example");
    let example = std::fs::read_to_string(&example_path)
        .unwrap_or_else(|error| panic!("read {}: {error}", example_path.display()));

    for required in [
        "AUTH_SESSION_DIGEST_KEY=",
        "PRIVACY_WORKER_REALTIME_PUBLISH_TOKEN=",
        "PRIVACY_WORKER_ID=",
        "PRIVACY_EXPORT_ACTIVE_KEY_ID=v1:",
        "PRIVACY_EXPORT_KEYRING_JSON={",
        "PRIVACY_REQUEST_IP_PEPPER=",
        "PRIVACY_POLICY_VERSION=",
        "PRIVACY_WORKER_CLAIM_TIMEOUT_SECS=",
        "PRIVACY_WORKER_LEASE_SECS=",
        "PRIVACY_WORKER_JOB_TIMEOUT_SECS=",
        "PRIVACY_EXPORT_DELIVERY_TOKEN_TTL_SECS=",
        "PRIVACY_RETENTION_INTERVAL_SECS=",
        "PRIVACY_OUTBOX_RETENTION_DAYS=",
        "PRIVACY_WORKER_READINESS_STALE_SECS=",
    ] {
        assert!(
            example.lines().any(|line| line.starts_with(required)),
            "missing privacy worker example setting {required}"
        );
    }
    assert!(
        example.contains("openssl rand -base64 32")
            && example.contains("keep it in a secret manager")
            && example.contains("openssl rand -base64 48")
            && example.contains("placeholder is intentionally too short")
            && example.contains("retain old keys until every artifact using them has")
            && example.contains("AUTH_SESSION_DIGEST_KEY=replace-me")
            && example.contains("PRIVACY_REQUEST_IP_PEPPER=replace-me")
            && example.contains("\"v1:replace-with-active-rotation-id\":\"replace-me\"")
            && !example.contains("PRIVACY_EXPORT_KEY_ID=")
            && !example.contains("PRIVACY_EXPORT_KEY_BASE64="),
        "privacy examples must be rotation-safe and deliberately fail closed until provisioned"
    );
}
