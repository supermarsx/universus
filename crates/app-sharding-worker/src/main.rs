use platform_consensus::LeaseCoordinator;
use platform_db::{Database, ShardServerUpsert};
use platform_sharding::{ShardConfig, ShardingCatalog};
use platform_tenancy::{TenantAccessLevel, TenantContext};
use platform_worker_runtime::WorkerRuntime;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::oneshot;
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
    let cycle_lease_secs = i64_env("SHARD_CYCLE_LEASE_SECS", 30);
    let runtime_max_inflight = std::env::var("SHARD_RUNTIME_MAX_INFLIGHT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(32);
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
        cycle_lease_secs,
        runtime_max_inflight,
        message_retry_delay_secs,
        message_max_attempts,
        has_realtime_url = realtime_url.is_some(),
        "sharding worker started"
    );
    let lease_coordinator = Arc::new(LeaseCoordinator::new());
    let shard_tenant = std::env::var("SHARD_TENANT_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "tenant-default".to_string());
    let shard_region = std::env::var("SERVER_REGION")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "global".to_string());
    let sharding_catalog = Arc::new(ShardingCatalog::with_consensus(Arc::clone(&lease_coordinator)));
    let runtime = Arc::new(WorkerRuntime::current(runtime_max_inflight));
    let tenant_context = TenantContext {
        tenant_id: shard_tenant.clone(),
        tenant_name: Some("Sharding Worker".to_string()),
        access_level: TenantAccessLevel::Worker,
    };
    sharding_catalog
        .register_shard(ShardConfig {
            shard_id: server_id.clone(),
            region: shard_region.clone(),
            allowed_tenants: HashSet::from_iter(vec![shard_tenant.clone()]),
            consensus_resource: Some(format!("shard:{server_id}:leader")),
        })
        .await;

    let mut heartbeat_tick = tokio::time::interval(Duration::from_secs(heartbeat_secs));
    let mut stale_tick = tokio::time::interval(Duration::from_secs(stale_check_secs));

    loop {
        tokio::select! {
            _ = heartbeat_tick.tick() => {
                if let Some(heartbeat_lease) = acquire_cycle_lease(
                    lease_coordinator.as_ref(),
                    &format!("sharding:{server_id}:heartbeat"),
                    &worker_id,
                    cycle_lease_secs,
                )
                .await
                {
                    let sync_catalog = Arc::clone(&sharding_catalog);
                    let server_id_for_task = server_id.clone();
                    let worker_id_for_task = worker_id.clone();
                    let realtime_for_task = realtime_url.clone();
                    run_cycle_in_runtime(
                        Arc::clone(&runtime),
                        tenant_context.clone(),
                        "sharding.heartbeat",
                        async move {
                            sync_platform_shard_catalog(
                                sync_catalog.as_ref(),
                                &server_id_for_task,
                                &worker_id_for_task,
                                cycle_lease_secs,
                                realtime_for_task.as_deref(),
                            )
                            .await;
                            heartbeat_cycle(realtime_for_task.as_deref()).await;
                        },
                    )
                    .await;
                    lease_coordinator
                        .release(&heartbeat_lease.resource, &heartbeat_lease.owner)
                        .await;
                    platform_observability::emit_consensus_snapshot(
                        SERVICE_NAME,
                        lease_coordinator.as_ref(),
                        4,
                    )
                    .await;
                }

                if let Some(message_lease) = acquire_cycle_lease(
                    lease_coordinator.as_ref(),
                    &format!("sharding:{server_id}:messages"),
                    &worker_id,
                    cycle_lease_secs,
                )
                .await
                {
                    let server_id_for_task = server_id.clone();
                    let worker_id_for_task = worker_id.clone();
                    let realtime_for_task = realtime_url.clone();
                    run_cycle_in_runtime(
                        Arc::clone(&runtime),
                        tenant_context.clone(),
                        "sharding.messages",
                        async move {
                            process_inbound_messages(
                                &server_id_for_task,
                                &worker_id_for_task,
                                message_lease_secs,
                                message_retry_delay_secs,
                                message_max_attempts,
                                realtime_for_task.as_deref(),
                            )
                            .await;
                        },
                    )
                    .await;
                    lease_coordinator
                        .release(&message_lease.resource, &message_lease.owner)
                        .await;
                    platform_observability::emit_consensus_snapshot(
                        SERVICE_NAME,
                        lease_coordinator.as_ref(),
                        4,
                    )
                    .await;
                }
            }
            _ = stale_tick.tick() => {
                if let Some(stale_lease) = acquire_cycle_lease(
                    lease_coordinator.as_ref(),
                    &format!("sharding:{server_id}:stale-check"),
                    &worker_id,
                    cycle_lease_secs,
                )
                .await
                {
                    let realtime_for_task = realtime_url.clone();
                    run_cycle_in_runtime(
                        Arc::clone(&runtime),
                        tenant_context.clone(),
                        "sharding.stale_check",
                        async move {
                            stale_check_cycle(stale_after_secs, realtime_for_task.as_deref()).await;
                        },
                    )
                    .await;
                    lease_coordinator
                        .release(&stale_lease.resource, &stale_lease.owner)
                        .await;
                    platform_observability::emit_consensus_snapshot(
                        SERVICE_NAME,
                        lease_coordinator.as_ref(),
                        4,
                    )
                    .await;
                }
            }
        }

        if run_once {
            break;
        }
    }

    runtime.shutdown(Duration::from_secs(5)).await;
    sleep(Duration::from_millis(25)).await;
}

async fn run_cycle_in_runtime<F>(
    runtime: Arc<WorkerRuntime>,
    context: TenantContext,
    cycle_name: &str,
    cycle: F,
) where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    let (done_tx, done_rx) = oneshot::channel::<()>();
    let schedule = runtime.spawn_tenant_task(context, async move {
        cycle.await;
        let _ = done_tx.send(());
        Ok(())
    });
    match schedule {
        Ok(()) => {
            if done_rx.await.is_err() {
                tracing::warn!(
                    service = SERVICE_NAME,
                    cycle = cycle_name,
                    "runtime cycle completed without completion signal"
                );
            }
            let stats = runtime.stats().await;
            tracing::debug!(
                service = SERVICE_NAME,
                cycle = cycle_name,
                total_inflight = stats.total_inflight,
                tenant_count = stats.per_tenant.len(),
                "sharding worker runtime stats"
            );
        }
        Err(error) => {
            tracing::warn!(
                service = SERVICE_NAME,
                cycle = cycle_name,
                %error,
                "failed to schedule sharding runtime cycle"
            );
        }
    }
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

async fn acquire_cycle_lease(
    lease_coordinator: &LeaseCoordinator,
    resource: &str,
    owner: &str,
    lease_secs: i64,
) -> Option<platform_consensus::LeaseToken> {
    let ttl = Duration::from_secs(lease_secs.max(1) as u64);
    match lease_coordinator.acquire(resource, owner, ttl).await {
        Ok(lease) => Some(lease),
        Err(error) => {
            tracing::warn!(
                service = SERVICE_NAME,
                %resource,
                %owner,
                %error,
                "skipping sharding cycle; lease already held"
            );
            None
        }
    }
}

async fn sync_platform_shard_catalog(
    catalog: &ShardingCatalog,
    shard_id: &str,
    worker_id: &str,
    lease_secs: i64,
    realtime_url: Option<&str>,
) {
    let ttl = Duration::from_secs(lease_secs.max(1) as u64);
    match catalog.assign_leader(shard_id, worker_id, ttl).await {
        Ok(leader) => {
            let summaries = catalog.summarize_shards().await;
            if let Some(summary) = summaries.iter().find(|summary| summary.shard_id == shard_id) {
                tracing::info!(
                    service = SERVICE_NAME,
                    shard_id = %summary.shard_id,
                    region = %summary.region,
                    assigned_node = ?summary.assigned_node,
                    tenant_count = summary.tenant_count,
                    leader_owner = %leader.lease.owner,
                    "platform sharding catalog synchronized"
                );
                if let Some(url) = realtime_url {
                    publish_ops_event(
                        url,
                        "shard.catalog_sync",
                        &serde_json::json!({
                            "shardId": summary.shard_id,
                            "region": summary.region,
                            "assignedNode": summary.assigned_node,
                            "tenantCount": summary.tenant_count,
                            "leaderOwner": leader.lease.owner
                        }),
                    )
                    .await;
                }
            }
        }
        Err(error) => {
            tracing::warn!(
                service = SERVICE_NAME,
                shard_id,
                worker_id,
                %error,
                "failed to synchronize platform sharding catalog"
            );
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
