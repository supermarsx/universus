use platform_db::{Database, ShardServerUpsert};
use tokio::time::{sleep, Duration};

const SERVICE_NAME: &str = "app-sharding-worker";

#[tokio::main]
async fn main() {
    platform_observability::init(SERVICE_NAME);

    let heartbeat_secs = u64_env("SHARD_HEARTBEAT_INTERVAL_SECS", 30);
    let stale_check_secs = u64_env("SHARD_STALE_CHECK_INTERVAL_SECS", 60);
    let stale_after_secs = i64_env("SHARD_STALE_AFTER_SECS", 120);
    let run_once = bool_env("SHARD_WORKER_RUN_ONCE");

    tracing::info!(
        service = SERVICE_NAME,
        heartbeat_secs,
        stale_check_secs,
        stale_after_secs,
        run_once,
        "sharding worker started"
    );

    let mut heartbeat_tick = tokio::time::interval(Duration::from_secs(heartbeat_secs));
    let mut stale_tick = tokio::time::interval(Duration::from_secs(stale_check_secs));

    loop {
        tokio::select! {
            _ = heartbeat_tick.tick() => {
                heartbeat_cycle().await;
            }
            _ = stale_tick.tick() => {
                stale_check_cycle(stale_after_secs).await;
            }
        }

        if run_once {
            break;
        }
    }

    sleep(Duration::from_millis(25)).await;
}

async fn heartbeat_cycle() {
    let Some(database) = Database::from_env() else {
        tracing::warn!(
            service = SERVICE_NAME,
            "DATABASE_URL not configured; skipping shard heartbeat cycle"
        );
        return;
    };

    let input = ShardServerUpsert {
        server_id: std::env::var("SERVER_ID").unwrap_or_else(|_| "rust-shard-1".to_string()),
        server_type: std::env::var("SERVER_TYPE").unwrap_or_else(|_| "game".to_string()),
        region: std::env::var("SERVER_REGION").unwrap_or_else(|_| "global".to_string()),
        endpoint: std::env::var("SERVER_ENDPOINT")
            .unwrap_or_else(|_| "http://localhost:3000".to_string()),
        status: "online".to_string(),
        current_load: i64_env("SERVER_CURRENT_LOAD", 0),
        max_capacity: i64_env("SERVER_MAX_CAPACITY", 1000),
        health_score: f64_env("SERVER_HEALTH_SCORE", 1.0),
    };

    match database.upsert_shard_server(input).await {
        Ok(server) => tracing::info!(
            service = SERVICE_NAME,
            server_id = %server.server_id,
            current_load = server.current_load,
            max_capacity = server.max_capacity,
            status = %server.status,
            "shard heartbeat upserted"
        ),
        Err(error) => tracing::error!(service = SERVICE_NAME, %error, "shard heartbeat failed"),
    }
}

async fn stale_check_cycle(stale_after_secs: i64) {
    let Some(database) = Database::from_env() else {
        tracing::warn!(
            service = SERVICE_NAME,
            "DATABASE_URL not configured; skipping stale shard check"
        );
        return;
    };

    let expired = database
        .expire_stale_shard_servers(stale_after_secs)
        .await
        .unwrap_or_else(|error| {
            tracing::error!(service = SERVICE_NAME, %error, "failed stale shard expiration");
            0
        });

    let stats = database.shard_routing_stats().await;
    match stats {
        Ok(stats) => tracing::info!(
            service = SERVICE_NAME,
            expired,
            total_servers = stats.total_servers,
            healthy_servers = stats.healthy_servers,
            overloaded_servers = stats.overloaded_servers,
            migration_count = stats.migration_count,
            "stale shard check completed"
        ),
        Err(error) => tracing::warn!(
            service = SERVICE_NAME,
            expired,
            %error,
            "stale shard check completed without stats"
        ),
    }
}

fn bool_env(key: &str) -> bool {
    std::env::var(key)
        .map(|value| value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn u64_env(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

fn i64_env(key: &str, default: i64) -> i64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(default)
}

fn f64_env(key: &str, default: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(default)
}
