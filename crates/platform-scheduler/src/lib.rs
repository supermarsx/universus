//! Tenant-aware scheduler that registers jobs, validates shard placement, and triggers
//! work using `platform-tenant-routing`/`platform-sharding`.

use anyhow::Result;
use platform_sharding::{ShardSummary, ShardingCatalog};
use platform_tenancy::{TenantAccessLevel, TenantContext};
use platform_tenant_routing::{RoutingError, TenantRouter, TenantRoutingDecision};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::RwLock;
use tokio::time::Instant;

pub type JobHandler = Arc<dyn Fn(TenantRoutingDecision) -> JobFuture + Send + Sync>;

pub type JobFuture = Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>>;

#[derive(Debug, Clone)]
pub struct JobConfig {
    pub job_id: String,
    pub tenant_id: String,
    pub description: String,
    pub shard_id: String,
    pub interval: Duration,
}

pub struct ScheduledJob {
    config: JobConfig,
    handler: JobHandler,
    last_run: Option<Instant>,
}

impl ScheduledJob {
    fn new(config: JobConfig, handler: JobHandler) -> Self {
        Self {
            config,
            handler,
            last_run: None,
        }
    }
}

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
}

pub struct Scheduler {
    tenant_router: Arc<TenantRouter>,
    catalog: Arc<ShardingCatalog>,
    jobs: RwLock<HashMap<String, ScheduledJob>>,
}

impl Scheduler {
    pub fn new(tenant_router: Arc<TenantRouter>, catalog: Arc<ShardingCatalog>) -> Self {
        Self {
            tenant_router,
            catalog,
            jobs: RwLock::new(HashMap::new()),
        }
    }

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
        handler_result.map_err(SchedulerError::HandlerFailure)?;

        job.last_run = Some(Instant::now());
        Ok(())
    }
}

impl ScheduledJob {
    fn make_context(&self) -> TenantContext {
        TenantContext {
            tenant_id: self.config.tenant_id.clone(),
            tenant_name: Some(self.config.description.clone()),
            access_level: TenantAccessLevel::Worker,
        }
    }
}

impl Scheduler {
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

#[cfg(test)]
mod tests {
    use super::*;
    use platform_consensus::LeaseCoordinator;
    use platform_sharding::ShardConfig;
    use platform_tenant_routing::TenantRouteConfig;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::collections::HashSet;
    use std::iter::FromIterator;
    use tokio::sync::Notify;

    #[tokio::test]
    async fn registers_and_triggers_job() {
        let tenant_router = Arc::new(TenantRouter::new());
        let catalog = Arc::new(ShardingCatalog::new());

        catalog
            .register_shard(ShardConfig {
                shard_id: "shard-x".into(),
                region: "eu".into(),
                allowed_tenants: HashSet::from_iter(vec!["tenant1".to_string()]),
                consensus_resource: None,
            })
            .await;

        let scheduler = Scheduler::new(tenant_router.clone(), Arc::clone(&catalog));
        tenant_router
            .register(TenantRouteConfig {
                tenant_id: "tenant1".into(),
                shard_id: "shard-x".into(),
                queue_name: "default".into(),
                region: "eu".into(),
                max_inflight: 1,
                max_per_second: 10,
                consensus_resource: None,
                lease_ttl: Duration::from_secs(5),
            })
            .await;
        let job = JobConfig {
            job_id: "job-a".into(),
            tenant_id: "tenant1".into(),
            description: "task".into(),
            shard_id: "shard-x".into(),
            interval: Duration::from_secs(60),
        };

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

        scheduler
            .register_job(job, handler)
            .await
            .expect("register ok");
        scheduler.trigger_job("job-a").await.expect("trigger");
        assert!(*marker.lock().await);
    }

    #[tokio::test]
    async fn shard_mismatch_blocks_job() {
        let tenant_router = Arc::new(TenantRouter::new());
        let catalog = Arc::new(ShardingCatalog::new());

        catalog
            .register_shard(ShardConfig {
                shard_id: "shard-other".into(),
                region: "eu".into(),
                allowed_tenants: HashSet::from_iter(vec!["tenant2".to_string()]),
                consensus_resource: None,
            })
            .await;

        let scheduler = Scheduler::new(tenant_router.clone(), catalog);
        tenant_router
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
        };

        let handler: JobHandler = Arc::new(|_| Box::pin(async { Ok(()) }));
        scheduler.register_job(job, handler).await.unwrap();
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
        };
        let job_b = JobConfig {
            job_id: "job-b".into(),
            tenant_id: "tenant1".into(),
            description: "follow-up task".into(),
            shard_id: "shard-x".into(),
            interval: Duration::from_secs(60),
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
}
