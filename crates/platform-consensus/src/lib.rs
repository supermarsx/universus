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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeaseEventKind {
    Acquired,
    AcquireFailed,
    Renewed,
    Released,
    ReleaseRejected,
    Expired,
}

#[derive(Debug, Clone)]
pub struct LeaseEvent {
    pub kind: LeaseEventKind,
    pub resource: String,
    pub owner: String,
    pub observed_at: Instant,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LeaseMetrics {
    pub acquired: u64,
    pub acquire_failed: u64,
    pub renewed: u64,
    pub released: u64,
    pub release_rejected: u64,
    pub expired: u64,
}

#[derive(Clone)]
pub struct LeaseCoordinator {
    leases: Arc<RwLock<HashMap<String, LeaseToken>>>,
    events: Arc<RwLock<Vec<LeaseEvent>>>,
    metrics: Arc<RwLock<LeaseMetrics>>,
}

impl LeaseCoordinator {
    pub fn new() -> Self {
        Self {
            leases: Arc::new(RwLock::new(HashMap::new())),
            events: Arc::new(RwLock::new(Vec::new())),
            metrics: Arc::new(RwLock::new(LeaseMetrics::default())),
        }
    }

    /// Try to acquire a lease for a given resource; returns lease token when successful.
    pub async fn acquire(&self, resource: &str, owner: &str, ttl: Duration) -> Result<LeaseToken> {
        let mut write = self.leases.write().await;
        self.prune_expired_locked(&mut write, Instant::now()).await;
        let now = Instant::now();
        if let Some(existing) = write.get(resource) {
            if existing.expires_at > now {
                self.record_event(
                    LeaseEventKind::AcquireFailed,
                    resource.to_string(),
                    owner.to_string(),
                    now,
                )
                .await;
                anyhow::bail!("resource {} already leased", resource);
            }
        }
        let token = LeaseToken {
            resource: resource.to_string(),
            owner: owner.to_string(),
            expires_at: now + ttl,
        };
        write.insert(resource.to_string(), token.clone());
        self.record_event(
            LeaseEventKind::Acquired,
            resource.to_string(),
            owner.to_string(),
            now,
        )
        .await;
        Ok(token)
    }

    /// Try to renew an existing lease owned by `owner`; returns refreshed token.
    pub async fn renew(&self, resource: &str, owner: &str, ttl: Duration) -> Result<LeaseToken> {
        let mut write = self.leases.write().await;
        let now = Instant::now();
        self.prune_expired_locked(&mut write, now).await;
        let existing = write
            .get(resource)
            .ok_or_else(|| anyhow::anyhow!("resource {} not leased", resource))?;
        if existing.owner != owner {
            anyhow::bail!("resource {} owned by {}", resource, existing.owner);
        }
        let token = LeaseToken {
            resource: resource.to_string(),
            owner: owner.to_string(),
            expires_at: now + ttl,
        };
        write.insert(resource.to_string(), token.clone());
        self.record_event(
            LeaseEventKind::Renewed,
            resource.to_string(),
            owner.to_string(),
            now,
        )
        .await;
        Ok(token)
    }

    /// Return the active lease for a resource (if any). Expired leases are cleaned up.
    pub async fn active_lease(&self, resource: &str) -> Option<LeaseToken> {
        let mut write = self.leases.write().await;
        self.prune_expired_locked(&mut write, Instant::now()).await;
        write.get(resource).cloned()
    }

    /// Release lease for a resource.
    pub async fn release(&self, resource: &str, owner: &str) -> bool {
        let mut write = self.leases.write().await;
        self.prune_expired_locked(&mut write, Instant::now()).await;
        if let Some(existing) = write.get(resource) {
            if existing.owner == owner {
                write.remove(resource);
                self.record_event(
                    LeaseEventKind::Released,
                    resource.to_string(),
                    owner.to_string(),
                    Instant::now(),
                )
                .await;
                return true;
            }
        }
        self.record_event(
            LeaseEventKind::ReleaseRejected,
            resource.to_string(),
            owner.to_string(),
            Instant::now(),
        )
        .await;
        false
    }

    /// Snapshot lease lifecycle counters used for observability dashboards/tests.
    pub async fn metrics_snapshot(&self) -> LeaseMetrics {
        *self.metrics.read().await
    }

    /// Return the latest lease events. If `limit` is zero, all events are returned.
    pub async fn recent_events(&self, limit: usize) -> Vec<LeaseEvent> {
        let events = self.events.read().await;
        if limit == 0 || limit >= events.len() {
            return events.clone();
        }
        events[events.len() - limit..].to_vec()
    }

    async fn prune_expired_locked(
        &self,
        leases: &mut HashMap<String, LeaseToken>,
        now: Instant,
    ) {
        let expired: Vec<(String, String)> = leases
            .iter()
            .filter(|(_, token)| token.expires_at <= now)
            .map(|(resource, token)| (resource.clone(), token.owner.clone()))
            .collect();

        for (resource, owner) in expired {
            leases.remove(&resource);
            self.record_event(LeaseEventKind::Expired, resource, owner, now)
                .await;
        }
    }

    async fn record_event(
        &self,
        kind: LeaseEventKind,
        resource: String,
        owner: String,
        observed_at: Instant,
    ) {
        let mut metrics = self.metrics.write().await;
        match kind {
            LeaseEventKind::Acquired => metrics.acquired += 1,
            LeaseEventKind::AcquireFailed => metrics.acquire_failed += 1,
            LeaseEventKind::Renewed => metrics.renewed += 1,
            LeaseEventKind::Released => metrics.released += 1,
            LeaseEventKind::ReleaseRejected => metrics.release_rejected += 1,
            LeaseEventKind::Expired => metrics.expired += 1,
        }
        drop(metrics);

        let mut events = self.events.write().await;
        events.push(LeaseEvent {
            kind,
            resource,
            owner,
            observed_at,
        });
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
        assert!(coordinator.release("resource", "worker").await);
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

        let metrics = coordinator.metrics_snapshot().await;
        assert_eq!(metrics.expired, 1);
    }

    #[tokio::test(start_paused = true)]
    async fn contention_updates_metrics_and_events() {
        let coordinator = LeaseCoordinator::new();
        coordinator
            .acquire("resource", "owner-a", Duration::from_secs(30))
            .await
            .expect("owner-a acquires");
        let conflict = coordinator
            .acquire("resource", "owner-b", Duration::from_secs(30))
            .await;
        assert!(conflict.is_err());

        let metrics = coordinator.metrics_snapshot().await;
        assert_eq!(metrics.acquired, 1);
        assert_eq!(metrics.acquire_failed, 1);

        let events = coordinator.recent_events(2).await;
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].kind, LeaseEventKind::Acquired);
        assert_eq!(events[1].kind, LeaseEventKind::AcquireFailed);
    }

    #[tokio::test(start_paused = true)]
    async fn renew_extends_lease_and_blocks_other_owner() {
        let coordinator = LeaseCoordinator::new();
        let ttl = Duration::from_secs(5);
        coordinator
            .acquire("resource", "owner-a", ttl)
            .await
            .expect("initial acquire");
        tokio::time::advance(Duration::from_secs(3)).await;
        coordinator
            .renew("resource", "owner-a", ttl)
            .await
            .expect("renew");

        tokio::time::advance(Duration::from_secs(3)).await;
        let conflict = coordinator.acquire("resource", "owner-b", ttl).await;
        assert!(conflict.is_err(), "renew should keep original owner active");

        tokio::time::advance(Duration::from_secs(3)).await;
        let recovered = coordinator
            .acquire("resource", "owner-b", ttl)
            .await
            .expect("lease should eventually expire");
        assert_eq!(recovered.owner, "owner-b");

        let metrics = coordinator.metrics_snapshot().await;
        assert_eq!(metrics.renewed, 1);
    }

    #[tokio::test]
    async fn release_rejected_when_owner_mismatched() {
        let coordinator = LeaseCoordinator::new();
        coordinator
            .acquire("resource", "owner-a", Duration::from_secs(10))
            .await
            .expect("initial acquire");
        assert!(!coordinator.release("resource", "owner-b").await);

        let active = coordinator.active_lease("resource").await;
        assert!(active.is_some());
        assert_eq!(active.unwrap().owner, "owner-a");

        let metrics = coordinator.metrics_snapshot().await;
        assert_eq!(metrics.release_rejected, 1);
    }
}
