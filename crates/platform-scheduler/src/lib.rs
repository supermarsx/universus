//! Tenant-aware scheduler that registers jobs, validates shard placement, and triggers
//! work using `platform-tenant-routing`/`platform-sharding`.
//!
//! Features:
//! - Register recurring and one-shot jobs with interval-based scheduling
//! - Job priority (higher priority runs first)
//! - Job pausing/resuming
//! - Job removal and listing
//! - Execution history with success/failure tracking
//! - Shard validation before execution
//! - Consensus lease integration via tenant routing
//! - Tick-based scheduler loop (process all due jobs)
//! - Configurable max retries per job

#![forbid(unsafe_code)]

use anyhow::Result;
use platform_sharding::{ShardSummary, ShardingCatalog};
use platform_tenancy::{TenantAccessLevel, TenantContext};
use platform_tenant_routing::{RoutingError, TenantRouter, TenantRoutingDecision};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::RwLock;
use tokio::time::Instant;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

pub type JobHandler = Arc<dyn Fn(TenantRoutingDecision) -> JobFuture + Send + Sync>;

pub type JobFuture = Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>>;

/// Whether the job repeats or runs once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobKind {
    /// Recurring job that fires every `interval`.
    Recurring,
    /// Fires once and is automatically removed after execution.
    OneShot,
}

/// Current lifecycle state of a registered job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobState {
    /// Job is active and will be considered on the next tick.
    Active,
    /// Job is paused and will not run until resumed.
    Paused,
    /// One-shot job that has completed its single execution.
    Completed,
}

/// Configuration for a scheduled job.
#[derive(Debug, Clone)]
pub struct JobConfig {
    pub job_id: String,
    pub tenant_id: String,
    pub description: String,
    pub shard_id: String,
    pub interval: Duration,
    /// Job kind — recurring or one-shot. Default: Recurring.
    pub kind: JobKind,
    /// Priority (higher = runs first in a tick). Default: 100.
    pub priority: u32,
    /// Maximum consecutive failures before the job is auto-paused. 0 = unlimited.
    pub max_failures: u32,
}

impl JobConfig {
    /// Create a basic recurring job config.
    pub fn recurring(job_id: &str, tenant_id: &str, shard_id: &str, interval: Duration) -> Self {
        Self {
            job_id: job_id.into(),
            tenant_id: tenant_id.into(),
            description: String::new(),
            shard_id: shard_id.into(),
            interval,
            kind: JobKind::Recurring,
            priority: 100,
            max_failures: 0,
        }
    }

    /// Create a one-shot job config.
    pub fn one_shot(job_id: &str, tenant_id: &str, shard_id: &str) -> Self {
        Self {
            job_id: job_id.into(),
            tenant_id: tenant_id.into(),
            description: String::new(),
            shard_id: shard_id.into(),
            interval: Duration::ZERO,
            kind: JobKind::OneShot,
            priority: 100,
            max_failures: 0,
        }
    }

    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = desc.into();
        self
    }

    pub fn with_priority(mut self, p: u32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_max_failures(mut self, n: u32) -> Self {
        self.max_failures = n;
        self
    }
}

// ---------------------------------------------------------------------------
// Execution history
// ---------------------------------------------------------------------------

/// Outcome of a single job execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub job_id: String,
    pub tenant_id: String,
    pub success: bool,
    pub error_message: Option<String>,
    pub timestamp: String,
}

// ---------------------------------------------------------------------------
// Job snapshot
// ---------------------------------------------------------------------------

/// Read-only snapshot of a scheduled job for external queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSnapshot {
    pub job_id: String,
    pub tenant_id: String,
    pub description: String,
    pub shard_id: String,
    pub kind: JobKind,
    pub state: JobState,
    pub priority: u32,
    pub interval_secs: u64,
    pub total_runs: u64,
    pub total_successes: u64,
    pub total_failures: u64,
    pub consecutive_failures: u32,
}

// ---------------------------------------------------------------------------
// Internal job
// ---------------------------------------------------------------------------

struct ScheduledJob {
    config: JobConfig,
    handler: JobHandler,
    state: JobState,
    last_run: Option<Instant>,
    total_runs: u64,
    total_successes: u64,
    total_failures: u64,
    consecutive_failures: u32,
}

impl ScheduledJob {
    fn new(config: JobConfig, handler: JobHandler) -> Self {
        Self {
            state: JobState::Active,
            handler,
            last_run: None,
            total_runs: 0,
            total_successes: 0,
            total_failures: 0,
            consecutive_failures: 0,
            config,
        }
    }

    fn is_due(&self) -> bool {
        if self.state != JobState::Active {
            return false;
        }
        match self.config.kind {
            JobKind::OneShot => self.last_run.is_none(),
            JobKind::Recurring => match self.last_run {
                None => true,
                Some(last) => Instant::now().duration_since(last) >= self.config.interval,
            },
        }
    }

    fn make_context(&self) -> TenantContext {
        TenantContext {
            tenant_id: self.config.tenant_id.clone(),
            tenant_name: Some(self.config.description.clone()),
            access_level: TenantAccessLevel::Worker,
        }
    }

    fn snapshot(&self) -> JobSnapshot {
        JobSnapshot {
            job_id: self.config.job_id.clone(),
            tenant_id: self.config.tenant_id.clone(),
            description: self.config.description.clone(),
            shard_id: self.config.shard_id.clone(),
            kind: self.config.kind,
            state: self.state,
            priority: self.config.priority,
            interval_secs: self.config.interval.as_secs(),
            total_runs: self.total_runs,
            total_successes: self.total_successes,
            total_failures: self.total_failures,
            consecutive_failures: self.consecutive_failures,
        }
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum SchedulerError {
    #[error("job {0} already registered")]
    AlreadyRegistered(String),

    #[error("job {0} not found")]
    JobMissing(String),

    #[error("tenant {0} not routed to shard {1}")]
    ShardMismatch(String, String),

    #[error("routing failure: {0}")]
    RoutingFailure(#[from] RoutingError),

    #[error("job handler failure: {0}")]
    HandlerFailure(#[source] anyhow::Error),

    #[error("job {0} is not active (state: {1})")]
    JobNotActive(String, String),
}

// ---------------------------------------------------------------------------
// Scheduler
// ---------------------------------------------------------------------------

pub struct Scheduler {
    tenant_router: Arc<TenantRouter>,
    catalog: Arc<ShardingCatalog>,
    jobs: RwLock<HashMap<String, ScheduledJob>>,
    history: RwLock<Vec<ExecutionRecord>>,
    max_history: usize,
}

impl Scheduler {
    pub fn new(tenant_router: Arc<TenantRouter>, catalog: Arc<ShardingCatalog>) -> Self {
        Self {
            tenant_router,
            catalog,
            jobs: RwLock::new(HashMap::new()),
            history: RwLock::new(Vec::new()),
            max_history: 1000,
        }
    }

    pub fn with_max_history(
        tenant_router: Arc<TenantRouter>,
        catalog: Arc<ShardingCatalog>,
        max_history: usize,
    ) -> Self {
        Self {
            tenant_router,
            catalog,
            jobs: RwLock::new(HashMap::new()),
            history: RwLock::new(Vec::new()),
            max_history,
        }
    }

    // ---- Registration ----

    pub async fn register_job(
        &self,
        config: JobConfig,
        handler: JobHandler,
    ) -> Result<(), SchedulerError> {
        let mut jobs = self.jobs.write().await;
        if jobs.contains_key(&config.job_id) {
            return Err(SchedulerError::AlreadyRegistered(config.job_id));
        }
        jobs.insert(config.job_id.clone(), ScheduledJob::new(config, handler));
        Ok(())
    }

    /// Remove a job by ID. Returns true if the job existed.
    pub async fn remove_job(&self, job_id: &str) -> bool {
        let mut jobs = self.jobs.write().await;
        jobs.remove(job_id).is_some()
    }

    /// Pause a job. Returns error if job not found.
    pub async fn pause_job(&self, job_id: &str) -> Result<(), SchedulerError> {
        let mut jobs = self.jobs.write().await;
        let job = jobs
            .get_mut(job_id)
            .ok_or_else(|| SchedulerError::JobMissing(job_id.into()))?;
        job.state = JobState::Paused;
        Ok(())
    }

    /// Resume a paused job. Returns error if job not found.
    pub async fn resume_job(&self, job_id: &str) -> Result<(), SchedulerError> {
        let mut jobs = self.jobs.write().await;
        let job = jobs
            .get_mut(job_id)
            .ok_or_else(|| SchedulerError::JobMissing(job_id.into()))?;
        if job.state == JobState::Paused {
            job.state = JobState::Active;
            job.consecutive_failures = 0;
        }
        Ok(())
    }

    // ---- Triggering ----

    pub async fn trigger_job(&self, job_id: &str) -> Result<(), SchedulerError> {
        let mut jobs = self.jobs.write().await;
        let job = jobs
            .get_mut(job_id)
            .ok_or_else(|| SchedulerError::JobMissing(job_id.to_string()))?;

        self.verify_shard(&job.config).await?;

        let context = job.make_context();
        let decision = self
            .tenant_router
            .route_request(context)
            .await
            .map_err(SchedulerError::RoutingFailure)?;
        let lease_token = decision.lease_token.clone();

        let handler_result = (job.handler)(decision).await;
        if let Some(token) = lease_token {
            self.tenant_router.release_lease(token).await;
        }

        job.total_runs += 1;
        job.last_run = Some(Instant::now());

        let record = match &handler_result {
            Ok(_) => {
                job.total_successes += 1;
                job.consecutive_failures = 0;
                ExecutionRecord {
                    job_id: job.config.job_id.clone(),
                    tenant_id: job.config.tenant_id.clone(),
                    success: true,
                    error_message: None,
                    timestamp: now_iso(),
                }
            }
            Err(e) => {
                job.total_failures += 1;
                job.consecutive_failures += 1;
                let record = ExecutionRecord {
                    job_id: job.config.job_id.clone(),
                    tenant_id: job.config.tenant_id.clone(),
                    success: false,
                    error_message: Some(e.to_string()),
                    timestamp: now_iso(),
                };
                // Auto-pause on max_failures.
                if job.config.max_failures > 0
                    && job.consecutive_failures >= job.config.max_failures
                {
                    job.state = JobState::Paused;
                }
                record
            }
        };

        // One-shot jobs mark as completed after first run.
        if job.config.kind == JobKind::OneShot {
            job.state = JobState::Completed;
        }

        // Record history.
        {
            let mut history = self.history.write().await;
            history.push(record);
            if history.len() > self.max_history {
                let excess = history.len() - self.max_history;
                history.drain(..excess);
            }
        }

        handler_result.map_err(SchedulerError::HandlerFailure)?;
        Ok(())
    }

    /// Run one tick of the scheduler: find all due jobs, sorted by priority (descending),
    /// and trigger them in order. Returns the number of jobs triggered and any errors.
    pub async fn tick(&self) -> Vec<(String, Result<(), SchedulerError>)> {
        // Collect due jobs.
        let due_jobs: Vec<(String, u32)> = {
            let jobs = self.jobs.read().await;
            let mut due: Vec<_> = jobs
                .values()
                .filter(|j| j.is_due())
                .map(|j| (j.config.job_id.clone(), j.config.priority))
                .collect();
            due.sort_by_key(|entry| std::cmp::Reverse(entry.1)); // descending priority
            due
        };

        let mut results = Vec::new();
        for (job_id, _priority) in due_jobs {
            let result = self.trigger_job(&job_id).await;
            results.push((job_id, result));
        }
        results
    }

    // ---- Query ----

    /// Get a snapshot of a specific job.
    pub async fn job_snapshot(&self, job_id: &str) -> Option<JobSnapshot> {
        let jobs = self.jobs.read().await;
        jobs.get(job_id).map(|j| j.snapshot())
    }

    /// List snapshots of all registered jobs.
    pub async fn list_jobs(&self) -> Vec<JobSnapshot> {
        let jobs = self.jobs.read().await;
        jobs.values().map(|j| j.snapshot()).collect()
    }

    /// List jobs filtered by tenant.
    pub async fn jobs_by_tenant(&self, tenant_id: &str) -> Vec<JobSnapshot> {
        let jobs = self.jobs.read().await;
        jobs.values()
            .filter(|j| j.config.tenant_id == tenant_id)
            .map(|j| j.snapshot())
            .collect()
    }

    /// List jobs filtered by state.
    pub async fn jobs_by_state(&self, state: JobState) -> Vec<JobSnapshot> {
        let jobs = self.jobs.read().await;
        jobs.values()
            .filter(|j| j.state == state)
            .map(|j| j.snapshot())
            .collect()
    }

    /// Number of registered jobs.
    pub async fn job_count(&self) -> usize {
        let jobs = self.jobs.read().await;
        jobs.len()
    }

    /// Number of active (due-eligible) jobs.
    pub async fn active_job_count(&self) -> usize {
        let jobs = self.jobs.read().await;
        jobs.values()
            .filter(|j| j.state == JobState::Active)
            .count()
    }

    // ---- History ----

    /// Get all execution history records.
    pub async fn execution_history(&self) -> Vec<ExecutionRecord> {
        self.history.read().await.clone()
    }

    /// Get execution history for a specific job.
    pub async fn job_history(&self, job_id: &str) -> Vec<ExecutionRecord> {
        self.history
            .read()
            .await
            .iter()
            .filter(|r| r.job_id == job_id)
            .cloned()
            .collect()
    }

    /// Clear all execution history.
    pub async fn clear_history(&self) {
        self.history.write().await.clear();
    }

    // ---- Internal ----

    async fn verify_shard(&self, config: &JobConfig) -> Result<(), SchedulerError> {
        let shard = self.catalog.route_tenant(&config.tenant_id).await;
        match shard {
            Some(ShardSummary { shard_id, .. }) if shard_id == config.shard_id => Ok(()),
            Some(ShardSummary { shard_id, .. }) => Err(SchedulerError::ShardMismatch(
                config.tenant_id.clone(),
                shard_id,
            )),
            None => Err(SchedulerError::ShardMismatch(
                config.tenant_id.clone(),
                config.shard_id.clone(),
            )),
        }
    }
}

fn now_iso() -> String {
    platform_common::iso8601_now()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use platform_consensus::LeaseCoordinator;
    use platform_sharding::ShardConfig;
    use platform_tenant_routing::TenantRouteConfig;
    use std::collections::HashSet;
    use std::iter::FromIterator;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use tokio::sync::Notify;

    // Helper: set up a router + catalog with one shard and one tenant route.
    async fn setup() -> (Arc<TenantRouter>, Arc<ShardingCatalog>) {
        let router = Arc::new(TenantRouter::new());
        let catalog = Arc::new(ShardingCatalog::new());

        catalog
            .register_shard(ShardConfig {
                shard_id: "shard-x".into(),
                region: "eu".into(),
                allowed_tenants: HashSet::from_iter(vec!["tenant1".to_string()]),
                consensus_resource: None,
            })
            .await;

        router
            .register(TenantRouteConfig {
                tenant_id: "tenant1".into(),
                shard_id: "shard-x".into(),
                queue_name: "default".into(),
                region: "eu".into(),
                max_inflight: 10,
                max_per_second: 100,
                consensus_resource: None,
                lease_ttl: Duration::from_secs(5),
            })
            .await;

        (router, catalog)
    }

    fn noop_handler() -> JobHandler {
        Arc::new(|_| Box::pin(async { Ok(()) }))
    }

    fn counting_handler(counter: Arc<AtomicU64>) -> JobHandler {
        Arc::new(move |_| {
            let counter = counter.clone();
            Box::pin(async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
        })
    }

    fn failing_handler(msg: &'static str) -> JobHandler {
        Arc::new(move |_| Box::pin(async move { anyhow::bail!(msg) }))
    }

    // ---- Basic registration & triggering (backward compat) ----

    #[tokio::test]
    async fn registers_and_triggers_job() {
        let (router, catalog) = setup().await;
        let scheduler = Scheduler::new(router, catalog);

        let marker = Arc::new(tokio::sync::Mutex::new(false));
        let handler_marker = marker.clone();
        let handler: JobHandler = Arc::new(move |_| {
            let handler_marker = handler_marker.clone();
            Box::pin(async move {
                let mut lock = handler_marker.lock().await;
                *lock = true;
                Ok(())
            })
        });

        let job = JobConfig {
            job_id: "job-a".into(),
            tenant_id: "tenant1".into(),
            description: "task".into(),
            shard_id: "shard-x".into(),
            interval: Duration::from_secs(60),
            kind: JobKind::Recurring,
            priority: 100,
            max_failures: 0,
        };
        scheduler.register_job(job, handler).await.unwrap();
        scheduler.trigger_job("job-a").await.unwrap();
        assert!(*marker.lock().await);
    }

    #[tokio::test]
    async fn shard_mismatch_blocks_job() {
        let router = Arc::new(TenantRouter::new());
        let catalog = Arc::new(ShardingCatalog::new());

        catalog
            .register_shard(ShardConfig {
                shard_id: "shard-other".into(),
                region: "eu".into(),
                allowed_tenants: HashSet::from_iter(vec!["tenant2".to_string()]),
                consensus_resource: None,
            })
            .await;

        let scheduler = Scheduler::new(router.clone(), catalog);
        router
            .register(TenantRouteConfig {
                tenant_id: "tenant1".into(),
                shard_id: "shard-other".into(),
                queue_name: "default".into(),
                region: "eu".into(),
                max_inflight: 1,
                max_per_second: 10,
                consensus_resource: None,
                lease_ttl: Duration::from_secs(5),
            })
            .await;

        let job = JobConfig {
            job_id: "job-b".into(),
            tenant_id: "tenant1".into(),
            description: "task".into(),
            shard_id: "shard-x".into(),
            interval: Duration::from_secs(30),
            kind: JobKind::Recurring,
            priority: 100,
            max_failures: 0,
        };
        scheduler.register_job(job, noop_handler()).await.unwrap();
        let err = scheduler.trigger_job("job-b").await.unwrap_err();
        assert!(matches!(err, SchedulerError::ShardMismatch(_, _)));
    }

    #[tokio::test]
    async fn lease_contention_then_failover_between_schedulers() {
        let lease_coordinator = Arc::new(LeaseCoordinator::new());
        let tenant_router = Arc::new(TenantRouter::with_leases(Arc::clone(&lease_coordinator)));
        let catalog = Arc::new(ShardingCatalog::new());

        catalog
            .register_shard(ShardConfig {
                shard_id: "shard-x".into(),
                region: "eu".into(),
                allowed_tenants: HashSet::from_iter(vec!["tenant1".to_string()]),
                consensus_resource: None,
            })
            .await;

        tenant_router
            .register(TenantRouteConfig {
                tenant_id: "tenant1".into(),
                shard_id: "shard-x".into(),
                queue_name: "default".into(),
                region: "eu".into(),
                max_inflight: 2,
                max_per_second: 10,
                consensus_resource: Some("tenant:tenant1:job".into()),
                lease_ttl: Duration::from_secs(10),
            })
            .await;

        let scheduler_a = Arc::new(Scheduler::new(
            Arc::clone(&tenant_router),
            Arc::clone(&catalog),
        ));
        let scheduler_b = Arc::new(Scheduler::new(
            Arc::clone(&tenant_router),
            Arc::clone(&catalog),
        ));

        let job_a = JobConfig {
            job_id: "job-a".into(),
            tenant_id: "tenant1".into(),
            description: "blocking task".into(),
            shard_id: "shard-x".into(),
            interval: Duration::from_secs(60),
            kind: JobKind::Recurring,
            priority: 100,
            max_failures: 0,
        };
        let job_b = JobConfig {
            job_id: "job-b".into(),
            tenant_id: "tenant1".into(),
            description: "follow-up task".into(),
            shard_id: "shard-x".into(),
            interval: Duration::from_secs(60),
            kind: JobKind::Recurring,
            priority: 100,
            max_failures: 0,
        };

        let started = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let started_handler = Arc::clone(&started);
        let release_handler = Arc::clone(&release);
        let handler_a: JobHandler = Arc::new(move |_| {
            let started_handler = Arc::clone(&started_handler);
            let release_handler = Arc::clone(&release_handler);
            Box::pin(async move {
                started_handler.notify_waiters();
                release_handler.notified().await;
                Ok(())
            })
        });

        let ran_job_b = Arc::new(AtomicBool::new(false));
        let ran_job_b_handler = Arc::clone(&ran_job_b);
        let handler_b: JobHandler = Arc::new(move |_| {
            let ran_job_b_handler = Arc::clone(&ran_job_b_handler);
            Box::pin(async move {
                ran_job_b_handler.store(true, Ordering::SeqCst);
                Ok(())
            })
        });

        scheduler_a.register_job(job_a, handler_a).await.unwrap();
        scheduler_b.register_job(job_b, handler_b).await.unwrap();

        let scheduler_a_task = {
            let scheduler = Arc::clone(&scheduler_a);
            tokio::spawn(async move { scheduler.trigger_job("job-a").await })
        };
        started.notified().await;

        let err = scheduler_b.trigger_job("job-b").await.unwrap_err();
        assert!(matches!(
            err,
            SchedulerError::RoutingFailure(RoutingError::LeaseFailure(_))
        ));

        release.notify_waiters();
        scheduler_a_task.await.unwrap().unwrap();

        scheduler_b.trigger_job("job-b").await.unwrap();
        assert!(ran_job_b.load(Ordering::SeqCst));
    }

    // ---- New tests ----

    #[tokio::test]
    async fn duplicate_registration_errors() {
        let (router, catalog) = setup().await;
        let scheduler = Scheduler::new(router, catalog);
        let job = JobConfig::recurring("j1", "tenant1", "shard-x", Duration::from_secs(60));
        scheduler
            .register_job(job.clone(), noop_handler())
            .await
            .unwrap();
        let err = scheduler
            .register_job(job, noop_handler())
            .await
            .unwrap_err();
        assert!(matches!(err, SchedulerError::AlreadyRegistered(_)));
    }

    #[tokio::test]
    async fn remove_job() {
        let (router, catalog) = setup().await;
        let scheduler = Scheduler::new(router, catalog);
        let job = JobConfig::recurring("j1", "tenant1", "shard-x", Duration::from_secs(60));
        scheduler.register_job(job, noop_handler()).await.unwrap();
        assert!(scheduler.remove_job("j1").await);
        assert!(!scheduler.remove_job("j1").await);
        assert_eq!(scheduler.job_count().await, 0);
    }

    #[tokio::test]
    async fn trigger_missing_job_errors() {
        let (router, catalog) = setup().await;
        let scheduler = Scheduler::new(router, catalog);
        let err = scheduler.trigger_job("ghost").await.unwrap_err();
        assert!(matches!(err, SchedulerError::JobMissing(_)));
    }

    #[tokio::test]
    async fn pause_and_resume() {
        let (router, catalog) = setup().await;
        let scheduler = Scheduler::new(router, catalog);
        let job = JobConfig::recurring("j1", "tenant1", "shard-x", Duration::from_secs(60));
        scheduler.register_job(job, noop_handler()).await.unwrap();

        scheduler.pause_job("j1").await.unwrap();
        let snap = scheduler.job_snapshot("j1").await.unwrap();
        assert_eq!(snap.state, JobState::Paused);

        scheduler.resume_job("j1").await.unwrap();
        let snap = scheduler.job_snapshot("j1").await.unwrap();
        assert_eq!(snap.state, JobState::Active);
    }

    #[tokio::test]
    async fn pause_missing_job_errors() {
        let (router, catalog) = setup().await;
        let scheduler = Scheduler::new(router, catalog);
        assert!(matches!(
            scheduler.pause_job("nope").await,
            Err(SchedulerError::JobMissing(_))
        ));
    }

    #[tokio::test]
    async fn one_shot_job_completes_after_run() {
        let (router, catalog) = setup().await;
        let scheduler = Scheduler::new(router, catalog);
        let counter = Arc::new(AtomicU64::new(0));
        let job = JobConfig::one_shot("j1", "tenant1", "shard-x");
        scheduler
            .register_job(job, counting_handler(counter.clone()))
            .await
            .unwrap();

        scheduler.trigger_job("j1").await.unwrap();
        let snap = scheduler.job_snapshot("j1").await.unwrap();
        assert_eq!(snap.state, JobState::Completed);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn execution_history_recorded() {
        let (router, catalog) = setup().await;
        let scheduler = Scheduler::new(router, catalog);
        let job = JobConfig::recurring("j1", "tenant1", "shard-x", Duration::from_secs(60));
        scheduler.register_job(job, noop_handler()).await.unwrap();

        scheduler.trigger_job("j1").await.unwrap();
        scheduler.trigger_job("j1").await.unwrap();

        let history = scheduler.execution_history().await;
        assert_eq!(history.len(), 2);
        assert!(history.iter().all(|r| r.success));
    }

    #[tokio::test]
    async fn execution_history_records_failures() {
        let (router, catalog) = setup().await;
        let scheduler = Scheduler::new(router, catalog);
        let job = JobConfig::recurring("j1", "tenant1", "shard-x", Duration::from_secs(60));
        scheduler
            .register_job(job, failing_handler("oops"))
            .await
            .unwrap();

        let _ = scheduler.trigger_job("j1").await;
        let history = scheduler.execution_history().await;
        assert_eq!(history.len(), 1);
        assert!(!history[0].success);
        assert!(history[0].error_message.as_ref().unwrap().contains("oops"));
    }

    #[tokio::test]
    async fn job_history_filtered_by_id() {
        let (router, catalog) = setup().await;
        let scheduler = Scheduler::new(router, catalog);
        let j1 = JobConfig::recurring("j1", "tenant1", "shard-x", Duration::from_secs(60));
        let j2 = JobConfig::recurring("j2", "tenant1", "shard-x", Duration::from_secs(60));
        scheduler.register_job(j1, noop_handler()).await.unwrap();
        scheduler.register_job(j2, noop_handler()).await.unwrap();

        scheduler.trigger_job("j1").await.unwrap();
        scheduler.trigger_job("j2").await.unwrap();
        scheduler.trigger_job("j1").await.unwrap();

        let h1 = scheduler.job_history("j1").await;
        assert_eq!(h1.len(), 2);
        let h2 = scheduler.job_history("j2").await;
        assert_eq!(h2.len(), 1);
    }

    #[tokio::test]
    async fn clear_history() {
        let (router, catalog) = setup().await;
        let scheduler = Scheduler::new(router, catalog);
        let job = JobConfig::recurring("j1", "tenant1", "shard-x", Duration::from_secs(60));
        scheduler.register_job(job, noop_handler()).await.unwrap();
        scheduler.trigger_job("j1").await.unwrap();
        assert_eq!(scheduler.execution_history().await.len(), 1);
        scheduler.clear_history().await;
        assert_eq!(scheduler.execution_history().await.len(), 0);
    }

    #[tokio::test]
    async fn history_capped_at_max() {
        let (router, catalog) = setup().await;
        let scheduler = Scheduler::with_max_history(router, catalog, 3);
        let job = JobConfig::recurring("j1", "tenant1", "shard-x", Duration::from_secs(0));
        scheduler.register_job(job, noop_handler()).await.unwrap();

        for _ in 0..5 {
            scheduler.trigger_job("j1").await.unwrap();
        }
        assert_eq!(scheduler.execution_history().await.len(), 3);
    }

    #[tokio::test]
    async fn auto_pause_on_max_failures() {
        let (router, catalog) = setup().await;
        let scheduler = Scheduler::new(router, catalog);
        let job = JobConfig::recurring("j1", "tenant1", "shard-x", Duration::from_secs(60))
            .with_max_failures(2);
        scheduler
            .register_job(job, failing_handler("fail"))
            .await
            .unwrap();

        // First failure.
        let _ = scheduler.trigger_job("j1").await;
        let snap = scheduler.job_snapshot("j1").await.unwrap();
        assert_eq!(snap.state, JobState::Active);
        assert_eq!(snap.consecutive_failures, 1);

        // Second failure -> auto-pause.
        let _ = scheduler.trigger_job("j1").await;
        let snap = scheduler.job_snapshot("j1").await.unwrap();
        assert_eq!(snap.state, JobState::Paused);
        assert_eq!(snap.consecutive_failures, 2);
    }

    #[tokio::test]
    async fn success_resets_consecutive_failures() {
        let (router, catalog) = setup().await;
        let scheduler = Scheduler::new(router, catalog);
        let counter = Arc::new(AtomicU64::new(0));
        let c = counter.clone();
        // Handler that fails on first call, succeeds on second.
        let handler: JobHandler = Arc::new(move |_| {
            let c = c.clone();
            Box::pin(async move {
                let n = c.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    anyhow::bail!("first failure")
                }
                Ok(())
            })
        });

        let job = JobConfig::recurring("j1", "tenant1", "shard-x", Duration::from_secs(60))
            .with_max_failures(3);
        scheduler.register_job(job, handler).await.unwrap();

        let _ = scheduler.trigger_job("j1").await;
        let snap = scheduler.job_snapshot("j1").await.unwrap();
        assert_eq!(snap.consecutive_failures, 1);

        scheduler.trigger_job("j1").await.unwrap();
        let snap = scheduler.job_snapshot("j1").await.unwrap();
        assert_eq!(snap.consecutive_failures, 0);
        assert_eq!(snap.total_successes, 1);
        assert_eq!(snap.total_failures, 1);
    }

    #[tokio::test]
    async fn list_jobs() {
        let (router, catalog) = setup().await;
        let scheduler = Scheduler::new(router, catalog);
        let j1 = JobConfig::recurring("j1", "tenant1", "shard-x", Duration::from_secs(60));
        let j2 = JobConfig::recurring("j2", "tenant1", "shard-x", Duration::from_secs(60));
        scheduler.register_job(j1, noop_handler()).await.unwrap();
        scheduler.register_job(j2, noop_handler()).await.unwrap();
        assert_eq!(scheduler.list_jobs().await.len(), 2);
        assert_eq!(scheduler.job_count().await, 2);
    }

    #[tokio::test]
    async fn jobs_by_tenant() {
        let (router, catalog) = setup().await;
        // Add second tenant.
        catalog
            .register_shard(ShardConfig {
                shard_id: "shard-y".into(),
                region: "eu".into(),
                allowed_tenants: HashSet::from_iter(vec!["tenant2".to_string()]),
                consensus_resource: None,
            })
            .await;
        router
            .register(TenantRouteConfig {
                tenant_id: "tenant2".into(),
                shard_id: "shard-y".into(),
                queue_name: "default".into(),
                region: "eu".into(),
                max_inflight: 10,
                max_per_second: 100,
                consensus_resource: None,
                lease_ttl: Duration::from_secs(5),
            })
            .await;

        let scheduler = Scheduler::new(router, catalog);
        let j1 = JobConfig::recurring("j1", "tenant1", "shard-x", Duration::from_secs(60));
        let j2 = JobConfig::recurring("j2", "tenant2", "shard-y", Duration::from_secs(60));
        scheduler.register_job(j1, noop_handler()).await.unwrap();
        scheduler.register_job(j2, noop_handler()).await.unwrap();

        let t1_jobs = scheduler.jobs_by_tenant("tenant1").await;
        assert_eq!(t1_jobs.len(), 1);
        assert_eq!(t1_jobs[0].tenant_id, "tenant1");
    }

    #[tokio::test]
    async fn jobs_by_state() {
        let (router, catalog) = setup().await;
        let scheduler = Scheduler::new(router, catalog);
        let j1 = JobConfig::recurring("j1", "tenant1", "shard-x", Duration::from_secs(60));
        let j2 = JobConfig::recurring("j2", "tenant1", "shard-x", Duration::from_secs(60));
        scheduler.register_job(j1, noop_handler()).await.unwrap();
        scheduler.register_job(j2, noop_handler()).await.unwrap();
        scheduler.pause_job("j2").await.unwrap();

        let active = scheduler.jobs_by_state(JobState::Active).await;
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].job_id, "j1");

        let paused = scheduler.jobs_by_state(JobState::Paused).await;
        assert_eq!(paused.len(), 1);
        assert_eq!(paused[0].job_id, "j2");
    }

    #[tokio::test]
    async fn active_job_count() {
        let (router, catalog) = setup().await;
        let scheduler = Scheduler::new(router, catalog);
        let j1 = JobConfig::recurring("j1", "tenant1", "shard-x", Duration::from_secs(60));
        let j2 = JobConfig::recurring("j2", "tenant1", "shard-x", Duration::from_secs(60));
        scheduler.register_job(j1, noop_handler()).await.unwrap();
        scheduler.register_job(j2, noop_handler()).await.unwrap();
        assert_eq!(scheduler.active_job_count().await, 2);
        scheduler.pause_job("j1").await.unwrap();
        assert_eq!(scheduler.active_job_count().await, 1);
    }

    #[tokio::test]
    async fn job_priority_in_config() {
        let (router, catalog) = setup().await;
        let scheduler = Scheduler::new(router, catalog);
        let job = JobConfig::recurring("j1", "tenant1", "shard-x", Duration::from_secs(60))
            .with_priority(200)
            .with_description("high priority job");
        scheduler.register_job(job, noop_handler()).await.unwrap();
        let snap = scheduler.job_snapshot("j1").await.unwrap();
        assert_eq!(snap.priority, 200);
        assert_eq!(snap.description, "high priority job");
    }

    #[tokio::test]
    async fn tick_processes_due_jobs() {
        let (router, catalog) = setup().await;
        let scheduler = Scheduler::new(router, catalog);
        let c1 = Arc::new(AtomicU64::new(0));
        let c2 = Arc::new(AtomicU64::new(0));

        // j1: interval 0 (always due), j2: interval 0 (always due).
        let j1 = JobConfig::recurring("j1", "tenant1", "shard-x", Duration::from_secs(0))
            .with_priority(200);
        let j2 = JobConfig::recurring("j2", "tenant1", "shard-x", Duration::from_secs(0))
            .with_priority(50);
        scheduler
            .register_job(j1, counting_handler(c1.clone()))
            .await
            .unwrap();
        scheduler
            .register_job(j2, counting_handler(c2.clone()))
            .await
            .unwrap();

        let results = scheduler.tick().await;
        assert_eq!(results.len(), 2);
        // Higher priority runs first.
        assert_eq!(results[0].0, "j1");
        assert_eq!(results[1].0, "j2");
        assert_eq!(c1.load(Ordering::SeqCst), 1);
        assert_eq!(c2.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn tick_skips_paused_jobs() {
        let (router, catalog) = setup().await;
        let scheduler = Scheduler::new(router, catalog);
        let counter = Arc::new(AtomicU64::new(0));
        let j1 = JobConfig::recurring("j1", "tenant1", "shard-x", Duration::from_secs(0));
        scheduler
            .register_job(j1, counting_handler(counter.clone()))
            .await
            .unwrap();
        scheduler.pause_job("j1").await.unwrap();

        let results = scheduler.tick().await;
        assert_eq!(results.len(), 0);
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn tick_skips_not_yet_due() {
        let (router, catalog) = setup().await;
        let scheduler = Scheduler::new(router, catalog);
        let counter = Arc::new(AtomicU64::new(0));
        let j1 = JobConfig::recurring("j1", "tenant1", "shard-x", Duration::from_secs(3600));
        scheduler
            .register_job(j1, counting_handler(counter.clone()))
            .await
            .unwrap();

        // First tick: job has never run, so it's due.
        let results = scheduler.tick().await;
        assert_eq!(results.len(), 1);
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        // Second tick immediately: not due (interval is 1 hour).
        let results = scheduler.tick().await;
        assert_eq!(results.len(), 0);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn one_shot_not_due_after_run() {
        let (router, catalog) = setup().await;
        let scheduler = Scheduler::new(router, catalog);
        let counter = Arc::new(AtomicU64::new(0));
        let j1 = JobConfig::one_shot("j1", "tenant1", "shard-x");
        scheduler
            .register_job(j1, counting_handler(counter.clone()))
            .await
            .unwrap();

        let results = scheduler.tick().await;
        assert_eq!(results.len(), 1);
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        // After running once, should not be due.
        let results = scheduler.tick().await;
        assert_eq!(results.len(), 0);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn job_snapshot_returns_none_for_unknown() {
        let (router, catalog) = setup().await;
        let scheduler = Scheduler::new(router, catalog);
        assert!(scheduler.job_snapshot("nope").await.is_none());
    }

    // ---- Serialization ----

    #[test]
    fn job_kind_serialization() {
        let kind = JobKind::OneShot;
        let json = serde_json::to_string(&kind).unwrap();
        let parsed: JobKind = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, JobKind::OneShot);
    }

    #[test]
    fn job_state_serialization() {
        let state = JobState::Paused;
        let json = serde_json::to_string(&state).unwrap();
        let parsed: JobState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, JobState::Paused);
    }

    #[test]
    fn job_snapshot_serialization() {
        let snap = JobSnapshot {
            job_id: "j1".into(),
            tenant_id: "t1".into(),
            description: "test".into(),
            shard_id: "s1".into(),
            kind: JobKind::Recurring,
            state: JobState::Active,
            priority: 100,
            interval_secs: 60,
            total_runs: 10,
            total_successes: 9,
            total_failures: 1,
            consecutive_failures: 0,
        };
        let json = serde_json::to_string(&snap).unwrap();
        let parsed: JobSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.job_id, "j1");
        assert_eq!(parsed.total_runs, 10);
    }

    #[test]
    fn execution_record_serialization() {
        let record = ExecutionRecord {
            job_id: "j1".into(),
            tenant_id: "t1".into(),
            success: false,
            error_message: Some("boom".into()),
            timestamp: "2026-01-01T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&record).unwrap();
        let parsed: ExecutionRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.error_message.as_deref(), Some("boom"));
    }
}
