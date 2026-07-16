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
fn compose_keeps_privacy_processing_fail_closed_and_migration_gated() {
    let compose_path = repo_file("docker-compose.yml");
    let compose = std::fs::read_to_string(&compose_path)
        .unwrap_or_else(|error| panic!("read {}: {error}", compose_path.display()));
    let worker = service_block(&compose, "rust-privacy-worker");

    for required in [
        "BIN_NAME: app-privacy-worker",
        "<<: *database-dependencies",
        "rust-realtime-gateway:\n        condition: service_started",
        "DATABASE_URL: \"${DATABASE_URL_INTERNAL:?",
        "PLATFORM_EVENTS_SERVICE_TOKEN: ${PRIVACY_WORKER_REALTIME_PUBLISH_TOKEN:?",
        "PRIVACY_WORKER_ID: ${PRIVACY_WORKER_ID:?",
        "PRIVACY_EXPORT_KEY_ID: ${PRIVACY_EXPORT_KEY_ID:?",
        "PRIVACY_EXPORT_KEY_BASE64: ${PRIVACY_EXPORT_KEY_BASE64:?",
        "PRIVACY_WORKER_RUN_ONCE: \"false\"",
        "test: [\"CMD\", \"/usr/local/bin/app-privacy-worker\", \"healthcheck\"]",
        "stop_grace_period: 80s",
    ] {
        assert!(
            worker.contains(required),
            "privacy worker deployment contract is missing {required}"
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
        "PRIVACY_WORKER_REALTIME_PUBLISH_TOKEN=",
        "PRIVACY_WORKER_ID=",
        "PRIVACY_EXPORT_KEY_ID=v1:",
        "PRIVACY_EXPORT_KEY_BASE64=",
        "PRIVACY_WORKER_CLAIM_TIMEOUT_SECS=",
        "PRIVACY_WORKER_LEASE_SECS=",
        "PRIVACY_WORKER_JOB_TIMEOUT_SECS=",
    ] {
        assert!(
            example.lines().any(|line| line.starts_with(required)),
            "missing privacy worker example setting {required}"
        );
    }
    assert!(
        example.contains("openssl rand -base64 32")
            && example.contains("keep it in a secret manager")
            && example.contains("replace-with-base64-encoded-32-byte-key"),
        "the example must require an explicitly provisioned key, not ship a reusable fixture"
    );
}
