//! Platform-level adapter registry that wraps `adapter-db`, injects tenant context, enforces consensus leases, and reports health.

use adapter_db::{bootstrap_from_json, AdapterEntry, AdapterRegistry};
use anyhow::{Context, Result};
use platform_consensus::{LeaseCoordinator, LeaseToken};
use platform_tenancy::TenantContext;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use tokio::time::Duration;

#[derive(Debug, Clone, Serialize)]
pub struct PlatformAdapterDefinition {
    pub name: String,
    pub tenant: String,
    pub driver: String,
    pub info: String,
}

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

#[derive(Debug, Error)]
pub enum PlatformAdapterError {
    #[error("tenant {0} not registered")]
    TenantMissing(String),

    #[error("lease acquisition failed: {0}")]
    LeaseFailure(#[source] anyhow::Error),
}

pub struct PlatformAdapterRegistry {
    registry: Arc<AdapterRegistry>,
    definitions: HashMap<String, PlatformAdapterDefinition>,
    lease_coordinator: Arc<LeaseCoordinator>,
    lease_ttl: Duration,
}

impl PlatformAdapterDefinition {
    fn from_entry(entry: &AdapterEntry) -> Self {
        let (driver, info, tenant) = match &entry.driver {
            adapter_db::AdapterDriver::Postgres { url, tenant } => {
                ("postgres", url.clone(), tenant.clone())
            }
            adapter_db::AdapterDriver::Mysql { url, tenant } => {
                ("mysql", url.clone(), tenant.clone())
            }
            adapter_db::AdapterDriver::JsonFile { path, tenant } => {
                ("jsonfile", path.clone(), tenant.clone())
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

impl PlatformAdapterRegistry {
    pub fn empty(lease_coordinator: Arc<LeaseCoordinator>, lease_ttl: Duration) -> Self {
        Self {
            registry: Arc::new(AdapterRegistry::new()),
            definitions: HashMap::new(),
            lease_coordinator,
            lease_ttl,
        }
    }

    pub async fn from_json_file<P: AsRef<Path>>(
        path: P,
        lease_coordinator: Arc<LeaseCoordinator>,
        lease_ttl: Duration,
    ) -> Result<Self> {
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("reading adapter registry from {:?}", path.as_ref()))?;
        Self::from_json(&contents, lease_coordinator, lease_ttl).await
    }

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
        })
    }

    pub async fn acquire_adapter_for_tenant(
        &self,
        context: &TenantContext,
        resource_hint: Option<&str>,
    ) -> Result<PlatformAdapterLease, PlatformAdapterError> {
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

        let lease = if let Some(resource) = resource_hint {
            Some(
                self.lease_coordinator
                    .acquire(resource, &context.tenant_id, self.lease_ttl)
                    .await
                    .map_err(PlatformAdapterError::LeaseFailure)?,
            )
        } else {
            None
        };

        Ok(PlatformAdapterLease {
            adapter,
            lease,
            definition,
        })
    }

    pub async fn release_lease(&self, lease: LeaseToken) {
        self.lease_coordinator
            .release(&lease.resource, &lease.owner)
            .await;
    }

    pub fn health_snapshot(&self) -> Vec<PlatformAdapterDefinition> {
        self.definitions.values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform_consensus::LeaseCoordinator;
    use platform_tenancy::{TenantAccessLevel, TenantContext};
    use std::env::temp_dir;
    use tokio::fs::File;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn adapter_registry_roundtrip() -> Result<()> {
        let dir = temp_dir().join("platform-adapter-test");
        let path = dir.join("adapter.json");
        tokio::fs::create_dir_all(&dir).await?;
        let mut file = File::create(&path).await?;
        let data_path = dir.join("tenant-data.json");
        let mut data_file = File::create(&data_path).await?;
        data_file.write_all(br#"{}"#).await?;
        data_file.flush().await?;
        drop(data_file);

        let config = serde_json::json!([
            {
                "name": "t-json",
                "driver": "jsonfile",
                "tenant": "tenant-a",
                "path": data_path.to_string_lossy()
            }
        ]);
        file.write_all(config.to_string().as_bytes()).await?;
        file.flush().await?;
        drop(file);

        let lease_coordinator = Arc::new(LeaseCoordinator::new());
        let registry = PlatformAdapterRegistry::from_json_file(
            &path,
            lease_coordinator,
            Duration::from_secs(1),
        )
        .await
        .context("bootstrap adapter registry")?;

        let context = TenantContext {
            tenant_id: "tenant-a".into(),
            tenant_name: None,
            access_level: TenantAccessLevel::Worker,
        };
        let lease = registry
            .acquire_adapter_for_tenant(&context, Some("adapter:tenant-a"))
            .await
            .expect("adapter ready");
        assert_eq!(lease.definition.tenant, "tenant-a");
        Ok(())
    }

    #[tokio::test]
    async fn health_snapshot_reports_definitions() -> Result<()> {
        let dir = temp_dir().join("platform-adapter-test-health");
        let path = dir.join("adapter.json");
        tokio::fs::create_dir_all(&dir).await?;
        let mut file = File::create(&path).await?;
        let data_path = dir.join("tenant-data.json");
        let mut data_file = File::create(&data_path).await?;
        data_file.write_all(br#"{}"#).await?;
        data_file.flush().await?;

        let config = serde_json::json!([
            {
                "name": "health-json",
                "driver": "jsonfile",
                "tenant": "tenant-health",
                "path": data_path.to_string_lossy()
            }
        ]);
        file.write_all(config.to_string().as_bytes()).await?;
        file.flush().await?;

        let lease_coordinator = Arc::new(LeaseCoordinator::new());
        let registry = PlatformAdapterRegistry::from_json_file(
            &path,
            lease_coordinator,
            Duration::from_secs(1),
        )
        .await?;

        let snapshot = registry.health_snapshot();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].tenant, "tenant-health");
        Ok(())
    }
}
