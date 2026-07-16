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
fn compose_requires_one_shared_production_jwt_secret_for_rust_services() {
    let compose_path = repo_file("docker-compose.yml");
    let compose = std::fs::read_to_string(&compose_path)
        .unwrap_or_else(|error| panic!("read {}: {error}", compose_path.display()));

    assert!(
        compose.contains("x-auth-environment: &auth-environment"),
        "compose must declare a reusable auth environment"
    );
    assert!(
        compose.contains("UNIVERSUS_ENV: production"),
        "compose auth environment must activate production secret validation"
    );
    assert!(
        compose.contains("JWT_SECRET: ${JWT_SECRET:?"),
        "compose must fail interpolation when JWT_SECRET is missing"
    );

    for service in [
        "rust-api-gateway",
        "rust-web-frontend",
        "rust-admin-api",
        "rust-bot-api",
        "rust-bot-worker",
        "rust-sms-api",
        "rust-email-worker",
        "rust-analytics-worker",
        "rust-realtime-gateway",
        "rust-core-engine",
        "rust-app-core-engine",
        "rust-notifications-worker",
        "rust-chat-worker",
        "rust-scheduler-worker",
        "rust-sharding-worker",
    ] {
        assert!(
            service_block(&compose, service).contains("<<: *auth-environment"),
            "{service} must inherit the shared auth environment"
        );
    }

    assert!(
        service_block(&compose, "rust-realtime-gateway").contains("REDIS_URL: redis://redis:6379"),
        "production realtime gateway must use Redis cross-replica fanout"
    );
    let realtime = service_block(&compose, "rust-realtime-gateway");
    assert!(
        realtime.contains("redis:\n        condition: service_healthy"),
        "realtime gateway must wait for healthy Redis"
    );
    assert!(
        realtime.contains("REALTIME_ALLOWED_ORIGINS: ${REALTIME_ALLOWED_ORIGINS:?"),
        "production WebSocket cookie authentication needs an explicit origin allowlist"
    );

    let frontend = service_block(&compose, "rust-web-frontend");
    assert!(
        frontend.contains("COOKIE_SECURE: ${COOKIE_SECURE:-true}"),
        "production frontend cookies must default to Secure"
    );
    assert!(
        frontend.contains(
            "UNIVERSUS_ALLOW_INSECURE_LOCAL_HTTP_COOKIE: ${UNIVERSUS_ALLOW_INSECURE_LOCAL_HTTP_COOKIE:-false}"
        ),
        "the dangerous local HTTP override must default to disabled"
    );
}

#[test]
fn example_secret_is_long_but_explicitly_non_production() {
    let example_path = repo_file(".env.example");
    let example = std::fs::read_to_string(&example_path)
        .unwrap_or_else(|error| panic!("read {}: {error}", example_path.display()));
    let secret = example
        .lines()
        .find_map(|line| line.strip_prefix("JWT_SECRET="))
        .expect("JWT_SECRET example");

    assert!(
        secret.len() >= 32,
        "JWT_SECRET example documents minimum length"
    );

    let config = platform_auth::AuthConfig {
        jwt_secret: secret.to_string(),
        ..platform_auth::AuthConfig::default()
    };
    assert_eq!(
        config.validate_for_environment("production"),
        Err(platform_auth::AuthConfigError::InsecureProductionSecret),
        "the documented placeholder must never be accepted as a production secret"
    );

    assert!(
        example.lines().any(|line| line == "COOKIE_SECURE=true"),
        "the production environment example must enable Secure cookies"
    );
    assert!(
        example
            .lines()
            .any(|line| line == "# UNIVERSUS_ALLOW_INSECURE_LOCAL_HTTP_COOKIE=true"),
        "the local HTTP escape hatch must remain visibly opt-in"
    );
}
