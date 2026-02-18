//! Consensus helpers for multi-tenant leader election and lease acquisition.
//! Services can use `LeaseCoordinator` to obtain short-lived leadership for shared resources.

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct LeaseToken {
    pub resource: String,
    pub owner: String,
    pub expires_at: Instant,
}

#[derive(Clone)]
pub struct LeaseCoordinator {
    leases: Arc<RwLock<HashMap<String, LeaseToken>>>,
}

impl LeaseCoordinator {
    pub fn new() -> Self {
        Self {
            leases: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Try to acquire a lease for a given resource; returns lease token when successful.
    pub async fn acquire(&self, resource: &str, owner: &str, ttl: Duration) -> Result<LeaseToken> {
        let mut write = self.leases.write().await;
        let now = Instant::now();
        if let Some(existing) = write.get(resource) {
            if existing.expires_at > now {
                anyhow::bail!("resource {} already leased", resource);
            }
        }
        let token = LeaseToken {
            resource: resource.to_string(),
            owner: owner.to_string(),
            expires_at: now + ttl,
        };
        write.insert(resource.to_string(), token.clone());
        Ok(token)
    }

    /// Release lease for a resource.
    pub async fn release(&self, resource: &str, owner: &str) {
        let mut write = self.leases.write().await;
        if let Some(existing) = write.get(resource) {
            if existing.owner == owner {
                write.remove(resource);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Duration;

    #[tokio::test]
    async fn acquire_and_release() {
        let coordinator = LeaseCoordinator::new();
        let lease = coordinator
            .acquire("resource", "worker", Duration::from_secs(1))
            .await
            .expect("acquire works");
        assert_eq!(lease.owner, "worker");
        coordinator.release("resource", "worker").await;
        let lease2 = coordinator
            .acquire("resource", "worker2", Duration::from_secs(1))
            .await
            .expect("acquire again");
        assert_eq!(lease2.owner, "worker2");
    }

    #[tokio::test(start_paused = true)]
    async fn lease_expires_after_ttl() {
        let coordinator = LeaseCoordinator::new();
        let ttl = Duration::from_secs(5);
        let resource = "resource";
        let first = coordinator
            .acquire(resource, "owner-a", ttl)
            .await
            .expect("initial acquire");

        tokio::time::advance(Duration::from_secs(2)).await;
        let conflict = coordinator.acquire(resource, "owner-b", ttl).await;
        assert!(conflict.is_err(), "lease still valid before ttl");

        tokio::time::advance(Duration::from_secs(5)).await;
        let second = coordinator
            .acquire(resource, "owner-b", ttl)
            .await
            .expect("lease expired");
        assert_eq!(second.owner, "owner-b");
        assert_eq!(second.resource, first.resource);
    }
}
