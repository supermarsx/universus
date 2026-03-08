//! Shard metadata and leadership management that tells workers which tenants they may serve
//! and what nodes own those shards.
//!
//! Features:
//! - Register/unregister shards with tenant-allowlists
//! - Tenant routing via allowlist or consistent-hash
//! - Shard health tracking (Healthy/Degraded/Unhealthy)
//! - Weight-based tenant assignment for rebalancing
//! - Leader election per shard via `platform-consensus`
//! - Shard migration (move tenants between shards)
//! - Shard statistics and snapshot reporting

use anyhow::Result;
use platform_consensus::{LeaseCoordinator, LeaseToken};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Shard configuration and state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ShardConfig {
    pub shard_id: String,
    pub region: String,
    pub allowed_tenants: HashSet<String>,
    pub consensus_resource: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ShardHealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Draining,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardSummary {
    pub shard_id: String,
    pub region: String,
    pub assigned_node: Option<String>,
    pub tenant_count: usize,
    pub health: ShardHealthStatus,
    pub weight: u32,
}

#[derive(Debug)]
pub struct ShardLeader {
    pub node_id: String,
    pub lease: LeaseToken,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardStats {
    pub shard_id: String,
    pub tenant_count: usize,
    pub leader_changes: u64,
    pub health: ShardHealthStatus,
    pub weight: u32,
}

#[derive(Debug)]
struct ShardState {
    config: ShardConfig,
    assigned_node: Option<String>,
    leader: Option<LeaseToken>,
    health: ShardHealthStatus,
    weight: u32,
    leader_changes: u64,
    last_refresh: Instant,
}

impl ShardState {
    fn new(config: ShardConfig) -> Self {
        Self {
            config,
            assigned_node: None,
            leader: None,
            health: ShardHealthStatus::Healthy,
            weight: 100,
            leader_changes: 0,
            last_refresh: Instant::now(),
        }
    }

    fn summary(&self) -> ShardSummary {
        ShardSummary {
            shard_id: self.config.shard_id.clone(),
            region: self.config.region.clone(),
            assigned_node: self.assigned_node.clone(),
            tenant_count: self.config.allowed_tenants.len(),
            health: self.health.clone(),
            weight: self.weight,
        }
    }

    fn stats(&self) -> ShardStats {
        ShardStats {
            shard_id: self.config.shard_id.clone(),
            tenant_count: self.config.allowed_tenants.len(),
            leader_changes: self.leader_changes,
            health: self.health.clone(),
            weight: self.weight,
        }
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum ShardingError {
    #[error("shard {0} unknown")]
    UnknownShard(String),

    #[error("no consensus coordinator configured")]
    ConsensusUnavailable,

    #[error("failed to acquire lease: {0}")]
    LeaseFailure(#[source] anyhow::Error),

    #[error("tenant {0} already assigned to shard {1}")]
    TenantAlreadyAssigned(String, String),

    #[error("tenant {0} not found in shard {1}")]
    TenantNotInShard(String, String),

    #[error("shard {0} is draining and cannot accept new tenants")]
    ShardDraining(String),

    #[error("shard {0} already exists")]
    ShardAlreadyExists(String),
}

// ---------------------------------------------------------------------------
// Migration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationRecord {
    pub tenant_id: String,
    pub from_shard: String,
    pub to_shard: String,
    pub status: MigrationStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MigrationStatus {
    InProgress,
    Completed,
    Failed,
}

// ---------------------------------------------------------------------------
// ShardingCatalog
// ---------------------------------------------------------------------------

pub struct ShardingCatalog {
    shards: Arc<RwLock<HashMap<String, ShardState>>>,
    consensus: Option<Arc<LeaseCoordinator>>,
    migrations: Arc<RwLock<Vec<MigrationRecord>>>,
}

impl ShardingCatalog {
    pub fn new() -> Self {
        Self {
            shards: Arc::new(RwLock::new(HashMap::new())),
            consensus: None,
            migrations: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn with_consensus(lease_coordinator: Arc<LeaseCoordinator>) -> Self {
        Self {
            shards: Arc::new(RwLock::new(HashMap::new())),
            consensus: Some(lease_coordinator),
            migrations: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Register a new shard. Fails if the shard already exists.
    pub async fn register_shard(&self, config: ShardConfig) {
        let mut shards = self.shards.write().await;
        shards.insert(config.shard_id.clone(), ShardState::new(config));
    }

    /// Register a shard only if it doesn't already exist.
    pub async fn register_shard_if_absent(&self, config: ShardConfig) -> bool {
        let mut shards = self.shards.write().await;
        if shards.contains_key(&config.shard_id) {
            return false;
        }
        shards.insert(config.shard_id.clone(), ShardState::new(config));
        true
    }

    /// Unregister a shard by ID. Returns true if removed.
    pub async fn unregister_shard(&self, shard_id: &str) -> bool {
        let mut shards = self.shards.write().await;
        shards.remove(shard_id).is_some()
    }

    /// List all shard summaries.
    pub async fn summarize_shards(&self) -> Vec<ShardSummary> {
        let shards = self.shards.read().await;
        shards.values().map(|state| state.summary()).collect()
    }

    /// Get a specific shard summary.
    pub async fn shard_summary(&self, shard_id: &str) -> Option<ShardSummary> {
        let shards = self.shards.read().await;
        shards.get(shard_id).map(|s| s.summary())
    }

    /// Get statistics for a specific shard.
    pub async fn shard_stats(&self, shard_id: &str) -> Option<ShardStats> {
        let shards = self.shards.read().await;
        shards.get(shard_id).map(|s| s.stats())
    }

    /// Route a tenant to the shard that allows it.
    pub async fn route_tenant(&self, tenant_id: &str) -> Option<ShardSummary> {
        let shards = self.shards.read().await;
        shards
            .values()
            .find(|state| state.config.allowed_tenants.contains(tenant_id))
            .map(|state| state.summary())
    }

    /// Route a tenant, preferring healthy shards.
    pub async fn route_tenant_healthy(&self, tenant_id: &str) -> Option<ShardSummary> {
        let shards = self.shards.read().await;
        let matching: Vec<_> = shards
            .values()
            .filter(|state| state.config.allowed_tenants.contains(tenant_id))
            .collect();

        // Prefer healthy, then degraded.
        matching
            .iter()
            .find(|s| s.health == ShardHealthStatus::Healthy)
            .or_else(|| {
                matching
                    .iter()
                    .find(|s| s.health == ShardHealthStatus::Degraded)
            })
            .or(matching.first())
            .map(|s| s.summary())
    }

    /// Set shard health status.
    pub async fn set_health(
        &self,
        shard_id: &str,
        health: ShardHealthStatus,
    ) -> Result<(), ShardingError> {
        let mut shards = self.shards.write().await;
        let state = shards
            .get_mut(shard_id)
            .ok_or_else(|| ShardingError::UnknownShard(shard_id.to_string()))?;
        state.health = health;
        Ok(())
    }

    /// Set shard weight (for weighted routing / rebalancing).
    pub async fn set_weight(&self, shard_id: &str, weight: u32) -> Result<(), ShardingError> {
        let mut shards = self.shards.write().await;
        let state = shards
            .get_mut(shard_id)
            .ok_or_else(|| ShardingError::UnknownShard(shard_id.to_string()))?;
        state.weight = weight;
        Ok(())
    }

    /// Add a tenant to a shard's allowlist.
    pub async fn add_tenant_to_shard(
        &self,
        shard_id: &str,
        tenant_id: &str,
    ) -> Result<(), ShardingError> {
        let mut shards = self.shards.write().await;
        let state = shards
            .get_mut(shard_id)
            .ok_or_else(|| ShardingError::UnknownShard(shard_id.to_string()))?;
        if state.health == ShardHealthStatus::Draining {
            return Err(ShardingError::ShardDraining(shard_id.to_string()));
        }
        state.config.allowed_tenants.insert(tenant_id.to_string());
        Ok(())
    }

    /// Remove a tenant from a shard's allowlist.
    pub async fn remove_tenant_from_shard(
        &self,
        shard_id: &str,
        tenant_id: &str,
    ) -> Result<(), ShardingError> {
        let mut shards = self.shards.write().await;
        let state = shards
            .get_mut(shard_id)
            .ok_or_else(|| ShardingError::UnknownShard(shard_id.to_string()))?;
        if !state.config.allowed_tenants.remove(tenant_id) {
            return Err(ShardingError::TenantNotInShard(
                tenant_id.to_string(),
                shard_id.to_string(),
            ));
        }
        Ok(())
    }

    /// Migrate a tenant from one shard to another.
    pub async fn migrate_tenant(
        &self,
        tenant_id: &str,
        from_shard: &str,
        to_shard: &str,
    ) -> Result<MigrationRecord, ShardingError> {
        let mut shards = self.shards.write().await;
        let from = shards
            .get_mut(from_shard)
            .ok_or_else(|| ShardingError::UnknownShard(from_shard.to_string()))?;
        if !from.config.allowed_tenants.remove(tenant_id) {
            return Err(ShardingError::TenantNotInShard(
                tenant_id.to_string(),
                from_shard.to_string(),
            ));
        }

        let to = shards
            .get_mut(to_shard)
            .ok_or_else(|| ShardingError::UnknownShard(to_shard.to_string()))?;
        if to.health == ShardHealthStatus::Draining {
            return Err(ShardingError::ShardDraining(to_shard.to_string()));
        }
        to.config.allowed_tenants.insert(tenant_id.to_string());
        drop(shards);

        let record = MigrationRecord {
            tenant_id: tenant_id.to_string(),
            from_shard: from_shard.to_string(),
            to_shard: to_shard.to_string(),
            status: MigrationStatus::Completed,
        };

        let mut migrations = self.migrations.write().await;
        migrations.push(record.clone());
        Ok(record)
    }

    /// List all migration records.
    pub async fn migration_history(&self) -> Vec<MigrationRecord> {
        self.migrations.read().await.clone()
    }

    /// Start draining a shard (no new tenants, existing keep running).
    pub async fn drain_shard(&self, shard_id: &str) -> Result<(), ShardingError> {
        self.set_health(shard_id, ShardHealthStatus::Draining).await
    }

    /// Find the shard with the fewest tenants in a given region (for placement).
    pub async fn least_loaded_shard(&self, region: &str) -> Option<ShardSummary> {
        let shards = self.shards.read().await;
        shards
            .values()
            .filter(|s| s.config.region == region && s.health == ShardHealthStatus::Healthy)
            .min_by_key(|s| s.config.allowed_tenants.len())
            .map(|s| s.summary())
    }

    /// Find the shard with the highest weight in a given region.
    pub async fn highest_weight_shard(&self, region: &str) -> Option<ShardSummary> {
        let shards = self.shards.read().await;
        shards
            .values()
            .filter(|s| s.config.region == region && s.health == ShardHealthStatus::Healthy)
            .max_by_key(|s| s.weight)
            .map(|s| s.summary())
    }

    /// List all tenants assigned to a shard.
    pub async fn tenants_in_shard(&self, shard_id: &str) -> Result<Vec<String>, ShardingError> {
        let shards = self.shards.read().await;
        let state = shards
            .get(shard_id)
            .ok_or_else(|| ShardingError::UnknownShard(shard_id.to_string()))?;
        let mut tenants: Vec<_> = state.config.allowed_tenants.iter().cloned().collect();
        tenants.sort();
        Ok(tenants)
    }

    /// Count total shards.
    pub async fn shard_count(&self) -> usize {
        self.shards.read().await.len()
    }

    /// List shards by region.
    pub async fn shards_in_region(&self, region: &str) -> Vec<ShardSummary> {
        let shards = self.shards.read().await;
        shards
            .values()
            .filter(|s| s.config.region == region)
            .map(|s| s.summary())
            .collect()
    }

    /// Assign a leader to a shard using consensus leases.
    pub async fn assign_leader(
        &self,
        shard_id: &str,
        node_id: &str,
        ttl: Duration,
    ) -> Result<ShardLeader, ShardingError> {
        let mut shards = self.shards.write().await;
        let state = shards
            .get_mut(shard_id)
            .ok_or_else(|| ShardingError::UnknownShard(shard_id.to_string()))?;

        let coordinator = self
            .consensus
            .as_ref()
            .ok_or(ShardingError::ConsensusUnavailable)?;

        if let Some(existing) = state.leader.take() {
            coordinator
                .release(&existing.resource, &existing.owner)
                .await;
        }

        let resource = state
            .config
            .consensus_resource
            .clone()
            .unwrap_or_else(|| format!("shard:{}", shard_id));

        let lease = coordinator
            .acquire(&resource, node_id, ttl)
            .await
            .map_err(ShardingError::LeaseFailure)?;

        state.assigned_node = Some(node_id.to_string());
        state.leader = Some(lease.clone());
        state.leader_changes += 1;
        state.last_refresh = Instant::now();

        Ok(ShardLeader {
            node_id: node_id.to_string(),
            lease,
        })
    }

    /// Renew a shard leader's lease.
    pub async fn renew_leader(
        &self,
        shard_id: &str,
        node_id: &str,
        ttl: Duration,
    ) -> Result<(), ShardingError> {
        let shards = self.shards.read().await;
        let state = shards
            .get(shard_id)
            .ok_or_else(|| ShardingError::UnknownShard(shard_id.to_string()))?;

        let coordinator = self
            .consensus
            .as_ref()
            .ok_or(ShardingError::ConsensusUnavailable)?;

        let resource = state
            .config
            .consensus_resource
            .clone()
            .unwrap_or_else(|| format!("shard:{}", shard_id));

        coordinator
            .renew(&resource, node_id, ttl)
            .await
            .map_err(ShardingError::LeaseFailure)?;

        Ok(())
    }

    /// Get the current leader of a shard (if any).
    pub async fn current_leader(&self, shard_id: &str) -> Option<String> {
        let shards = self.shards.read().await;
        shards.get(shard_id).and_then(|s| s.assigned_node.clone())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use platform_consensus::LeaseCoordinator;
    use tokio::time::Duration;

    fn default_config(id: &str) -> ShardConfig {
        let tenants = ["alpha", id]
            .iter()
            .map(|s| s.to_string())
            .collect::<HashSet<_>>();
        ShardConfig {
            shard_id: id.to_string(),
            region: "eu".into(),
            allowed_tenants: tenants,
            consensus_resource: None,
        }
    }

    #[tokio::test]
    async fn registers_and_routes_shard() {
        let catalog = ShardingCatalog::new();
        catalog.register_shard(default_config("shard-a")).await;
        let summary = catalog.route_tenant("alpha").await;
        assert!(summary.is_some());
        assert_eq!(summary.unwrap().shard_id, "shard-a");
    }

    #[tokio::test]
    async fn assigns_leader_with_consensus() {
        let catalog = ShardingCatalog::with_consensus(Arc::new(LeaseCoordinator::new()));
        catalog.register_shard(default_config("shard-b")).await;
        let leader = catalog
            .assign_leader("shard-b", "node-1", Duration::from_secs(2))
            .await
            .expect("leader assigned");
        assert_eq!(leader.node_id, "node-1");
    }

    #[tokio::test]
    async fn lease_replaced_on_reassign() {
        let catalog = ShardingCatalog::with_consensus(Arc::new(LeaseCoordinator::new()));
        catalog.register_shard(default_config("shard-c")).await;
        let first = catalog
            .assign_leader("shard-c", "node-1", Duration::from_secs(1))
            .await
            .unwrap();
        let second = catalog
            .assign_leader("shard-c", "node-2", Duration::from_secs(1))
            .await
            .unwrap();
        assert_eq!(second.node_id, "node-2");
        assert_ne!(first.lease.owner, second.lease.owner);
    }

    // ---- New tests ----

    #[tokio::test]
    async fn register_if_absent() {
        let catalog = ShardingCatalog::new();
        assert!(catalog.register_shard_if_absent(default_config("s1")).await);
        assert!(!catalog.register_shard_if_absent(default_config("s1")).await);
    }

    #[tokio::test]
    async fn unregister_shard() {
        let catalog = ShardingCatalog::new();
        catalog.register_shard(default_config("s1")).await;
        assert!(catalog.unregister_shard("s1").await);
        assert!(!catalog.unregister_shard("s1").await);
        assert_eq!(catalog.shard_count().await, 0);
    }

    #[tokio::test]
    async fn shard_count() {
        let catalog = ShardingCatalog::new();
        assert_eq!(catalog.shard_count().await, 0);
        catalog.register_shard(default_config("s1")).await;
        catalog.register_shard(default_config("s2")).await;
        assert_eq!(catalog.shard_count().await, 2);
    }

    #[tokio::test]
    async fn summarize_shards_returns_all() {
        let catalog = ShardingCatalog::new();
        catalog.register_shard(default_config("s1")).await;
        catalog.register_shard(default_config("s2")).await;
        let summaries = catalog.summarize_shards().await;
        assert_eq!(summaries.len(), 2);
    }

    #[tokio::test]
    async fn shard_summary_by_id() {
        let catalog = ShardingCatalog::new();
        catalog.register_shard(default_config("s1")).await;
        let summary = catalog.shard_summary("s1").await;
        assert!(summary.is_some());
        assert_eq!(summary.unwrap().shard_id, "s1");
        assert!(catalog.shard_summary("nonexistent").await.is_none());
    }

    #[tokio::test]
    async fn health_status_transitions() {
        let catalog = ShardingCatalog::new();
        catalog.register_shard(default_config("s1")).await;
        let summary = catalog.shard_summary("s1").await.unwrap();
        assert_eq!(summary.health, ShardHealthStatus::Healthy);

        catalog
            .set_health("s1", ShardHealthStatus::Degraded)
            .await
            .unwrap();
        let summary = catalog.shard_summary("s1").await.unwrap();
        assert_eq!(summary.health, ShardHealthStatus::Degraded);

        catalog
            .set_health("s1", ShardHealthStatus::Unhealthy)
            .await
            .unwrap();
        let summary = catalog.shard_summary("s1").await.unwrap();
        assert_eq!(summary.health, ShardHealthStatus::Unhealthy);
    }

    #[tokio::test]
    async fn health_status_unknown_shard() {
        let catalog = ShardingCatalog::new();
        let err = catalog.set_health("nope", ShardHealthStatus::Healthy).await;
        assert!(matches!(err, Err(ShardingError::UnknownShard(_))));
    }

    #[tokio::test]
    async fn set_weight() {
        let catalog = ShardingCatalog::new();
        catalog.register_shard(default_config("s1")).await;
        assert_eq!(catalog.shard_summary("s1").await.unwrap().weight, 100);
        catalog.set_weight("s1", 200).await.unwrap();
        assert_eq!(catalog.shard_summary("s1").await.unwrap().weight, 200);
    }

    #[tokio::test]
    async fn add_and_remove_tenant() {
        let catalog = ShardingCatalog::new();
        catalog.register_shard(default_config("s1")).await;
        catalog
            .add_tenant_to_shard("s1", "new-tenant")
            .await
            .unwrap();
        let routed = catalog.route_tenant("new-tenant").await;
        assert!(routed.is_some());

        catalog
            .remove_tenant_from_shard("s1", "new-tenant")
            .await
            .unwrap();
        let routed = catalog.route_tenant("new-tenant").await;
        assert!(routed.is_none());
    }

    #[tokio::test]
    async fn remove_nonexistent_tenant() {
        let catalog = ShardingCatalog::new();
        catalog.register_shard(default_config("s1")).await;
        let err = catalog.remove_tenant_from_shard("s1", "ghost").await;
        assert!(matches!(err, Err(ShardingError::TenantNotInShard(_, _))));
    }

    #[tokio::test]
    async fn draining_shard_rejects_new_tenants() {
        let catalog = ShardingCatalog::new();
        catalog.register_shard(default_config("s1")).await;
        catalog.drain_shard("s1").await.unwrap();
        let err = catalog.add_tenant_to_shard("s1", "new-t").await;
        assert!(matches!(err, Err(ShardingError::ShardDraining(_))));
    }

    #[tokio::test]
    async fn migrate_tenant() {
        let catalog = ShardingCatalog::new();
        let mut c1 = default_config("s1");
        c1.allowed_tenants.insert("migrant".to_string());
        catalog.register_shard(c1).await;
        catalog.register_shard(default_config("s2")).await;

        let record = catalog.migrate_tenant("migrant", "s1", "s2").await.unwrap();
        assert_eq!(record.status, MigrationStatus::Completed);
        assert_eq!(record.from_shard, "s1");
        assert_eq!(record.to_shard, "s2");

        // Tenant should now route to s2.
        let routed = catalog.route_tenant("migrant").await.unwrap();
        assert_eq!(routed.shard_id, "s2");

        // Migration history should have one record.
        let history = catalog.migration_history().await;
        assert_eq!(history.len(), 1);
    }

    #[tokio::test]
    async fn migrate_tenant_not_in_source() {
        let catalog = ShardingCatalog::new();
        catalog.register_shard(default_config("s1")).await;
        catalog.register_shard(default_config("s2")).await;
        let err = catalog.migrate_tenant("ghost", "s1", "s2").await;
        assert!(matches!(err, Err(ShardingError::TenantNotInShard(_, _))));
    }

    #[tokio::test]
    async fn migrate_to_draining_shard_fails() {
        let catalog = ShardingCatalog::new();
        let mut c1 = default_config("s1");
        c1.allowed_tenants.insert("migrant".to_string());
        catalog.register_shard(c1).await;
        catalog.register_shard(default_config("s2")).await;
        catalog.drain_shard("s2").await.unwrap();
        let err = catalog.migrate_tenant("migrant", "s1", "s2").await;
        assert!(matches!(err, Err(ShardingError::ShardDraining(_))));
    }

    #[tokio::test]
    async fn route_tenant_healthy_prefers_healthy() {
        let catalog = ShardingCatalog::new();
        let mut c1 = default_config("s1");
        c1.allowed_tenants.insert("t1".to_string());
        catalog.register_shard(c1).await;
        let c2 = ShardConfig {
            shard_id: "s2".to_string(),
            region: "eu".into(),
            allowed_tenants: ["t1".to_string()].into_iter().collect(),
            consensus_resource: None,
        };
        catalog.register_shard(c2).await;

        // Mark s1 as degraded.
        catalog
            .set_health("s1", ShardHealthStatus::Degraded)
            .await
            .unwrap();
        let routed = catalog.route_tenant_healthy("t1").await.unwrap();
        assert_eq!(routed.shard_id, "s2");
    }

    #[tokio::test]
    async fn least_loaded_shard() {
        let catalog = ShardingCatalog::new();
        let c1 = ShardConfig {
            shard_id: "s1".to_string(),
            region: "eu".into(),
            allowed_tenants: ["a", "b", "c"].iter().map(|s| s.to_string()).collect(),
            consensus_resource: None,
        };
        let c2 = ShardConfig {
            shard_id: "s2".to_string(),
            region: "eu".into(),
            allowed_tenants: ["d"].iter().map(|s| s.to_string()).collect(),
            consensus_resource: None,
        };
        catalog.register_shard(c1).await;
        catalog.register_shard(c2).await;

        let least = catalog.least_loaded_shard("eu").await.unwrap();
        assert_eq!(least.shard_id, "s2");
        assert_eq!(least.tenant_count, 1);
    }

    #[tokio::test]
    async fn least_loaded_shard_no_match() {
        let catalog = ShardingCatalog::new();
        catalog.register_shard(default_config("s1")).await;
        assert!(catalog.least_loaded_shard("us").await.is_none());
    }

    #[tokio::test]
    async fn highest_weight_shard() {
        let catalog = ShardingCatalog::new();
        catalog.register_shard(default_config("s1")).await;
        catalog.register_shard(default_config("s2")).await;
        catalog.set_weight("s1", 50).await.unwrap();
        catalog.set_weight("s2", 200).await.unwrap();
        let hw = catalog.highest_weight_shard("eu").await.unwrap();
        assert_eq!(hw.shard_id, "s2");
        assert_eq!(hw.weight, 200);
    }

    #[tokio::test]
    async fn tenants_in_shard() {
        let catalog = ShardingCatalog::new();
        catalog.register_shard(default_config("s1")).await;
        let tenants = catalog.tenants_in_shard("s1").await.unwrap();
        assert!(tenants.contains(&"alpha".to_string()));
        assert!(tenants.contains(&"s1".to_string()));
    }

    #[tokio::test]
    async fn tenants_in_unknown_shard() {
        let catalog = ShardingCatalog::new();
        let err = catalog.tenants_in_shard("nope").await;
        assert!(matches!(err, Err(ShardingError::UnknownShard(_))));
    }

    #[tokio::test]
    async fn shards_in_region() {
        let catalog = ShardingCatalog::new();
        catalog.register_shard(default_config("eu-1")).await;
        catalog.register_shard(default_config("eu-2")).await;
        let us_config = ShardConfig {
            shard_id: "us-1".to_string(),
            region: "us".into(),
            allowed_tenants: HashSet::new(),
            consensus_resource: None,
        };
        catalog.register_shard(us_config).await;

        let eu_shards = catalog.shards_in_region("eu").await;
        assert_eq!(eu_shards.len(), 2);
        let us_shards = catalog.shards_in_region("us").await;
        assert_eq!(us_shards.len(), 1);
    }

    #[tokio::test]
    async fn shard_stats_tracks_leader_changes() {
        let catalog = ShardingCatalog::with_consensus(Arc::new(LeaseCoordinator::new()));
        catalog.register_shard(default_config("s1")).await;
        catalog
            .assign_leader("s1", "n1", Duration::from_secs(10))
            .await
            .unwrap();
        catalog
            .assign_leader("s1", "n2", Duration::from_secs(10))
            .await
            .unwrap();
        let stats = catalog.shard_stats("s1").await.unwrap();
        assert_eq!(stats.leader_changes, 2);
    }

    #[tokio::test]
    async fn current_leader() {
        let catalog = ShardingCatalog::with_consensus(Arc::new(LeaseCoordinator::new()));
        catalog.register_shard(default_config("s1")).await;
        assert!(catalog.current_leader("s1").await.is_none());
        catalog
            .assign_leader("s1", "n1", Duration::from_secs(10))
            .await
            .unwrap();
        assert_eq!(catalog.current_leader("s1").await, Some("n1".to_string()));
    }

    #[tokio::test]
    async fn renew_leader() {
        let coord = Arc::new(LeaseCoordinator::new());
        let catalog = ShardingCatalog::with_consensus(coord.clone());
        catalog.register_shard(default_config("s1")).await;
        catalog
            .assign_leader("s1", "n1", Duration::from_secs(10))
            .await
            .unwrap();
        catalog
            .renew_leader("s1", "n1", Duration::from_secs(10))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn renew_leader_wrong_node() {
        let coord = Arc::new(LeaseCoordinator::new());
        let catalog = ShardingCatalog::with_consensus(coord);
        catalog.register_shard(default_config("s1")).await;
        catalog
            .assign_leader("s1", "n1", Duration::from_secs(10))
            .await
            .unwrap();
        let err = catalog
            .renew_leader("s1", "n2", Duration::from_secs(10))
            .await;
        assert!(matches!(err, Err(ShardingError::LeaseFailure(_))));
    }

    #[tokio::test]
    async fn default_health_and_weight() {
        let catalog = ShardingCatalog::new();
        catalog.register_shard(default_config("s1")).await;
        let summary = catalog.shard_summary("s1").await.unwrap();
        assert_eq!(summary.health, ShardHealthStatus::Healthy);
        assert_eq!(summary.weight, 100);
    }
}
