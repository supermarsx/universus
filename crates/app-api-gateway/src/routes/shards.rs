use axum::extract::{Path, Query};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use platform_db::{Database, ShardServerUpsert};
use serde::{Deserialize, Serialize};

use crate::response::{bad_request, not_found, success};
use crate::state::{
    AppState, RegisterShardServerInput, RoutingStatsSnapshot, ShardHealthSnapshot,
    ShardServerSnapshot,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegisterServerRequest {
    server_id: Option<String>,
    server_type: Option<String>,
    region: Option<String>,
    endpoint: Option<String>,
    status: Option<String>,
    current_load: Option<i64>,
    max_capacity: Option<i64>,
    health_score: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct ServerListQuery {
    region: Option<String>,
    status: Option<String>,
    server_type: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServerPayload {
    server_id: String,
    server_type: String,
    region: String,
    endpoint: String,
    status: String,
    current_load: i64,
    max_capacity: i64,
    health_score: f64,
    last_heartbeat_unix: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthPayload {
    server_id: String,
    status: String,
    health_score: f64,
    current_load: i64,
    max_capacity: i64,
    load_percent: f64,
    last_heartbeat_unix: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RoutingStatsPayload {
    total_servers: usize,
    healthy_servers: usize,
    overloaded_servers: usize,
    total_capacity: i64,
    total_load: i64,
    average_load_percent: f64,
    migration_count: i64,
}

pub fn router() -> Router {
    Router::new()
        .route("/api/shards/servers", get(list_servers_handler))
        .route(
            "/api/shards/servers/register",
            post(register_server_handler),
        )
        .route("/api/shards/servers/:id/health", get(server_health_handler))
        .route(
            "/api/shards/routing/player/:id",
            get(routing_player_handler),
        )
        .route(
            "/api/shards/routing/servers/available",
            get(routing_available_servers_handler),
        )
        .route("/api/shards/routing/stats", get(routing_stats_handler))
        .route("/api/shards/health/overview", get(health_overview_handler))
        .route("/api/shards/messages/status", get(messages_status_handler))
}

async fn list_servers_handler(
    Extension(db): Extension<Option<Database>>,
    Extension(app_state): Extension<AppState>,
    Query(query): Query<ServerListQuery>,
) -> Response {
    let mut servers = if let Some(database) = db {
        database
            .list_shard_servers()
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|row| ShardServerSnapshot {
                        server_id: row.server_id,
                        server_type: row.server_type,
                        region: row.region,
                        endpoint: row.endpoint,
                        status: row.status,
                        current_load: row.current_load,
                        max_capacity: row.max_capacity,
                        health_score: row.health_score,
                        last_heartbeat_unix: row.last_heartbeat_unix,
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|_| app_state.list_shard_servers())
    } else {
        app_state.list_shard_servers()
    };
    if let Some(region) = query.region {
        servers.retain(|entry| entry.region.eq_ignore_ascii_case(&region));
    }
    if let Some(status) = query.status {
        servers.retain(|entry| entry.status.eq_ignore_ascii_case(&status));
    }
    if let Some(server_type) = query.server_type {
        servers.retain(|entry| entry.server_type.eq_ignore_ascii_case(&server_type));
    }

    success(servers.into_iter().map(server_payload).collect::<Vec<_>>())
}

async fn register_server_handler(
    Extension(db): Extension<Option<Database>>,
    Extension(app_state): Extension<AppState>,
    Json(payload): Json<RegisterServerRequest>,
) -> Response {
    let server_id = payload
        .server_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let Some(server_id) = server_id else {
        return bad_request("serverId is required");
    };

    let input = RegisterShardServerInput {
        server_id,
        server_type: payload
            .server_type
            .unwrap_or_else(|| "game".to_string())
            .trim()
            .to_lowercase(),
        region: payload.region.unwrap_or_else(|| "global".to_string()),
        endpoint: payload
            .endpoint
            .unwrap_or_else(|| "http://localhost:3000".to_string()),
        status: payload.status.unwrap_or_else(|| "online".to_string()),
        current_load: payload.current_load.unwrap_or(0),
        max_capacity: payload.max_capacity.unwrap_or(1_000),
        health_score: payload.health_score.unwrap_or(1.0),
    };

    if let Some(database) = db {
        let db_input = ShardServerUpsert {
            server_id: input.server_id.clone(),
            server_type: input.server_type.clone(),
            region: input.region.clone(),
            endpoint: input.endpoint.clone(),
            status: input.status.clone(),
            current_load: input.current_load,
            max_capacity: input.max_capacity,
            health_score: input.health_score,
        };
        if let Ok(row) = database.upsert_shard_server(db_input).await {
            return success(ServerPayload {
                server_id: row.server_id,
                server_type: row.server_type,
                region: row.region,
                endpoint: row.endpoint,
                status: row.status,
                current_load: row.current_load,
                max_capacity: row.max_capacity,
                health_score: row.health_score,
                last_heartbeat_unix: row.last_heartbeat_unix,
            });
        }
    }

    match app_state.register_shard_server(input) {
        Ok(server) => success(server_payload(server)),
        Err(message) => bad_request(message),
    }
}

async fn server_health_handler(
    Extension(db): Extension<Option<Database>>,
    Extension(app_state): Extension<AppState>,
    Path(id): Path<String>,
) -> Response {
    if let Some(database) = db {
        if let Ok(Some(health)) = database.shard_health(&id).await {
            return success(HealthPayload {
                server_id: health.server_id,
                status: health.status,
                health_score: health.health_score,
                current_load: health.current_load,
                max_capacity: health.max_capacity,
                load_percent: health.load_percent,
                last_heartbeat_unix: health.last_heartbeat_unix,
            });
        }
    }

    match app_state.shard_server_health(&id) {
        Some(health) => success(health_payload(health)),
        None => not_found("Server not found"),
    }
}

async fn routing_stats_handler(
    Extension(db): Extension<Option<Database>>,
    Extension(app_state): Extension<AppState>,
) -> Response {
    if let Some(database) = db {
        if let Ok(stats) = database.shard_routing_stats().await {
            return success(RoutingStatsPayload {
                total_servers: stats.total_servers,
                healthy_servers: stats.healthy_servers,
                overloaded_servers: stats.overloaded_servers,
                total_capacity: stats.total_capacity,
                total_load: stats.total_load,
                average_load_percent: stats.average_load_percent,
                migration_count: stats.migration_count,
            });
        }
    }
    success(routing_stats_payload(app_state.shard_routing_stats()))
}

async fn routing_player_handler(
    Extension(db): Extension<Option<Database>>,
    Extension(app_state): Extension<AppState>,
    Path(player_id): Path<String>,
) -> Response {
    let servers = if let Some(database) = db {
        database
            .list_shard_servers()
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|row| ShardServerSnapshot {
                        server_id: row.server_id,
                        server_type: row.server_type,
                        region: row.region,
                        endpoint: row.endpoint,
                        status: row.status,
                        current_load: row.current_load,
                        max_capacity: row.max_capacity,
                        health_score: row.health_score,
                        last_heartbeat_unix: row.last_heartbeat_unix,
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|_| app_state.list_shard_servers())
    } else {
        app_state.list_shard_servers()
    };
    if servers.is_empty() {
        return not_found("No shard servers available");
    }

    let selected_index = stable_bucket(&player_id, servers.len());
    let selected = &servers[selected_index];
    success(serde_json::json!({
        "playerId": player_id,
        "serverId": selected.server_id,
        "region": selected.region,
        "endpoint": selected.endpoint
    }))
}

async fn routing_available_servers_handler(
    Extension(db): Extension<Option<Database>>,
    Extension(app_state): Extension<AppState>,
) -> Response {
    let servers = if let Some(database) = db {
        database
            .list_shard_servers()
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|row| ShardServerSnapshot {
                        server_id: row.server_id,
                        server_type: row.server_type,
                        region: row.region,
                        endpoint: row.endpoint,
                        status: row.status,
                        current_load: row.current_load,
                        max_capacity: row.max_capacity,
                        health_score: row.health_score,
                        last_heartbeat_unix: row.last_heartbeat_unix,
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|_| app_state.list_shard_servers())
    } else {
        app_state.list_shard_servers()
    };
    let available = servers
        .into_iter()
        .filter(|server| server.status.eq_ignore_ascii_case("online"))
        .filter(|server| server.current_load < server.max_capacity)
        .map(server_payload)
        .collect::<Vec<_>>();
    success(available)
}

async fn health_overview_handler(
    Extension(db): Extension<Option<Database>>,
    Extension(app_state): Extension<AppState>,
) -> Response {
    let stats = if let Some(database) = db {
        database
            .shard_routing_stats()
            .await
            .map(|row| RoutingStatsSnapshot {
                total_servers: row.total_servers,
                healthy_servers: row.healthy_servers,
                overloaded_servers: row.overloaded_servers,
                total_capacity: row.total_capacity,
                total_load: row.total_load,
                average_load_percent: row.average_load_percent,
                migration_count: row.migration_count,
            })
            .unwrap_or_else(|_| app_state.shard_routing_stats())
    } else {
        app_state.shard_routing_stats()
    };
    success(serde_json::json!({
        "status": if stats.healthy_servers > 0 { "healthy" } else { "degraded" },
        "totalServers": stats.total_servers,
        "healthyServers": stats.healthy_servers,
        "averageLoadPercent": stats.average_load_percent
    }))
}

async fn messages_status_handler(
    Extension(db): Extension<Option<Database>>,
    Extension(app_state): Extension<AppState>,
) -> Response {
    let stats = if let Some(database) = db {
        database
            .shard_routing_stats()
            .await
            .map(|row| RoutingStatsSnapshot {
                total_servers: row.total_servers,
                healthy_servers: row.healthy_servers,
                overloaded_servers: row.overloaded_servers,
                total_capacity: row.total_capacity,
                total_load: row.total_load,
                average_load_percent: row.average_load_percent,
                migration_count: row.migration_count,
            })
            .unwrap_or_else(|_| app_state.shard_routing_stats())
    } else {
        app_state.shard_routing_stats()
    };
    success(serde_json::json!({
        "connectedServers": stats.total_servers,
        "deliveryMode": "at-least-once",
        "queueLag": 0,
        "status": "ok"
    }))
}

fn server_payload(server: ShardServerSnapshot) -> ServerPayload {
    ServerPayload {
        server_id: server.server_id,
        server_type: server.server_type,
        region: server.region,
        endpoint: server.endpoint,
        status: server.status,
        current_load: server.current_load,
        max_capacity: server.max_capacity,
        health_score: server.health_score,
        last_heartbeat_unix: server.last_heartbeat_unix,
    }
}

fn health_payload(health: ShardHealthSnapshot) -> HealthPayload {
    HealthPayload {
        server_id: health.server_id,
        status: health.status,
        health_score: health.health_score,
        current_load: health.current_load,
        max_capacity: health.max_capacity,
        load_percent: health.load_percent,
        last_heartbeat_unix: health.last_heartbeat_unix,
    }
}

fn routing_stats_payload(stats: RoutingStatsSnapshot) -> RoutingStatsPayload {
    RoutingStatsPayload {
        total_servers: stats.total_servers,
        healthy_servers: stats.healthy_servers,
        overloaded_servers: stats.overloaded_servers,
        total_capacity: stats.total_capacity,
        total_load: stats.total_load,
        average_load_percent: stats.average_load_percent,
        migration_count: stats.migration_count,
    }
}

fn stable_bucket(value: &str, buckets: usize) -> usize {
    value
        .bytes()
        .fold(0usize, |acc, byte| acc.wrapping_add(byte as usize))
        % buckets.max(1)
}
