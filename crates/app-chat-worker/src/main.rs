//! Chat moderation cleanup worker.
//!
//! Periodically runs cleanup cycles that:
//! - Remove expired chat restrictions (mutes, bans, etc.)
//! - Log active restriction counts for auditing
//! - Publish operational events on cleanup completion
//!
//! Configuration via environment variables:
//! - `CHAT_CLEANUP_INTERVAL_SECS` — interval between cleanup cycles (default: 3600)
//! - `CHAT_CLEANUP_RUN_ONCE` — run one cycle then exit (default: false)
//! - `CHAT_WORKER_MAX_INFLIGHT` — max concurrent tasks (default: 8)
//! - `CHAT_WORKER_TENANT_ID` — tenant ID for worker context
//! - `CHAT_WORKER_TENANT_NAME` — tenant display name
//! - `CHAT_RESTRICTION_AUDIT_LIMIT` — max restrictions to fetch for audit (default: 500)
//! - `REALTIME_GATEWAY_URL` — URL for publishing operational events

use std::time::{SystemTime, UNIX_EPOCH};

use platform_db::Database;
use platform_tenancy::{TenantAccessLevel, TenantContext};
use platform_worker_runtime::WorkerRuntime;
use tokio::sync::oneshot;
use tokio::time::{sleep, Duration};

const SERVICE_NAME: &str = "app-chat-worker";

/// Worker configuration parsed from environment.
struct WorkerConfig {
    interval_secs: u64,
    run_once: bool,
    max_inflight: usize,
    audit_limit: i64,
    tenant_context: TenantContext,
    realtime_url: Option<String>,
}

impl WorkerConfig {
    fn from_env() -> Self {
        Self {
            interval_secs: parse_env("CHAT_CLEANUP_INTERVAL_SECS", 3600),
            run_once: std::env::var("CHAT_CLEANUP_RUN_ONCE")
                .map(|v| v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            max_inflight: parse_env("CHAT_WORKER_MAX_INFLIGHT", 8),
            audit_limit: parse_env("CHAT_RESTRICTION_AUDIT_LIMIT", 500),
            tenant_context: tenant_context_from_env(),
            realtime_url: std::env::var("REALTIME_GATEWAY_URL")
                .ok()
                .filter(|v| !v.trim().is_empty()),
        }
    }
}

/// Cleanup cycle result for reporting and events.
struct CleanupResult {
    expired_removed: i64,
    active_mutes: i64,
    active_bans: i64,
    active_other: i64,
}

impl CleanupResult {
    fn total_active(&self) -> i64 {
        self.active_mutes + self.active_bans + self.active_other
    }
}

#[tokio::main]
async fn main() {
    platform_observability::init(SERVICE_NAME);

    let config = WorkerConfig::from_env();
    let runtime = WorkerRuntime::current(config.max_inflight);

    tracing::info!(
        service = SERVICE_NAME,
        interval_secs = config.interval_secs,
        run_once = config.run_once,
        max_inflight = config.max_inflight,
        audit_limit = config.audit_limit,
        tenant_id = %config.tenant_context.tenant_id,
        has_realtime_url = config.realtime_url.is_some(),
        "chat cleanup worker started"
    );

    let mut cycle_count: u64 = 0;
    loop {
        cycle_count += 1;
        let context = config.tenant_context.clone();
        let (done_tx, done_rx) = oneshot::channel::<CleanupResult>();
        let audit_limit = config.audit_limit;
        let cycle = cycle_count;

        match runtime.spawn_tenant_task(context, async move {
            let result = run_cleanup_cycle(audit_limit, cycle).await;
            let _ = done_tx.send(result);
            Ok(())
        }) {
            Ok(_job_id) => {
                match done_rx.await {
                    Ok(result) => {
                        tracing::info!(
                            service = SERVICE_NAME,
                            cycle = cycle_count,
                            expired_removed = result.expired_removed,
                            active_mutes = result.active_mutes,
                            active_bans = result.active_bans,
                            active_other = result.active_other,
                            total_active = result.total_active(),
                            "cleanup cycle completed"
                        );
                        if let Some(url) = &config.realtime_url {
                            publish_ops_event(
                                url,
                                "chat.cleanup.completed",
                                &serde_json::json!({
                                    "cycle": cycle_count,
                                    "expiredRemoved": result.expired_removed,
                                    "activeMutes": result.active_mutes,
                                    "activeBans": result.active_bans,
                                    "activeOther": result.active_other,
                                    "totalActive": result.total_active()
                                }),
                            )
                            .await;
                        }
                    }
                    Err(_) => {
                        tracing::warn!(
                            service = SERVICE_NAME,
                            cycle = cycle_count,
                            "chat cleanup task ended before completion signal"
                        );
                    }
                }
                let stats = runtime.stats().await;
                tracing::info!(
                    service = SERVICE_NAME,
                    total_inflight = stats.total_inflight,
                    total_completed = stats.total_completed,
                    total_failed = stats.total_failed,
                    tenant_count = stats.per_tenant.len(),
                    "chat worker runtime stats"
                );
            }
            Err(error) => {
                tracing::warn!(
                    service = SERVICE_NAME,
                    %error,
                    cycle = cycle_count,
                    "failed to schedule chat cleanup cycle"
                );
            }
        }

        if config.run_once {
            break;
        }
        sleep(Duration::from_secs(config.interval_secs)).await;
    }

    runtime.shutdown(Duration::from_secs(5)).await;
    tracing::info!(
        service = SERVICE_NAME,
        total_cycles = cycle_count,
        "worker shutdown complete"
    );
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|v| v.as_secs() as i64)
        .unwrap_or(0)
}

async fn run_cleanup_cycle(audit_limit: i64, cycle: u64) -> CleanupResult {
    let Some(database) = Database::from_env() else {
        tracing::warn!(
            service = SERVICE_NAME,
            cycle,
            "DATABASE_URL not configured; skipping chat cleanup cycle"
        );
        return CleanupResult {
            expired_removed: 0,
            active_mutes: 0,
            active_bans: 0,
            active_other: 0,
        };
    };

    let now = now_unix();

    // Phase 1: Remove expired restrictions
    let expired_removed = database
        .cleanup_expired_chat_restrictions(now)
        .await
        .unwrap_or_else(|error| {
            tracing::error!(service = SERVICE_NAME, cycle, %error, "failed expired restriction cleanup");
            0
        });

    // Phase 2: Audit active restrictions by type
    let (active_mutes, active_bans, active_other) =
        audit_active_restrictions(&database, audit_limit, cycle).await;

    CleanupResult {
        expired_removed,
        active_mutes,
        active_bans,
        active_other,
    }
}

/// Fetches active restrictions and counts them by type for monitoring.
async fn audit_active_restrictions(database: &Database, limit: i64, cycle: u64) -> (i64, i64, i64) {
    let restrictions = match database.list_chat_restrictions(None, None, limit).await {
        Ok(rows) => rows,
        Err(error) => {
            tracing::error!(
                service = SERVICE_NAME,
                cycle,
                %error,
                "failed to list chat restrictions for audit"
            );
            return (0, 0, 0);
        }
    };

    let now = now_unix();
    let mut mutes: i64 = 0;
    let mut bans: i64 = 0;
    let mut other: i64 = 0;

    for r in &restrictions {
        // Only count active (non-expired) restrictions
        // The DB query already filters expired ones, but double-check in case
        if let Some(expires) = r.expires_at_unix {
            if expires > 0 && expires <= now {
                continue;
            }
        }
        match r.restriction_type.as_str() {
            "mute" => mutes += 1,
            "ban" => bans += 1,
            _ => other += 1,
        }
    }

    (mutes, bans, other)
}

async fn publish_ops_event(base_url: &str, event_type: &str, payload: &serde_json::Value) {
    let event = platform_events::build_event(event_type, payload);
    if let Err(error) = platform_events::publish_http(base_url, "ops.chat", &event).await {
        tracing::warn!(
            service = SERVICE_NAME,
            event_type,
            %error,
            "failed to publish ops event"
        );
    }
}

fn tenant_context_from_env() -> TenantContext {
    let tenant_id = std::env::var("CHAT_WORKER_TENANT_ID")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "tenant-default".to_string());
    let tenant_name = std::env::var("CHAT_WORKER_TENANT_NAME")
        .ok()
        .filter(|v| !v.trim().is_empty());

    TenantContext {
        tenant_id,
        tenant_name,
        access_level: TenantAccessLevel::Worker,
    }
}

fn parse_env<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<T>().ok())
        .unwrap_or(default)
}
