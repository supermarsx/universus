mod gameplay;
mod privacy;

pub use gameplay::*;
pub use privacy::*;

use deadpool_postgres::{ManagerConfig, Pool, RecyclingMethod, Runtime};
use tokio_postgres::{types::Json, NoTls};

#[derive(Clone)]
pub struct Database {
    pool: Pool,
}

pub type DbResult<T> = Result<T, String>;

/// Durable account identity used by the API authentication layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountRow {
    pub id: String,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub role: String,
    pub universe_id: Option<i64>,
    pub is_banned: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountCreateInput {
    pub username: String,
    pub email: String,
    pub password_hash: String,
}

impl AccountCreateInput {
    pub fn normalized(mut self) -> Self {
        self.username = self.username.trim().to_string();
        self.email = normalize_account_email(&self.email);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountCreateError {
    Duplicate,
    Database(String),
}

#[derive(Clone)]
pub struct AnalyticsUsageRow {
    pub event_type: String,
    pub count: i64,
}

#[derive(Clone)]
pub struct NotificationRow {
    pub id: i64,
    pub user_id: i64,
    pub title: String,
    pub message: String,
    pub category: String,
    pub priority: i16,
    pub is_read: bool,
    pub created_at_unix: i64,
    pub read_at_unix: Option<i64>,
}

#[derive(Clone)]
pub struct NotificationCreateInput {
    pub user_id: i64,
    pub title: String,
    pub message: String,
    pub category: String,
    pub priority: i16,
}

#[derive(Clone)]
pub struct NotificationPreferenceRow {
    pub user_id: i64,
    pub category: String,
    pub enabled: bool,
    pub min_priority: i16,
}

#[derive(Clone)]
pub struct NotificationPreferenceUpsert {
    pub user_id: i64,
    pub category: String,
    pub enabled: bool,
    pub min_priority: i16,
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

#[derive(Clone)]
pub struct CrossServerMessageRow {
    pub id: i64,
    pub source_server_id: String,
    pub target_server_id: String,
    pub message_type: String,
    pub payload: serde_json::Value,
    pub status: String,
    pub attempt_count: i32,
    pub last_error: Option<String>,
}

#[derive(Clone)]
pub struct CrossServerMessageCreateInput {
    pub source_server_id: String,
    pub target_server_id: String,
    pub message_type: String,
    pub payload: serde_json::Value,
}

#[derive(Clone)]
pub struct CrossServerMessageStats {
    pub queued: i64,
    pub retry: i64,
    pub processing: i64,
    pub failed: i64,
    pub processed: i64,
    pub queue_lag_seconds: i64,
}

#[derive(Clone)]
pub struct UniverseRow {
    pub id: i64,
    pub name: String,
    pub speed: i32,
    pub registration_open: bool,
}

#[derive(Clone)]
pub struct UniverseCreateInput {
    pub name: String,
    pub speed: i32,
    pub registration_open: bool,
}

#[derive(Clone)]
pub struct UniverseUpsert {
    pub id: i64,
    pub name: String,
    pub speed: i32,
    pub registration_open: bool,
}

#[derive(Clone)]
pub struct UniverseStatsRow {
    pub universe_id: i64,
    pub active_players: i64,
    pub occupied_planets: i64,
    pub active_wars: i64,
}

#[derive(Clone)]
pub struct AcsGroupRow {
    pub id: i64,
    pub mission_type: String,
    pub target_galaxy: i32,
    pub target_system: i32,
    pub target_position: i32,
    pub member_count: i32,
    pub departure_window_start: String,
    pub departure_window_end: String,
    pub notes: Option<String>,
}

#[derive(Clone)]
pub struct AcsGroupCreateInput {
    pub mission_type: String,
    pub target_galaxy: i32,
    pub target_system: i32,
    pub target_position: i32,
    pub departure_window_start: String,
    pub departure_window_end: String,
    pub notes: Option<String>,
}

#[derive(Clone)]
pub struct AcsGroupUpsert {
    pub id: i64,
    pub mission_type: String,
    pub target_galaxy: i32,
    pub target_system: i32,
    pub target_position: i32,
    pub departure_window_start: String,
    pub departure_window_end: String,
    pub notes: Option<String>,
}

#[derive(Clone)]
pub struct MoonRow {
    pub id: i64,
    pub planet_id: i64,
    pub name: String,
    pub diameter: i32,
    pub has_jump_gate: bool,
}

#[derive(Clone)]
pub struct MoonCreateInput {
    pub planet_id: i64,
    pub name: String,
    pub diameter: i32,
    pub has_jump_gate: bool,
}

#[derive(Clone)]
pub struct MoonUpsert {
    pub id: i64,
    pub planet_id: i64,
    pub name: String,
    pub diameter: i32,
    pub has_jump_gate: bool,
}

#[derive(Clone)]
pub struct RipDestroyRequestRow {
    pub id: i64,
    pub mission_id: String,
    pub source_moon_id: i64,
    pub target_moon_id: i64,
    pub num_deathstars: i32,
    pub speed_percent: f64,
    pub status: String,
    pub requested_at_unix: i64,
}

#[derive(Clone)]
pub struct RipDestroyRequestCreateInput {
    pub mission_id: String,
    pub source_moon_id: i64,
    pub target_moon_id: i64,
    pub num_deathstars: i32,
    pub speed_percent: f64,
    pub status: String,
    pub requested_at_unix: i64,
}

#[derive(Clone)]
pub struct RipDestroyRequestUpsert {
    pub mission_id: String,
    pub source_moon_id: i64,
    pub target_moon_id: i64,
    pub num_deathstars: i32,
    pub speed_percent: f64,
    pub status: String,
    pub requested_at_unix: i64,
}

#[derive(Clone)]
pub struct ScheduledTaskRow {
    pub id: i64,
    pub task_type: String,
    pub payload: serde_json::Value,
    pub status: String,
    pub run_at_unix: i64,
    pub attempt_count: i32,
    pub lease_owner: Option<String>,
    pub lease_until_unix: Option<i64>,
}

#[derive(Clone)]
pub struct ScheduledTaskCreateInput {
    pub task_type: String,
    pub payload: serde_json::Value,
    pub run_at_unix: i64,
    pub task_key: Option<String>,
}

#[derive(Clone)]
pub struct ChatRestrictionRow {
    pub id: i64,
    pub user_id: i64,
    pub channel_id: Option<i64>,
    pub restriction_type: String,
    pub reason: String,
    pub restricted_by: i64,
    pub expires_at_unix: Option<i64>,
    pub created_at_unix: i64,
}

#[derive(Clone)]
pub struct ChatRestrictionUpsert {
    pub user_id: i64,
    pub channel_id: Option<i64>,
    pub restriction_type: String,
    pub reason: String,
    pub restricted_by: i64,
    pub expires_at_unix: Option<i64>,
}

impl Database {
    pub fn from_env() -> Option<Self> {
        Self::try_from_env().ok().flatten()
    }

    /// Build a database pool from the runtime configuration without silently
    /// discarding malformed URLs or pool construction errors.
    pub fn try_from_env() -> DbResult<Option<Self>> {
        let Some(database_url) = std::env::var("DATABASE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
        else {
            return Ok(None);
        };
        Self::from_database_url(&database_url).map(Some)
    }

    /// Build a database pool from an explicit URL. This is primarily useful
    /// for dependency injection and isolated integration tests; production
    /// callers should normally use [`Database::try_from_env`].
    pub fn from_database_url(database_url: &str) -> DbResult<Self> {
        let config = database_url
            .parse::<tokio_postgres::Config>()
            .map_err(|error| format!("invalid DATABASE_URL: {error}"))?;
        let mgr_config = ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        };
        let manager = deadpool_postgres::Manager::from_config(config, NoTls, mgr_config);
        let pool = Pool::builder(manager)
            .max_size(16)
            .runtime(Runtime::Tokio1)
            .build()
            .map_err(|error| format!("unable to create database pool: {error}"))?;
        Ok(Self { pool })
    }

    pub async fn ping(&self) -> DbResult<()> {
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        client
            .simple_query("SELECT 1")
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub async fn account_repository_ready(&self) -> DbResult<()> {
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let row = client
            .query_one(
                "SELECT to_regclass('public.users') IS NOT NULL
                    AND to_regclass('public.idx_users_username_normalized') IS NOT NULL
                    AND to_regclass('public.idx_users_email_normalized') IS NOT NULL
                    AND (SELECT COUNT(*) = 7
                         FROM information_schema.columns
                         WHERE table_schema = 'public'
                           AND table_name = 'users'
                           AND column_name IN ('id', 'username', 'email', 'password_hash',
                                               'last_login', 'is_admin', 'is_banned')) AS ready",
                &[],
            )
            .await
            .map_err(|error| error.to_string())?;
        if row.get::<_, bool>("ready") {
            Ok(())
        } else {
            Err(
                "users authentication schema is missing; run ordered database migrations"
                    .to_string(),
            )
        }
    }

    pub async fn create_account(
        &self,
        input: AccountCreateInput,
    ) -> Result<AccountRow, AccountCreateError> {
        self.register_account_with_starting_state(input).await
    }

    pub async fn account_by_normalized_email(&self, email: &str) -> DbResult<Option<AccountRow>> {
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let row = client
            .query_opt(
                "SELECT id::TEXT AS id, username, email, password_hash,
                        CASE WHEN is_admin THEN 'admin' ELSE 'player' END AS role,
                        universe_id, is_banned
                 FROM users
                 WHERE LOWER(BTRIM(email)) = $1
                 LIMIT 1",
                &[&normalize_account_email(email)],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(row.map(|row| map_account_row(&row)))
    }

    pub async fn account_by_id(&self, account_id: &str) -> DbResult<Option<AccountRow>> {
        let Ok(account_id) = account_id.parse::<i32>() else {
            return Ok(None);
        };
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let row = client
            .query_opt(
                "SELECT id::TEXT AS id, username, email, password_hash,
                        CASE WHEN is_admin THEN 'admin' ELSE 'player' END AS role,
                        universe_id, is_banned
                 FROM users
                 WHERE id = $1
                 LIMIT 1",
                &[&account_id],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(row.map(|row| map_account_row(&row)))
    }

    pub async fn update_account_last_login(&self, account_id: &str) -> DbResult<()> {
        let account_id = account_id
            .parse::<i32>()
            .map_err(|_| "invalid account id".to_string())?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let updated = client
            .execute(
                "UPDATE users SET last_login = now() WHERE id = $1",
                &[&account_id],
            )
            .await
            .map_err(|error| error.to_string())?;
        if updated == 1 {
            Ok(())
        } else {
            Err("account disappeared before last-login update".to_string())
        }
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
                    event_properties JSONB,
                    user_id BIGINT,
                    user_agent TEXT,
                    ip_address TEXT,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
                );
                ALTER TABLE analytics_events
                    ADD COLUMN IF NOT EXISTS properties JSONB;
                ALTER TABLE analytics_events
                    ADD COLUMN IF NOT EXISTS event_properties JSONB;
                ALTER TABLE analytics_events
                    ADD COLUMN IF NOT EXISTS user_agent TEXT;
                ALTER TABLE analytics_events
                    ADD COLUMN IF NOT EXISTS ip_address TEXT;
                ALTER TABLE analytics_events
                    ADD COLUMN IF NOT EXISTS user_id BIGINT;
                ALTER TABLE analytics_events
                    ADD COLUMN IF NOT EXISTS session_id TEXT;
                ALTER TABLE analytics_events
                    ADD COLUMN IF NOT EXISTS event_type TEXT;
                ALTER TABLE analytics_events
                    ADD COLUMN IF NOT EXISTS created_at TIMESTAMPTZ NOT NULL DEFAULT now();
                UPDATE analytics_events
                SET properties = COALESCE(properties, event_properties),
                    event_properties = COALESCE(event_properties, properties)
                WHERE properties IS NULL OR event_properties IS NULL;",
            )
            .await
            .map_err(|error| error.to_string())
    }

    async fn ensure_notifications_schema(&self) -> DbResult<()> {
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        client
            .batch_execute(
                "CREATE TABLE IF NOT EXISTS notifications (
                    id BIGSERIAL PRIMARY KEY,
                    user_id BIGINT NOT NULL,
                    title TEXT NOT NULL,
                    message TEXT NOT NULL,
                    category TEXT NOT NULL,
                    priority SMALLINT NOT NULL DEFAULT 1,
                    is_read BOOLEAN NOT NULL DEFAULT FALSE,
                    is_archived BOOLEAN NOT NULL DEFAULT FALSE,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                    read_at TIMESTAMPTZ,
                    archived_at TIMESTAMPTZ,
                    expires_at TIMESTAMPTZ
                );
                ALTER TABLE notifications
                    ADD COLUMN IF NOT EXISTS is_archived BOOLEAN NOT NULL DEFAULT FALSE;
                ALTER TABLE notifications
                    ADD COLUMN IF NOT EXISTS archived_at TIMESTAMPTZ;
                ALTER TABLE notifications
                    ADD COLUMN IF NOT EXISTS expires_at TIMESTAMPTZ;
                CREATE INDEX IF NOT EXISTS idx_notifications_user_created
                    ON notifications (user_id, created_at DESC);
                CREATE TABLE IF NOT EXISTS notification_preferences (
                    user_id BIGINT NOT NULL,
                    category TEXT NOT NULL,
                    enabled BOOLEAN NOT NULL DEFAULT TRUE,
                    min_priority SMALLINT NOT NULL DEFAULT 1,
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                    PRIMARY KEY (user_id, category)
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
                CREATE TABLE IF NOT EXISTS cross_server_messages (
                    id BIGSERIAL PRIMARY KEY,
                    source_server_id TEXT NOT NULL,
                    target_server_id TEXT NOT NULL,
                    message_type TEXT NOT NULL,
                    payload JSONB NOT NULL,
                    status TEXT NOT NULL DEFAULT 'queued',
                    attempt_count INTEGER NOT NULL DEFAULT 0,
                    lease_owner TEXT,
                    lease_until TIMESTAMPTZ,
                    last_error TEXT,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                    processed_at TIMESTAMPTZ
                );
                CREATE INDEX IF NOT EXISTS idx_cross_server_messages_target_status
                    ON cross_server_messages (target_server_id, status, created_at);
                INSERT INTO shard_routing_meta (id, migration_count)
                VALUES (1, 0)
                ON CONFLICT (id) DO NOTHING;",
            )
            .await
            .map_err(|error| error.to_string())
    }

    async fn ensure_universe_schema(&self) -> DbResult<()> {
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        client
            .batch_execute(
                "CREATE TABLE IF NOT EXISTS universes (
                    id BIGINT PRIMARY KEY GENERATED BY DEFAULT AS IDENTITY,
                    name TEXT NOT NULL UNIQUE,
                    speed INTEGER NOT NULL,
                    registration_open BOOLEAN NOT NULL DEFAULT true,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
                );",
            )
            .await
            .map_err(|error| error.to_string())
    }

    async fn ensure_acs_schema(&self) -> DbResult<()> {
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        client
            .batch_execute(
                "CREATE TABLE IF NOT EXISTS acs_groups (
                    id BIGINT PRIMARY KEY GENERATED BY DEFAULT AS IDENTITY,
                    mission_type TEXT NOT NULL,
                    target_galaxy INTEGER NOT NULL,
                    target_system INTEGER NOT NULL,
                    target_position INTEGER NOT NULL,
                    departure_window_start TEXT NOT NULL,
                    departure_window_end TEXT NOT NULL,
                    notes TEXT,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
                );
                CREATE TABLE IF NOT EXISTS acs_group_members (
                    group_id BIGINT NOT NULL REFERENCES acs_groups(id) ON DELETE CASCADE,
                    planet_id BIGINT NOT NULL,
                    joined_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                    PRIMARY KEY (group_id, planet_id)
                );",
            )
            .await
            .map_err(|error| error.to_string())
    }

    async fn ensure_moon_schema(&self) -> DbResult<()> {
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        client
            .batch_execute(
                "CREATE TABLE IF NOT EXISTS moons (
                    id BIGINT PRIMARY KEY GENERATED BY DEFAULT AS IDENTITY,
                    planet_id BIGINT NOT NULL UNIQUE,
                    name TEXT NOT NULL,
                    diameter INTEGER NOT NULL,
                    has_jump_gate BOOLEAN NOT NULL DEFAULT false,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
                );",
            )
            .await
            .map_err(|error| error.to_string())
    }

    async fn ensure_rip_schema(&self) -> DbResult<()> {
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        client
            .batch_execute(
                "CREATE TABLE IF NOT EXISTS rip_destroy_requests (
                    id BIGINT PRIMARY KEY GENERATED BY DEFAULT AS IDENTITY,
                    mission_id TEXT NOT NULL UNIQUE,
                    source_moon_id BIGINT NOT NULL,
                    target_moon_id BIGINT NOT NULL,
                    num_deathstars INTEGER NOT NULL,
                    speed_percent DOUBLE PRECISION NOT NULL,
                    status TEXT NOT NULL,
                    requested_at_unix BIGINT NOT NULL,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
                );
                CREATE INDEX IF NOT EXISTS idx_rip_destroy_requests_source_moon_id
                    ON rip_destroy_requests (source_moon_id);
                CREATE INDEX IF NOT EXISTS idx_rip_destroy_requests_target_moon_id
                    ON rip_destroy_requests (target_moon_id);",
            )
            .await
            .map_err(|error| error.to_string())
    }

    async fn ensure_scheduler_schema(&self) -> DbResult<()> {
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        client
            .batch_execute(
                "CREATE TABLE IF NOT EXISTS scheduled_tasks (
                    id BIGSERIAL PRIMARY KEY,
                    task_type TEXT NOT NULL,
                    payload JSONB NOT NULL,
                    status TEXT NOT NULL DEFAULT 'queued',
                    run_at TIMESTAMPTZ NOT NULL,
                    task_key TEXT,
                    attempt_count INTEGER NOT NULL DEFAULT 0,
                    last_error TEXT,
                    lease_owner TEXT,
                    lease_until TIMESTAMPTZ,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                    completed_at TIMESTAMPTZ
                );
                CREATE INDEX IF NOT EXISTS idx_scheduled_tasks_due
                    ON scheduled_tasks (status, run_at);
                CREATE INDEX IF NOT EXISTS idx_scheduled_tasks_lease
                    ON scheduled_tasks (lease_until);
                CREATE UNIQUE INDEX IF NOT EXISTS idx_scheduled_tasks_task_key
                    ON scheduled_tasks (task_key);",
            )
            .await
            .map_err(|error| error.to_string())
    }

    async fn ensure_chat_schema(&self) -> DbResult<()> {
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        client
            .batch_execute(
                "CREATE TABLE IF NOT EXISTS chat_restrictions (
                    id BIGSERIAL PRIMARY KEY,
                    user_id BIGINT NOT NULL,
                    channel_id BIGINT,
                    restriction_type TEXT NOT NULL,
                    reason TEXT NOT NULL,
                    restricted_by BIGINT NOT NULL,
                    expires_at TIMESTAMPTZ,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
                );
                CREATE INDEX IF NOT EXISTS idx_chat_restrictions_user
                    ON chat_restrictions (user_id);
                CREATE INDEX IF NOT EXISTS idx_chat_restrictions_expiry
                    ON chat_restrictions (expires_at);
                CREATE INDEX IF NOT EXISTS idx_chat_restrictions_scope
                    ON chat_restrictions (user_id, channel_id, restriction_type);",
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
        self.track_analytics_event_detailed(event_type, session_id, properties, user_id, None, None)
            .await
    }

    pub async fn track_analytics_event_detailed(
        &self,
        event_type: &str,
        session_id: Option<&str>,
        properties: Option<serde_json::Value>,
        user_id: Option<i64>,
        user_agent: Option<&str>,
        ip_address: Option<&str>,
    ) -> DbResult<()> {
        self.ensure_analytics_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        client
            .execute(
                "INSERT INTO analytics_events
                    (event_type, session_id, properties, event_properties, user_id, user_agent, ip_address)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
                &[
                    &event_type,
                    &session_id,
                    &properties.as_ref().map(Json),
                    &properties.as_ref().map(Json),
                    &user_id,
                    &user_agent,
                    &ip_address,
                ],
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

    pub async fn upsert_shard_server(&self, input: ShardServerUpsert) -> DbResult<ShardServerRow> {
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
        Ok(rows
            .into_iter()
            .map(|row| map_shard_server_row(&row))
            .collect())
    }

    pub async fn shard_health(&self, server_id: &str) -> DbResult<Option<ShardHealthRow>> {
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

    pub async fn expire_stale_shard_servers(&self, stale_after_secs: i64) -> DbResult<i64> {
        self.ensure_shard_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let now = unix_timestamp();
        let stale_before = now - stale_after_secs.max(1);
        let affected = client
            .execute(
                "UPDATE shard_servers
                 SET status = 'offline'
                 WHERE last_heartbeat_unix < $1
                   AND status <> 'offline'",
                &[&stale_before],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(affected as i64)
    }

    pub async fn enqueue_cross_server_message(
        &self,
        input: CrossServerMessageCreateInput,
    ) -> DbResult<CrossServerMessageRow> {
        self.ensure_shard_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let row = client
            .query_one(
                "INSERT INTO cross_server_messages
                    (source_server_id, target_server_id, message_type, payload)
                 VALUES ($1, $2, $3, $4)
                 RETURNING
                    id,
                    source_server_id,
                    target_server_id,
                    message_type,
                    payload,
                    status,
                    attempt_count,
                    last_error",
                &[
                    &input.source_server_id,
                    &input.target_server_id,
                    &input.message_type,
                    &Json(input.payload),
                ],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(map_cross_server_message_row(&row))
    }

    pub async fn claim_cross_server_messages(
        &self,
        target_server_id: &str,
        worker_id: &str,
        limit: i64,
        lease_seconds: i64,
    ) -> DbResult<Vec<CrossServerMessageRow>> {
        self.ensure_shard_schema().await?;
        let mut client = self.pool.get().await.map_err(|error| error.to_string())?;
        let safe_limit = limit.clamp(1, 500);
        let safe_lease = lease_seconds.max(5);
        let tx = client
            .transaction()
            .await
            .map_err(|error| error.to_string())?;
        let rows = tx
            .query(
                "WITH candidates AS (
                    SELECT id
                    FROM cross_server_messages
                    WHERE target_server_id = $1
                      AND status IN ('queued', 'retry')
                      AND (lease_until IS NULL OR lease_until < now())
                    ORDER BY created_at ASC, id ASC
                    FOR UPDATE SKIP LOCKED
                    LIMIT $2
                )
                UPDATE cross_server_messages m
                SET
                    status = 'processing',
                    lease_owner = $3,
                    lease_until = now() + ($4::BIGINT * INTERVAL '1 second'),
                    updated_at = now()
                FROM candidates
                WHERE m.id = candidates.id
                RETURNING
                    m.id,
                    m.source_server_id,
                    m.target_server_id,
                    m.message_type,
                    m.payload,
                    m.status,
                    m.attempt_count,
                    m.last_error",
                &[&target_server_id, &safe_limit, &worker_id, &safe_lease],
            )
            .await
            .map_err(|error| error.to_string())?;
        tx.commit().await.map_err(|error| error.to_string())?;
        Ok(rows
            .into_iter()
            .map(|row| map_cross_server_message_row(&row))
            .collect())
    }

    pub async fn ack_cross_server_message(&self, message_id: i64) -> DbResult<bool> {
        self.ensure_shard_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let affected = client
            .execute(
                "UPDATE cross_server_messages
                 SET
                    status = 'processed',
                    lease_owner = NULL,
                    lease_until = NULL,
                    processed_at = now(),
                    updated_at = now()
                 WHERE id = $1",
                &[&message_id],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(affected > 0)
    }

    pub async fn fail_cross_server_message(
        &self,
        message_id: i64,
        error_message: &str,
        retry_delay_secs: i64,
        max_attempts: i32,
    ) -> DbResult<bool> {
        self.ensure_shard_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let safe_delay = retry_delay_secs.max(1);
        let safe_attempts = max_attempts.max(1);
        let affected = client
            .execute(
                "UPDATE cross_server_messages
                 SET
                    attempt_count = attempt_count + 1,
                    status = CASE
                        WHEN attempt_count + 1 >= $4 THEN 'failed'
                        ELSE 'retry'
                    END,
                    lease_owner = NULL,
                    lease_until = CASE
                        WHEN attempt_count + 1 >= $4 THEN NULL
                        ELSE now() + ($3::BIGINT * INTERVAL '1 second')
                    END,
                    last_error = $2,
                    updated_at = now()
                 WHERE id = $1",
                &[&message_id, &error_message, &safe_delay, &safe_attempts],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(affected > 0)
    }

    pub async fn cross_server_message_stats(
        &self,
        target_server_id: Option<&str>,
    ) -> DbResult<CrossServerMessageStats> {
        self.ensure_shard_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let row = client
            .query_one(
                "SELECT
                    COUNT(*) FILTER (WHERE status = 'queued')::BIGINT AS queued,
                    COUNT(*) FILTER (WHERE status = 'retry')::BIGINT AS retry,
                    COUNT(*) FILTER (WHERE status = 'processing')::BIGINT AS processing,
                    COUNT(*) FILTER (WHERE status = 'failed')::BIGINT AS failed,
                    COUNT(*) FILTER (WHERE status = 'processed')::BIGINT AS processed,
                    COALESCE(
                        FLOOR(EXTRACT(EPOCH FROM (now() - MIN(created_at))))::BIGINT,
                        0
                    ) AS queue_lag_seconds
                 FROM cross_server_messages
                 WHERE ($1::TEXT IS NULL OR target_server_id = $1)
                   AND status IN ('queued', 'retry', 'processing', 'failed', 'processed')",
                &[&target_server_id],
            )
            .await
            .map_err(|error| error.to_string())?;

        Ok(CrossServerMessageStats {
            queued: row.get::<_, i64>("queued"),
            retry: row.get::<_, i64>("retry"),
            processing: row.get::<_, i64>("processing"),
            failed: row.get::<_, i64>("failed"),
            processed: row.get::<_, i64>("processed"),
            queue_lag_seconds: row.get::<_, i64>("queue_lag_seconds"),
        })
    }

    pub async fn requeue_failed_cross_server_messages(
        &self,
        target_server_id: Option<&str>,
        limit: i64,
    ) -> DbResult<i64> {
        self.ensure_shard_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let safe_limit = limit.clamp(1, 5000);
        let affected = client
            .execute(
                "WITH candidates AS (
                    SELECT id
                    FROM cross_server_messages
                    WHERE status = 'failed'
                      AND ($1::TEXT IS NULL OR target_server_id = $1)
                    ORDER BY updated_at ASC, id ASC
                    LIMIT $2
                )
                UPDATE cross_server_messages m
                SET
                    status = 'retry',
                    lease_owner = NULL,
                    lease_until = NULL,
                    last_error = NULL,
                    updated_at = now()
                FROM candidates
                WHERE m.id = candidates.id",
                &[&target_server_id, &safe_limit],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(affected as i64)
    }

    pub async fn list_cross_server_messages(
        &self,
        target_server_id: Option<&str>,
        status: Option<&str>,
        limit: i64,
    ) -> DbResult<Vec<CrossServerMessageRow>> {
        self.ensure_shard_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let safe_limit = limit.clamp(1, 500);
        let rows = client
            .query(
                "SELECT
                    id,
                    source_server_id,
                    target_server_id,
                    message_type,
                    payload,
                    status,
                    attempt_count,
                    last_error
                 FROM cross_server_messages
                 WHERE ($1::TEXT IS NULL OR target_server_id = $1)
                   AND ($2::TEXT IS NULL OR status = $2)
                 ORDER BY updated_at DESC, id DESC
                 LIMIT $3",
                &[&target_server_id, &status, &safe_limit],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(rows
            .into_iter()
            .map(|row| map_cross_server_message_row(&row))
            .collect())
    }

    pub async fn enqueue_scheduled_task(
        &self,
        input: ScheduledTaskCreateInput,
    ) -> DbResult<ScheduledTaskRow> {
        self.ensure_scheduler_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let row = client
            .query_one(
                "INSERT INTO scheduled_tasks (task_type, payload, run_at, task_key)
                 VALUES ($1, $2, TIMESTAMPTZ 'epoch' + ($3::BIGINT * INTERVAL '1 second'), $4)
                 ON CONFLICT (task_key) DO UPDATE SET
                    run_at = LEAST(scheduled_tasks.run_at, EXCLUDED.run_at),
                    updated_at = now()
                 RETURNING
                    id,
                    task_type,
                    payload,
                    status,
                    EXTRACT(EPOCH FROM run_at)::BIGINT AS run_at_unix,
                    attempt_count,
                    lease_owner,
                    CASE
                        WHEN lease_until IS NULL THEN NULL
                        ELSE EXTRACT(EPOCH FROM lease_until)::BIGINT
                    END AS lease_until_unix",
                &[
                    &input.task_type,
                    &Json(input.payload),
                    &input.run_at_unix,
                    &input.task_key,
                ],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(map_scheduled_task_row(&row))
    }

    pub async fn claim_due_scheduled_tasks(
        &self,
        worker_id: &str,
        limit: i64,
        lease_seconds: i64,
    ) -> DbResult<Vec<ScheduledTaskRow>> {
        self.ensure_scheduler_schema().await?;
        let mut client = self.pool.get().await.map_err(|error| error.to_string())?;
        let safe_limit = limit.clamp(1, 500);
        let safe_lease = lease_seconds.max(5);
        let tx = client
            .transaction()
            .await
            .map_err(|error| error.to_string())?;
        let rows = tx
            .query(
                "WITH candidates AS (
                    SELECT id
                    FROM scheduled_tasks
                    WHERE status IN ('queued', 'retry', 'running')
                      AND run_at <= now()
                      AND (lease_until IS NULL OR lease_until < now())
                    ORDER BY run_at ASC, id ASC
                    FOR UPDATE SKIP LOCKED
                    LIMIT $1
                )
                UPDATE scheduled_tasks t
                SET
                    status = 'running',
                    lease_owner = $2,
                    lease_until = now() + ($3::BIGINT * INTERVAL '1 second'),
                    updated_at = now()
                FROM candidates
                WHERE t.id = candidates.id
                RETURNING
                    t.id,
                    t.task_type,
                    t.payload,
                    t.status,
                    EXTRACT(EPOCH FROM t.run_at)::BIGINT AS run_at_unix,
                    t.attempt_count,
                    t.lease_owner,
                    CASE
                        WHEN t.lease_until IS NULL THEN NULL
                        ELSE EXTRACT(EPOCH FROM t.lease_until)::BIGINT
                    END AS lease_until_unix",
                &[&safe_limit, &worker_id, &safe_lease],
            )
            .await
            .map_err(|error| error.to_string())?;
        tx.commit().await.map_err(|error| error.to_string())?;
        Ok(rows
            .into_iter()
            .map(|row| map_scheduled_task_row(&row))
            .collect())
    }

    /// Complete by id for legacy callers that do not participate in leases.
    /// Lease-based workers must use `complete_scheduled_task_for_owner`.
    pub async fn complete_scheduled_task(&self, task_id: i64) -> DbResult<bool> {
        self.ensure_scheduler_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let affected = client
            .execute(
                "UPDATE scheduled_tasks
                 SET
                    status = 'completed',
                    lease_owner = NULL,
                    lease_until = NULL,
                    completed_at = now(),
                    updated_at = now()
                 WHERE id = $1",
                &[&task_id],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(affected > 0)
    }

    /// Complete a running task only while `lease_owner` still holds its live
    /// lease. A worker whose lease expired cannot overwrite a retry or a newer
    /// worker's result after the task is re-claimed.
    pub async fn complete_scheduled_task_for_owner(
        &self,
        task_id: i64,
        lease_owner: &str,
    ) -> DbResult<bool> {
        self.ensure_scheduler_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let affected = client
            .execute(
                "UPDATE scheduled_tasks
                 SET
                    status = 'completed',
                    lease_owner = NULL,
                    lease_until = NULL,
                    completed_at = now(),
                    updated_at = now()
                 WHERE id = $1
                   AND status = 'running'
                   AND lease_owner = $2
                   AND lease_until >= now()",
                &[&task_id, &lease_owner],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(affected > 0)
    }

    /// Fail by id for legacy callers that do not participate in leases.
    /// Lease-based workers must use `fail_scheduled_task_for_owner`.
    pub async fn fail_scheduled_task(
        &self,
        task_id: i64,
        error_message: &str,
        retry_delay_secs: i64,
        max_attempts: i32,
    ) -> DbResult<bool> {
        self.ensure_scheduler_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let delay_secs = retry_delay_secs.max(1);
        let attempts = max_attempts.max(1);
        let affected = client
            .execute(
                "UPDATE scheduled_tasks
                 SET
                    attempt_count = attempt_count + 1,
                    status = CASE
                        WHEN attempt_count + 1 >= $4 THEN 'failed'
                        ELSE 'retry'
                    END,
                    run_at = CASE
                        WHEN attempt_count + 1 >= $4 THEN run_at
                        ELSE now() + ($3::BIGINT * INTERVAL '1 second')
                    END,
                    last_error = $2,
                    lease_owner = NULL,
                    lease_until = NULL,
                    updated_at = now()
                 WHERE id = $1",
                &[&task_id, &error_message, &delay_secs, &attempts],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(affected > 0)
    }

    /// Record failure only for the worker that still owns the live lease.
    /// This is the scheduler-safe counterpart to the legacy id-only method.
    pub async fn fail_scheduled_task_for_owner(
        &self,
        task_id: i64,
        lease_owner: &str,
        error_message: &str,
        retry_delay_secs: i64,
        max_attempts: i32,
    ) -> DbResult<bool> {
        self.ensure_scheduler_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let delay_secs = retry_delay_secs.max(1);
        let attempts = max_attempts.max(1);
        let affected = client
            .execute(
                "UPDATE scheduled_tasks
                 SET
                    attempt_count = attempt_count + 1,
                    status = CASE
                        WHEN attempt_count + 1 >= $5 THEN 'failed'
                        ELSE 'retry'
                    END,
                    run_at = CASE
                        WHEN attempt_count + 1 >= $5 THEN run_at
                        ELSE now() + ($4::BIGINT * INTERVAL '1 second')
                    END,
                    last_error = $3,
                    lease_owner = NULL,
                    lease_until = NULL,
                    updated_at = now()
                 WHERE id = $1
                   AND status = 'running'
                   AND lease_owner = $2
                   AND lease_until >= now()",
                &[
                    &task_id,
                    &lease_owner,
                    &error_message,
                    &delay_secs,
                    &attempts,
                ],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(affected > 0)
    }

    pub async fn upsert_chat_restriction(
        &self,
        input: ChatRestrictionUpsert,
    ) -> DbResult<ChatRestrictionRow> {
        self.ensure_chat_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;

        client
            .execute(
                "DELETE FROM chat_restrictions
                 WHERE user_id = $1
                   AND restriction_type = $2
                   AND (
                     (channel_id IS NULL AND $3::BIGINT IS NULL)
                     OR channel_id = $3
                   )",
                &[&input.user_id, &input.restriction_type, &input.channel_id],
            )
            .await
            .map_err(|error| error.to_string())?;

        let row = client
            .query_one(
                "INSERT INTO chat_restrictions
                    (user_id, channel_id, restriction_type, reason, restricted_by, expires_at)
                 VALUES ($1, $2, $3, $4, $5, CASE WHEN $6::BIGINT IS NULL THEN NULL ELSE TIMESTAMPTZ 'epoch' + ($6::BIGINT * INTERVAL '1 second') END)
                 RETURNING
                    id,
                    user_id,
                    channel_id,
                    restriction_type,
                    reason,
                    restricted_by,
                    CASE WHEN expires_at IS NULL THEN NULL ELSE EXTRACT(EPOCH FROM expires_at)::BIGINT END AS expires_at_unix,
                    EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at_unix",
                &[
                    &input.user_id,
                    &input.channel_id,
                    &input.restriction_type,
                    &input.reason,
                    &input.restricted_by,
                    &input.expires_at_unix,
                ],
            )
            .await
            .map_err(|error| error.to_string())?;

        Ok(map_chat_restriction_row(&row))
    }

    pub async fn remove_chat_restriction(
        &self,
        user_id: i64,
        channel_id: Option<i64>,
        restriction_type: &str,
    ) -> DbResult<bool> {
        self.ensure_chat_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let affected = client
            .execute(
                "DELETE FROM chat_restrictions
                 WHERE user_id = $1
                   AND restriction_type = $2
                   AND (
                     (channel_id IS NULL AND $3::BIGINT IS NULL)
                     OR channel_id = $3
                   )",
                &[&user_id, &restriction_type, &channel_id],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(affected > 0)
    }

    pub async fn list_chat_restrictions(
        &self,
        user_id: Option<i64>,
        channel_id: Option<i64>,
        limit: i64,
    ) -> DbResult<Vec<ChatRestrictionRow>> {
        self.ensure_chat_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let safe_limit = limit.clamp(1, 500);
        let rows = client
            .query(
                "SELECT
                    id,
                    user_id,
                    channel_id,
                    restriction_type,
                    reason,
                    restricted_by,
                    CASE WHEN expires_at IS NULL THEN NULL ELSE EXTRACT(EPOCH FROM expires_at)::BIGINT END AS expires_at_unix,
                    EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at_unix
                 FROM chat_restrictions
                 WHERE ($1::BIGINT IS NULL OR user_id = $1)
                   AND (
                     $2::BIGINT IS NULL
                     OR channel_id = $2
                     OR (channel_id IS NULL AND $2::BIGINT IS NULL)
                   )
                   AND (expires_at IS NULL OR expires_at > now())
                 ORDER BY id DESC
                 LIMIT $3",
                &[&user_id, &channel_id, &safe_limit],
            )
            .await
            .map_err(|error| error.to_string())?;

        Ok(rows
            .into_iter()
            .map(|row| map_chat_restriction_row(&row))
            .collect())
    }

    pub async fn cleanup_expired_chat_restrictions(&self, now_unix: i64) -> DbResult<i64> {
        self.ensure_chat_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let affected = client
            .execute(
                "DELETE FROM chat_restrictions
                 WHERE expires_at IS NOT NULL
                   AND expires_at <= TIMESTAMPTZ 'epoch' + ($1::BIGINT * INTERVAL '1 second')",
                &[&now_unix],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(affected as i64)
    }

    pub async fn create_notification(
        &self,
        input: NotificationCreateInput,
    ) -> DbResult<NotificationRow> {
        self.ensure_notifications_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let row = client
            .query_one(
                "INSERT INTO notifications (user_id, title, message, category, priority)
                 VALUES ($1, $2, $3, $4, $5)
                 RETURNING
                    id,
                    user_id,
                    title,
                    message,
                    category,
                    priority,
                    is_read,
                    EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at_unix,
                    CASE
                        WHEN read_at IS NULL THEN NULL
                        ELSE EXTRACT(EPOCH FROM read_at)::BIGINT
                    END AS read_at_unix",
                &[
                    &input.user_id,
                    &input.title,
                    &input.message,
                    &input.category,
                    &input.priority,
                ],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(map_notification_row(&row))
    }

    pub async fn list_notifications(
        &self,
        user_id: i64,
        unread_only: bool,
        limit: i64,
    ) -> DbResult<Vec<NotificationRow>> {
        self.ensure_notifications_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let safe_limit = limit.clamp(1, 500);
        let rows = client
            .query(
                "SELECT
                    id,
                    user_id,
                    title,
                    message,
                    category,
                    priority,
                    is_read,
                    EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at_unix,
                    CASE
                        WHEN read_at IS NULL THEN NULL
                        ELSE EXTRACT(EPOCH FROM read_at)::BIGINT
                    END AS read_at_unix
                 FROM notifications
                 WHERE user_id = $1
                   AND ($2::BOOLEAN = FALSE OR is_read = FALSE)
                   AND is_archived = FALSE
                   AND (expires_at IS NULL OR expires_at > now())
                 ORDER BY id DESC
                 LIMIT $3",
                &[&user_id, &unread_only, &safe_limit],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(rows
            .into_iter()
            .map(|row| map_notification_row(&row))
            .collect())
    }

    pub async fn notification_unread_count(&self, user_id: i64) -> DbResult<i64> {
        self.ensure_notifications_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let row = client
            .query_one(
                "SELECT COUNT(*)::BIGINT AS unread_count
                 FROM notifications
                 WHERE user_id = $1
                   AND is_read = FALSE
                   AND is_archived = FALSE
                   AND (expires_at IS NULL OR expires_at > now())",
                &[&user_id],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(row.get::<_, i64>("unread_count"))
    }

    pub async fn mark_notification_read(
        &self,
        user_id: i64,
        notification_id: i64,
    ) -> DbResult<bool> {
        self.ensure_notifications_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let affected = client
            .execute(
                "UPDATE notifications
                 SET is_read = TRUE,
                     read_at = COALESCE(read_at, now())
                 WHERE id = $1
                   AND user_id = $2",
                &[&notification_id, &user_id],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(affected > 0)
    }

    pub async fn mark_all_notifications_read(&self, user_id: i64) -> DbResult<i64> {
        self.ensure_notifications_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let affected = client
            .execute(
                "UPDATE notifications
                 SET is_read = TRUE,
                     read_at = COALESCE(read_at, now())
                 WHERE user_id = $1
                   AND is_read = FALSE",
                &[&user_id],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(affected as i64)
    }

    pub async fn archive_notification(&self, user_id: i64, notification_id: i64) -> DbResult<bool> {
        self.ensure_notifications_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let affected = client
            .execute(
                "UPDATE notifications
                 SET is_archived = TRUE,
                     archived_at = COALESCE(archived_at, now())
                 WHERE id = $1
                   AND user_id = $2",
                &[&notification_id, &user_id],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(affected > 0)
    }

    pub async fn cleanup_expired_notifications(&self) -> DbResult<i64> {
        self.ensure_notifications_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let affected = client
            .execute(
                "DELETE FROM notifications
                 WHERE expires_at IS NOT NULL
                   AND expires_at < now()",
                &[],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(affected as i64)
    }

    pub async fn cleanup_archived_notifications_older_than_days(&self, days: i32) -> DbResult<i64> {
        self.ensure_notifications_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let keep_days = days.max(1);
        let affected = client
            .execute(
                "DELETE FROM notifications
                 WHERE is_archived = TRUE
                   AND archived_at IS NOT NULL
                   AND archived_at < now() - ($1::TEXT || ' days')::INTERVAL",
                &[&keep_days],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(affected as i64)
    }

    pub async fn list_notification_preferences(
        &self,
        user_id: i64,
    ) -> DbResult<Vec<NotificationPreferenceRow>> {
        self.ensure_notifications_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let rows = client
            .query(
                "SELECT user_id, category, enabled, min_priority
                 FROM notification_preferences
                 WHERE user_id = $1
                 ORDER BY category ASC",
                &[&user_id],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(rows
            .into_iter()
            .map(|row| map_notification_preference_row(&row))
            .collect())
    }

    pub async fn notification_preference(
        &self,
        user_id: i64,
        category: &str,
    ) -> DbResult<Option<NotificationPreferenceRow>> {
        self.ensure_notifications_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let row = client
            .query_opt(
                "SELECT user_id, category, enabled, min_priority
                 FROM notification_preferences
                 WHERE user_id = $1
                   AND category = $2",
                &[&user_id, &category],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(row.map(|row| map_notification_preference_row(&row)))
    }

    pub async fn upsert_notification_preference(
        &self,
        input: NotificationPreferenceUpsert,
    ) -> DbResult<NotificationPreferenceRow> {
        self.ensure_notifications_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let row = client
            .query_one(
                "INSERT INTO notification_preferences (user_id, category, enabled, min_priority)
                 VALUES ($1, $2, $3, $4)
                 ON CONFLICT (user_id, category) DO UPDATE SET
                    enabled = EXCLUDED.enabled,
                    min_priority = EXCLUDED.min_priority,
                    updated_at = now()
                 RETURNING user_id, category, enabled, min_priority",
                &[
                    &input.user_id,
                    &input.category,
                    &input.enabled,
                    &input.min_priority,
                ],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(map_notification_preference_row(&row))
    }

    pub async fn list_universes(&self) -> DbResult<Vec<UniverseRow>> {
        self.ensure_universe_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let rows = client
            .query(
                "SELECT id, name, speed, registration_open
                 FROM universes
                 ORDER BY id ASC",
                &[],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(rows.into_iter().map(|row| map_universe_row(&row)).collect())
    }

    pub async fn get_universe(&self, id: i64) -> DbResult<Option<UniverseRow>> {
        self.ensure_universe_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let row = client
            .query_opt(
                "SELECT id, name, speed, registration_open
                 FROM universes
                 WHERE id = $1",
                &[&id],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(row.map(|row| map_universe_row(&row)))
    }

    pub async fn create_universe(&self, input: UniverseCreateInput) -> DbResult<UniverseRow> {
        self.ensure_universe_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let row = client
            .query_one(
                "INSERT INTO universes (name, speed, registration_open)
                 VALUES ($1, $2, $3)
                 RETURNING id, name, speed, registration_open",
                &[&input.name, &input.speed, &input.registration_open],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(map_universe_row(&row))
    }

    pub async fn upsert_universe(&self, input: UniverseUpsert) -> DbResult<UniverseRow> {
        self.ensure_universe_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let row = client
            .query_one(
                "INSERT INTO universes (id, name, speed, registration_open)
                 VALUES ($1, $2, $3, $4)
                 ON CONFLICT (id) DO UPDATE SET
                    name = EXCLUDED.name,
                    speed = EXCLUDED.speed,
                    registration_open = EXCLUDED.registration_open,
                    updated_at = now()
                 RETURNING id, name, speed, registration_open",
                &[
                    &input.id,
                    &input.name,
                    &input.speed,
                    &input.registration_open,
                ],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(map_universe_row(&row))
    }

    pub async fn delete_universe(&self, id: i64) -> DbResult<bool> {
        self.ensure_universe_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let affected = client
            .execute("DELETE FROM universes WHERE id = $1", &[&id])
            .await
            .map_err(|error| error.to_string())?;
        Ok(affected > 0)
    }

    pub async fn universe_stats(&self, id: i64) -> DbResult<UniverseStatsRow> {
        self.ensure_universe_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let row = client
            .query_one(
                "SELECT
                    $1::BIGINT AS universe_id,
                    0::BIGINT AS active_players,
                    0::BIGINT AS occupied_planets,
                    0::BIGINT AS active_wars",
                &[&id],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(UniverseStatsRow {
            universe_id: row.get::<_, i64>("universe_id"),
            active_players: row.get::<_, i64>("active_players"),
            occupied_planets: row.get::<_, i64>("occupied_planets"),
            active_wars: row.get::<_, i64>("active_wars"),
        })
    }

    pub async fn list_acs_groups(&self) -> DbResult<Vec<AcsGroupRow>> {
        self.ensure_acs_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let rows = client
            .query(
                "SELECT
                    g.id,
                    g.mission_type,
                    g.target_galaxy,
                    g.target_system,
                    g.target_position,
                    g.departure_window_start,
                    g.departure_window_end,
                    g.notes,
                    COUNT(m.planet_id)::INT AS member_count
                 FROM acs_groups g
                 LEFT JOIN acs_group_members m ON m.group_id = g.id
                 GROUP BY g.id
                 ORDER BY g.id ASC",
                &[],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(rows
            .into_iter()
            .map(|row| map_acs_group_row(&row))
            .collect())
    }

    pub async fn get_acs_group(&self, id: i64) -> DbResult<Option<AcsGroupRow>> {
        self.ensure_acs_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let row = client
            .query_opt(
                "SELECT
                    g.id,
                    g.mission_type,
                    g.target_galaxy,
                    g.target_system,
                    g.target_position,
                    g.departure_window_start,
                    g.departure_window_end,
                    g.notes,
                    COUNT(m.planet_id)::INT AS member_count
                 FROM acs_groups g
                 LEFT JOIN acs_group_members m ON m.group_id = g.id
                 WHERE g.id = $1
                 GROUP BY g.id",
                &[&id],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(row.map(|row| map_acs_group_row(&row)))
    }

    pub async fn create_acs_group(&self, input: AcsGroupCreateInput) -> DbResult<AcsGroupRow> {
        self.ensure_acs_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let created = client
            .query_one(
                "INSERT INTO acs_groups
                    (mission_type, target_galaxy, target_system, target_position, departure_window_start, departure_window_end, notes)
                 VALUES ($1,$2,$3,$4,$5,$6,$7)
                 RETURNING id",
                &[
                    &input.mission_type,
                    &input.target_galaxy,
                    &input.target_system,
                    &input.target_position,
                    &input.departure_window_start,
                    &input.departure_window_end,
                    &input.notes,
                ],
            )
            .await
            .map_err(|error| error.to_string())?;
        let group_id = created.get::<_, i64>("id");
        self.get_acs_group(group_id)
            .await?
            .ok_or_else(|| "Failed to fetch created ACS group".to_string())
    }

    pub async fn upsert_acs_group(&self, input: AcsGroupUpsert) -> DbResult<AcsGroupRow> {
        self.ensure_acs_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        client
            .execute(
                "INSERT INTO acs_groups
                    (id, mission_type, target_galaxy, target_system, target_position, departure_window_start, departure_window_end, notes)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
                 ON CONFLICT (id) DO UPDATE SET
                    mission_type = EXCLUDED.mission_type,
                    target_galaxy = EXCLUDED.target_galaxy,
                    target_system = EXCLUDED.target_system,
                    target_position = EXCLUDED.target_position,
                    departure_window_start = EXCLUDED.departure_window_start,
                    departure_window_end = EXCLUDED.departure_window_end,
                    notes = EXCLUDED.notes,
                    updated_at = now()",
                &[
                    &input.id,
                    &input.mission_type,
                    &input.target_galaxy,
                    &input.target_system,
                    &input.target_position,
                    &input.departure_window_start,
                    &input.departure_window_end,
                    &input.notes,
                ],
            )
            .await
            .map_err(|error| error.to_string())?;
        self.get_acs_group(input.id)
            .await?
            .ok_or_else(|| "Failed to fetch upserted ACS group".to_string())
    }

    pub async fn join_acs_group(&self, group_id: i64, planet_id: i64) -> DbResult<bool> {
        self.ensure_acs_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let affected = client
            .execute(
                "INSERT INTO acs_group_members (group_id, planet_id)
                 VALUES ($1, $2)
                 ON CONFLICT (group_id, planet_id) DO NOTHING",
                &[&group_id, &planet_id],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(affected > 0)
    }

    pub async fn leave_acs_group(&self, group_id: i64) -> DbResult<bool> {
        self.ensure_acs_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let affected = client
            .execute(
                "DELETE FROM acs_group_members
                 WHERE ctid IN (
                    SELECT ctid FROM acs_group_members
                    WHERE group_id = $1
                    LIMIT 1
                 )",
                &[&group_id],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(affected > 0)
    }

    pub async fn delete_acs_group(&self, id: i64) -> DbResult<bool> {
        self.ensure_acs_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let affected = client
            .execute("DELETE FROM acs_groups WHERE id = $1", &[&id])
            .await
            .map_err(|error| error.to_string())?;
        Ok(affected > 0)
    }

    pub async fn list_moons(&self) -> DbResult<Vec<MoonRow>> {
        self.ensure_moon_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let rows = client
            .query(
                "SELECT id, planet_id, name, diameter, has_jump_gate
                 FROM moons
                 ORDER BY id ASC",
                &[],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(rows.into_iter().map(|row| map_moon_row(&row)).collect())
    }

    pub async fn moon_by_planet_id(&self, planet_id: i64) -> DbResult<Option<MoonRow>> {
        self.ensure_moon_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let row = client
            .query_opt(
                "SELECT id, planet_id, name, diameter, has_jump_gate
                 FROM moons
                 WHERE planet_id = $1",
                &[&planet_id],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(row.map(|row| map_moon_row(&row)))
    }

    pub async fn moon_by_id(&self, moon_id: i64) -> DbResult<Option<MoonRow>> {
        self.ensure_moon_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let row = client
            .query_opt(
                "SELECT id, planet_id, name, diameter, has_jump_gate
                 FROM moons
                 WHERE id = $1",
                &[&moon_id],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(row.map(|row| map_moon_row(&row)))
    }

    pub async fn create_moon(&self, input: MoonCreateInput) -> DbResult<MoonRow> {
        self.ensure_moon_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let row = client
            .query_one(
                "INSERT INTO moons (planet_id, name, diameter, has_jump_gate)
                 VALUES ($1, $2, $3, $4)
                 RETURNING id, planet_id, name, diameter, has_jump_gate",
                &[
                    &input.planet_id,
                    &input.name,
                    &input.diameter,
                    &input.has_jump_gate,
                ],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(map_moon_row(&row))
    }

    pub async fn upsert_moon(&self, input: MoonUpsert) -> DbResult<MoonRow> {
        self.ensure_moon_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let row = client
            .query_one(
                "INSERT INTO moons (id, planet_id, name, diameter, has_jump_gate)
                 VALUES ($1, $2, $3, $4, $5)
                 ON CONFLICT (id) DO UPDATE SET
                    planet_id = EXCLUDED.planet_id,
                    name = EXCLUDED.name,
                    diameter = EXCLUDED.diameter,
                    has_jump_gate = EXCLUDED.has_jump_gate,
                    updated_at = now()
                 RETURNING id, planet_id, name, diameter, has_jump_gate",
                &[
                    &input.id,
                    &input.planet_id,
                    &input.name,
                    &input.diameter,
                    &input.has_jump_gate,
                ],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(map_moon_row(&row))
    }

    pub async fn delete_moon(&self, moon_id: i64) -> DbResult<bool> {
        self.ensure_moon_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let affected = client
            .execute("DELETE FROM moons WHERE id = $1", &[&moon_id])
            .await
            .map_err(|error| error.to_string())?;
        Ok(affected > 0)
    }

    pub async fn queue_rip_attack(
        &self,
        input: RipDestroyRequestCreateInput,
    ) -> DbResult<RipDestroyRequestRow> {
        self.ensure_rip_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let row = client
            .query_one(
                "INSERT INTO rip_destroy_requests
                    (mission_id, source_moon_id, target_moon_id, num_deathstars, speed_percent, status, requested_at_unix)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)
                 RETURNING id, mission_id, source_moon_id, target_moon_id, num_deathstars, speed_percent, status, requested_at_unix",
                &[
                    &input.mission_id,
                    &input.source_moon_id,
                    &input.target_moon_id,
                    &input.num_deathstars,
                    &input.speed_percent,
                    &input.status,
                    &input.requested_at_unix,
                ],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(map_rip_destroy_request_row(&row))
    }

    pub async fn upsert_rip_attack(
        &self,
        input: RipDestroyRequestUpsert,
    ) -> DbResult<RipDestroyRequestRow> {
        self.ensure_rip_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let row = client
            .query_one(
                "INSERT INTO rip_destroy_requests
                    (mission_id, source_moon_id, target_moon_id, num_deathstars, speed_percent, status, requested_at_unix)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)
                 ON CONFLICT (mission_id) DO UPDATE SET
                    source_moon_id = EXCLUDED.source_moon_id,
                    target_moon_id = EXCLUDED.target_moon_id,
                    num_deathstars = EXCLUDED.num_deathstars,
                    speed_percent = EXCLUDED.speed_percent,
                    status = EXCLUDED.status,
                    requested_at_unix = EXCLUDED.requested_at_unix,
                    updated_at = now()
                 RETURNING id, mission_id, source_moon_id, target_moon_id, num_deathstars, speed_percent, status, requested_at_unix",
                &[
                    &input.mission_id,
                    &input.source_moon_id,
                    &input.target_moon_id,
                    &input.num_deathstars,
                    &input.speed_percent,
                    &input.status,
                    &input.requested_at_unix,
                ],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(map_rip_destroy_request_row(&row))
    }

    pub async fn rip_attack_by_mission_id(
        &self,
        mission_id: &str,
    ) -> DbResult<Option<RipDestroyRequestRow>> {
        self.ensure_rip_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let row = client
            .query_opt(
                "SELECT id, mission_id, source_moon_id, target_moon_id, num_deathstars, speed_percent, status, requested_at_unix
                 FROM rip_destroy_requests
                 WHERE mission_id = $1",
                &[&mission_id],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(row.map(|row| map_rip_destroy_request_row(&row)))
    }

    pub async fn list_rip_attacks(&self, limit: i64) -> DbResult<Vec<RipDestroyRequestRow>> {
        self.ensure_rip_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let safe_limit = limit.clamp(1, 500);
        let rows = client
            .query(
                "SELECT id, mission_id, source_moon_id, target_moon_id, num_deathstars, speed_percent, status, requested_at_unix
                 FROM rip_destroy_requests
                 ORDER BY id DESC
                 LIMIT $1",
                &[&safe_limit],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(rows
            .into_iter()
            .map(|row| map_rip_destroy_request_row(&row))
            .collect())
    }

    pub async fn delete_rip_attack(&self, mission_id: &str) -> DbResult<bool> {
        self.ensure_rip_schema().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let affected = client
            .execute(
                "DELETE FROM rip_destroy_requests WHERE mission_id = $1",
                &[&mission_id],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(affected > 0)
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

fn map_universe_row(row: &tokio_postgres::Row) -> UniverseRow {
    UniverseRow {
        id: row.get::<_, i64>("id"),
        name: row.get::<_, String>("name"),
        speed: row.get::<_, i32>("speed"),
        registration_open: row.get::<_, bool>("registration_open"),
    }
}

fn map_acs_group_row(row: &tokio_postgres::Row) -> AcsGroupRow {
    AcsGroupRow {
        id: row.get::<_, i64>("id"),
        mission_type: row.get::<_, String>("mission_type"),
        target_galaxy: row.get::<_, i32>("target_galaxy"),
        target_system: row.get::<_, i32>("target_system"),
        target_position: row.get::<_, i32>("target_position"),
        member_count: row.get::<_, i32>("member_count"),
        departure_window_start: row.get::<_, String>("departure_window_start"),
        departure_window_end: row.get::<_, String>("departure_window_end"),
        notes: row.get::<_, Option<String>>("notes"),
    }
}

fn map_moon_row(row: &tokio_postgres::Row) -> MoonRow {
    MoonRow {
        id: row.get::<_, i64>("id"),
        planet_id: row.get::<_, i64>("planet_id"),
        name: row.get::<_, String>("name"),
        diameter: row.get::<_, i32>("diameter"),
        has_jump_gate: row.get::<_, bool>("has_jump_gate"),
    }
}

fn map_rip_destroy_request_row(row: &tokio_postgres::Row) -> RipDestroyRequestRow {
    RipDestroyRequestRow {
        id: row.get::<_, i64>("id"),
        mission_id: row.get::<_, String>("mission_id"),
        source_moon_id: row.get::<_, i64>("source_moon_id"),
        target_moon_id: row.get::<_, i64>("target_moon_id"),
        num_deathstars: row.get::<_, i32>("num_deathstars"),
        speed_percent: row.get::<_, f64>("speed_percent"),
        status: row.get::<_, String>("status"),
        requested_at_unix: row.get::<_, i64>("requested_at_unix"),
    }
}

fn map_notification_row(row: &tokio_postgres::Row) -> NotificationRow {
    NotificationRow {
        id: row.get::<_, i64>("id"),
        user_id: row.get::<_, i64>("user_id"),
        title: row.get::<_, String>("title"),
        message: row.get::<_, String>("message"),
        category: row.get::<_, String>("category"),
        priority: row.get::<_, i16>("priority"),
        is_read: row.get::<_, bool>("is_read"),
        created_at_unix: row.get::<_, i64>("created_at_unix"),
        read_at_unix: row.get::<_, Option<i64>>("read_at_unix"),
    }
}

fn map_notification_preference_row(row: &tokio_postgres::Row) -> NotificationPreferenceRow {
    NotificationPreferenceRow {
        user_id: row.get::<_, i64>("user_id"),
        category: row.get::<_, String>("category"),
        enabled: row.get::<_, bool>("enabled"),
        min_priority: row.get::<_, i16>("min_priority"),
    }
}

fn map_scheduled_task_row(row: &tokio_postgres::Row) -> ScheduledTaskRow {
    ScheduledTaskRow {
        id: row.get::<_, i64>("id"),
        task_type: row.get::<_, String>("task_type"),
        payload: row.get::<_, Json<serde_json::Value>>("payload").0,
        status: row.get::<_, String>("status"),
        run_at_unix: row.get::<_, i64>("run_at_unix"),
        attempt_count: row.get::<_, i32>("attempt_count"),
        lease_owner: row.get::<_, Option<String>>("lease_owner"),
        lease_until_unix: row.get::<_, Option<i64>>("lease_until_unix"),
    }
}

fn map_cross_server_message_row(row: &tokio_postgres::Row) -> CrossServerMessageRow {
    CrossServerMessageRow {
        id: row.get::<_, i64>("id"),
        source_server_id: row.get::<_, String>("source_server_id"),
        target_server_id: row.get::<_, String>("target_server_id"),
        message_type: row.get::<_, String>("message_type"),
        payload: row.get::<_, Json<serde_json::Value>>("payload").0,
        status: row.get::<_, String>("status"),
        attempt_count: row.get::<_, i32>("attempt_count"),
        last_error: row.get::<_, Option<String>>("last_error"),
    }
}

fn map_chat_restriction_row(row: &tokio_postgres::Row) -> ChatRestrictionRow {
    ChatRestrictionRow {
        id: row.get::<_, i64>("id"),
        user_id: row.get::<_, i64>("user_id"),
        channel_id: row.get::<_, Option<i64>>("channel_id"),
        restriction_type: row.get::<_, String>("restriction_type"),
        reason: row.get::<_, String>("reason"),
        restricted_by: row.get::<_, i64>("restricted_by"),
        expires_at_unix: row.get::<_, Option<i64>>("expires_at_unix"),
        created_at_unix: row.get::<_, i64>("created_at_unix"),
    }
}

fn map_account_row(row: &tokio_postgres::Row) -> AccountRow {
    AccountRow {
        id: row.get::<_, String>("id"),
        username: row.get::<_, String>("username"),
        email: row.get::<_, String>("email"),
        password_hash: row.get::<_, String>("password_hash"),
        role: row.get::<_, String>("role"),
        universe_id: row.get::<_, Option<i64>>("universe_id"),
        is_banned: row.get::<_, bool>("is_banned"),
    }
}

pub fn normalize_account_email(email: &str) -> String {
    email.trim().to_ascii_lowercase()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_input_normalizes_email_and_trims_username() {
        let input = AccountCreateInput {
            username: "  Commander  ".to_string(),
            email: "  COMMANDER@Example.COM ".to_string(),
            password_hash: "$argon2id$v=19$m=19456,t=2,p=1$example$hash".to_string(),
        }
        .normalized();

        assert_eq!(input.username, "Commander");
        assert_eq!(input.email, "commander@example.com");
        assert!(input.password_hash.starts_with("$argon2id$"));
    }

    #[test]
    fn account_email_normalization_is_idempotent() {
        let normalized = normalize_account_email(" A@B.COM ");
        assert_eq!(normalized, "a@b.com");
        assert_eq!(normalize_account_email(&normalized), normalized);
    }
}
