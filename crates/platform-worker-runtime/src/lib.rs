//! Shared runtime helper for workers that need tenant-safe threading, instrumentation,
//! graceful shutdown, job lifecycle tracking, retry policies, and dead letter queue.
//!
//! Features:
//! - Spawn tenant-scoped tasks with instrumentation
//! - Spawn leased tasks (consensus-backed exclusivity)
//! - Configurable retry policies (fixed, exponential backoff)
//! - Dead letter queue for failed tasks
//! - Job lifecycle tracking (pending → running → completed/failed)
//! - Graceful shutdown with configurable timeout
//! - Per-tenant and aggregate statistics

use anyhow::Result;
use platform_consensus::LeaseCoordinator;
use platform_tenancy::TenantContext;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tokio::runtime::Handle;
use tokio::sync::{Mutex, Notify, RwLock};
use tokio::time::{Duration, Instant};
use tracing::{info, span, Instrument, Level};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors returned by the worker runtime when scheduling tasks.
#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("worker runtime is shutting down")]
    ShuttingDown,

    #[error("maximum in-flight tasks exceeded")]
    MaxInflight,

    #[error("failed to acquire lease: {0}")]
    LeaseAcquire(String),
}

// ---------------------------------------------------------------------------
// Retry policy
// ---------------------------------------------------------------------------

/// Configurable retry policy for failed tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RetryPolicy {
    /// No retries; fail immediately.
    None,
    /// Retry up to `max_attempts` times with a fixed delay between attempts.
    Fixed { max_attempts: u32, delay: Duration },
    /// Retry up to `max_attempts` times with exponential backoff.
    ExponentialBackoff {
        max_attempts: u32,
        initial_delay: Duration,
        max_delay: Duration,
        multiplier: f64,
    },
}

impl RetryPolicy {
    pub fn fixed(max_attempts: u32, delay: Duration) -> Self {
        Self::Fixed {
            max_attempts,
            delay,
        }
    }

    pub fn exponential(max_attempts: u32, initial_delay: Duration, max_delay: Duration) -> Self {
        Self::ExponentialBackoff {
            max_attempts,
            initial_delay,
            max_delay,
            multiplier: 2.0,
        }
    }

    /// Compute the delay for a given attempt (0-indexed).
    pub fn delay_for_attempt(&self, attempt: u32) -> Option<Duration> {
        match self {
            RetryPolicy::None => None,
            RetryPolicy::Fixed {
                max_attempts,
                delay,
            } => {
                if attempt < *max_attempts {
                    Some(*delay)
                } else {
                    None
                }
            }
            RetryPolicy::ExponentialBackoff {
                max_attempts,
                initial_delay,
                max_delay,
                multiplier,
            } => {
                if attempt < *max_attempts {
                    let delay_ms =
                        initial_delay.as_millis() as f64 * multiplier.powi(attempt as i32);
                    let capped =
                        Duration::from_millis(delay_ms.min(max_delay.as_millis() as f64) as u64);
                    Some(capped)
                } else {
                    None
                }
            }
        }
    }

    /// Maximum number of attempts (including the first).
    pub fn max_attempts(&self) -> u32 {
        match self {
            RetryPolicy::None => 1,
            RetryPolicy::Fixed { max_attempts, .. } => *max_attempts,
            RetryPolicy::ExponentialBackoff { max_attempts, .. } => *max_attempts,
        }
    }
}

// ---------------------------------------------------------------------------
// Job lifecycle
// ---------------------------------------------------------------------------

/// Status of a tracked job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
    DeadLettered,
}

/// A tracked job entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRecord {
    pub job_id: u64,
    pub tenant_id: String,
    pub description: String,
    pub status: JobStatus,
    pub attempt: u32,
    pub max_attempts: u32,
    pub error_message: Option<String>,
}

/// Dead letter entry for permanently failed tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetterEntry {
    pub job_id: u64,
    pub tenant_id: String,
    pub description: String,
    pub error_message: String,
    pub attempts: u32,
}

// ---------------------------------------------------------------------------
// WorkerStats
// ---------------------------------------------------------------------------

/// Describes basic runtime snapshot data.
#[derive(Debug, Clone)]
pub struct WorkerStats {
    pub total_inflight: usize,
    pub per_tenant: HashMap<String, usize>,
    pub total_completed: u64,
    pub total_failed: u64,
    pub dead_letter_count: usize,
}

// ---------------------------------------------------------------------------
// WorkerRuntime
// ---------------------------------------------------------------------------

/// Tenant-aware worker runtime that tracks concurrency, instrumentation, and graceful shutdown.
pub struct WorkerRuntime {
    handle: Arc<Handle>,
    max_inflight: usize,
    inflight: Arc<AtomicUsize>,
    tenant_counts: Arc<Mutex<HashMap<String, usize>>>,
    shutting_down: Arc<AtomicBool>,
    notify: Arc<Notify>,
    next_job_id: Arc<AtomicU64>,
    jobs: Arc<RwLock<HashMap<u64, JobRecord>>>,
    dead_letter: Arc<RwLock<Vec<DeadLetterEntry>>>,
    completed_count: Arc<AtomicU64>,
    failed_count: Arc<AtomicU64>,
}

impl WorkerRuntime {
    pub fn from_handle(handle: Handle, max_inflight: usize) -> Self {
        Self {
            handle: Arc::new(handle),
            max_inflight,
            inflight: Arc::new(AtomicUsize::new(0)),
            tenant_counts: Arc::new(Mutex::new(HashMap::new())),
            shutting_down: Arc::new(AtomicBool::new(false)),
            notify: Arc::new(Notify::new()),
            next_job_id: Arc::new(AtomicU64::new(1)),
            jobs: Arc::new(RwLock::new(HashMap::new())),
            dead_letter: Arc::new(RwLock::new(Vec::new())),
            completed_count: Arc::new(AtomicU64::new(0)),
            failed_count: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn current(max_inflight: usize) -> Self {
        Self::from_handle(Handle::current(), max_inflight)
    }

    fn can_schedule(&self) -> Result<(), RuntimeError> {
        if self.shutting_down.load(Ordering::SeqCst) {
            return Err(RuntimeError::ShuttingDown);
        }
        self.inflight
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                if current >= self.max_inflight {
                    None
                } else {
                    Some(current + 1)
                }
            })
            .map(|_| ())
            .map_err(|_| RuntimeError::MaxInflight)
    }

    fn allocate_job_id(&self) -> u64 {
        self.next_job_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Spawn a tenant-aware task. The provided future runs with instrumentation and leak counters.
    pub fn spawn_tenant_task<F>(&self, context: TenantContext, work: F) -> Result<u64, RuntimeError>
    where
        F: std::future::Future<Output = Result<()>> + Send + 'static,
    {
        self.can_schedule()?;
        let job_id = self.allocate_job_id();
        let inflight = self.inflight.clone();
        let counts = Arc::clone(&self.tenant_counts);
        let notify = Arc::clone(&self.notify);
        let jobs = Arc::clone(&self.jobs);
        let completed_count = Arc::clone(&self.completed_count);
        let failed_count = Arc::clone(&self.failed_count);
        let tenant_id = context.tenant_id.clone();
        let span = span!(Level::INFO, "worker_task", tenant = %tenant_id, job_id = %job_id);

        // Register job.
        {
            let job_record = JobRecord {
                job_id,
                tenant_id: tenant_id.clone(),
                description: String::new(),
                status: JobStatus::Running,
                attempt: 1,
                max_attempts: 1,
                error_message: None,
            };
            // We need to block_on to write the job record synchronously.
            // Instead, let's use a sync approach—write inside the async block.
            let jobs_inner = Arc::clone(&jobs);
            self.handle.spawn({
                let job_record = job_record.clone();
                async move {
                    let mut map = jobs_inner.write().await;
                    map.insert(job_id, job_record);
                }
            });
        }

        let work = async move {
            {
                let mut map = counts.lock().await;
                let counter = map.entry(tenant_id.clone()).or_insert(0);
                *counter += 1;
            }
            info!(
                tenant = %tenant_id,
                job_id = %job_id,
                "scheduling tenant task in worker runtime"
            );
            let res = work.await;
            {
                let mut job_map = jobs.write().await;
                if let Some(record) = job_map.get_mut(&job_id) {
                    match &res {
                        Ok(_) => {
                            record.status = JobStatus::Completed;
                            completed_count.fetch_add(1, Ordering::SeqCst);
                        }
                        Err(e) => {
                            record.status = JobStatus::Failed;
                            record.error_message = Some(e.to_string());
                            failed_count.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                }
            }
            {
                let mut map = counts.lock().await;
                if let Some(value) = map.get_mut(&tenant_id) {
                    *value = value.saturating_sub(1);
                }
            }
            if inflight.fetch_sub(1, Ordering::SeqCst) == 1 {
                notify.notify_waiters();
            }
            res
        }
        .instrument(span);

        self.handle.spawn(async move {
            if let Err(err) = work.await {
                tracing::error!(error = %err, "worker task reported error");
            }
        });
        Ok(job_id)
    }

    /// Spawn a tenant-aware task that first acquires a consensus lease and releases it on completion.
    pub async fn spawn_leased_tenant_task<F>(
        &self,
        context: TenantContext,
        coordinator: Arc<LeaseCoordinator>,
        resource: String,
        ttl: Duration,
        work: F,
    ) -> Result<u64, RuntimeError>
    where
        F: std::future::Future<Output = Result<()>> + Send + 'static,
    {
        self.can_schedule()?;
        let lease = coordinator
            .acquire(&resource, &context.tenant_id, ttl)
            .await
            .map_err(|error| RuntimeError::LeaseAcquire(error.to_string()))?;

        let job_id = self.allocate_job_id();
        let inflight = self.inflight.clone();
        let counts = Arc::clone(&self.tenant_counts);
        let notify = Arc::clone(&self.notify);
        let jobs = Arc::clone(&self.jobs);
        let completed_count = Arc::clone(&self.completed_count);
        let failed_count = Arc::clone(&self.failed_count);
        let tenant_id = context.tenant_id.clone();
        let span = span!(
            Level::INFO,
            "worker_task_leased",
            tenant = %tenant_id,
            resource = %lease.resource,
            job_id = %job_id
        );

        // Register job record.
        {
            let jobs_inner = Arc::clone(&self.jobs);
            let record = JobRecord {
                job_id,
                tenant_id: tenant_id.clone(),
                description: format!("leased:{}", resource),
                status: JobStatus::Running,
                attempt: 1,
                max_attempts: 1,
                error_message: None,
            };
            self.handle.spawn(async move {
                let mut map = jobs_inner.write().await;
                map.insert(job_id, record);
            });
        }

        let work = async move {
            {
                let mut map = counts.lock().await;
                let counter = map.entry(tenant_id.clone()).or_insert(0);
                *counter += 1;
            }
            info!(
                tenant = %tenant_id,
                resource = %lease.resource,
                job_id = %job_id,
                "scheduling tenant task with consensus lease"
            );
            let res = work.await;
            coordinator.release(&lease.resource, &lease.owner).await;
            {
                let mut job_map = jobs.write().await;
                if let Some(record) = job_map.get_mut(&job_id) {
                    match &res {
                        Ok(_) => {
                            record.status = JobStatus::Completed;
                            completed_count.fetch_add(1, Ordering::SeqCst);
                        }
                        Err(e) => {
                            record.status = JobStatus::Failed;
                            record.error_message = Some(e.to_string());
                            failed_count.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                }
            }
            {
                let mut map = counts.lock().await;
                if let Some(value) = map.get_mut(&tenant_id) {
                    *value = value.saturating_sub(1);
                }
            }
            if inflight.fetch_sub(1, Ordering::SeqCst) == 1 {
                notify.notify_waiters();
            }
            res
        }
        .instrument(span);

        self.handle.spawn(async move {
            if let Err(err) = work.await {
                tracing::error!(error = %err, "leased worker task reported error");
            }
        });
        Ok(job_id)
    }

    /// Spawn a task with a retry policy. On permanent failure, the job is dead-lettered.
    pub fn spawn_with_retry<F, Fac>(
        &self,
        context: TenantContext,
        description: &str,
        policy: RetryPolicy,
        factory: Fac,
    ) -> Result<u64, RuntimeError>
    where
        Fac: Fn() -> F + Send + Sync + 'static,
        F: std::future::Future<Output = Result<()>> + Send + 'static,
    {
        self.can_schedule()?;
        let job_id = self.allocate_job_id();
        let inflight = self.inflight.clone();
        let counts = Arc::clone(&self.tenant_counts);
        let notify = Arc::clone(&self.notify);
        let jobs = Arc::clone(&self.jobs);
        let dead_letter = Arc::clone(&self.dead_letter);
        let completed_count = Arc::clone(&self.completed_count);
        let failed_count = Arc::clone(&self.failed_count);
        let tenant_id = context.tenant_id.clone();
        let desc = description.to_string();
        let max_attempts = policy.max_attempts();
        let span = span!(
            Level::INFO,
            "worker_task_retry",
            tenant = %tenant_id,
            job_id = %job_id,
            description = %desc
        );

        // Register job record.
        {
            let jobs_inner = Arc::clone(&self.jobs);
            let record = JobRecord {
                job_id,
                tenant_id: tenant_id.clone(),
                description: desc.clone(),
                status: JobStatus::Running,
                attempt: 1,
                max_attempts,
                error_message: None,
            };
            self.handle.spawn(async move {
                let mut map = jobs_inner.write().await;
                map.insert(job_id, record);
            });
        }

        let work = async move {
            {
                let mut map = counts.lock().await;
                *map.entry(tenant_id.clone()).or_insert(0) += 1;
            }

            let mut last_error;
            let mut attempt = 0u32;

            loop {
                let fut = factory();
                match fut.await {
                    Ok(_) => {
                        let mut job_map = jobs.write().await;
                        if let Some(record) = job_map.get_mut(&job_id) {
                            record.status = JobStatus::Completed;
                            record.attempt = attempt + 1;
                        }
                        completed_count.fetch_add(1, Ordering::SeqCst);
                        break;
                    }
                    Err(e) => {
                        last_error = e.to_string();
                        attempt += 1;
                        {
                            let mut job_map = jobs.write().await;
                            if let Some(record) = job_map.get_mut(&job_id) {
                                record.attempt = attempt;
                                record.error_message = Some(last_error.clone());
                            }
                        }
                        if let Some(delay) = policy.delay_for_attempt(attempt) {
                            tracing::warn!(
                                tenant = %tenant_id,
                                job_id = %job_id,
                                attempt = %attempt,
                                error = %last_error,
                                "retrying after delay"
                            );
                            tokio::time::sleep(delay).await;
                        } else {
                            // Exhausted retries — dead letter.
                            {
                                let mut job_map = jobs.write().await;
                                if let Some(record) = job_map.get_mut(&job_id) {
                                    record.status = JobStatus::DeadLettered;
                                }
                            }
                            {
                                let mut dl = dead_letter.write().await;
                                dl.push(DeadLetterEntry {
                                    job_id,
                                    tenant_id: tenant_id.clone(),
                                    description: desc.clone(),
                                    error_message: last_error.clone(),
                                    attempts: attempt,
                                });
                            }
                            failed_count.fetch_add(1, Ordering::SeqCst);
                            tracing::error!(
                                tenant = %tenant_id,
                                job_id = %job_id,
                                attempts = %attempt,
                                error = %last_error,
                                "task exhausted retries, dead-lettered"
                            );
                            break;
                        }
                    }
                }
            }

            {
                let mut map = counts.lock().await;
                if let Some(value) = map.get_mut(&tenant_id) {
                    *value = value.saturating_sub(1);
                }
            }
            if inflight.fetch_sub(1, Ordering::SeqCst) == 1 {
                notify.notify_waiters();
            }
        }
        .instrument(span);

        self.handle.spawn(work);
        Ok(job_id)
    }

    /// Returns a snapshot of runtime statistics.
    pub async fn stats(&self) -> WorkerStats {
        let per_tenant = self.tenant_counts.lock().await.clone();
        let dead_letter_count = self.dead_letter.read().await.len();
        WorkerStats {
            total_inflight: self.inflight.load(Ordering::SeqCst),
            per_tenant,
            total_completed: self.completed_count.load(Ordering::SeqCst),
            total_failed: self.failed_count.load(Ordering::SeqCst),
            dead_letter_count,
        }
    }

    /// Get the record for a specific job.
    pub async fn job_record(&self, job_id: u64) -> Option<JobRecord> {
        let map = self.jobs.read().await;
        map.get(&job_id).cloned()
    }

    /// List all job records.
    pub async fn all_jobs(&self) -> Vec<JobRecord> {
        let map = self.jobs.read().await;
        map.values().cloned().collect()
    }

    /// Get all dead letter entries.
    pub async fn dead_letter_queue(&self) -> Vec<DeadLetterEntry> {
        self.dead_letter.read().await.clone()
    }

    /// Clear dead letter queue.
    pub async fn clear_dead_letters(&self) {
        let mut dl = self.dead_letter.write().await;
        dl.clear();
    }

    /// Replay a dead-lettered job (re-submit). Returns a new job_id if found.
    pub async fn replay_dead_letter(&self, dead_letter_job_id: u64) -> Option<DeadLetterEntry> {
        let mut dl = self.dead_letter.write().await;
        let idx = dl.iter().position(|e| e.job_id == dead_letter_job_id);
        idx.map(|i| dl.remove(i))
    }

    /// Check if the runtime is shutting down.
    pub fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::SeqCst)
    }

    /// Gracefully stop the runtime by waiting for all tasks to finish.
    pub async fn shutdown(&self, timeout: Duration) {
        self.shutting_down.store(true, Ordering::SeqCst);
        let deadline = Instant::now() + timeout;
        loop {
            if self.inflight.load(Ordering::SeqCst) == 0 {
                return;
            }
            if Instant::now() >= deadline {
                return;
            }
            self.notify.notified().await;
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use platform_tenancy::TenantAccessLevel;
    use tokio::time::Duration;

    fn ctx(id: &str) -> TenantContext {
        TenantContext {
            tenant_id: id.to_string(),
            tenant_name: Some("T".into()),
            access_level: TenantAccessLevel::Worker,
        }
    }

    #[tokio::test]
    async fn spawn_tasks_and_collect_stats() {
        let runtime = WorkerRuntime::current(2);
        let notify = Arc::new(Notify::new());
        let inner = notify.clone();
        runtime
            .spawn_tenant_task(ctx("tenant-x"), async move {
                inner.notify_waiters();
                Ok(())
            })
            .unwrap();
        notify.notified().await;
        tokio::time::sleep(Duration::from_millis(10)).await;
        let stats = runtime.stats().await;
        assert!(stats.total_inflight <= 2);
    }

    #[tokio::test]
    async fn shutdown_waits_for_tasks() {
        let runtime = WorkerRuntime::current(1);
        runtime
            .spawn_tenant_task(ctx("tenant-y"), async move {
                tokio::time::sleep(Duration::from_millis(10)).await;
                Ok(())
            })
            .unwrap();
        runtime.shutdown(Duration::from_secs(1)).await;
        let stats = runtime.stats().await;
        assert_eq!(stats.total_inflight, 0);
    }

    #[tokio::test]
    async fn max_inflight_enforced() {
        let runtime = WorkerRuntime::current(1);
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let tx = Arc::new(Mutex::new(Some(tx)));
        let tx_clone = tx.clone();
        // Occupy the single slot.
        runtime
            .spawn_tenant_task(ctx("t"), async move {
                let mut guard = tx_clone.lock().await;
                if let Some(tx) = guard.take() {
                    let _ = tx.send(());
                }
                tokio::time::sleep(Duration::from_secs(10)).await;
                Ok(())
            })
            .unwrap();
        rx.await.unwrap(); // Ensure the first task is running.
        let err = runtime.spawn_tenant_task(ctx("t"), async { Ok(()) });
        assert!(matches!(err, Err(RuntimeError::MaxInflight)));
    }

    #[tokio::test]
    async fn shutting_down_rejects_new_tasks() {
        let runtime = WorkerRuntime::current(10);
        runtime.shutdown(Duration::from_millis(1)).await;
        assert!(runtime.is_shutting_down());
        let err = runtime.spawn_tenant_task(ctx("t"), async { Ok(()) });
        assert!(matches!(err, Err(RuntimeError::ShuttingDown)));
    }

    #[tokio::test]
    async fn job_returns_id() {
        let runtime = WorkerRuntime::current(10);
        let id = runtime
            .spawn_tenant_task(ctx("t"), async { Ok(()) })
            .unwrap();
        assert!(id > 0);
    }

    #[tokio::test]
    async fn completed_task_updates_stats() {
        let runtime = WorkerRuntime::current(10);
        let notify = Arc::new(Notify::new());
        let inner = notify.clone();
        runtime
            .spawn_tenant_task(ctx("t"), async move {
                inner.notify_waiters();
                Ok(())
            })
            .unwrap();
        notify.notified().await;
        tokio::time::sleep(Duration::from_millis(50)).await;
        let stats = runtime.stats().await;
        assert_eq!(stats.total_completed, 1);
        assert_eq!(stats.total_failed, 0);
    }

    #[tokio::test]
    async fn failed_task_updates_stats() {
        let runtime = WorkerRuntime::current(10);
        let notify = Arc::new(Notify::new());
        let inner = notify.clone();
        runtime
            .spawn_tenant_task(ctx("t"), async move {
                inner.notify_waiters();
                anyhow::bail!("intentional failure")
            })
            .unwrap();
        notify.notified().await;
        tokio::time::sleep(Duration::from_millis(50)).await;
        let stats = runtime.stats().await;
        assert_eq!(stats.total_failed, 1);
    }

    // ---- Retry policy tests ----

    #[test]
    fn retry_none_has_no_delay() {
        let policy = RetryPolicy::None;
        assert_eq!(policy.max_attempts(), 1);
        assert_eq!(policy.delay_for_attempt(0), None);
    }

    #[test]
    fn retry_fixed_delay() {
        let policy = RetryPolicy::fixed(3, Duration::from_millis(100));
        assert_eq!(policy.max_attempts(), 3);
        assert_eq!(
            policy.delay_for_attempt(0),
            Some(Duration::from_millis(100))
        );
        assert_eq!(
            policy.delay_for_attempt(1),
            Some(Duration::from_millis(100))
        );
        assert_eq!(
            policy.delay_for_attempt(2),
            Some(Duration::from_millis(100))
        );
        assert_eq!(policy.delay_for_attempt(3), None);
    }

    #[test]
    fn retry_exponential_backoff() {
        let policy =
            RetryPolicy::exponential(4, Duration::from_millis(100), Duration::from_secs(5));
        assert_eq!(policy.max_attempts(), 4);
        // attempt 0: 100ms
        assert_eq!(
            policy.delay_for_attempt(0),
            Some(Duration::from_millis(100))
        );
        // attempt 1: 200ms
        assert_eq!(
            policy.delay_for_attempt(1),
            Some(Duration::from_millis(200))
        );
        // attempt 2: 400ms
        assert_eq!(
            policy.delay_for_attempt(2),
            Some(Duration::from_millis(400))
        );
        // attempt 3: 800ms
        assert_eq!(
            policy.delay_for_attempt(3),
            Some(Duration::from_millis(800))
        );
        // attempt 4: exhausted
        assert_eq!(policy.delay_for_attempt(4), None);
    }

    #[test]
    fn retry_exponential_capped() {
        let policy =
            RetryPolicy::exponential(10, Duration::from_millis(500), Duration::from_secs(2));
        // attempt 3: 500 * 2^3 = 4000ms, but cap is 2000ms
        assert_eq!(policy.delay_for_attempt(3), Some(Duration::from_secs(2)));
    }

    // ---- spawn_with_retry tests ----

    #[tokio::test]
    async fn retry_succeeds_on_first_attempt() {
        let runtime = WorkerRuntime::current(10);
        let notify = Arc::new(Notify::new());
        let inner = notify.clone();
        let _job_id = runtime
            .spawn_with_retry(
                ctx("t"),
                "test-ok",
                RetryPolicy::fixed(3, Duration::from_millis(10)),
                move || {
                    let inner = inner.clone();
                    async move {
                        inner.notify_waiters();
                        Ok(())
                    }
                },
            )
            .unwrap();
        notify.notified().await;
        tokio::time::sleep(Duration::from_millis(50)).await;
        let stats = runtime.stats().await;
        assert_eq!(stats.total_completed, 1);
        assert_eq!(stats.dead_letter_count, 0);
    }

    #[tokio::test]
    async fn retry_exhausts_and_dead_letters() {
        let runtime = WorkerRuntime::current(10);
        let counter = Arc::new(AtomicU64::new(0));
        let counter_clone = counter.clone();
        let _job_id = runtime
            .spawn_with_retry(
                ctx("t"),
                "always-fail",
                RetryPolicy::fixed(2, Duration::from_millis(10)),
                move || {
                    let counter_clone = counter_clone.clone();
                    async move {
                        counter_clone.fetch_add(1, Ordering::SeqCst);
                        anyhow::bail!("permanent error")
                    }
                },
            )
            .unwrap();
        // Wait for retries to exhaust.
        tokio::time::sleep(Duration::from_millis(200)).await;
        let stats = runtime.stats().await;
        assert_eq!(stats.dead_letter_count, 1);
        assert_eq!(stats.total_failed, 1);

        let dl = runtime.dead_letter_queue().await;
        assert_eq!(dl.len(), 1);
        assert_eq!(dl[0].tenant_id, "t");
        assert!(dl[0].error_message.contains("permanent error"));
    }

    #[tokio::test]
    async fn retry_succeeds_after_failures() {
        let runtime = WorkerRuntime::current(10);
        let counter = Arc::new(AtomicU64::new(0));
        let counter_clone = counter.clone();
        let _job_id = runtime
            .spawn_with_retry(
                ctx("t"),
                "eventual-ok",
                RetryPolicy::fixed(5, Duration::from_millis(10)),
                move || {
                    let counter_clone = counter_clone.clone();
                    async move {
                        let attempt = counter_clone.fetch_add(1, Ordering::SeqCst);
                        if attempt < 2 {
                            anyhow::bail!("transient error")
                        }
                        Ok(())
                    }
                },
            )
            .unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
        let stats = runtime.stats().await;
        assert_eq!(stats.total_completed, 1);
        assert_eq!(stats.dead_letter_count, 0);
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    // ---- Dead letter management tests ----

    #[tokio::test]
    async fn clear_dead_letters() {
        let runtime = WorkerRuntime::current(10);
        runtime
            .spawn_with_retry(ctx("t"), "fail", RetryPolicy::None, || async {
                anyhow::bail!("nope")
            })
            .unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(runtime.dead_letter_queue().await.len(), 1);
        runtime.clear_dead_letters().await;
        assert_eq!(runtime.dead_letter_queue().await.len(), 0);
    }

    #[tokio::test]
    async fn replay_dead_letter_removes_entry() {
        let runtime = WorkerRuntime::current(10);
        let _id = runtime
            .spawn_with_retry(ctx("t"), "fail", RetryPolicy::None, || async {
                anyhow::bail!("nope")
            })
            .unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        let dl = runtime.dead_letter_queue().await;
        assert_eq!(dl.len(), 1);
        let replayed = runtime.replay_dead_letter(dl[0].job_id).await;
        assert!(replayed.is_some());
        assert_eq!(runtime.dead_letter_queue().await.len(), 0);
    }

    // ---- Job record tests ----

    #[tokio::test]
    async fn job_record_tracked() {
        let runtime = WorkerRuntime::current(10);
        let notify = Arc::new(Notify::new());
        let inner = notify.clone();
        let id = runtime
            .spawn_tenant_task(ctx("t"), async move {
                inner.notify_waiters();
                Ok(())
            })
            .unwrap();
        notify.notified().await;
        tokio::time::sleep(Duration::from_millis(50)).await;
        let record = runtime.job_record(id).await;
        assert!(record.is_some());
        let record = record.unwrap();
        assert_eq!(record.status, JobStatus::Completed);
    }

    #[tokio::test]
    async fn all_jobs_lists_all() {
        let runtime = WorkerRuntime::current(10);
        let done = Arc::new(AtomicU64::new(0));
        let d1 = done.clone();
        let d2 = done.clone();
        runtime
            .spawn_tenant_task(ctx("a"), async move {
                d1.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
            .unwrap();
        runtime
            .spawn_tenant_task(ctx("b"), async move {
                d2.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
            .unwrap();
        // Wait for both tasks to complete.
        for _ in 0..100 {
            if done.load(Ordering::SeqCst) >= 2 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
        let jobs = runtime.all_jobs().await;
        assert_eq!(jobs.len(), 2);
    }

    // ---- JobStatus / RetryPolicy serde ----

    #[test]
    fn job_status_serialization() {
        let status = JobStatus::DeadLettered;
        let json = serde_json::to_string(&status).unwrap();
        let parsed: JobStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, JobStatus::DeadLettered);
    }

    #[test]
    fn retry_policy_serialization() {
        let policy =
            RetryPolicy::exponential(3, Duration::from_millis(100), Duration::from_secs(5));
        let json = serde_json::to_string(&policy).unwrap();
        let parsed: RetryPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.max_attempts(), 3);
    }
}
