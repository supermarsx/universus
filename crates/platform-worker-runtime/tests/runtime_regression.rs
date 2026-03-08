use anyhow::Result;
use platform_consensus::LeaseCoordinator;
use platform_tenancy::{TenantAccessLevel, TenantContext};
use platform_worker_runtime::{RuntimeError, WorkerRuntime};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::time::{timeout, Duration};

fn tenant_context(id: &str) -> TenantContext {
    TenantContext {
        tenant_id: id.to_string(),
        tenant_name: Some(format!("Tenant {id}")),
        access_level: TenantAccessLevel::Worker,
    }
}

#[tokio::test]
async fn leak_counter_returns_to_zero_after_batch() -> Result<()> {
    let runtime = WorkerRuntime::current(2048);
    let total = 500usize;
    let completed = Arc::new(AtomicUsize::new(0));

    for idx in 0..total {
        let completed = Arc::clone(&completed);
        runtime
            .spawn_tenant_task(tenant_context("tenant-load"), async move {
                completed.fetch_add(1, Ordering::SeqCst);
                if idx % 17 == 0 {
                    tokio::task::yield_now().await;
                }
                Ok(())
            })
            .expect("task should schedule");
    }

    timeout(Duration::from_secs(5), async {
        loop {
            if completed.load(Ordering::SeqCst) == total {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("all tasks should finish");

    runtime.shutdown(Duration::from_secs(2)).await;
    let stats = runtime.stats().await;
    assert_eq!(stats.total_inflight, 0);
    assert_eq!(stats.per_tenant.get("tenant-load"), Some(&0));
    Ok(())
}

#[tokio::test]
async fn max_inflight_enforces_backpressure() -> Result<()> {
    let runtime = WorkerRuntime::current(1);
    let (tx, rx) = oneshot::channel::<()>();

    runtime
        .spawn_tenant_task(tenant_context("tenant-a"), async move {
            let _ = rx.await;
            Ok(())
        })
        .expect("first task accepted");

    let second = runtime.spawn_tenant_task(tenant_context("tenant-a"), async move { Ok(()) });
    assert!(matches!(second, Err(RuntimeError::MaxInflight)));

    let _ = tx.send(());
    runtime.shutdown(Duration::from_secs(1)).await;
    Ok(())
}

#[tokio::test]
async fn shutdown_rejects_future_tasks() -> Result<()> {
    let runtime = WorkerRuntime::current(4);
    runtime.shutdown(Duration::from_millis(1)).await;

    let err = runtime.spawn_tenant_task(tenant_context("tenant-b"), async move { Ok(()) });
    assert!(matches!(err, Err(RuntimeError::ShuttingDown)));
    Ok(())
}

#[tokio::test]
async fn leased_task_holds_and_releases_consensus_lease() -> Result<()> {
    let runtime = WorkerRuntime::current(2);
    let coordinator = Arc::new(LeaseCoordinator::new());
    let (tx, rx) = oneshot::channel::<()>();
    let resource = "runtime:tenant-c:job".to_string();

    runtime
        .spawn_leased_tenant_task(
            tenant_context("tenant-c"),
            Arc::clone(&coordinator),
            resource.clone(),
            Duration::from_secs(10),
            async move {
                let _ = rx.await;
                Ok(())
            },
        )
        .await
        .expect("leased task should schedule");

    let lease_conflict = coordinator
        .acquire(&resource, "tenant-d", Duration::from_secs(10))
        .await;
    assert!(
        lease_conflict.is_err(),
        "lease must be held while task runs"
    );

    let _ = tx.send(());
    runtime.shutdown(Duration::from_secs(1)).await;

    let lease_after = coordinator
        .acquire(&resource, "tenant-d", Duration::from_secs(10))
        .await;
    assert!(
        lease_after.is_ok(),
        "lease should be released after task completes"
    );
    Ok(())
}
