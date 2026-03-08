#![forbid(unsafe_code)]

//! Tenant-safe migration runner that acquires consensus leases and emits
//! status for admin tooling.
//!
//! Features:
//! - Register migrations per tenant with unique IDs
//! - Run single / all-pending / by-ID migrations with lease protection
//! - Dry-run validation (checks script is non-empty without executing)
//! - Rollback support
//! - Batch execution with `run_all_pending`
//! - State queries: count by state, check completion, list pending
//! - Transfer migrations between adapters via `MigrationTransfer`

use adapter_db::{export_migration_snapshot, import_migration_snapshot, DatabaseAdapter};
use anyhow::{anyhow, Result};
use platform_consensus::LeaseCoordinator;
use platform_tenancy::TenantContext;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration as StdDuration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::Mutex;
use tokio::time::Duration;

// ---------------------------------------------------------------------------
// Spec
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub struct MigrationSpec {
    pub id: String,
    pub tenant_id: String,
    pub description: String,
    pub script: String,
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum MigrationState {
    Pending,
    Running,
    Applied,
    Failed,
    RolledBack,
    DryRunOk,
    DryRunFailed,
}

// ---------------------------------------------------------------------------
// Lease info
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MigrationLeaseInfo {
    pub resource: String,
    pub owner: String,
    pub ttl_seconds: u64,
}

// ---------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct MigrationStatus {
    pub tenant_id: String,
    pub migration_id: String,
    pub description: String,
    pub state: MigrationState,
    pub message: Option<String>,
    pub lease: Option<MigrationLeaseInfo>,
    pub last_updated_epoch: u64,
}

// ---------------------------------------------------------------------------
// State counts
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
pub struct MigrationStateCounts {
    pub pending: usize,
    pub running: usize,
    pub applied: usize,
    pub failed: usize,
    pub rolled_back: usize,
    pub dry_run_ok: usize,
    pub dry_run_failed: usize,
}

// ---------------------------------------------------------------------------
// Internal stored entry
// ---------------------------------------------------------------------------

struct StoredMigration {
    spec: MigrationSpec,
    state: MigrationState,
    message: Option<String>,
    lease: Option<MigrationLeaseInfo>,
    last_updated_epoch: u64,
}

impl StoredMigration {
    fn new(spec: MigrationSpec) -> Self {
        Self {
            spec,
            state: MigrationState::Pending,
            message: None,
            lease: None,
            last_updated_epoch: now_epoch_seconds(),
        }
    }

    fn to_status(&self) -> MigrationStatus {
        MigrationStatus {
            tenant_id: self.spec.tenant_id.clone(),
            migration_id: self.spec.id.clone(),
            description: self.spec.description.clone(),
            state: self.state.clone(),
            message: self.message.clone(),
            lease: self.lease.clone(),
            last_updated_epoch: self.last_updated_epoch,
        }
    }
}

// ---------------------------------------------------------------------------
// Registration error
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistrationError {
    DuplicateId {
        tenant_id: String,
        migration_id: String,
    },
    EmptyId,
    EmptyScript {
        migration_id: String,
    },
}

impl std::fmt::Display for RegistrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateId {
                tenant_id,
                migration_id,
            } => write!(
                f,
                "migration '{migration_id}' already registered for tenant '{tenant_id}'"
            ),
            Self::EmptyId => write!(f, "migration ID must not be empty"),
            Self::EmptyScript { migration_id } => {
                write!(f, "migration '{migration_id}' has an empty script")
            }
        }
    }
}

impl std::error::Error for RegistrationError {}

// ---------------------------------------------------------------------------
// Runner
// ---------------------------------------------------------------------------

pub struct MigrationRunner {
    migrations: Mutex<HashMap<String, Vec<StoredMigration>>>,
    lease_coordinator: Arc<LeaseCoordinator>,
    lease_ttl: Duration,
}

impl MigrationRunner {
    pub fn new(lease_coordinator: Arc<LeaseCoordinator>, lease_ttl: Duration) -> Self {
        Self {
            migrations: Mutex::new(HashMap::new()),
            lease_coordinator,
            lease_ttl,
        }
    }

    pub fn default() -> Self {
        Self::new(Arc::new(LeaseCoordinator::new()), Duration::from_secs(30))
    }

    // --- Registration ---

    /// Register a migration. Returns an error if the ID is duplicate or invalid.
    pub async fn register(&self, spec: MigrationSpec) -> Result<(), RegistrationError> {
        if spec.id.trim().is_empty() {
            return Err(RegistrationError::EmptyId);
        }
        if spec.script.trim().is_empty() {
            return Err(RegistrationError::EmptyScript {
                migration_id: spec.id.clone(),
            });
        }

        let mut lock = self.migrations.lock().await;
        let entry = lock.entry(spec.tenant_id.clone()).or_default();

        if entry.iter().any(|e| e.spec.id == spec.id) {
            return Err(RegistrationError::DuplicateId {
                tenant_id: spec.tenant_id.clone(),
                migration_id: spec.id.clone(),
            });
        }

        entry.push(StoredMigration::new(spec));
        Ok(())
    }

    /// Register a migration without validation (legacy compat).
    pub async fn register_unchecked(&self, spec: MigrationSpec) {
        let mut lock = self.migrations.lock().await;
        let entry = lock.entry(spec.tenant_id.clone()).or_default();
        entry.push(StoredMigration::new(spec));
    }

    // --- Listing ---

    pub async fn list_for_tenant(&self, tenant_id: &str) -> Vec<MigrationStatus> {
        let lock = self.migrations.lock().await;
        lock.get(tenant_id)
            .map(|list| list.iter().map(|entry| entry.to_status()).collect())
            .unwrap_or_default()
    }

    pub async fn list_all(&self) -> Vec<MigrationStatus> {
        let lock = self.migrations.lock().await;
        lock.values()
            .flat_map(|entries| entries.iter().map(|entry| entry.to_status()))
            .collect()
    }

    /// List only pending migrations for a tenant, in registration order.
    pub async fn list_pending(&self, tenant_id: &str) -> Vec<MigrationStatus> {
        let lock = self.migrations.lock().await;
        lock.get(tenant_id)
            .map(|entries| {
                entries
                    .iter()
                    .filter(|e| e.state == MigrationState::Pending)
                    .map(|e| e.to_status())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Count migrations by state for a tenant.
    pub async fn state_counts(&self, tenant_id: &str) -> MigrationStateCounts {
        let lock = self.migrations.lock().await;
        let entries = match lock.get(tenant_id) {
            Some(e) => e,
            None => return MigrationStateCounts::default(),
        };

        let mut counts = MigrationStateCounts::default();
        for entry in entries {
            match entry.state {
                MigrationState::Pending => counts.pending += 1,
                MigrationState::Running => counts.running += 1,
                MigrationState::Applied => counts.applied += 1,
                MigrationState::Failed => counts.failed += 1,
                MigrationState::RolledBack => counts.rolled_back += 1,
                MigrationState::DryRunOk => counts.dry_run_ok += 1,
                MigrationState::DryRunFailed => counts.dry_run_failed += 1,
            }
        }
        counts
    }

    /// Check whether all migrations for a tenant have been applied.
    pub async fn all_applied(&self, tenant_id: &str) -> bool {
        let lock = self.migrations.lock().await;
        match lock.get(tenant_id) {
            Some(entries) => {
                !entries.is_empty() && entries.iter().all(|e| e.state == MigrationState::Applied)
            }
            None => false,
        }
    }

    /// Total number of registered migrations across all tenants.
    pub async fn total_count(&self) -> usize {
        let lock = self.migrations.lock().await;
        lock.values().map(|v| v.len()).sum()
    }

    // --- Execution ---

    pub async fn run_for_tenant(
        &self,
        tenant: &TenantContext,
        adapter: Arc<dyn DatabaseAdapter>,
        migration_id: Option<&str>,
    ) -> Result<MigrationStatus> {
        let target_id = migration_id.map(|s| s.to_string());

        let spec = {
            let mut lock = self.migrations.lock().await;
            let entries = lock.get_mut(&tenant.tenant_id).ok_or_else(|| {
                anyhow!("no migrations registered for tenant {}", tenant.tenant_id)
            })?;

            let index = if let Some(ref id) = target_id {
                entries.iter().position(|entry| entry.spec.id == *id)
            } else {
                entries
                    .iter()
                    .position(|entry| entry.state == MigrationState::Pending)
            }
            .ok_or_else(|| anyhow!("no runnable migration for tenant {}", tenant.tenant_id))?;

            let entry = &mut entries[index];
            if entry.state != MigrationState::Pending {
                return Err(anyhow!(
                    "migration {} is already running or complete",
                    entry.spec.id
                ));
            }
            entry.state = MigrationState::Running;
            entry.last_updated_epoch = now_epoch_seconds();
            entry.message = None;
            entry.lease = None;
            entry.spec.clone()
        };

        let resource = format!("migration:{}", tenant.tenant_id);
        let lease = self
            .lease_coordinator
            .acquire(&resource, &tenant.tenant_id, self.lease_ttl)
            .await?;
        let lease_info = MigrationLeaseInfo {
            resource: lease.resource.clone(),
            owner: lease.owner.clone(),
            ttl_seconds: self.lease_ttl.as_secs(),
        };

        let execution = adapter.execute_script(&spec.script).await;
        self.lease_coordinator
            .release(&lease.resource, &lease.owner)
            .await;

        let mut lock = self.migrations.lock().await;
        let entries = lock
            .get_mut(&tenant.tenant_id)
            .expect("entry exists after previous lookup");
        let stored = entries
            .iter_mut()
            .find(|entry| entry.spec.id == spec.id)
            .expect("entry still present");

        stored.lease = Some(lease_info);
        stored.last_updated_epoch = now_epoch_seconds();
        match execution {
            Ok(output) => {
                stored.state = MigrationState::Applied;
                stored.message = Some(output);
            }
            Err(err) => {
                stored.state = MigrationState::Failed;
                stored.message = Some(format!("migration failed: {err}"));
            }
        }

        Ok(stored.to_status())
    }

    /// Run all pending migrations for a tenant in registration order.
    /// Stops on the first failure and returns all statuses up to that point.
    pub async fn run_all_pending(
        &self,
        tenant: &TenantContext,
        adapter: Arc<dyn DatabaseAdapter>,
    ) -> Result<Vec<MigrationStatus>> {
        let mut results = Vec::new();

        loop {
            let has_pending = {
                let lock = self.migrations.lock().await;
                lock.get(&tenant.tenant_id)
                    .map(|entries| entries.iter().any(|e| e.state == MigrationState::Pending))
                    .unwrap_or(false)
            };

            if !has_pending {
                break;
            }

            let status = self.run_for_tenant(tenant, adapter.clone(), None).await?;
            let failed = status.state == MigrationState::Failed;
            results.push(status);

            if failed {
                break;
            }
        }

        Ok(results)
    }

    // --- Dry run ---

    /// Validate a migration without executing. Sets state to `DryRunOk` or
    /// `DryRunFailed`.
    pub async fn dry_run(&self, tenant_id: &str, migration_id: &str) -> Result<MigrationStatus> {
        let mut lock = self.migrations.lock().await;
        let entries = lock
            .get_mut(tenant_id)
            .ok_or_else(|| anyhow!("no migrations registered for tenant {tenant_id}"))?;
        let stored = entries
            .iter_mut()
            .find(|e| e.spec.id == migration_id)
            .ok_or_else(|| anyhow!("migration {migration_id} not found for tenant {tenant_id}"))?;

        if stored.state != MigrationState::Pending {
            return Err(anyhow!(
                "can only dry-run pending migrations, current state: {:?}",
                stored.state
            ));
        }

        stored.last_updated_epoch = now_epoch_seconds();

        if stored.spec.script.trim().is_empty() {
            stored.state = MigrationState::DryRunFailed;
            stored.message = Some("script is empty".to_string());
        } else {
            stored.state = MigrationState::DryRunOk;
            stored.message = Some(format!(
                "dry run OK, script length {} bytes",
                stored.spec.script.len()
            ));
        }

        Ok(stored.to_status())
    }

    /// Reset a dry-run or failed migration back to pending so it can be retried.
    pub async fn reset_to_pending(
        &self,
        tenant_id: &str,
        migration_id: &str,
    ) -> Result<MigrationStatus> {
        let mut lock = self.migrations.lock().await;
        let entries = lock
            .get_mut(tenant_id)
            .ok_or_else(|| anyhow!("no migrations registered for tenant {tenant_id}"))?;
        let stored = entries
            .iter_mut()
            .find(|e| e.spec.id == migration_id)
            .ok_or_else(|| anyhow!("migration {migration_id} not found for tenant {tenant_id}"))?;

        match stored.state {
            MigrationState::Failed
            | MigrationState::RolledBack
            | MigrationState::DryRunOk
            | MigrationState::DryRunFailed => {
                stored.state = MigrationState::Pending;
                stored.message = Some("reset to pending".to_string());
                stored.lease = None;
                stored.last_updated_epoch = now_epoch_seconds();
                Ok(stored.to_status())
            }
            _ => Err(anyhow!(
                "cannot reset migration in state {:?}",
                stored.state
            )),
        }
    }

    // --- Rollback ---

    pub async fn rollback(&self, tenant_id: &str, migration_id: &str) -> Result<MigrationStatus> {
        let resource = format!("migration:{}", tenant_id);
        self.lease_coordinator.release(&resource, tenant_id).await;

        let mut lock = self.migrations.lock().await;
        let entries = lock
            .get_mut(tenant_id)
            .ok_or_else(|| anyhow!("tenant {tenant_id} has no migrations"))?;
        let stored = entries
            .iter_mut()
            .find(|entry| entry.spec.id == migration_id)
            .ok_or_else(|| anyhow!("migration {migration_id} not found"))?;

        stored.state = MigrationState::RolledBack;
        stored.message = Some("rolled back by admin".into());
        stored.last_updated_epoch = now_epoch_seconds();

        Ok(stored.to_status())
    }
}

// ---------------------------------------------------------------------------
// Transfer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct MigrationTransferStatus {
    pub tenant_id: String,
    pub source_adapter: String,
    pub source_driver: String,
    pub target_adapter: String,
    pub target_driver: String,
    pub script_size: usize,
    pub message: String,
}

pub struct MigrationTransfer;

impl MigrationTransfer {
    pub fn new() -> Self {
        Self
    }

    pub async fn transfer(
        &self,
        source_adapter: Arc<dyn DatabaseAdapter>,
        target_adapter: Arc<dyn DatabaseAdapter>,
    ) -> Result<MigrationTransferStatus> {
        let snapshot = export_migration_snapshot(source_adapter.clone()).await?;
        let import_message = import_migration_snapshot(target_adapter.clone(), &snapshot).await?;

        Ok(MigrationTransferStatus {
            tenant_id: snapshot.tenant,
            source_adapter: snapshot.name,
            source_driver: snapshot.driver,
            target_adapter: target_adapter.name().to_string(),
            target_driver: target_adapter.driver_name().to_string(),
            script_size: snapshot.script_log.len(),
            message: import_message,
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn now_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| StdDuration::from_secs(0))
        .as_secs()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use adapter_db::bootstrap_from_json;
    use platform_tenancy::TenantAccessLevel;
    use rusqlite::Connection;
    use serde_json::json;
    use std::env::temp_dir;
    use tokio::fs::{create_dir_all, remove_dir_all, File};
    use tokio::io::AsyncWriteExt;

    fn spec(id: &str, tenant: &str, script: &str) -> MigrationSpec {
        MigrationSpec {
            id: id.into(),
            tenant_id: tenant.into(),
            description: format!("migration {id}"),
            script: script.into(),
        }
    }

    fn tenant(id: &str) -> TenantContext {
        TenantContext {
            tenant_id: id.into(),
            tenant_name: Some(format!("Tenant {id}")),
            access_level: TenantAccessLevel::Admin,
        }
    }

    async fn recreate_dir(path: &std::path::Path) {
        let _ = remove_dir_all(path).await;
        create_dir_all(path).await.unwrap();
    }

    /// Helper: set up a jsonfile adapter for a tenant.
    async fn setup_json_adapter(dir_name: &str, tenant_id: &str) -> Arc<dyn DatabaseAdapter> {
        let dir = temp_dir().join(format!("migrations-{dir_name}"));
        let path = dir.join("data.json");
        recreate_dir(&dir).await;
        let mut file = File::create(&path).await.unwrap();
        file.write_all(br#"{}"#).await.unwrap();
        file.flush().await.unwrap();

        let config = json!([{
            "name": format!("json-{tenant_id}"),
            "driver": "jsonfile",
            "tenant": tenant_id,
            "path": path.to_string_lossy()
        }])
        .to_string();

        let registry = bootstrap_from_json(&config).await.unwrap();
        registry.get_for_tenant(tenant_id).await.unwrap()
    }

    // --- Registration ---

    #[tokio::test]
    async fn register_and_list() {
        let runner = MigrationRunner::default();
        runner
            .register(spec("001", "t1", "SELECT 1"))
            .await
            .unwrap();
        runner
            .register(spec("002", "t1", "SELECT 2"))
            .await
            .unwrap();

        let list = runner.list_for_tenant("t1").await;
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].migration_id, "001");
        assert_eq!(list[1].migration_id, "002");
    }

    #[tokio::test]
    async fn register_duplicate_id_fails() {
        let runner = MigrationRunner::default();
        runner
            .register(spec("001", "t1", "SELECT 1"))
            .await
            .unwrap();
        let err = runner
            .register(spec("001", "t1", "SELECT 2"))
            .await
            .unwrap_err();
        assert_eq!(
            err,
            RegistrationError::DuplicateId {
                tenant_id: "t1".into(),
                migration_id: "001".into(),
            }
        );
    }

    #[tokio::test]
    async fn register_empty_id_fails() {
        let runner = MigrationRunner::default();
        let err = runner
            .register(spec("", "t1", "SELECT 1"))
            .await
            .unwrap_err();
        assert_eq!(err, RegistrationError::EmptyId);
    }

    #[tokio::test]
    async fn register_empty_script_fails() {
        let runner = MigrationRunner::default();
        let err = runner.register(spec("001", "t1", "  ")).await.unwrap_err();
        assert!(matches!(err, RegistrationError::EmptyScript { .. }));
    }

    #[tokio::test]
    async fn register_same_id_different_tenants_ok() {
        let runner = MigrationRunner::default();
        runner
            .register(spec("001", "t1", "SELECT 1"))
            .await
            .unwrap();
        runner
            .register(spec("001", "t2", "SELECT 1"))
            .await
            .unwrap();
        assert_eq!(runner.list_for_tenant("t1").await.len(), 1);
        assert_eq!(runner.list_for_tenant("t2").await.len(), 1);
    }

    // --- Run single migration ---

    #[tokio::test]
    async fn register_and_run_migration() -> Result<()> {
        let runner = MigrationRunner::default();
        runner
            .register(spec("001", "tenant-a", "SELECT 1"))
            .await
            .unwrap();

        let adapter = setup_json_adapter("run-single", "tenant-a").await;
        let t = tenant("tenant-a");

        let status = runner.run_for_tenant(&t, adapter, None).await?;
        assert_eq!(status.state, MigrationState::Applied);
        assert!(status.message.is_some());
        assert!(status.lease.is_some());
        Ok(())
    }

    #[tokio::test]
    async fn run_by_id() -> Result<()> {
        let runner = MigrationRunner::default();
        runner.register(spec("a", "t", "SELECT 1")).await.unwrap();
        runner.register(spec("b", "t", "SELECT 2")).await.unwrap();

        let adapter = setup_json_adapter("run-by-id", "t").await;
        let t = tenant("t");

        // Run migration "b" specifically (skipping "a")
        let status = runner.run_for_tenant(&t, adapter, Some("b")).await?;
        assert_eq!(status.migration_id, "b");
        assert_eq!(status.state, MigrationState::Applied);

        // "a" should still be pending
        let list = runner.list_for_tenant("t").await;
        assert_eq!(list[0].state, MigrationState::Pending);
        Ok(())
    }

    #[tokio::test]
    async fn run_no_pending_errors() {
        let runner = MigrationRunner::default();
        let t = tenant("nonexistent");
        let adapter = setup_json_adapter("run-none", "nonexistent").await;
        let err = runner.run_for_tenant(&t, adapter, None).await;
        assert!(err.is_err());
    }

    // --- Run all pending ---

    #[tokio::test]
    async fn run_all_pending_runs_in_order() -> Result<()> {
        let runner = MigrationRunner::default();
        runner.register(spec("001", "t", "SELECT 1")).await.unwrap();
        runner.register(spec("002", "t", "SELECT 2")).await.unwrap();
        runner.register(spec("003", "t", "SELECT 3")).await.unwrap();

        let adapter = setup_json_adapter("run-all", "t").await;
        let t = tenant("t");

        let results = runner.run_all_pending(&t, adapter).await?;
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|s| s.state == MigrationState::Applied));
        assert_eq!(results[0].migration_id, "001");
        assert_eq!(results[1].migration_id, "002");
        assert_eq!(results[2].migration_id, "003");
        Ok(())
    }

    #[tokio::test]
    async fn run_all_pending_returns_empty_when_none() -> Result<()> {
        let runner = MigrationRunner::default();
        runner.register(spec("001", "t", "SELECT 1")).await.unwrap();

        let adapter = setup_json_adapter("run-all-empty", "t").await;
        let t = tenant("t");

        // Run first
        let _ = runner.run_for_tenant(&t, adapter.clone(), None).await?;
        // Now run_all_pending should find nothing
        let results = runner.run_all_pending(&t, adapter).await?;
        assert!(results.is_empty());
        Ok(())
    }

    // --- Rollback ---

    #[tokio::test]
    async fn rollback_sets_state() -> Result<()> {
        let runner = MigrationRunner::default();
        runner
            .register(spec("002", "tenant-b", "SELECT 1"))
            .await
            .unwrap();

        let status = runner.rollback("tenant-b", "002").await?;
        assert_eq!(status.state, MigrationState::RolledBack);
        assert!(status.message.unwrap().contains("rolled back"));
        Ok(())
    }

    #[tokio::test]
    async fn rollback_nonexistent_migration_errors() {
        let runner = MigrationRunner::default();
        let err = runner.rollback("t", "999").await;
        assert!(err.is_err());
    }

    // --- Dry run ---

    #[tokio::test]
    async fn dry_run_ok() -> Result<()> {
        let runner = MigrationRunner::default();
        runner.register(spec("001", "t", "SELECT 1")).await.unwrap();

        let status = runner.dry_run("t", "001").await?;
        assert_eq!(status.state, MigrationState::DryRunOk);
        assert!(status.message.unwrap().contains("dry run OK"));
        Ok(())
    }

    #[tokio::test]
    async fn dry_run_nonpending_fails() -> Result<()> {
        let runner = MigrationRunner::default();
        runner.register(spec("001", "t", "SELECT 1")).await.unwrap();

        // Run the migration first
        let adapter = setup_json_adapter("dry-run-np", "t").await;
        let _ = runner.run_for_tenant(&tenant("t"), adapter, None).await?;

        // Now dry_run should fail since it's Applied
        let err = runner.dry_run("t", "001").await;
        assert!(err.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn dry_run_nonexistent_errors() {
        let runner = MigrationRunner::default();
        let err = runner.dry_run("t", "999").await;
        assert!(err.is_err());
    }

    // --- Reset to pending ---

    #[tokio::test]
    async fn reset_failed_to_pending() -> Result<()> {
        let runner = MigrationRunner::default();
        runner.register(spec("001", "t", "SELECT 1")).await.unwrap();

        // Roll it back first
        let _ = runner.rollback("t", "001").await?;
        let status = runner.reset_to_pending("t", "001").await?;
        assert_eq!(status.state, MigrationState::Pending);
        Ok(())
    }

    #[tokio::test]
    async fn reset_dry_run_to_pending() -> Result<()> {
        let runner = MigrationRunner::default();
        runner.register(spec("001", "t", "SELECT 1")).await.unwrap();
        let _ = runner.dry_run("t", "001").await?;

        let status = runner.reset_to_pending("t", "001").await?;
        assert_eq!(status.state, MigrationState::Pending);
        Ok(())
    }

    #[tokio::test]
    async fn reset_applied_fails() -> Result<()> {
        let runner = MigrationRunner::default();
        runner.register(spec("001", "t", "SELECT 1")).await.unwrap();

        let adapter = setup_json_adapter("reset-applied", "t").await;
        let _ = runner.run_for_tenant(&tenant("t"), adapter, None).await?;

        let err = runner.reset_to_pending("t", "001").await;
        assert!(err.is_err());
        Ok(())
    }

    // --- State counts ---

    #[tokio::test]
    async fn state_counts_track_correctly() -> Result<()> {
        let runner = MigrationRunner::default();
        runner.register(spec("001", "t", "SELECT 1")).await.unwrap();
        runner.register(spec("002", "t", "SELECT 2")).await.unwrap();
        runner.register(spec("003", "t", "SELECT 3")).await.unwrap();

        let counts = runner.state_counts("t").await;
        assert_eq!(counts.pending, 3);
        assert_eq!(counts.applied, 0);

        let adapter = setup_json_adapter("state-counts", "t").await;
        let _ = runner
            .run_for_tenant(&tenant("t"), adapter, Some("001"))
            .await?;

        let counts = runner.state_counts("t").await;
        assert_eq!(counts.pending, 2);
        assert_eq!(counts.applied, 1);
        Ok(())
    }

    #[tokio::test]
    async fn state_counts_empty_tenant() {
        let runner = MigrationRunner::default();
        let counts = runner.state_counts("nobody").await;
        assert_eq!(counts, MigrationStateCounts::default());
    }

    // --- All applied ---

    #[tokio::test]
    async fn all_applied_true_when_all_done() -> Result<()> {
        let runner = MigrationRunner::default();
        runner.register(spec("001", "t", "SELECT 1")).await.unwrap();

        assert!(!runner.all_applied("t").await);

        let adapter = setup_json_adapter("all-applied", "t").await;
        let _ = runner.run_for_tenant(&tenant("t"), adapter, None).await?;

        assert!(runner.all_applied("t").await);
        Ok(())
    }

    #[tokio::test]
    async fn all_applied_false_when_no_tenant() {
        let runner = MigrationRunner::default();
        assert!(!runner.all_applied("ghost").await);
    }

    // --- List pending ---

    #[tokio::test]
    async fn list_pending_filters_correctly() -> Result<()> {
        let runner = MigrationRunner::default();
        runner.register(spec("001", "t", "SELECT 1")).await.unwrap();
        runner.register(spec("002", "t", "SELECT 2")).await.unwrap();

        let adapter = setup_json_adapter("list-pending", "t").await;
        let _ = runner
            .run_for_tenant(&tenant("t"), adapter, Some("001"))
            .await?;

        let pending = runner.list_pending("t").await;
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].migration_id, "002");
        Ok(())
    }

    // --- Total count ---

    #[tokio::test]
    async fn total_count_across_tenants() {
        let runner = MigrationRunner::default();
        runner
            .register(spec("001", "t1", "SELECT 1"))
            .await
            .unwrap();
        runner
            .register(spec("002", "t1", "SELECT 2"))
            .await
            .unwrap();
        runner
            .register(spec("001", "t2", "SELECT 1"))
            .await
            .unwrap();

        assert_eq!(runner.total_count().await, 3);
    }

    // --- List all ---

    #[tokio::test]
    async fn list_all_spans_tenants() {
        let runner = MigrationRunner::default();
        runner
            .register(spec("001", "t1", "SELECT 1"))
            .await
            .unwrap();
        runner
            .register(spec("001", "t2", "SELECT 1"))
            .await
            .unwrap();

        let all = runner.list_all().await;
        assert_eq!(all.len(), 2);
    }

    // --- Registration error display ---

    #[test]
    fn registration_error_display() {
        let e1 = RegistrationError::DuplicateId {
            tenant_id: "t1".into(),
            migration_id: "001".into(),
        };
        assert!(e1.to_string().contains("already registered"));

        let e2 = RegistrationError::EmptyId;
        assert!(e2.to_string().contains("empty"));

        let e3 = RegistrationError::EmptyScript {
            migration_id: "m1".into(),
        };
        assert!(e3.to_string().contains("empty script"));
    }

    // --- MigrationState serialization ---

    #[test]
    fn migration_state_serializes() {
        let json = serde_json::to_string(&MigrationState::Applied).unwrap();
        assert!(json.contains("Applied"));
    }

    // --- MigrationTransfer ---

    #[tokio::test]
    async fn migration_transfer_copies_json_to_sqlite() -> Result<()> {
        let dir = temp_dir().join("migration-transfer-2");
        let json_path = dir.join("tenant-data.json");
        let sqlite_path = dir.join("tenant-db.sqlite3");
        let sqlite_log = dir.join("tenant-db.log.sql");
        recreate_dir(&dir).await;

        let mut file = File::create(&json_path).await?;
        file.write_all(br#"{}"#).await?;
        file.flush().await?;

        let json_config = json!([{
            "name": "tenant-json",
            "driver": "jsonfile",
            "tenant": "tenant-migrate",
            "path": json_path.to_string_lossy()
        }])
        .to_string();

        let json_registry = bootstrap_from_json(&json_config).await?;
        let json_adapter = json_registry
            .get_for_tenant("tenant-migrate")
            .await
            .expect("json adapter");

        json_adapter
            .execute_script(
                "CREATE TABLE mig_transfer (id INTEGER PRIMARY KEY);\nINSERT INTO mig_transfer (id) VALUES (42);",
            )
            .await?;

        let sqlite_config = json!([{
            "name": "tenant-sqlite",
            "driver": "sqlite",
            "tenant": "tenant-migrate",
            "path": sqlite_path.to_string_lossy(),
            "logPath": sqlite_log.to_string_lossy()
        }])
        .to_string();

        let sqlite_registry = bootstrap_from_json(&sqlite_config).await?;
        let sqlite_adapter = sqlite_registry
            .get_for_tenant("tenant-migrate")
            .await
            .expect("sqlite adapter");

        let transfer = MigrationTransfer::new();
        let status = transfer
            .transfer(json_adapter, sqlite_adapter.clone())
            .await?;

        assert_eq!(status.tenant_id, "tenant-migrate");
        assert_eq!(status.source_driver, "jsonfile");
        assert_eq!(status.target_driver, "sqlite");
        assert!(status.script_size > 0);

        let conn = Connection::open(&sqlite_path)?;
        let value: i64 = conn.query_row("SELECT id FROM mig_transfer LIMIT 1;", [], |row| {
            row.get::<_, i64>(0)
        })?;
        assert_eq!(value, 42);

        Ok(())
    }

    // --- MigrationStateCounts serialization ---

    #[test]
    fn state_counts_serializes() {
        let counts = MigrationStateCounts {
            pending: 2,
            applied: 1,
            ..Default::default()
        };
        let json = serde_json::to_string(&counts).unwrap();
        assert!(json.contains("\"pending\":2"));
        assert!(json.contains("\"applied\":1"));
    }

    // --- MigrationLeaseInfo equality ---

    #[test]
    fn lease_info_equality() {
        let a = MigrationLeaseInfo {
            resource: "r".into(),
            owner: "o".into(),
            ttl_seconds: 30,
        };
        let b = a.clone();
        assert_eq!(a, b);
    }

    // --- Transfer status serialization ---

    #[test]
    fn transfer_status_serializes() {
        let ts = MigrationTransferStatus {
            tenant_id: "t1".into(),
            source_adapter: "src".into(),
            source_driver: "jsonfile".into(),
            target_adapter: "tgt".into(),
            target_driver: "sqlite".into(),
            script_size: 100,
            message: "ok".into(),
        };
        let json = serde_json::to_string(&ts).unwrap();
        assert!(json.contains("\"script_size\":100"));
    }
}
