//! Shared runtime helper for workers that need tenant-safe threading, instrumentation, and graceful shutdown.

use anyhow::Result;
use platform_tenancy::TenantContext;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tokio::runtime::Handle;
use tokio::sync::{Mutex, Notify};
use tokio::time::{Duration, Instant};
use tracing::{info, span, Instrument, Level};

/// Errors returned by the worker runtime when scheduling tasks.
#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("worker runtime is shutting down")]
    ShuttingDown,

    #[error("maximum in-flight tasks exceeded")]
    MaxInflight,
}

/// Describes basic runtime snapshot data.
#[derive(Debug, Clone)]
pub struct WorkerStats {
    pub total_inflight: usize,
    pub per_tenant: HashMap<String, usize>,
}

/// Tenant-aware worker runtime that tracks concurrency, instrumentation, and graceful shutdown.
pub struct WorkerRuntime {
    handle: Arc<Handle>,
    max_inflight: usize,
    inflight: Arc<AtomicUsize>,
    tenant_counts: Arc<Mutex<HashMap<String, usize>>>,
    shutting_down: Arc<AtomicBool>,
    notify: Arc<Notify>,
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

    /// Spawn a tenant-aware task. The provided future runs with instrumentation and leak counters.
    pub fn spawn_tenant_task<F>(&self, context: TenantContext, work: F) -> Result<(), RuntimeError>
    where
        F: std::future::Future<Output = Result<()>> + Send + 'static,
    {
        self.can_schedule()?;
        let inflight = self.inflight.clone();
        let counts = Arc::clone(&self.tenant_counts);
        let notify = Arc::clone(&self.notify);
        let tenant_id = context.tenant_id.clone();
        let span = span!(Level::INFO, "worker_task", tenant = %tenant_id);
        let work = async move {
            {
                let mut map = counts.lock().await;
                let counter = map.entry(tenant_id.clone()).or_insert(0);
                *counter += 1;
            }
            info!(
                tenant = %tenant_id,
                "scheduling tenant task in worker runtime"
            );
            let res = work.await;
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
        Ok(())
    }

    /// Returns a snapshot of runtime statistics.
    pub async fn stats(&self) -> WorkerStats {
        let per_tenant = self.tenant_counts.lock().await.clone();
        WorkerStats {
            total_inflight: self.inflight.load(Ordering::SeqCst),
            per_tenant,
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use platform_tenancy::TenantAccessLevel;
    use tokio::time::Duration;

    #[tokio::test]
    async fn spawn_tasks_and_collect_stats() {
        let runtime = WorkerRuntime::current(2);
        let context = TenantContext {
            tenant_id: "tenant-x".into(),
            tenant_name: Some("T".into()),
            access_level: TenantAccessLevel::Worker,
        };
        let notify = Arc::new(Notify::new());
        let inner = notify.clone();
        runtime
            .spawn_tenant_task(context.clone(), async move {
                inner.notify_waiters();
                Ok(())
            })
            .unwrap();
        notify.notified().await;
        let stats = runtime.stats().await;
        assert!(stats.total_inflight <= 2);
        assert_eq!(*stats.per_tenant.get("tenant-x").unwrap_or(&0), 0);
    }

    #[tokio::test]
    async fn shutdown_waits_for_tasks() {
        let runtime = WorkerRuntime::current(1);
        let context = TenantContext {
            tenant_id: "tenant-y".into(),
            tenant_name: Some("T".into()),
            access_level: TenantAccessLevel::Worker,
        };
        runtime
            .spawn_tenant_task(context, async move {
                tokio::time::sleep(Duration::from_millis(10)).await;
                Ok(())
            })
            .unwrap();
        runtime.shutdown(Duration::from_secs(1)).await;
        let stats = runtime.stats().await;
        assert_eq!(stats.total_inflight, 0);
    }
}
