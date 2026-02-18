//! Tenant-safe migration runner that acquires consensus leases and emits status for admin tooling.

use adapter_db::DatabaseAdapter;
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

#[derive(Debug, Deserialize, Clone)]
pub struct MigrationSpec {
    pub id: String,
    pub tenant_id: String,
    pub description: String,
    pub script: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum MigrationState {
    Pending,
    Running,
    Applied,
    Failed,
    RolledBack,
}

#[derive(Debug, Clone, Serialize)]
pub struct MigrationLeaseInfo {
    pub resource: String,
    pub owner: String,
    pub ttl_seconds: u64,
}

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

    pub async fn register(&self, spec: MigrationSpec) {
        let mut lock = self.migrations.lock().await;
        let entry = lock.entry(spec.tenant_id.clone()).or_insert_with(Vec::new);
        entry.push(StoredMigration::new(spec));
    }

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

        stored.lease = Some(lease_info.clone());
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

fn now_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| StdDuration::from_secs(0))
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use adapter_db::bootstrap_from_json;
    use serde_json::json;
    use std::env::temp_dir;
    use tokio::fs::{create_dir_all, File};
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn register_and_run_migration() -> Result<()> {
        let runner = MigrationRunner::default();
        runner
            .register(MigrationSpec {
                id: "001".into(),
                tenant_id: "tenant-a".into(),
                description: "bump".into(),
                script: "SELECT 1".into(),
            })
            .await;

        let dir = temp_dir().join("migrations-json");
        let path = dir.join("data.json");
        create_dir_all(&dir).await?;
        let mut file = File::create(&path).await?;
        file.write_all(br#"{}"#).await?;
        file.flush().await?;

        let config = json!([
            {
                "name": "tenant-json",
                "driver": "jsonfile",
                "tenant": "tenant-a",
                "path": path.to_string_lossy()
            }
        ])
        .to_string();

        let registry = bootstrap_from_json(&config).await?;
        let adapter = registry
            .get_for_tenant("tenant-a")
            .await
            .expect("adapter configured");

        let tenant = TenantContext {
            tenant_id: "tenant-a".into(),
            tenant_name: Some("Tenant A".into()),
            access_level: platform_tenancy::TenantAccessLevel::Admin,
        };

        let status = runner
            .run_for_tenant(&tenant, adapter, None)
            .await
            .expect("runs");
        assert_eq!(status.state, MigrationState::Applied);
        assert!(status.message.is_some());
        Ok(())
    }

    #[tokio::test]
    async fn rollback_sets_state() -> Result<()> {
        let runner = MigrationRunner::default();
        runner
            .register(MigrationSpec {
                id: "002".into(),
                tenant_id: "tenant-b".into(),
                description: "rollback test".into(),
                script: "SELECT 1".into(),
            })
            .await;

        let status = runner
            .rollback("tenant-b", "002")
            .await
            .expect("rollback succeed");
        assert_eq!(status.state, MigrationState::RolledBack);
        Ok(())
    }
}
