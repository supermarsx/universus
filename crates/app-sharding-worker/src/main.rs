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
    let server_id = std::env::var("SERVER_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "rust-shard-1".to_string());
    let worker_id = std::env::var("SHARD_WORKER_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("app-sharding-worker:{server_id}"));
    let message_lease_secs = i64_env("SHARD_MESSAGE_LEASE_SECS", 30);
    let message_retry_delay_secs = i64_env("SHARD_MESSAGE_RETRY_DELAY_SECS", 15);
    let message_max_attempts = i32_env("SHARD_MESSAGE_MAX_ATTEMPTS", 3);
    let realtime_url = std::env::var("REALTIME_GATEWAY_URL")
        .ok()
        .filter(|value| !value.trim().is_empty());

    tracing::info!(
        service = SERVICE_NAME,
        heartbeat_secs,
        stale_check_secs,
        stale_after_secs,
        run_once,
        server_id,
        worker_id,
        message_lease_secs,
        message_retry_delay_secs,
        message_max_attempts,
        has_realtime_url = realtime_url.is_some(),
        "sharding worker started"
    );

    let mut heartbeat_tick = tokio::time::interval(Duration::from_secs(heartbeat_secs));
    let mut stale_tick = tokio::time::interval(Duration::from_secs(stale_check_secs));

    loop {
        tokio::select! {
            _ = heartbeat_tick.tick() => {
                heartbeat_cycle(realtime_url.as_deref()).await;
                process_inbound_messages(
                    &server_id,
                    &worker_id,
                    message_lease_secs,
                    message_retry_delay_secs,
                    message_max_attempts,
                    realtime_url.as_deref(),
                )
                .await;
            }
            _ = stale_tick.tick() => {
                stale_check_cycle(stale_after_secs, realtime_url.as_deref()).await;
            }
        }

        if run_once {
            break;
        }
    }

    sleep(Duration::from_millis(25)).await;
}

async fn heartbeat_cycle(realtime_url: Option<&str>) {
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
        Ok(server) => {
            tracing::info!(
                service = SERVICE_NAME,
                server_id = %server.server_id,
                current_load = server.current_load,
                max_capacity = server.max_capacity,
                status = %server.status,
                "shard heartbeat upserted"
            );
            if let Some(url) = realtime_url {
                publish_ops_event(
                    url,
                    "shard.heartbeat",
                    &serde_json::json!({
                        "serverId": server.server_id,
                        "status": server.status,
                        "currentLoad": server.current_load,
                        "maxCapacity": server.max_capacity
                    }),
                )
                .await;
            }
        }
        Err(error) => tracing::error!(service = SERVICE_NAME, %error, "shard heartbeat failed"),
    }
}

async fn stale_check_cycle(stale_after_secs: i64, realtime_url: Option<&str>) {
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
        Ok(stats) => {
            tracing::info!(
                service = SERVICE_NAME,
                expired,
                total_servers = stats.total_servers,
                healthy_servers = stats.healthy_servers,
                overloaded_servers = stats.overloaded_servers,
                migration_count = stats.migration_count,
                "stale shard check completed"
            );
            if let Some(url) = realtime_url {
                publish_ops_event(
                    url,
                    "shard.stale_check",
                    &serde_json::json!({
                        "expired": expired,
                        "totalServers": stats.total_servers,
                        "healthyServers": stats.healthy_servers,
                        "overloadedServers": stats.overloaded_servers
                    }),
                )
                .await;
            }
        }
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

fn i32_env(key: &str, default: i32) -> i32 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(default)
}

async fn publish_ops_event(base_url: &str, event_type: &str, payload: &serde_json::Value) {
    let event = platform_events::build_event(event_type, payload);
    let _ = platform_events::publish_http(base_url, "ops.sharding", &event).await;
}

async fn process_inbound_messages(
    server_id: &str,
    worker_id: &str,
    lease_secs: i64,
    retry_delay_secs: i64,
    max_attempts: i32,
    realtime_url: Option<&str>,
) {
    let Some(database) = Database::from_env() else {
        tracing::warn!(
            service = SERVICE_NAME,
            server_id,
            "DATABASE_URL not configured; skipping inbound message processing"
        );
        return;
    };

    let claimed = match database
        .claim_cross_server_messages(server_id, worker_id, 32, lease_secs)
        .await
    {
        Ok(messages) => messages,
        Err(error) => {
            tracing::error!(service = SERVICE_NAME, server_id, %error, "failed claim cross-server messages");
            return;
        }
    };

    if claimed.is_empty() {
        return;
    }

    for message in claimed {
        let process_result = process_cross_server_message(&message.message_type, &message.payload);
        match process_result {
            Ok(result) => {
                let _ = database.ack_cross_server_message(message.id).await;
                tracing::info!(
                    service = SERVICE_NAME,
                    message_id = message.id,
                    message_type = %message.message_type,
                    source_server_id = %message.source_server_id,
                    target_server_id = %message.target_server_id,
                    "cross-server message processed"
                );
                if let Some(url) = realtime_url {
                    publish_ops_event(
                        url,
                        "shard.message_processed",
                        &serde_json::json!({
                            "messageId": message.id,
                            "messageType": message.message_type,
                            "sourceServerId": message.source_server_id,
                            "targetServerId": message.target_server_id,
                            "result": result
                        }),
                    )
                    .await;
                }
            }
            Err(error_message) => {
                let _ = database
                    .fail_cross_server_message(
                        message.id,
                        &error_message,
                        retry_delay_secs,
                        max_attempts,
                    )
                    .await;
                tracing::warn!(
                    service = SERVICE_NAME,
                    message_id = message.id,
                    message_type = %message.message_type,
                    error = %error_message,
                    "cross-server message processing failed"
                );
                if let Some(url) = realtime_url {
                    publish_ops_event(
                        url,
                        "shard.message_failed",
                        &serde_json::json!({
                            "messageId": message.id,
                            "messageType": message.message_type,
                            "error": error_message
                        }),
                    )
                    .await;
                }
            }
        }
    }
}

fn process_cross_server_message(
    message_type: &str,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    match message_type {
        "route_update" => Ok(serde_json::json!({
            "applied": true,
            "kind": "route_update",
            "payload": payload
        })),
        "player_migration" => Ok(serde_json::json!({
            "applied": true,
            "kind": "player_migration",
            "payload": payload
        })),
        "broadcast" => Ok(serde_json::json!({
            "applied": true,
            "kind": "broadcast",
            "payload": payload
        })),
        other => Err(format!("unsupported message type: {other}")),
    }
}
