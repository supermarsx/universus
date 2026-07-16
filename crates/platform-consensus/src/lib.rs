//! Consensus helpers for multi-tenant leader election, lease acquisition,
//! distributed locks, and fencing tokens.
//!
//! Services use [`LeaseCoordinator`] for short-lived leadership over shared resources,
//! [`LeaderElection`] for durable leader election with automatic heartbeat,
//! [`DistributedLock`] for mutual exclusion (read/write), and [`FencingTokenIssuer`]
//! to prevent stale writes after leader failover.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// LeaseToken
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LeaseToken {
    pub resource: String,
    pub owner: String,
    pub expires_at: Instant,
}

// ---------------------------------------------------------------------------
// LeaseEvent / LeaseMetrics
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// LeaseCoordinator
// ---------------------------------------------------------------------------

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

    /// List all currently active (non-expired) leases.
    pub async fn active_leases(&self) -> Vec<LeaseToken> {
        let mut write = self.leases.write().await;
        self.prune_expired_locked(&mut write, Instant::now()).await;
        write.values().cloned().collect()
    }

    /// Force-revoke a lease regardless of owner (admin operation).
    pub async fn force_revoke(&self, resource: &str) -> bool {
        let mut write = self.leases.write().await;
        if let Some(token) = write.remove(resource) {
            self.record_event(
                LeaseEventKind::Released,
                resource.to_string(),
                token.owner.clone(),
                Instant::now(),
            )
            .await;
            true
        } else {
            false
        }
    }

    /// Clear all events from the event log.
    pub async fn clear_events(&self) {
        let mut events = self.events.write().await;
        events.clear();
    }

    async fn prune_expired_locked(&self, leases: &mut HashMap<String, LeaseToken>, now: Instant) {
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

impl Default for LeaseCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// FencingTokenIssuer — monotonically increasing tokens to guard stale writes
// ---------------------------------------------------------------------------

/// Issues monotonically increasing fencing tokens. Each time a leader acquires a
/// lease, it receives a fencing token. Downstream services can reject requests
/// with a stale (lower) token.
#[derive(Debug, Clone)]
pub struct FencingTokenIssuer {
    counter: Arc<AtomicU64>,
}

impl FencingTokenIssuer {
    pub fn new() -> Self {
        Self {
            counter: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Issue the next fencing token.
    pub fn issue(&self) -> u64 {
        self.counter.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Return the most recently issued token value (0 if none issued yet).
    pub fn current(&self) -> u64 {
        self.counter.load(Ordering::SeqCst)
    }

    /// Validate that `token` is not stale. A token is valid if it is >= `min_token`.
    pub fn validate(token: u64, min_token: u64) -> bool {
        token >= min_token
    }
}

impl Default for FencingTokenIssuer {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// FencedLease — lease + fencing token pair
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FencedLease {
    pub resource: String,
    pub owner: String,
    pub fencing_token: u64,
}

// ---------------------------------------------------------------------------
// FencedLeaseCoordinator — LeaseCoordinator + automatic fencing tokens
// ---------------------------------------------------------------------------

/// Wraps a [`LeaseCoordinator`] and a [`FencingTokenIssuer`] so every lease
/// acquisition returns a fencing token. The token can be attached to downstream
/// operations and rejected by services that see a newer token.
#[derive(Clone)]
pub struct FencedLeaseCoordinator {
    inner: LeaseCoordinator,
    issuer: FencingTokenIssuer,
    /// Tracks the latest fencing token per resource.
    latest_tokens: Arc<RwLock<HashMap<String, u64>>>,
}

impl FencedLeaseCoordinator {
    pub fn new() -> Self {
        Self {
            inner: LeaseCoordinator::new(),
            issuer: FencingTokenIssuer::new(),
            latest_tokens: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn from_coordinator(coordinator: LeaseCoordinator) -> Self {
        Self {
            inner: coordinator,
            issuer: FencingTokenIssuer::new(),
            latest_tokens: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Acquire a fenced lease. On success, a monotonically increasing fencing token
    /// is assigned to the lease.
    pub async fn acquire(&self, resource: &str, owner: &str, ttl: Duration) -> Result<FencedLease> {
        let _lease = self.inner.acquire(resource, owner, ttl).await?;
        let token = self.issuer.issue();
        let mut map = self.latest_tokens.write().await;
        map.insert(resource.to_string(), token);
        Ok(FencedLease {
            resource: resource.to_string(),
            owner: owner.to_string(),
            fencing_token: token,
        })
    }

    /// Check whether a fencing token is still valid for a resource (i.e. no newer
    /// token has been issued).
    pub async fn is_token_valid(&self, resource: &str, token: u64) -> bool {
        let map = self.latest_tokens.read().await;
        match map.get(resource) {
            Some(&latest) => token >= latest,
            None => false,
        }
    }

    /// Return the latest fencing token for a resource.
    pub async fn latest_token(&self, resource: &str) -> Option<u64> {
        let map = self.latest_tokens.read().await;
        map.get(resource).copied()
    }

    /// Release the underlying lease.
    pub async fn release(&self, resource: &str, owner: &str) -> bool {
        self.inner.release(resource, owner).await
    }

    /// Renew the underlying lease (fencing token stays the same).
    pub async fn renew(&self, resource: &str, owner: &str, ttl: Duration) -> Result<()> {
        self.inner.renew(resource, owner, ttl).await?;
        Ok(())
    }

    pub async fn metrics_snapshot(&self) -> LeaseMetrics {
        self.inner.metrics_snapshot().await
    }

    /// Access the inner coordinator for lower-level operations.
    pub fn coordinator(&self) -> &LeaseCoordinator {
        &self.inner
    }
}

impl Default for FencedLeaseCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// LeaderElection — built on top of LeaseCoordinator
// ---------------------------------------------------------------------------

/// Status of a candidate in a leader election.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ElectionRole {
    Leader,
    Follower,
    Candidate,
}

/// Snapshot of the election state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElectionSnapshot {
    pub resource: String,
    pub role: ElectionRole,
    pub current_leader: Option<String>,
    pub term: u64,
}

/// Leader election built on [`LeaseCoordinator`]. A candidate calls `campaign()`
/// to attempt to become leader. The leader must call `heartbeat()` before the
/// lease expires or it loses leadership.
#[derive(Clone)]
pub struct LeaderElection {
    coordinator: LeaseCoordinator,
    resource: String,
    candidate_id: String,
    ttl: Duration,
    term: Arc<AtomicU64>,
}

impl LeaderElection {
    pub fn new(
        coordinator: LeaseCoordinator,
        resource: &str,
        candidate_id: &str,
        ttl: Duration,
    ) -> Self {
        Self {
            coordinator,
            resource: resource.to_string(),
            candidate_id: candidate_id.to_string(),
            ttl,
            term: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Attempt to become leader. Returns the current role after the attempt.
    pub async fn campaign(&self) -> ElectionRole {
        match self
            .coordinator
            .acquire(&self.resource, &self.candidate_id, self.ttl)
            .await
        {
            Ok(_) => {
                self.term.fetch_add(1, Ordering::SeqCst);
                ElectionRole::Leader
            }
            Err(_) => ElectionRole::Follower,
        }
    }

    /// Renew leadership (must be called before TTL expires).
    pub async fn heartbeat(&self) -> bool {
        self.coordinator
            .renew(&self.resource, &self.candidate_id, self.ttl)
            .await
            .is_ok()
    }

    /// Step down from leadership voluntarily.
    pub async fn resign(&self) -> bool {
        self.coordinator
            .release(&self.resource, &self.candidate_id)
            .await
    }

    /// Check if this candidate is currently the leader.
    pub async fn is_leader(&self) -> bool {
        match self.coordinator.active_lease(&self.resource).await {
            Some(token) => token.owner == self.candidate_id,
            None => false,
        }
    }

    /// Return who the current leader is (if any).
    pub async fn current_leader(&self) -> Option<String> {
        self.coordinator
            .active_lease(&self.resource)
            .await
            .map(|t| t.owner)
    }

    /// Snapshot of the current election state.
    pub async fn snapshot(&self) -> ElectionSnapshot {
        let leader = self.current_leader().await;
        let role = if leader.as_deref() == Some(&self.candidate_id) {
            ElectionRole::Leader
        } else if leader.is_some() {
            ElectionRole::Follower
        } else {
            ElectionRole::Candidate
        };
        ElectionSnapshot {
            resource: self.resource.clone(),
            role,
            current_leader: leader,
            term: self.term.load(Ordering::SeqCst),
        }
    }

    /// Return the current term number.
    pub fn term(&self) -> u64 {
        self.term.load(Ordering::SeqCst)
    }
}

// ---------------------------------------------------------------------------
// DistributedLock — read/write lock on top of LeaseCoordinator
// ---------------------------------------------------------------------------

/// Error returned by distributed lock operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockError {
    /// A write lock is held by another owner.
    WriteLocked(String),
    /// Too many concurrent readers (shouldn't happen in practice).
    TooManyReaders,
    /// Lock not found.
    NotFound,
    /// Owner mismatch.
    OwnerMismatch,
}

impl std::fmt::Display for LockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LockError::WriteLocked(owner) => write!(f, "write-locked by {}", owner),
            LockError::TooManyReaders => write!(f, "too many concurrent readers"),
            LockError::NotFound => write!(f, "lock not found"),
            LockError::OwnerMismatch => write!(f, "owner mismatch"),
        }
    }
}

impl std::error::Error for LockError {}

#[derive(Debug, Clone)]
struct LockState {
    /// If set, a write lock is held.
    writer: Option<String>,
    /// Active readers (owner -> count).
    readers: HashMap<String, u32>,
}

impl LockState {
    fn new() -> Self {
        Self {
            writer: None,
            readers: HashMap::new(),
        }
    }

    fn reader_count(&self) -> u32 {
        self.readers.values().sum()
    }
}

/// In-memory distributed read/write lock. Multiple readers can hold the lock
/// concurrently, but a writer requires exclusive access.
pub struct DistributedLock {
    locks: RwLock<HashMap<String, LockState>>,
    max_readers: u32,
}

impl DistributedLock {
    pub fn new(max_readers: u32) -> Self {
        Self {
            locks: RwLock::new(HashMap::new()),
            max_readers: max_readers.max(1),
        }
    }

    /// Acquire a read lock on `resource`.
    pub async fn acquire_read(&self, resource: &str, owner: &str) -> Result<(), LockError> {
        let mut map = self.locks.write().await;
        let state = map
            .entry(resource.to_string())
            .or_insert_with(LockState::new);

        if let Some(ref writer) = state.writer {
            return Err(LockError::WriteLocked(writer.clone()));
        }
        if state.reader_count() >= self.max_readers {
            return Err(LockError::TooManyReaders);
        }
        *state.readers.entry(owner.to_string()).or_insert(0) += 1;
        Ok(())
    }

    /// Release a read lock on `resource`.
    pub async fn release_read(&self, resource: &str, owner: &str) -> Result<(), LockError> {
        let mut map = self.locks.write().await;
        let state = map.get_mut(resource).ok_or(LockError::NotFound)?;
        let count = state
            .readers
            .get_mut(owner)
            .ok_or(LockError::OwnerMismatch)?;
        *count = count.saturating_sub(1);
        if *count == 0 {
            state.readers.remove(owner);
        }
        if state.writer.is_none() && state.readers.is_empty() {
            map.remove(resource);
        }
        Ok(())
    }

    /// Acquire a write lock on `resource`. Fails if any reader or another writer
    /// holds the lock.
    pub async fn acquire_write(&self, resource: &str, owner: &str) -> Result<(), LockError> {
        let mut map = self.locks.write().await;
        let state = map
            .entry(resource.to_string())
            .or_insert_with(LockState::new);

        if let Some(ref writer) = state.writer {
            return Err(LockError::WriteLocked(writer.clone()));
        }
        if state.reader_count() > 0 {
            // Readers are present — cannot acquire write lock.
            return Err(LockError::WriteLocked("readers_active".to_string()));
        }
        state.writer = Some(owner.to_string());
        Ok(())
    }

    /// Release a write lock on `resource`.
    pub async fn release_write(&self, resource: &str, owner: &str) -> Result<(), LockError> {
        let mut map = self.locks.write().await;
        let state = map.get_mut(resource).ok_or(LockError::NotFound)?;
        match &state.writer {
            Some(w) if w == owner => {
                state.writer = None;
                if state.readers.is_empty() {
                    map.remove(resource);
                }
                Ok(())
            }
            Some(_) => Err(LockError::OwnerMismatch),
            None => Err(LockError::NotFound),
        }
    }

    /// Check if a resource is write-locked.
    pub async fn is_write_locked(&self, resource: &str) -> bool {
        let map = self.locks.read().await;
        map.get(resource)
            .map(|s| s.writer.is_some())
            .unwrap_or(false)
    }

    /// Check if a resource has any readers.
    pub async fn has_readers(&self, resource: &str) -> bool {
        let map = self.locks.read().await;
        map.get(resource)
            .map(|s| s.reader_count() > 0)
            .unwrap_or(false)
    }

    /// Get the current reader count for a resource.
    pub async fn reader_count(&self, resource: &str) -> u32 {
        let map = self.locks.read().await;
        map.get(resource).map(|s| s.reader_count()).unwrap_or(0)
    }

    /// Get the writer for a resource (if any).
    pub async fn writer(&self, resource: &str) -> Option<String> {
        let map = self.locks.read().await;
        map.get(resource).and_then(|s| s.writer.clone())
    }

    /// List all resources that have active locks.
    pub async fn active_resources(&self) -> Vec<String> {
        let map = self.locks.read().await;
        map.keys().cloned().collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Duration;

    // ---- LeaseCoordinator tests ----

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

    #[tokio::test]
    async fn active_leases_lists_all() {
        let coordinator = LeaseCoordinator::new();
        coordinator
            .acquire("r1", "o1", Duration::from_secs(60))
            .await
            .unwrap();
        coordinator
            .acquire("r2", "o2", Duration::from_secs(60))
            .await
            .unwrap();
        coordinator
            .acquire("r3", "o3", Duration::from_secs(60))
            .await
            .unwrap();

        let all = coordinator.active_leases().await;
        assert_eq!(all.len(), 3);
    }

    #[tokio::test]
    async fn force_revoke_removes_lease() {
        let coordinator = LeaseCoordinator::new();
        coordinator
            .acquire("r", "o", Duration::from_secs(60))
            .await
            .unwrap();
        assert!(coordinator.force_revoke("r").await);
        assert!(coordinator.active_lease("r").await.is_none());
        // Can re-acquire after revoke.
        coordinator
            .acquire("r", "other", Duration::from_secs(60))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn force_revoke_nonexistent_returns_false() {
        let coordinator = LeaseCoordinator::new();
        assert!(!coordinator.force_revoke("nonexistent").await);
    }

    #[tokio::test]
    async fn clear_events_empties_log() {
        let coordinator = LeaseCoordinator::new();
        coordinator
            .acquire("r", "o", Duration::from_secs(60))
            .await
            .unwrap();
        assert!(!coordinator.recent_events(0).await.is_empty());
        coordinator.clear_events().await;
        assert!(coordinator.recent_events(0).await.is_empty());
    }

    #[tokio::test]
    async fn renew_fails_for_nonexistent_lease() {
        let coordinator = LeaseCoordinator::new();
        let result = coordinator
            .renew("nope", "owner", Duration::from_secs(5))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn renew_fails_for_wrong_owner() {
        let coordinator = LeaseCoordinator::new();
        coordinator
            .acquire("r", "owner-a", Duration::from_secs(60))
            .await
            .unwrap();
        let result = coordinator
            .renew("r", "owner-b", Duration::from_secs(60))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn release_nonexistent_returns_false() {
        let coordinator = LeaseCoordinator::new();
        assert!(!coordinator.release("nope", "owner").await);
    }

    // ---- FencingTokenIssuer tests ----

    #[test]
    fn fencing_tokens_are_monotonic() {
        let issuer = FencingTokenIssuer::new();
        assert_eq!(issuer.current(), 0);
        let t1 = issuer.issue();
        let t2 = issuer.issue();
        let t3 = issuer.issue();
        assert_eq!(t1, 1);
        assert_eq!(t2, 2);
        assert_eq!(t3, 3);
        assert_eq!(issuer.current(), 3);
    }

    #[test]
    fn fencing_token_validation() {
        assert!(FencingTokenIssuer::validate(5, 5));
        assert!(FencingTokenIssuer::validate(6, 5));
        assert!(!FencingTokenIssuer::validate(4, 5));
        assert!(FencingTokenIssuer::validate(1, 1));
        assert!(!FencingTokenIssuer::validate(0, 1));
    }

    // ---- FencedLeaseCoordinator tests ----

    #[tokio::test]
    async fn fenced_acquire_issues_increasing_tokens() {
        let fenced = FencedLeaseCoordinator::new();
        let lease1 = fenced
            .acquire("r1", "o1", Duration::from_secs(60))
            .await
            .unwrap();
        assert_eq!(lease1.fencing_token, 1);

        let lease2 = fenced
            .acquire("r2", "o2", Duration::from_secs(60))
            .await
            .unwrap();
        assert_eq!(lease2.fencing_token, 2);
        assert!(lease2.fencing_token > lease1.fencing_token);
    }

    #[tokio::test]
    async fn fenced_token_validity() {
        let fenced = FencedLeaseCoordinator::new();
        let lease = fenced
            .acquire("r1", "o1", Duration::from_secs(60))
            .await
            .unwrap();
        assert!(fenced.is_token_valid("r1", lease.fencing_token).await);
        assert!(!fenced.is_token_valid("r1", 0).await);
        assert!(!fenced.is_token_valid("nonexistent", 1).await);
    }

    #[tokio::test(start_paused = true)]
    async fn fenced_lease_superseded_after_expiry() {
        let fenced = FencedLeaseCoordinator::new();
        let lease1 = fenced
            .acquire("r1", "o1", Duration::from_secs(5))
            .await
            .unwrap();
        assert_eq!(lease1.fencing_token, 1);

        tokio::time::advance(Duration::from_secs(6)).await;

        let lease2 = fenced
            .acquire("r1", "o2", Duration::from_secs(5))
            .await
            .unwrap();
        assert_eq!(lease2.fencing_token, 2);

        // Old token is now stale.
        assert!(!fenced.is_token_valid("r1", lease1.fencing_token).await);
        assert!(fenced.is_token_valid("r1", lease2.fencing_token).await);
    }

    #[tokio::test]
    async fn fenced_latest_token() {
        let fenced = FencedLeaseCoordinator::new();
        assert_eq!(fenced.latest_token("r").await, None);
        fenced
            .acquire("r", "o", Duration::from_secs(60))
            .await
            .unwrap();
        assert_eq!(fenced.latest_token("r").await, Some(1));
    }

    #[tokio::test]
    async fn fenced_release_and_metrics() {
        let fenced = FencedLeaseCoordinator::new();
        fenced
            .acquire("r", "o", Duration::from_secs(60))
            .await
            .unwrap();
        assert!(fenced.release("r", "o").await);
        let m = fenced.metrics_snapshot().await;
        assert_eq!(m.acquired, 1);
        assert_eq!(m.released, 1);
    }

    #[tokio::test]
    async fn fenced_renew_keeps_token() {
        let fenced = FencedLeaseCoordinator::new();
        fenced
            .acquire("r", "o", Duration::from_secs(60))
            .await
            .unwrap();
        let t1 = fenced.latest_token("r").await.unwrap();
        fenced
            .renew("r", "o", Duration::from_secs(60))
            .await
            .unwrap();
        let t2 = fenced.latest_token("r").await.unwrap();
        assert_eq!(t1, t2, "renew should not change fencing token");
    }

    // ---- LeaderElection tests ----

    #[tokio::test]
    async fn leader_election_campaign() {
        let coordinator = LeaseCoordinator::new();
        let election = LeaderElection::new(
            coordinator.clone(),
            "leader-resource",
            "node-1",
            Duration::from_secs(10),
        );
        let role = election.campaign().await;
        assert_eq!(role, ElectionRole::Leader);
        assert!(election.is_leader().await);
        assert_eq!(election.current_leader().await, Some("node-1".to_string()));
        assert_eq!(election.term(), 1);
    }

    #[tokio::test]
    async fn leader_election_follower() {
        let coordinator = LeaseCoordinator::new();

        let leader = LeaderElection::new(
            coordinator.clone(),
            "resource",
            "node-1",
            Duration::from_secs(10),
        );
        let follower = LeaderElection::new(
            coordinator.clone(),
            "resource",
            "node-2",
            Duration::from_secs(10),
        );

        assert_eq!(leader.campaign().await, ElectionRole::Leader);
        assert_eq!(follower.campaign().await, ElectionRole::Follower);
        assert!(!follower.is_leader().await);
        assert_eq!(follower.current_leader().await, Some("node-1".to_string()));
    }

    #[tokio::test]
    async fn leader_election_heartbeat() {
        let coordinator = LeaseCoordinator::new();
        let election = LeaderElection::new(
            coordinator.clone(),
            "resource",
            "node-1",
            Duration::from_secs(10),
        );
        election.campaign().await;
        assert!(election.heartbeat().await);
        assert!(election.is_leader().await);
    }

    #[tokio::test]
    async fn leader_election_resign() {
        let coordinator = LeaseCoordinator::new();
        let election = LeaderElection::new(
            coordinator.clone(),
            "resource",
            "node-1",
            Duration::from_secs(10),
        );
        election.campaign().await;
        assert!(election.resign().await);
        assert!(!election.is_leader().await);
    }

    #[tokio::test]
    async fn leader_election_failover() {
        let coordinator = LeaseCoordinator::new();
        let leader = LeaderElection::new(
            coordinator.clone(),
            "resource",
            "node-1",
            Duration::from_secs(10),
        );
        let challenger = LeaderElection::new(
            coordinator.clone(),
            "resource",
            "node-2",
            Duration::from_secs(10),
        );

        leader.campaign().await;
        assert_eq!(challenger.campaign().await, ElectionRole::Follower);

        // Leader resigns — challenger can now win.
        leader.resign().await;
        assert_eq!(challenger.campaign().await, ElectionRole::Leader);
        assert!(challenger.is_leader().await);
        assert_eq!(challenger.term(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn leader_election_expires_and_new_leader() {
        let coordinator = LeaseCoordinator::new();
        let leader = LeaderElection::new(
            coordinator.clone(),
            "resource",
            "node-1",
            Duration::from_secs(5),
        );
        let challenger = LeaderElection::new(
            coordinator.clone(),
            "resource",
            "node-2",
            Duration::from_secs(5),
        );

        leader.campaign().await;
        tokio::time::advance(Duration::from_secs(6)).await;

        // Leader's lease expired; challenger can win.
        assert_eq!(challenger.campaign().await, ElectionRole::Leader);
        assert!(challenger.is_leader().await);
    }

    #[tokio::test]
    async fn leader_election_snapshot() {
        let coordinator = LeaseCoordinator::new();
        let election = LeaderElection::new(
            coordinator.clone(),
            "resource",
            "node-1",
            Duration::from_secs(10),
        );
        election.campaign().await;
        let snap = election.snapshot().await;
        assert_eq!(snap.role, ElectionRole::Leader);
        assert_eq!(snap.current_leader, Some("node-1".to_string()));
        assert_eq!(snap.term, 1);
    }

    #[tokio::test]
    async fn leader_election_no_leader_snapshot() {
        let coordinator = LeaseCoordinator::new();
        let election =
            LeaderElection::new(coordinator, "resource", "node-1", Duration::from_secs(10));
        let snap = election.snapshot().await;
        assert_eq!(snap.role, ElectionRole::Candidate);
        assert_eq!(snap.current_leader, None);
        assert_eq!(snap.term, 0);
    }

    #[tokio::test]
    async fn heartbeat_fails_when_not_leader() {
        let coordinator = LeaseCoordinator::new();
        let election =
            LeaderElection::new(coordinator, "resource", "node-1", Duration::from_secs(10));
        // Haven't campaigned yet.
        assert!(!election.heartbeat().await);
    }

    // ---- DistributedLock tests ----

    #[tokio::test]
    async fn distributed_lock_read_write_basic() {
        let lock = DistributedLock::new(10);
        lock.acquire_read("r", "reader-1").await.unwrap();
        lock.acquire_read("r", "reader-2").await.unwrap();
        assert_eq!(lock.reader_count("r").await, 2);

        // Can't write while readers exist.
        let err = lock.acquire_write("r", "writer-1").await;
        assert!(err.is_err());

        lock.release_read("r", "reader-1").await.unwrap();
        lock.release_read("r", "reader-2").await.unwrap();

        // Now write is possible.
        lock.acquire_write("r", "writer-1").await.unwrap();
        assert!(lock.is_write_locked("r").await);

        // Can't read while writer exists.
        let err = lock.acquire_read("r", "reader-3").await;
        assert!(err.is_err());

        lock.release_write("r", "writer-1").await.unwrap();
        assert!(!lock.is_write_locked("r").await);
    }

    #[tokio::test]
    async fn distributed_lock_write_exclusion() {
        let lock = DistributedLock::new(10);
        lock.acquire_write("r", "w1").await.unwrap();
        let err = lock.acquire_write("r", "w2").await;
        assert!(matches!(err, Err(LockError::WriteLocked(_))));
    }

    #[tokio::test]
    async fn distributed_lock_release_write_wrong_owner() {
        let lock = DistributedLock::new(10);
        lock.acquire_write("r", "w1").await.unwrap();
        let err = lock.release_write("r", "w2").await;
        assert_eq!(err, Err(LockError::OwnerMismatch));
    }

    #[tokio::test]
    async fn distributed_lock_release_nonexistent() {
        let lock = DistributedLock::new(10);
        let err = lock.release_read("r", "o").await;
        assert_eq!(err, Err(LockError::NotFound));
        let err = lock.release_write("r", "o").await;
        assert_eq!(err, Err(LockError::NotFound));
    }

    #[tokio::test]
    async fn distributed_lock_max_readers() {
        let lock = DistributedLock::new(2);
        lock.acquire_read("r", "r1").await.unwrap();
        lock.acquire_read("r", "r2").await.unwrap();
        let err = lock.acquire_read("r", "r3").await;
        assert_eq!(err, Err(LockError::TooManyReaders));
    }

    #[tokio::test]
    async fn distributed_lock_active_resources() {
        let lock = DistributedLock::new(10);
        lock.acquire_read("alpha", "r1").await.unwrap();
        lock.acquire_write("beta", "w1").await.unwrap();
        let mut resources = lock.active_resources().await;
        resources.sort();
        assert_eq!(resources, vec!["alpha", "beta"]);
    }

    #[tokio::test]
    async fn distributed_lock_writer_query() {
        let lock = DistributedLock::new(10);
        assert_eq!(lock.writer("r").await, None);
        lock.acquire_write("r", "w1").await.unwrap();
        assert_eq!(lock.writer("r").await, Some("w1".to_string()));
    }

    #[tokio::test]
    async fn distributed_lock_cleanup_after_release() {
        let lock = DistributedLock::new(10);
        lock.acquire_read("r", "r1").await.unwrap();
        lock.release_read("r", "r1").await.unwrap();
        // After all locks released, resource should be cleaned up.
        assert!(lock.active_resources().await.is_empty());
    }

    #[tokio::test]
    async fn distributed_lock_has_readers() {
        let lock = DistributedLock::new(10);
        assert!(!lock.has_readers("r").await);
        lock.acquire_read("r", "r1").await.unwrap();
        assert!(lock.has_readers("r").await);
    }

    #[tokio::test]
    async fn distributed_lock_same_reader_multiple_acquires() {
        let lock = DistributedLock::new(10);
        lock.acquire_read("r", "r1").await.unwrap();
        lock.acquire_read("r", "r1").await.unwrap();
        assert_eq!(lock.reader_count("r").await, 2);
        lock.release_read("r", "r1").await.unwrap();
        assert_eq!(lock.reader_count("r").await, 1);
        lock.release_read("r", "r1").await.unwrap();
        assert_eq!(lock.reader_count("r").await, 0);
    }
}
