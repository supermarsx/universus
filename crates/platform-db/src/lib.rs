use deadpool_postgres::{ManagerConfig, Pool, RecyclingMethod, Runtime};
use tokio_postgres::{types::Json, NoTls};

#[derive(Clone)]
pub struct Database {
    pool: Pool,
}

type DbResult<T> = Result<T, String>;

#[derive(Clone)]
pub struct AnalyticsUsageRow {
    pub event_type: String,
    pub count: i64,
}

#[derive(Clone)]
pub struct AnalyticsUsage {
    pub total_events: i64,
    pub active_users: i64,
    pub by_type: Vec<AnalyticsUsageRow>,
}

#[derive(Clone)]
pub struct ShardServerRow {
    pub server_id: String,
    pub server_type: String,
    pub region: String,
    pub endpoint: String,
    pub status: String,
    pub current_load: i64,
    pub max_capacity: i64,
    pub health_score: f64,
    pub last_heartbeat_unix: i64,
}

#[derive(Clone)]
pub struct ShardHealthRow {
    pub server_id: String,
    pub status: String,
    pub health_score: f64,
    pub current_load: i64,
    pub max_capacity: i64,
    pub load_percent: f64,
    pub last_heartbeat_unix: i64,
}

#[derive(Clone)]
pub struct ShardRoutingStats {
    pub total_servers: usize,
    pub healthy_servers: usize,
    pub overloaded_servers: usize,
    pub total_capacity: i64,
    pub total_load: i64,
    pub average_load_percent: f64,
    pub migration_count: i64,
}

#[derive(Clone)]
pub struct ShardServerUpsert {
    pub server_id: String,
    pub server_type: String,
    pub region: String,
    pub endpoint: String,
    pub status: String,
    pub current_load: i64,
    pub max_capacity: i64,
    pub health_score: f64,
}

impl Database {
    pub fn from_env() -> Option<Self> {
        let database_url = std::env::var("DATABASE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())?;
        let config = database_url.parse::<tokio_postgres::Config>().ok()?;
        let mgr_config = ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        };
        let manager = deadpool_postgres::Manager::from_config(config, NoTls, mgr_config);
        let pool = Pool::builder(manager)
            .max_size(16)
            .runtime(Runtime::Tokio1)
            .build()
            .ok()?;
        Some(Self { pool })
    }

    async fn ensure_analytics_schema(&self) -> DbResult<()> {
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        client
            .batch_execute(
                "CREATE TABLE IF NOT EXISTS analytics_events (
                    id BIGSERIAL PRIMARY KEY,
                    event_type TEXT NOT NULL,
                    session_id TEXT,
                    properties JSONB,
                    user_id BIGINT,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
                );",
            )
            .await
            .map_err(|error| error.to_string())
    }

    async fn ensure_shard_schema(&self) -> DbResult<()> {
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        client
            .batch_execute(
                "CREATE TABLE IF NOT EXISTS shard_servers (
                    server_id TEXT PRIMARY KEY,
                    server_type TEXT NOT NULL,
                    region TEXT NOT NULL,
                    endpoint TEXT NOT NULL,
                    status TEXT NOT NULL,
                    current_load BIGINT NOT NULL,
                    max_capacity BIGINT NOT NULL,
                    health_score DOUBLE PRECISION NOT NULL,
                    last_heartbeat_unix BIGINT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS shard_routing_meta (
                    id INTEGER PRIMARY KEY,
                    migration_count BIGINT NOT NULL
                );
                INSERT INTO shard_routing_meta (id, migration_count)
                VALUES (1, 0)
                ON CONFLICT (id) DO NOTHING;",
            )
            .await
            .map_err(|error| error.to_string())
    }

    pub async fn track_analytics_event(
        &self,
        event_type: &str,
        session_id: Option<&str>,
        properties: Option<serde_json::Value>,
        user_id: Option<i64>,
    ) -> DbResult<()> {
        self.ensure_analytics_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        client
            .execute(
                "INSERT INTO analytics_events (event_type, session_id, properties, user_id)
                 VALUES ($1, $2, $3, $4)",
                &[&event_type, &session_id, &properties.map(Json), &user_id],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub async fn analytics_usage(&self, days: i32) -> DbResult<AnalyticsUsage> {
        self.ensure_analytics_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let days = days.max(1);
        let total_row = client
            .query_one(
                "SELECT COUNT(*)::BIGINT AS total_events,
                        COUNT(DISTINCT COALESCE(user_id, 0))::BIGINT AS active_users
                 FROM analytics_events
                 WHERE created_at >= now() - ($1::TEXT || ' days')::INTERVAL",
                &[&days],
            )
            .await
            .map_err(|error| error.to_string())?;

        let by_type_rows = client
            .query(
                "SELECT event_type, COUNT(*)::BIGINT AS count
                 FROM analytics_events
                 WHERE created_at >= now() - ($1::TEXT || ' days')::INTERVAL
                 GROUP BY event_type
                 ORDER BY count DESC, event_type ASC",
                &[&days],
            )
            .await
            .map_err(|error| error.to_string())?;

        Ok(AnalyticsUsage {
            total_events: total_row.get::<_, i64>("total_events"),
            active_users: total_row.get::<_, i64>("active_users"),
            by_type: by_type_rows
                .into_iter()
                .map(|row| AnalyticsUsageRow {
                    event_type: row.get::<_, String>("event_type"),
                    count: row.get::<_, i64>("count"),
                })
                .collect(),
        })
    }

    pub async fn upsert_shard_server(
        &self,
        input: ShardServerUpsert,
    ) -> DbResult<ShardServerRow> {
        self.ensure_shard_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let now = unix_timestamp();

        let existing = client
            .query_opt(
                "SELECT server_id FROM shard_servers WHERE server_id = $1",
                &[&input.server_id],
            )
            .await
            .map_err(|error| error.to_string())?;

        if existing.is_some() {
            client
                .execute(
                    "UPDATE shard_routing_meta
                     SET migration_count = migration_count + 1
                     WHERE id = 1",
                    &[],
                )
                .await
                .map_err(|error| error.to_string())?;
        }

        let row = client
            .query_one(
                "INSERT INTO shard_servers
                    (server_id, server_type, region, endpoint, status, current_load, max_capacity, health_score, last_heartbeat_unix)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
                 ON CONFLICT (server_id) DO UPDATE SET
                    server_type = EXCLUDED.server_type,
                    region = EXCLUDED.region,
                    endpoint = EXCLUDED.endpoint,
                    status = EXCLUDED.status,
                    current_load = EXCLUDED.current_load,
                    max_capacity = EXCLUDED.max_capacity,
                    health_score = EXCLUDED.health_score,
                    last_heartbeat_unix = EXCLUDED.last_heartbeat_unix
                 RETURNING server_id, server_type, region, endpoint, status, current_load, max_capacity, health_score, last_heartbeat_unix",
                &[
                    &input.server_id,
                    &input.server_type,
                    &input.region,
                    &input.endpoint,
                    &input.status,
                    &input.current_load,
                    &input.max_capacity,
                    &input.health_score,
                    &now,
                ],
            )
            .await
            .map_err(|error| error.to_string())?;

        Ok(map_shard_server_row(&row))
    }

    pub async fn list_shard_servers(&self) -> DbResult<Vec<ShardServerRow>> {
        self.ensure_shard_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let rows = client
            .query(
                "SELECT server_id, server_type, region, endpoint, status, current_load, max_capacity, health_score, last_heartbeat_unix
                 FROM shard_servers
                 ORDER BY server_id ASC",
                &[],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(rows.into_iter().map(|row| map_shard_server_row(&row)).collect())
    }

    pub async fn shard_health(
        &self,
        server_id: &str,
    ) -> DbResult<Option<ShardHealthRow>> {
        self.ensure_shard_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let row = client
            .query_opt(
                "SELECT server_id, status, health_score, current_load, max_capacity, last_heartbeat_unix
                 FROM shard_servers
                 WHERE server_id = $1",
                &[&server_id],
            )
            .await
            .map_err(|error| error.to_string())?;

        Ok(row.map(|row| {
            let current_load = row.get::<_, i64>("current_load");
            let max_capacity = row.get::<_, i64>("max_capacity");
            ShardHealthRow {
                server_id: row.get::<_, String>("server_id"),
                status: row.get::<_, String>("status"),
                health_score: row.get::<_, f64>("health_score"),
                current_load,
                max_capacity,
                load_percent: load_percent(current_load, max_capacity),
                last_heartbeat_unix: row.get::<_, i64>("last_heartbeat_unix"),
            }
        }))
    }

    pub async fn shard_routing_stats(&self) -> DbResult<ShardRoutingStats> {
        self.ensure_shard_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;

        let row = client
            .query_one(
                "SELECT
                    COUNT(*)::BIGINT AS total_servers,
                    COUNT(*) FILTER (WHERE status = 'online' AND health_score >= 0.7)::BIGINT AS healthy_servers,
                    COUNT(*) FILTER (WHERE max_capacity > 0 AND (current_load::DOUBLE PRECISION / max_capacity::DOUBLE PRECISION) >= 0.8)::BIGINT AS overloaded_servers,
                    COALESCE(SUM(max_capacity), 0)::BIGINT AS total_capacity,
                    COALESCE(SUM(current_load), 0)::BIGINT AS total_load
                 FROM shard_servers",
                &[],
            )
            .await
            .map_err(|error| error.to_string())?;
        let meta = client
            .query_one(
                "SELECT migration_count FROM shard_routing_meta WHERE id = 1",
                &[],
            )
            .await
            .map_err(|error| error.to_string())?;

        let total_capacity = row.get::<_, i64>("total_capacity");
        let total_load = row.get::<_, i64>("total_load");
        let average_load_percent = if total_capacity <= 0 {
            0.0
        } else {
            round_2((total_load as f64 * 100.0) / total_capacity as f64)
        };

        Ok(ShardRoutingStats {
            total_servers: row.get::<_, i64>("total_servers").max(0) as usize,
            healthy_servers: row.get::<_, i64>("healthy_servers").max(0) as usize,
            overloaded_servers: row.get::<_, i64>("overloaded_servers").max(0) as usize,
            total_capacity,
            total_load,
            average_load_percent,
            migration_count: meta.get::<_, i64>("migration_count"),
        })
    }
}

fn map_shard_server_row(row: &tokio_postgres::Row) -> ShardServerRow {
    ShardServerRow {
        server_id: row.get::<_, String>("server_id"),
        server_type: row.get::<_, String>("server_type"),
        region: row.get::<_, String>("region"),
        endpoint: row.get::<_, String>("endpoint"),
        status: row.get::<_, String>("status"),
        current_load: row.get::<_, i64>("current_load"),
        max_capacity: row.get::<_, i64>("max_capacity"),
        health_score: row.get::<_, f64>("health_score"),
        last_heartbeat_unix: row.get::<_, i64>("last_heartbeat_unix"),
    }
}

fn unix_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn load_percent(current_load: i64, max_capacity: i64) -> f64 {
    if max_capacity <= 0 {
        0.0
    } else {
        round_2((current_load as f64 * 100.0) / max_capacity as f64)
    }
}

fn round_2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}
