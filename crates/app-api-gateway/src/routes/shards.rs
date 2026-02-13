use axum::extract::{Path, Query};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
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
        .route("/api/shards/routing/stats", get(routing_stats_handler))
}

async fn list_servers_handler(
    Extension(app_state): Extension<AppState>,
    Query(query): Query<ServerListQuery>,
) -> Response {
    let mut servers = app_state.list_shard_servers();
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

    match app_state.register_shard_server(input) {
        Ok(server) => success(server_payload(server)),
        Err(message) => bad_request(message),
    }
}

async fn server_health_handler(
    Extension(app_state): Extension<AppState>,
    Path(id): Path<String>,
) -> Response {
    match app_state.shard_server_health(&id) {
        Some(health) => success(health_payload(health)),
        None => not_found("Server not found"),
    }
}

async fn routing_stats_handler(Extension(app_state): Extension<AppState>) -> Response {
    success(routing_stats_payload(app_state.shard_routing_stats()))
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
