//! Shard metadata and leadership management that tells workers which tenants they may serve
//! and what nodes own those shards. It integrates with `platform-consensus` leases so shard leaders
//! stay elected, failover happens when leases lapse, and admission control can rely on authoritative
//! shard assignments.

use anyhow::Result;
use platform_consensus::{LeaseCoordinator, LeaseToken};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct ShardConfig {
    pub shard_id: String,
    pub region: String,
    pub allowed_tenants: HashSet<String>,
    pub consensus_resource: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ShardSummary {
    pub shard_id: String,
    pub region: String,
    pub assigned_node: Option<String>,
    pub tenant_count: usize,
}

#[derive(Debug)]
pub struct ShardLeader {
    pub node_id: String,
    pub lease: LeaseToken,
}

#[derive(Debug)]
pub struct ShardState {
    config: ShardConfig,
    assigned_node: Option<String>,
    leader: Option<LeaseToken>,
    last_refresh: Instant,
}

impl ShardState {
    fn new(config: ShardConfig) -> Self {
        Self {
            config,
            assigned_node: None,
            leader: None,
            last_refresh: Instant::now(),
        }
    }

    fn summary(&self) -> ShardSummary {
        ShardSummary {
            shard_id: self.config.shard_id.clone(),
            region: self.config.region.clone(),
            assigned_node: self.assigned_node.clone(),
            tenant_count: self.config.allowed_tenants.len(),
        }
    }
}

#[derive(Debug, Error)]
pub enum ShardingError {
    #[error("shard {0} unknown")]
    UnknownShard(String),

    #[error("no consensus coordinator configured")]
    ConsensusUnavailable,

    #[error("failed to acquire lease: {0}")]
    LeaseFailure(#[source] anyhow::Error),
}

pub struct ShardingCatalog {
    shards: Arc<RwLock<HashMap<String, ShardState>>>,
    consensus: Option<Arc<LeaseCoordinator>>,
}

impl ShardingCatalog {
    pub fn new() -> Self {
        Self {
            shards: Arc::new(RwLock::new(HashMap::new())),
            consensus: None,
        }
    }

    pub fn with_consensus(lease_coordinator: Arc<LeaseCoordinator>) -> Self {
        Self {
            shards: Arc::new(RwLock::new(HashMap::new())),
            consensus: Some(lease_coordinator),
        }
    }

    pub async fn register_shard(&self, config: ShardConfig) {
        let mut shards = self.shards.write().await;
        shards.insert(config.shard_id.clone(), ShardState::new(config));
    }

    pub async fn summarize_shards(&self) -> Vec<ShardSummary> {
        let shards = self.shards.read().await;
        shards.values().map(|state| state.summary()).collect()
    }

    pub async fn route_tenant(&self, tenant_id: &str) -> Option<ShardSummary> {
        let shards = self.shards.read().await;
        shards
            .values()
            .find(|state| state.config.allowed_tenants.contains(tenant_id))
            .map(|state| state.summary())
    }

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
        state.last_refresh = Instant::now();

        Ok(ShardLeader {
            node_id: node_id.to_string(),
            lease,
        })
    }
}

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
}
