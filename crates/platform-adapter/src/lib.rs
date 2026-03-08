#![forbid(unsafe_code)]

//! Platform-level adapter registry that wraps `adapter-db`, injects tenant
//! context, enforces consensus leases, and reports health.
//!
//! Provides:
//! - `PlatformAdapterRegistry` — manages adapters with lease-protected access
//! - `PlatformAdapterLease` — an acquired adapter handle with an optional lease
//! - `AdapterMetrics` — tracks acquire, release, and failure counts
//! - `execute_with_lease` — run a script through a lease-protected adapter
//! - `health_check` — validate adapter connectivity
//! - `list_adapters` / `get_adapter_info` — introspection

use adapter_db::{bootstrap_from_json, AdapterEntry, AdapterRegistry};
use anyhow::{Context, Result};
use platform_consensus::{LeaseCoordinator, LeaseToken};
use platform_tenancy::TenantContext;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tokio::time::Duration;

// ---------------------------------------------------------------------------
// Adapter definition
// ---------------------------------------------------------------------------

/// Metadata for a registered adapter (serialisable for admin / health endpoints).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PlatformAdapterDefinition {
    pub name: String,
    pub tenant: String,
    pub driver: String,
    pub info: String,
}

impl PlatformAdapterDefinition {
    fn from_entry(entry: &AdapterEntry) -> Self {
        let (driver, info, tenant) = match &entry.driver {
            adapter_db::AdapterDriver::Postgres { url, tenant, .. } => {
                ("postgres", url.clone(), tenant.clone())
            }
            adapter_db::AdapterDriver::Mysql { url, tenant, .. } => {
                ("mysql", url.clone(), tenant.clone())
            }
            adapter_db::AdapterDriver::JsonFile { path, tenant, .. } => {
                ("jsonfile", path.clone(), tenant.clone())
            }
            adapter_db::AdapterDriver::Sqlite { path, tenant, .. } => {
                ("sqlite", path.clone(), tenant.clone())
            }
        };
        Self {
            name: entry.name.clone(),
            tenant,
            driver: driver.to_string(),
            info,
        }
    }
}

// ---------------------------------------------------------------------------
// Adapter lease
// ---------------------------------------------------------------------------

/// An acquired adapter handle with an optional consensus lease.
pub struct PlatformAdapterLease {
    pub adapter: Arc<dyn adapter_db::DatabaseAdapter>,
    pub lease: Option<LeaseToken>,
    pub definition: PlatformAdapterDefinition,
}

impl std::fmt::Debug for PlatformAdapterLease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlatformAdapterLease")
            .field("definition", &self.definition)
            .field("lease", &self.lease)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum PlatformAdapterError {
    #[error("tenant {0} not registered")]
    TenantMissing(String),

    #[error("adapter {0} not found")]
    AdapterNotFound(String),

    #[error("lease acquisition failed: {0}")]
    LeaseFailure(#[source] anyhow::Error),

    #[error("script execution failed: {0}")]
    ExecutionFailed(#[source] anyhow::Error),

    #[error("health check failed for adapter {name}: {reason}")]
    HealthCheckFailed { name: String, reason: String },
}

// ---------------------------------------------------------------------------
// Metrics
// ---------------------------------------------------------------------------

/// Atomic counters for adapter operations.
#[derive(Debug, Default)]
pub struct AdapterMetrics {
    pub acquires: AtomicU64,
    pub acquire_failures: AtomicU64,
    pub releases: AtomicU64,
    pub executions: AtomicU64,
    pub execution_failures: AtomicU64,
    pub health_checks: AtomicU64,
    pub health_failures: AtomicU64,
}

/// Snapshot of adapter metrics (serialisable).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AdapterMetricsSnapshot {
    pub acquires: u64,
    pub acquire_failures: u64,
    pub releases: u64,
    pub executions: u64,
    pub execution_failures: u64,
    pub health_checks: u64,
    pub health_failures: u64,
}

impl AdapterMetrics {
    pub fn snapshot(&self) -> AdapterMetricsSnapshot {
        AdapterMetricsSnapshot {
            acquires: self.acquires.load(Ordering::Relaxed),
            acquire_failures: self.acquire_failures.load(Ordering::Relaxed),
            releases: self.releases.load(Ordering::Relaxed),
            executions: self.executions.load(Ordering::Relaxed),
            execution_failures: self.execution_failures.load(Ordering::Relaxed),
            health_checks: self.health_checks.load(Ordering::Relaxed),
            health_failures: self.health_failures.load(Ordering::Relaxed),
        }
    }
}

// ---------------------------------------------------------------------------
// Health check result
// ---------------------------------------------------------------------------

/// Result of a health probe on a single adapter.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AdapterHealthResult {
    pub name: String,
    pub tenant: String,
    pub driver: String,
    pub healthy: bool,
    pub message: Option<String>,
}

// ---------------------------------------------------------------------------
// Execution result
// ---------------------------------------------------------------------------

/// Result of executing a script through a lease-protected adapter.
#[derive(Debug, Clone, Serialize)]
pub struct AdapterExecutionResult {
    pub tenant: String,
    pub adapter_name: String,
    pub output: String,
    pub lease_resource: Option<String>,
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

pub struct PlatformAdapterRegistry {
    registry: Arc<AdapterRegistry>,
    definitions: HashMap<String, PlatformAdapterDefinition>,
    lease_coordinator: Arc<LeaseCoordinator>,
    lease_ttl: Duration,
    metrics: Arc<AdapterMetrics>,
}

impl PlatformAdapterRegistry {
    /// Create an empty registry (no adapters loaded).
    pub fn empty(lease_coordinator: Arc<LeaseCoordinator>, lease_ttl: Duration) -> Self {
        Self {
            registry: Arc::new(AdapterRegistry::new()),
            definitions: HashMap::new(),
            lease_coordinator,
            lease_ttl,
            metrics: Arc::new(AdapterMetrics::default()),
        }
    }

    /// Load the registry from a JSON config file on disk.
    pub async fn from_json_file<P: AsRef<Path>>(
        path: P,
        lease_coordinator: Arc<LeaseCoordinator>,
        lease_ttl: Duration,
    ) -> Result<Self> {
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("reading adapter registry from {:?}", path.as_ref()))?;
        Self::from_json(&contents, lease_coordinator, lease_ttl).await
    }

    /// Parse the registry from a JSON string.
    pub async fn from_json(
        config_json: &str,
        lease_coordinator: Arc<LeaseCoordinator>,
        lease_ttl: Duration,
    ) -> Result<Self> {
        let entries: Vec<AdapterEntry> = serde_json::from_str(config_json)?;
        let registry = bootstrap_from_json(config_json).await?;
        let definitions = entries
            .into_iter()
            .map(|entry| {
                (
                    entry.name.clone(),
                    PlatformAdapterDefinition::from_entry(&entry),
                )
            })
            .collect();
        Ok(Self {
            registry: Arc::new(registry),
            definitions,
            lease_coordinator,
            lease_ttl,
            metrics: Arc::new(AdapterMetrics::default()),
        })
    }

    /// Acquire an adapter for a tenant, optionally taking a consensus lease.
    pub async fn acquire_adapter_for_tenant(
        &self,
        context: &TenantContext,
        resource_hint: Option<&str>,
    ) -> Result<PlatformAdapterLease, PlatformAdapterError> {
        let adapter = self
            .registry
            .get_for_tenant(&context.tenant_id)
            .await
            .ok_or_else(|| {
                self.metrics
                    .acquire_failures
                    .fetch_add(1, Ordering::Relaxed);
                PlatformAdapterError::TenantMissing(context.tenant_id.clone())
            })?;

        let definition = self
            .definitions
            .values()
            .find(|def| def.tenant == context.tenant_id)
            .cloned()
            .ok_or_else(|| {
                self.metrics
                    .acquire_failures
                    .fetch_add(1, Ordering::Relaxed);
                PlatformAdapterError::TenantMissing(context.tenant_id.clone())
            })?;

        let lease = if let Some(resource) = resource_hint {
            Some(
                self.lease_coordinator
                    .acquire(resource, &context.tenant_id, self.lease_ttl)
                    .await
                    .map_err(|e| {
                        self.metrics
                            .acquire_failures
                            .fetch_add(1, Ordering::Relaxed);
                        PlatformAdapterError::LeaseFailure(e)
                    })?,
            )
        } else {
            None
        };

        self.metrics.acquires.fetch_add(1, Ordering::Relaxed);
        Ok(PlatformAdapterLease {
            adapter,
            lease,
            definition,
        })
    }

    /// Release a lease back to the coordinator.
    pub async fn release_lease(&self, lease: LeaseToken) {
        self.lease_coordinator
            .release(&lease.resource, &lease.owner)
            .await;
        self.metrics.releases.fetch_add(1, Ordering::Relaxed);
    }

    /// Execute a script through a lease-protected adapter.
    ///
    /// Acquires a lease, runs the script, releases the lease, and returns the
    /// execution output.
    pub async fn execute_with_lease(
        &self,
        context: &TenantContext,
        resource: &str,
        script: &str,
    ) -> Result<AdapterExecutionResult, PlatformAdapterError> {
        let lease_handle = self
            .acquire_adapter_for_tenant(context, Some(resource))
            .await?;

        let output = lease_handle
            .adapter
            .execute_script(script)
            .await
            .map_err(|e| {
                self.metrics
                    .execution_failures
                    .fetch_add(1, Ordering::Relaxed);
                PlatformAdapterError::ExecutionFailed(e)
            })?;

        self.metrics.executions.fetch_add(1, Ordering::Relaxed);

        let lease_resource = lease_handle.lease.as_ref().map(|l| l.resource.clone());

        let adapter_name = lease_handle.definition.name.clone();

        if let Some(lease) = lease_handle.lease {
            self.release_lease(lease).await;
        }

        Ok(AdapterExecutionResult {
            tenant: context.tenant_id.clone(),
            adapter_name,
            output,
            lease_resource,
        })
    }

    /// Probe an adapter's health by running a trivial script.
    pub async fn health_check(
        &self,
        context: &TenantContext,
    ) -> Result<AdapterHealthResult, PlatformAdapterError> {
        self.metrics.health_checks.fetch_add(1, Ordering::Relaxed);

        let adapter = self
            .registry
            .get_for_tenant(&context.tenant_id)
            .await
            .ok_or_else(|| PlatformAdapterError::TenantMissing(context.tenant_id.clone()))?;

        let definition = self
            .definitions
            .values()
            .find(|def| def.tenant == context.tenant_id)
            .cloned()
            .ok_or_else(|| PlatformAdapterError::TenantMissing(context.tenant_id.clone()))?;

        // Run a trivial no-op script as a health probe.
        match adapter.execute_script("SELECT 1").await {
            Ok(msg) => Ok(AdapterHealthResult {
                name: definition.name.clone(),
                tenant: definition.tenant.clone(),
                driver: definition.driver.clone(),
                healthy: true,
                message: Some(msg),
            }),
            Err(e) => {
                self.metrics.health_failures.fetch_add(1, Ordering::Relaxed);
                Ok(AdapterHealthResult {
                    name: definition.name.clone(),
                    tenant: definition.tenant.clone(),
                    driver: definition.driver.clone(),
                    healthy: false,
                    message: Some(e.to_string()),
                })
            }
        }
    }

    /// Return metadata for all registered adapters.
    pub fn health_snapshot(&self) -> Vec<PlatformAdapterDefinition> {
        self.definitions.values().cloned().collect()
    }

    /// List all adapter names.
    pub fn list_adapter_names(&self) -> Vec<String> {
        self.definitions.keys().cloned().collect()
    }

    /// Get the definition for a named adapter.
    pub fn get_adapter_info(&self, name: &str) -> Option<&PlatformAdapterDefinition> {
        self.definitions.get(name)
    }

    /// Find which adapter(s) serve a given tenant.
    pub fn adapters_for_tenant(&self, tenant_id: &str) -> Vec<PlatformAdapterDefinition> {
        self.definitions
            .values()
            .filter(|def| def.tenant == tenant_id)
            .cloned()
            .collect()
    }

    /// Return a snapshot of metrics counters.
    pub fn metrics_snapshot(&self) -> AdapterMetricsSnapshot {
        self.metrics.snapshot()
    }

    /// The number of registered adapters.
    pub fn adapter_count(&self) -> usize {
        self.definitions.len()
    }

    /// Access the underlying lease coordinator (for integration with migrations etc).
    pub fn lease_coordinator(&self) -> &Arc<LeaseCoordinator> {
        &self.lease_coordinator
    }

    /// Configured lease TTL.
    pub fn lease_ttl(&self) -> Duration {
        self.lease_ttl
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use platform_consensus::LeaseCoordinator;
    use platform_tenancy::{TenantAccessLevel, TenantContext};
    use std::env::temp_dir;
    use tokio::fs::File;
    use tokio::io::AsyncWriteExt;

    /// Helper: create a temp JSON config for a single jsonfile adapter.
    async fn setup_single_adapter(
        dir_name: &str,
        adapter_name: &str,
        tenant: &str,
    ) -> (std::path::PathBuf, String) {
        let dir = temp_dir().join(format!("platform-adapter-{dir_name}"));
        let config_path = dir.join("adapter.json");
        let data_path = dir.join("tenant-data.json");
        tokio::fs::create_dir_all(&dir).await.unwrap();

        let mut data_file = File::create(&data_path).await.unwrap();
        data_file.write_all(br#"{}"#).await.unwrap();
        data_file.flush().await.unwrap();

        let config = serde_json::json!([{
            "name": adapter_name,
            "driver": "jsonfile",
            "tenant": tenant,
            "path": data_path.to_string_lossy()
        }])
        .to_string();

        let mut config_file = File::create(&config_path).await.unwrap();
        config_file.write_all(config.as_bytes()).await.unwrap();
        config_file.flush().await.unwrap();

        (config_path, config)
    }

    fn tenant_context(tenant_id: &str) -> TenantContext {
        TenantContext {
            tenant_id: tenant_id.into(),
            tenant_name: None,
            access_level: TenantAccessLevel::Worker,
        }
    }

    // --- Basic round-trip ---

    #[tokio::test]
    async fn adapter_registry_roundtrip() -> Result<()> {
        let (_path, config) = setup_single_adapter("roundtrip", "t-json", "tenant-a").await;
        let lc = Arc::new(LeaseCoordinator::new());
        let registry =
            PlatformAdapterRegistry::from_json(&config, lc, Duration::from_secs(1)).await?;

        let context = tenant_context("tenant-a");
        let lease = registry
            .acquire_adapter_for_tenant(&context, Some("adapter:tenant-a"))
            .await
            .expect("adapter ready");
        assert_eq!(lease.definition.tenant, "tenant-a");
        assert!(lease.lease.is_some());
        Ok(())
    }

    // --- Health snapshot ---

    #[tokio::test]
    async fn health_snapshot_reports_definitions() -> Result<()> {
        let (_path, config) =
            setup_single_adapter("health-snap", "health-json", "tenant-health").await;
        let lc = Arc::new(LeaseCoordinator::new());
        let registry =
            PlatformAdapterRegistry::from_json(&config, lc, Duration::from_secs(1)).await?;

        let snapshot = registry.health_snapshot();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].tenant, "tenant-health");
        Ok(())
    }

    // --- Empty registry ---

    #[tokio::test]
    async fn empty_registry_has_no_adapters() {
        let lc = Arc::new(LeaseCoordinator::new());
        let registry = PlatformAdapterRegistry::empty(lc, Duration::from_secs(1));
        assert_eq!(registry.adapter_count(), 0);
        assert!(registry.health_snapshot().is_empty());
        assert!(registry.list_adapter_names().is_empty());
    }

    #[tokio::test]
    async fn empty_registry_returns_tenant_missing() {
        let lc = Arc::new(LeaseCoordinator::new());
        let registry = PlatformAdapterRegistry::empty(lc, Duration::from_secs(1));
        let context = tenant_context("nobody");
        let err = registry
            .acquire_adapter_for_tenant(&context, None)
            .await
            .unwrap_err();
        assert!(matches!(err, PlatformAdapterError::TenantMissing(_)));
    }

    // --- Adapter listing ---

    #[tokio::test]
    async fn list_adapter_names_and_info() -> Result<()> {
        let (_path, config) = setup_single_adapter("list-test", "my-adapter", "tenant-list").await;
        let lc = Arc::new(LeaseCoordinator::new());
        let registry =
            PlatformAdapterRegistry::from_json(&config, lc, Duration::from_secs(1)).await?;

        let names = registry.list_adapter_names();
        assert_eq!(names.len(), 1);
        assert!(names.contains(&"my-adapter".to_string()));

        let info = registry.get_adapter_info("my-adapter").unwrap();
        assert_eq!(info.tenant, "tenant-list");
        assert_eq!(info.driver, "jsonfile");

        assert!(registry.get_adapter_info("nonexistent").is_none());
        Ok(())
    }

    // --- Adapters for tenant ---

    #[tokio::test]
    async fn adapters_for_tenant_filters() -> Result<()> {
        let (_path, config) =
            setup_single_adapter("for-tenant", "filt-adapter", "tenant-filt").await;
        let lc = Arc::new(LeaseCoordinator::new());
        let registry =
            PlatformAdapterRegistry::from_json(&config, lc, Duration::from_secs(1)).await?;

        let matches = registry.adapters_for_tenant("tenant-filt");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "filt-adapter");

        let empty = registry.adapters_for_tenant("no-such-tenant");
        assert!(empty.is_empty());
        Ok(())
    }

    // --- Acquire without lease ---

    #[tokio::test]
    async fn acquire_without_lease() -> Result<()> {
        let (_path, config) =
            setup_single_adapter("no-lease", "nolease-adapter", "tenant-nolease").await;
        let lc = Arc::new(LeaseCoordinator::new());
        let registry =
            PlatformAdapterRegistry::from_json(&config, lc, Duration::from_secs(1)).await?;

        let context = tenant_context("tenant-nolease");
        let lease_handle = registry
            .acquire_adapter_for_tenant(&context, None)
            .await
            .unwrap();
        assert!(lease_handle.lease.is_none());
        assert_eq!(lease_handle.definition.tenant, "tenant-nolease");
        Ok(())
    }

    // --- Lease release ---

    #[tokio::test]
    async fn acquire_release_reacquire() -> Result<()> {
        let (_path, config) =
            setup_single_adapter("reacquire", "reacq-adapter", "tenant-reacq").await;
        let lc = Arc::new(LeaseCoordinator::new());
        let registry =
            PlatformAdapterRegistry::from_json(&config, lc, Duration::from_secs(2)).await?;

        let context = tenant_context("tenant-reacq");

        // First acquisition
        let handle = registry
            .acquire_adapter_for_tenant(&context, Some("res:reacq"))
            .await
            .unwrap();
        let lease = handle.lease.unwrap();

        // Release the lease
        registry.release_lease(lease).await;

        // Should be able to acquire again
        let handle2 = registry
            .acquire_adapter_for_tenant(&context, Some("res:reacq"))
            .await
            .unwrap();
        assert!(handle2.lease.is_some());
        Ok(())
    }

    // --- Lease conflict ---

    #[tokio::test]
    async fn double_acquire_same_resource_fails() -> Result<()> {
        let (_path, config) =
            setup_single_adapter("conflict", "conflict-adapter", "tenant-conflict").await;
        let lc = Arc::new(LeaseCoordinator::new());
        let registry =
            PlatformAdapterRegistry::from_json(&config, lc, Duration::from_secs(60)).await?;

        let context = tenant_context("tenant-conflict");

        let _first = registry
            .acquire_adapter_for_tenant(&context, Some("res:conflict"))
            .await
            .unwrap();

        let second = registry
            .acquire_adapter_for_tenant(&context, Some("res:conflict"))
            .await;
        assert!(second.is_err());
        assert!(matches!(
            second.unwrap_err(),
            PlatformAdapterError::LeaseFailure(_)
        ));
        Ok(())
    }

    // --- Execute with lease ---

    #[tokio::test]
    async fn execute_with_lease_succeeds() -> Result<()> {
        let (_path, config) = setup_single_adapter("exec", "exec-adapter", "tenant-exec").await;
        let lc = Arc::new(LeaseCoordinator::new());
        let registry =
            PlatformAdapterRegistry::from_json(&config, lc, Duration::from_secs(5)).await?;

        let context = tenant_context("tenant-exec");
        let result = registry
            .execute_with_lease(&context, "exec:tenant-exec", "SELECT 42")
            .await
            .unwrap();
        assert_eq!(result.tenant, "tenant-exec");
        assert_eq!(result.adapter_name, "exec-adapter");
        assert!(result.lease_resource.is_some());
        assert!(!result.output.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn execute_with_lease_missing_tenant() {
        let lc = Arc::new(LeaseCoordinator::new());
        let registry = PlatformAdapterRegistry::empty(lc, Duration::from_secs(1));
        let context = tenant_context("ghost");
        let err = registry
            .execute_with_lease(&context, "res:ghost", "SELECT 1")
            .await
            .unwrap_err();
        assert!(matches!(err, PlatformAdapterError::TenantMissing(_)));
    }

    // --- Health check ---

    #[tokio::test]
    async fn health_check_passes_for_json_adapter() -> Result<()> {
        let (_path, config) = setup_single_adapter("hc", "hc-adapter", "tenant-hc").await;
        let lc = Arc::new(LeaseCoordinator::new());
        let registry =
            PlatformAdapterRegistry::from_json(&config, lc, Duration::from_secs(1)).await?;

        let context = tenant_context("tenant-hc");
        let result = registry.health_check(&context).await.unwrap();
        assert!(result.healthy);
        assert_eq!(result.driver, "jsonfile");
        assert_eq!(result.tenant, "tenant-hc");
        Ok(())
    }

    #[tokio::test]
    async fn health_check_missing_tenant_errors() {
        let lc = Arc::new(LeaseCoordinator::new());
        let registry = PlatformAdapterRegistry::empty(lc, Duration::from_secs(1));
        let context = tenant_context("nobody");
        let err = registry.health_check(&context).await.unwrap_err();
        assert!(matches!(err, PlatformAdapterError::TenantMissing(_)));
    }

    // --- Metrics ---

    #[tokio::test]
    async fn metrics_track_operations() -> Result<()> {
        let (_path, config) =
            setup_single_adapter("metrics", "metrics-adapter", "tenant-metrics").await;
        let lc = Arc::new(LeaseCoordinator::new());
        let registry =
            PlatformAdapterRegistry::from_json(&config, lc, Duration::from_secs(5)).await?;

        let context = tenant_context("tenant-metrics");

        // Initial state
        let snap = registry.metrics_snapshot();
        assert_eq!(snap.acquires, 0);
        assert_eq!(snap.releases, 0);

        // Acquire without lease
        let _ = registry
            .acquire_adapter_for_tenant(&context, None)
            .await
            .unwrap();
        assert_eq!(registry.metrics_snapshot().acquires, 1);

        // Execute with lease (acquire + execute + release)
        let _ = registry
            .execute_with_lease(&context, "res:metrics", "SELECT 1")
            .await
            .unwrap();
        let snap = registry.metrics_snapshot();
        assert_eq!(snap.acquires, 2);
        assert_eq!(snap.executions, 1);
        assert_eq!(snap.releases, 1);

        // Health check
        let _ = registry.health_check(&context).await.unwrap();
        assert_eq!(registry.metrics_snapshot().health_checks, 1);

        Ok(())
    }

    #[tokio::test]
    async fn metrics_track_failures() {
        let lc = Arc::new(LeaseCoordinator::new());
        let registry = PlatformAdapterRegistry::empty(lc, Duration::from_secs(1));
        let context = tenant_context("ghost");

        let _ = registry.acquire_adapter_for_tenant(&context, None).await;
        assert_eq!(registry.metrics_snapshot().acquire_failures, 1);

        let _ = registry
            .execute_with_lease(&context, "res:ghost", "SELECT 1")
            .await;
        assert_eq!(registry.metrics_snapshot().acquire_failures, 2);
    }

    // --- Lease TTL accessor ---

    #[test]
    fn lease_ttl_returns_configured_value() {
        let lc = Arc::new(LeaseCoordinator::new());
        let registry = PlatformAdapterRegistry::empty(lc, Duration::from_secs(42));
        assert_eq!(registry.lease_ttl(), Duration::from_secs(42));
    }

    // --- Definition serialisation ---

    #[test]
    fn definition_serializes_to_json() {
        let def = PlatformAdapterDefinition {
            name: "test".to_string(),
            tenant: "t1".to_string(),
            driver: "postgres".to_string(),
            info: "localhost:5432".to_string(),
        };
        let json = serde_json::to_string(&def).unwrap();
        assert!(json.contains("\"name\":\"test\""));
        assert!(json.contains("\"driver\":\"postgres\""));
    }

    // --- AdapterMetrics standalone ---

    #[test]
    fn adapter_metrics_snapshot_reflects_increments() {
        let m = AdapterMetrics::default();
        m.acquires.fetch_add(3, Ordering::Relaxed);
        m.execution_failures.fetch_add(1, Ordering::Relaxed);
        let snap = m.snapshot();
        assert_eq!(snap.acquires, 3);
        assert_eq!(snap.execution_failures, 1);
        assert_eq!(snap.releases, 0);
    }

    // --- Error display ---

    #[test]
    fn error_display_messages() {
        let e1 = PlatformAdapterError::TenantMissing("t1".into());
        assert!(e1.to_string().contains("t1"));

        let e2 = PlatformAdapterError::AdapterNotFound("missing".into());
        assert!(e2.to_string().contains("missing"));

        let e3 = PlatformAdapterError::HealthCheckFailed {
            name: "pg".into(),
            reason: "timeout".into(),
        };
        assert!(e3.to_string().contains("timeout"));
    }

    // --- AdapterHealthResult serialization ---

    #[test]
    fn health_result_serializes() {
        let hr = AdapterHealthResult {
            name: "a1".into(),
            tenant: "t1".into(),
            driver: "sqlite".into(),
            healthy: true,
            message: Some("ok".into()),
        };
        let json = serde_json::to_string(&hr).unwrap();
        assert!(json.contains("\"healthy\":true"));
    }

    // --- AdapterExecutionResult serialization ---

    #[test]
    fn execution_result_serializes() {
        let er = AdapterExecutionResult {
            tenant: "t1".into(),
            adapter_name: "a1".into(),
            output: "done".into(),
            lease_resource: Some("res:t1".into()),
        };
        let json = serde_json::to_string(&er).unwrap();
        assert!(json.contains("\"output\":\"done\""));
    }
}
