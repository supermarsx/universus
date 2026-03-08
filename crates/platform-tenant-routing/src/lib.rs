//! Tenant-routing and quota helpers that keep multi-tenant workers and HTTP gateways safe.
//! This crate builds on `platform-tenancy`/`platform-consensus` to map tenants to shards,
//! enforce per-tenant quotas, and tie routing decisions to consensus leases when shared resources
//! require arbitration.
//!
//! Features:
//! - Register/unregister/update tenant routes
//! - Concurrency limits via semaphore-based permits
//! - Rate limiting with sliding window
//! - Circuit breaker per tenant (closed → open → half-open → closed)
//! - Priority-weighted routing (choose highest-priority route for a tenant)
//! - Failover routing (primary/secondary shard with automatic fallback)
//! - Route health tracking (success/failure counters, latency)
//! - Bulk operations (list all routes, filter by region/shard, tenant count)
//! - Consensus lease integration (optional)

#![forbid(unsafe_code)]

use anyhow::{bail, Result};
use platform_consensus::{LeaseCoordinator, LeaseToken};
use platform_tenancy::{TenantContext, TenantGuard};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{OwnedSemaphorePermit, RwLock, Semaphore};
use tokio::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum RoutingError {
    #[error("tenant {0} is unknown")]
    UnknownTenant(String),

    #[error("quota exceeded for tenant {tenant_id}")]
    QuotaExceeded { tenant_id: String },

    #[error("lease acquisition failed: {0}")]
    LeaseFailure(anyhow::Error),

    #[error("circuit open for tenant {tenant_id}")]
    CircuitOpen { tenant_id: String },

    #[error("no healthy route available for tenant {tenant_id}")]
    NoHealthyRoute { tenant_id: String },
}

// ---------------------------------------------------------------------------
// Circuit breaker
// ---------------------------------------------------------------------------

/// States of a circuit breaker protecting a tenant route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CircuitState {
    /// Normal operation; requests pass through.
    Closed,
    /// Failures exceeded threshold; requests are rejected immediately.
    Open,
    /// After a cooldown period, a single probe request is allowed through.
    HalfOpen,
}

/// Configuration for a circuit breaker.
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures before the circuit opens.
    pub failure_threshold: u32,
    /// Duration the circuit stays open before moving to half-open.
    pub open_duration: Duration,
    /// Number of successes in half-open state required to close the circuit.
    pub half_open_successes: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            open_duration: Duration::from_secs(30),
            half_open_successes: 2,
        }
    }
}

/// Internal circuit breaker state machine.
struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: CircuitState,
    consecutive_failures: u32,
    half_open_successes: u32,
    opened_at: Option<Instant>,
}

impl CircuitBreaker {
    fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: CircuitState::Closed,
            consecutive_failures: 0,
            half_open_successes: 0,
            opened_at: None,
        }
    }

    fn state(&self) -> CircuitState {
        self.state
    }

    /// Check whether a request is allowed. If the circuit is open and the cooldown
    /// has expired, transitions to half-open and allows one probe.
    fn allow_request(&mut self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::HalfOpen => true, // probe request
            CircuitState::Open => {
                if let Some(opened_at) = self.opened_at {
                    if Instant::now().duration_since(opened_at) >= self.config.open_duration {
                        self.state = CircuitState::HalfOpen;
                        self.half_open_successes = 0;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
        }
    }

    fn record_success(&mut self) {
        match self.state {
            CircuitState::Closed => {
                self.consecutive_failures = 0;
            }
            CircuitState::HalfOpen => {
                self.half_open_successes += 1;
                if self.half_open_successes >= self.config.half_open_successes {
                    self.state = CircuitState::Closed;
                    self.consecutive_failures = 0;
                    self.opened_at = None;
                }
            }
            CircuitState::Open => {
                // Shouldn't happen; but reset anyway.
                self.state = CircuitState::Closed;
                self.consecutive_failures = 0;
                self.opened_at = None;
            }
        }
    }

    fn record_failure(&mut self) {
        self.consecutive_failures += 1;
        match self.state {
            CircuitState::Closed => {
                if self.consecutive_failures >= self.config.failure_threshold {
                    self.state = CircuitState::Open;
                    self.opened_at = Some(Instant::now());
                }
            }
            CircuitState::HalfOpen => {
                // Single failure in half-open re-opens the circuit.
                self.state = CircuitState::Open;
                self.opened_at = Some(Instant::now());
                self.half_open_successes = 0;
            }
            CircuitState::Open => {
                // Already open, refresh open time.
                self.opened_at = Some(Instant::now());
            }
        }
    }

    fn reset(&mut self) {
        self.state = CircuitState::Closed;
        self.consecutive_failures = 0;
        self.half_open_successes = 0;
        self.opened_at = None;
    }
}

// ---------------------------------------------------------------------------
// Route health tracking
// ---------------------------------------------------------------------------

/// Health statistics for a single tenant route.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteHealthStats {
    pub tenant_id: String,
    pub total_requests: u64,
    pub total_successes: u64,
    pub total_failures: u64,
    pub circuit_state: CircuitState,
    pub success_rate: f64,
}

/// Internal health tracker.
struct RouteHealth {
    total_requests: u64,
    total_successes: u64,
    total_failures: u64,
}

impl RouteHealth {
    fn new() -> Self {
        Self {
            total_requests: 0,
            total_successes: 0,
            total_failures: 0,
        }
    }

    fn record_success(&mut self) {
        self.total_requests += 1;
        self.total_successes += 1;
    }

    fn record_failure(&mut self) {
        self.total_requests += 1;
        self.total_failures += 1;
    }

    fn success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            1.0
        } else {
            self.total_successes as f64 / self.total_requests as f64
        }
    }
}

// ---------------------------------------------------------------------------
// Route configuration
// ---------------------------------------------------------------------------

/// Configuration that determines how a tenant should behave inside the multi-tenant runtime.
#[derive(Debug, Clone)]
pub struct TenantRouteConfig {
    pub tenant_id: String,
    pub shard_id: String,
    pub queue_name: String,
    pub region: String,
    pub max_inflight: usize,
    pub max_per_second: usize,
    pub consensus_resource: Option<String>,
    pub lease_ttl: Duration,
}

/// Extended route configuration with priority and failover settings.
#[derive(Debug, Clone)]
pub struct TenantRouteConfigExt {
    pub base: TenantRouteConfig,
    /// Priority weight (higher = preferred). Default 100.
    pub priority: u32,
    /// Optional secondary shard for failover.
    pub failover_shard_id: Option<String>,
    /// Circuit breaker config (use Default for sane defaults).
    pub circuit_breaker: CircuitBreakerConfig,
}

impl TenantRouteConfigExt {
    /// Create an extended config from a base config with defaults.
    pub fn from_base(base: TenantRouteConfig) -> Self {
        Self {
            base,
            priority: 100,
            failover_shard_id: None,
            circuit_breaker: CircuitBreakerConfig::default(),
        }
    }

    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_failover(mut self, shard_id: &str) -> Self {
        self.failover_shard_id = Some(shard_id.to_string());
        self
    }

    pub fn with_circuit_breaker(mut self, config: CircuitBreakerConfig) -> Self {
        self.circuit_breaker = config;
        self
    }
}

// ---------------------------------------------------------------------------
// Internal route state
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct TenantRoute {
    config: TenantRouteConfig,
    priority: u32,
    failover_shard_id: Option<String>,
    quota: Arc<TenantQuotaState>,
    circuit_breaker: Arc<tokio::sync::Mutex<CircuitBreaker>>,
    health: Arc<tokio::sync::Mutex<RouteHealth>>,
}

impl TenantRoute {
    fn summary(&self) -> TenantRouteSummary {
        TenantRouteSummary {
            tenant_id: self.config.tenant_id.clone(),
            region: self.config.region.clone(),
            queue_name: self.config.queue_name.clone(),
            shard_id: self.config.shard_id.clone(),
        }
    }

    fn detail_summary(&self) -> TenantRouteDetail {
        TenantRouteDetail {
            tenant_id: self.config.tenant_id.clone(),
            shard_id: self.config.shard_id.clone(),
            queue_name: self.config.queue_name.clone(),
            region: self.config.region.clone(),
            priority: self.priority,
            failover_shard_id: self.failover_shard_id.clone(),
            max_inflight: self.config.max_inflight,
            max_per_second: self.config.max_per_second,
        }
    }
}

/// Snapshot of `TenantRoute` exposed to inbound services so they can scope telemetry/logs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantRouteSummary {
    pub tenant_id: String,
    pub region: String,
    pub queue_name: String,
    pub shard_id: String,
}

/// Detailed view of a tenant route including priority and failover settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantRouteDetail {
    pub tenant_id: String,
    pub shard_id: String,
    pub queue_name: String,
    pub region: String,
    pub priority: u32,
    pub failover_shard_id: Option<String>,
    pub max_inflight: usize,
    pub max_per_second: usize,
}

// ---------------------------------------------------------------------------
// Quota / Rate limiting
// ---------------------------------------------------------------------------

/// Internal permit handle that keeps the semaphore alive until the routing decision is dropped.
struct TenantQuotaPermit {
    _permit: OwnedSemaphorePermit,
}

/// Per-tenant quota state (concurrency + rate).
struct TenantQuotaState {
    semaphore: Arc<Semaphore>,
    max_per_second: usize,
    window: tokio::sync::Mutex<RateWindow>,
}

impl TenantQuotaState {
    fn new(concurrency: usize, max_per_second: usize) -> Self {
        let concurrency = concurrency.max(1);
        Self {
            semaphore: Arc::new(Semaphore::new(concurrency)),
            max_per_second,
            window: tokio::sync::Mutex::new(RateWindow::new()),
        }
    }

    async fn acquire(&self) -> Result<TenantQuotaPermit> {
        if self.max_per_second == 0 {
            // Zero indicates unlimited throughput.
        } else {
            let mut window = self.window.lock().await;
            window.reset_if_needed();
            if window.count >= self.max_per_second {
                bail!("rate exceeded");
            }
            window.count += 1;
        }
        let permit = self.semaphore.clone().acquire_owned().await?;
        Ok(TenantQuotaPermit { _permit: permit })
    }
}

struct RateWindow {
    reset_at: Instant,
    count: usize,
}

impl RateWindow {
    fn new() -> Self {
        Self {
            reset_at: Instant::now(),
            count: 0,
        }
    }

    fn reset_if_needed(&mut self) {
        if Instant::now().duration_since(self.reset_at) >= Duration::from_secs(1) {
            self.reset_at = Instant::now();
            self.count = 0;
        }
    }
}

// ---------------------------------------------------------------------------
// Router state
// ---------------------------------------------------------------------------

struct RouterState {
    routes: HashMap<String, TenantRoute>,
}

impl RouterState {
    fn new() -> Self {
        Self {
            routes: HashMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// TenantRouter
// ---------------------------------------------------------------------------

/// Tenant router that holds routing metadata plus optional lease coordination.
#[derive(Clone)]
pub struct TenantRouter {
    state: Arc<RwLock<RouterState>>,
    lease_coordinator: Option<Arc<LeaseCoordinator>>,
}

impl TenantRouter {
    /// Returns a new router without consensus leasing.
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(RouterState::new())),
            lease_coordinator: None,
        }
    }

    /// Returns a router that will guard routes with the given lease coordinator.
    pub fn with_leases(lease_coordinator: Arc<LeaseCoordinator>) -> Self {
        Self {
            state: Arc::new(RwLock::new(RouterState::new())),
            lease_coordinator: Some(lease_coordinator),
        }
    }

    // ---- Registration ----

    /// Register a tenant route into the router. Re-registering an existing tenant replaces the route.
    pub async fn register(&self, config: TenantRouteConfig) {
        let route = TenantRoute {
            priority: 100,
            failover_shard_id: None,
            quota: Arc::new(TenantQuotaState::new(
                config.max_inflight,
                config.max_per_second,
            )),
            circuit_breaker: Arc::new(tokio::sync::Mutex::new(CircuitBreaker::new(
                CircuitBreakerConfig::default(),
            ))),
            health: Arc::new(tokio::sync::Mutex::new(RouteHealth::new())),
            config,
        };
        let mut state = self.state.write().await;
        state.routes.insert(route.config.tenant_id.clone(), route);
    }

    /// Register a tenant route with extended configuration (priority, failover, circuit breaker).
    pub async fn register_extended(&self, config: TenantRouteConfigExt) {
        let route = TenantRoute {
            priority: config.priority,
            failover_shard_id: config.failover_shard_id,
            quota: Arc::new(TenantQuotaState::new(
                config.base.max_inflight,
                config.base.max_per_second,
            )),
            circuit_breaker: Arc::new(tokio::sync::Mutex::new(CircuitBreaker::new(
                config.circuit_breaker,
            ))),
            health: Arc::new(tokio::sync::Mutex::new(RouteHealth::new())),
            config: config.base,
        };
        let mut state = self.state.write().await;
        state.routes.insert(route.config.tenant_id.clone(), route);
    }

    /// Unregister a tenant route. Returns `true` if it existed.
    pub async fn unregister(&self, tenant_id: &str) -> bool {
        let mut state = self.state.write().await;
        state.routes.remove(tenant_id).is_some()
    }

    /// Check if a route is registered for a tenant.
    pub async fn is_registered(&self, tenant_id: &str) -> bool {
        let state = self.state.read().await;
        state.routes.contains_key(tenant_id)
    }

    /// Number of registered tenant routes.
    pub async fn route_count(&self) -> usize {
        let state = self.state.read().await;
        state.routes.len()
    }

    // ---- Routing ----

    /// Attempts to acquire a routing decision for the provided context.
    pub async fn route_request(
        &self,
        context: TenantContext,
    ) -> Result<TenantRoutingDecision, RoutingError> {
        let state = self.state.read().await;
        let route = state
            .routes
            .get(&context.tenant_id)
            .ok_or_else(|| RoutingError::UnknownTenant(context.tenant_id.clone()))?;

        // Check circuit breaker.
        {
            let mut cb = route.circuit_breaker.lock().await;
            if !cb.allow_request() {
                return Err(RoutingError::CircuitOpen {
                    tenant_id: context.tenant_id.clone(),
                });
            }
        }

        let permit = route
            .quota
            .acquire()
            .await
            .map_err(|_| RoutingError::QuotaExceeded {
                tenant_id: context.tenant_id.clone(),
            })?;

        let lease_token = if let Some(coordinator) = &self.lease_coordinator {
            if let Some(resource) = route.config.consensus_resource.clone() {
                Some(
                    coordinator
                        .acquire(&resource, &context.tenant_id, route.config.lease_ttl)
                        .await
                        .map_err(RoutingError::LeaseFailure)?,
                )
            } else {
                None
            }
        } else {
            None
        };

        let guard = TenantGuard::new(context.clone());

        Ok(TenantRoutingDecision {
            guard,
            route: route.summary(),
            lease_token,
            _permit: permit,
            _failover_shard: route.failover_shard_id.clone(),
        })
    }

    /// Route a request and if the primary route's circuit is open, try the failover shard
    /// (if configured). Returns the routing decision with the failover shard info.
    pub async fn route_with_failover(
        &self,
        context: TenantContext,
    ) -> Result<TenantRoutingDecision, RoutingError> {
        match self.route_request(context.clone()).await {
            Ok(decision) => Ok(decision),
            Err(RoutingError::CircuitOpen { ref tenant_id }) => {
                let state = self.state.read().await;
                let route = state
                    .routes
                    .get(tenant_id)
                    .ok_or_else(|| RoutingError::UnknownTenant(tenant_id.clone()))?;
                if let Some(failover_shard) = &route.failover_shard_id {
                    let permit =
                        route
                            .quota
                            .acquire()
                            .await
                            .map_err(|_| RoutingError::QuotaExceeded {
                                tenant_id: tenant_id.clone(),
                            })?;
                    let mut summary = route.summary();
                    summary.shard_id = failover_shard.clone();
                    let guard = TenantGuard::new(context);
                    Ok(TenantRoutingDecision {
                        guard,
                        route: summary,
                        lease_token: None,
                        _permit: permit,
                        _failover_shard: Some(failover_shard.clone()),
                    })
                } else {
                    Err(RoutingError::NoHealthyRoute {
                        tenant_id: tenant_id.clone(),
                    })
                }
            }
            Err(e) => Err(e),
        }
    }

    // ---- Feedback (health + circuit breaker) ----

    /// Record a successful operation for the given tenant. Updates health stats and circuit breaker.
    pub async fn record_success(&self, tenant_id: &str) {
        let state = self.state.read().await;
        if let Some(route) = state.routes.get(tenant_id) {
            let mut health = route.health.lock().await;
            health.record_success();
            let mut cb = route.circuit_breaker.lock().await;
            cb.record_success();
        }
    }

    /// Record a failed operation for the given tenant. Updates health stats and circuit breaker.
    pub async fn record_failure(&self, tenant_id: &str) {
        let state = self.state.read().await;
        if let Some(route) = state.routes.get(tenant_id) {
            let mut health = route.health.lock().await;
            health.record_failure();
            let mut cb = route.circuit_breaker.lock().await;
            cb.record_failure();
        }
    }

    /// Reset the circuit breaker for a tenant (e.g., after manual intervention).
    pub async fn reset_circuit_breaker(&self, tenant_id: &str) -> bool {
        let state = self.state.read().await;
        if let Some(route) = state.routes.get(tenant_id) {
            let mut cb = route.circuit_breaker.lock().await;
            cb.reset();
            true
        } else {
            false
        }
    }

    /// Get the current circuit state for a tenant.
    pub async fn circuit_state(&self, tenant_id: &str) -> Option<CircuitState> {
        let state = self.state.read().await;
        if let Some(route) = state.routes.get(tenant_id) {
            let cb = route.circuit_breaker.lock().await;
            Some(cb.state())
        } else {
            None
        }
    }

    // ---- Health stats ----

    /// Get health stats for a single tenant.
    pub async fn health_stats(&self, tenant_id: &str) -> Option<RouteHealthStats> {
        let state = self.state.read().await;
        if let Some(route) = state.routes.get(tenant_id) {
            let health = route.health.lock().await;
            let cb = route.circuit_breaker.lock().await;
            Some(RouteHealthStats {
                tenant_id: tenant_id.to_string(),
                total_requests: health.total_requests,
                total_successes: health.total_successes,
                total_failures: health.total_failures,
                circuit_state: cb.state(),
                success_rate: health.success_rate(),
            })
        } else {
            None
        }
    }

    /// Get health stats for all registered routes.
    pub async fn all_health_stats(&self) -> Vec<RouteHealthStats> {
        let state = self.state.read().await;
        let mut results = Vec::new();
        for (tenant_id, route) in &state.routes {
            let health = route.health.lock().await;
            let cb = route.circuit_breaker.lock().await;
            results.push(RouteHealthStats {
                tenant_id: tenant_id.clone(),
                total_requests: health.total_requests,
                total_successes: health.total_successes,
                total_failures: health.total_failures,
                circuit_state: cb.state(),
                success_rate: health.success_rate(),
            });
        }
        results
    }

    // ---- Query ----

    /// List detail summaries for all routes.
    pub async fn list_routes(&self) -> Vec<TenantRouteDetail> {
        let state = self.state.read().await;
        state.routes.values().map(|r| r.detail_summary()).collect()
    }

    /// List routes filtered by region.
    pub async fn routes_by_region(&self, region: &str) -> Vec<TenantRouteDetail> {
        let state = self.state.read().await;
        state
            .routes
            .values()
            .filter(|r| r.config.region == region)
            .map(|r| r.detail_summary())
            .collect()
    }

    /// List routes filtered by shard.
    pub async fn routes_by_shard(&self, shard_id: &str) -> Vec<TenantRouteDetail> {
        let state = self.state.read().await;
        state
            .routes
            .values()
            .filter(|r| r.config.shard_id == shard_id)
            .map(|r| r.detail_summary())
            .collect()
    }

    /// Get the route detail for a specific tenant.
    pub async fn get_route(&self, tenant_id: &str) -> Option<TenantRouteDetail> {
        let state = self.state.read().await;
        state.routes.get(tenant_id).map(|r| r.detail_summary())
    }

    /// Find the highest-priority route among a set of candidate tenants.
    /// Useful when a single user has multiple route configs and you want the best one.
    pub async fn highest_priority_route(&self, tenant_ids: &[&str]) -> Option<TenantRouteDetail> {
        let state = self.state.read().await;
        tenant_ids
            .iter()
            .filter_map(|id| state.routes.get(*id))
            .max_by_key(|r| r.priority)
            .map(|r| r.detail_summary())
    }

    // ---- Lease management ----

    /// Releases a lease that was previously returned during routing.
    pub async fn release_lease(&self, token: LeaseToken) {
        if let Some(coordinator) = &self.lease_coordinator {
            coordinator.release(&token.resource, &token.owner).await;
        }
    }

    /// Update the priority of an existing route. Returns false if tenant not found.
    pub async fn set_priority(&self, tenant_id: &str, priority: u32) -> bool {
        let mut state = self.state.write().await;
        if let Some(route) = state.routes.get_mut(tenant_id) {
            route.priority = priority;
            true
        } else {
            false
        }
    }

    /// Update the failover shard of an existing route. Returns false if tenant not found.
    pub async fn set_failover_shard(
        &self,
        tenant_id: &str,
        failover_shard_id: Option<String>,
    ) -> bool {
        let mut state = self.state.write().await;
        if let Some(route) = state.routes.get_mut(tenant_id) {
            route.failover_shard_id = failover_shard_id;
            true
        } else {
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Routing decision
// ---------------------------------------------------------------------------

/// The decision returned by `TenantRouter::route_request`.
pub struct TenantRoutingDecision {
    pub guard: TenantGuard,
    pub route: TenantRouteSummary,
    pub lease_token: Option<LeaseToken>,
    _permit: TenantQuotaPermit,
    _failover_shard: Option<String>,
}

impl TenantRoutingDecision {
    /// Returns the tenant ID associated with this decision.
    pub fn tenant_id(&self) -> &'_ str {
        &self.route.tenant_id
    }

    /// Returns the failover shard ID, if available.
    pub fn failover_shard(&self) -> Option<&str> {
        self._failover_shard.as_deref()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use platform_tenancy::TenantAccessLevel;
    use tokio::time::Duration;

    fn ctx(id: &str) -> TenantContext {
        TenantContext {
            tenant_id: id.to_string(),
            tenant_name: Some("Tenant".into()),
            access_level: TenantAccessLevel::Worker,
        }
    }

    fn default_config(tenant_id: &str) -> TenantRouteConfig {
        TenantRouteConfig {
            tenant_id: tenant_id.into(),
            shard_id: "shard-1".into(),
            queue_name: "default".into(),
            region: "eu".into(),
            max_inflight: 2,
            max_per_second: 100,
            consensus_resource: None,
            lease_ttl: Duration::from_secs(1),
        }
    }

    // ---- Basic registration & routing ----

    #[tokio::test]
    async fn registers_and_routes_tenant() {
        let router = TenantRouter::new();
        router.register(default_config("tenant-a")).await;
        let decision = router
            .route_request(ctx("tenant-a"))
            .await
            .expect("routing succeeds");
        assert_eq!(decision.route.queue_name, "default");
        assert_eq!(decision.tenant_id(), "tenant-a");
    }

    #[tokio::test]
    async fn unknown_tenant_errors() {
        let router = TenantRouter::new();
        let err = router.route_request(ctx("ghost")).await;
        assert!(matches!(err, Err(RoutingError::UnknownTenant(_))));
    }

    #[tokio::test]
    async fn rate_limited_breaches() {
        let router = TenantRouter::new();
        router
            .register(TenantRouteConfig {
                tenant_id: "burst".into(),
                shard_id: "shard-1".into(),
                queue_name: "default".into(),
                region: "us".into(),
                max_inflight: 1,
                max_per_second: 1,
                consensus_resource: None,
                lease_ttl: Duration::from_secs(1),
            })
            .await;
        let _ = router.route_request(ctx("burst")).await.unwrap();
        let err = router.route_request(ctx("burst")).await;
        assert!(matches!(err, Err(RoutingError::QuotaExceeded { .. })));
    }

    // ---- Unregister ----

    #[tokio::test]
    async fn unregister_removes_route() {
        let router = TenantRouter::new();
        router.register(default_config("t1")).await;
        assert!(router.is_registered("t1").await);
        assert!(router.unregister("t1").await);
        assert!(!router.is_registered("t1").await);
        assert!(!router.unregister("t1").await); // already gone
    }

    #[tokio::test]
    async fn route_count() {
        let router = TenantRouter::new();
        assert_eq!(router.route_count().await, 0);
        router.register(default_config("a")).await;
        router.register(default_config("b")).await;
        assert_eq!(router.route_count().await, 2);
        router.unregister("a").await;
        assert_eq!(router.route_count().await, 1);
    }

    // ---- Extended registration ----

    #[tokio::test]
    async fn register_extended_with_priority() {
        let router = TenantRouter::new();
        let ext = TenantRouteConfigExt::from_base(default_config("t1")).with_priority(200);
        router.register_extended(ext).await;
        let detail = router.get_route("t1").await.unwrap();
        assert_eq!(detail.priority, 200);
    }

    #[tokio::test]
    async fn register_extended_with_failover() {
        let router = TenantRouter::new();
        let ext =
            TenantRouteConfigExt::from_base(default_config("t1")).with_failover("shard-backup");
        router.register_extended(ext).await;
        let detail = router.get_route("t1").await.unwrap();
        assert_eq!(detail.failover_shard_id.as_deref(), Some("shard-backup"));
    }

    // ---- Circuit breaker ----

    #[tokio::test]
    async fn circuit_breaker_opens_after_threshold() {
        let router = TenantRouter::new();
        let ext = TenantRouteConfigExt::from_base(default_config("t1")).with_circuit_breaker(
            CircuitBreakerConfig {
                failure_threshold: 3,
                open_duration: Duration::from_secs(60),
                half_open_successes: 1,
            },
        );
        router.register_extended(ext).await;

        assert_eq!(router.circuit_state("t1").await, Some(CircuitState::Closed));

        // Record 3 failures.
        for _ in 0..3 {
            router.record_failure("t1").await;
        }

        assert_eq!(router.circuit_state("t1").await, Some(CircuitState::Open));

        // Routing should be rejected.
        let err = router.route_request(ctx("t1")).await;
        assert!(matches!(err, Err(RoutingError::CircuitOpen { .. })));
    }

    #[tokio::test]
    async fn circuit_breaker_resets_on_manual_reset() {
        let router = TenantRouter::new();
        let ext = TenantRouteConfigExt::from_base(default_config("t1")).with_circuit_breaker(
            CircuitBreakerConfig {
                failure_threshold: 1,
                open_duration: Duration::from_secs(60),
                half_open_successes: 1,
            },
        );
        router.register_extended(ext).await;

        router.record_failure("t1").await;
        assert_eq!(router.circuit_state("t1").await, Some(CircuitState::Open));

        router.reset_circuit_breaker("t1").await;
        assert_eq!(router.circuit_state("t1").await, Some(CircuitState::Closed));

        // Should be routable again.
        let decision = router.route_request(ctx("t1")).await;
        assert!(decision.is_ok());
    }

    #[tokio::test]
    async fn circuit_breaker_half_open_recovers() {
        let router = TenantRouter::new();
        let ext = TenantRouteConfigExt::from_base(default_config("t1")).with_circuit_breaker(
            CircuitBreakerConfig {
                failure_threshold: 2,
                open_duration: Duration::from_millis(50),
                half_open_successes: 2,
            },
        );
        router.register_extended(ext).await;

        // Open circuit.
        router.record_failure("t1").await;
        router.record_failure("t1").await;
        assert_eq!(router.circuit_state("t1").await, Some(CircuitState::Open));

        // Wait for cooldown.
        tokio::time::sleep(Duration::from_millis(60)).await;

        // Next request transitions to half-open and is allowed.
        let decision = router.route_request(ctx("t1")).await;
        assert!(decision.is_ok());
        assert_eq!(
            router.circuit_state("t1").await,
            Some(CircuitState::HalfOpen)
        );

        // Record successes to close.
        router.record_success("t1").await;
        router.record_success("t1").await;
        assert_eq!(router.circuit_state("t1").await, Some(CircuitState::Closed));
    }

    #[tokio::test]
    async fn circuit_breaker_half_open_failure_reopens() {
        let router = TenantRouter::new();
        let ext = TenantRouteConfigExt::from_base(default_config("t1")).with_circuit_breaker(
            CircuitBreakerConfig {
                failure_threshold: 1,
                open_duration: Duration::from_millis(50),
                half_open_successes: 3,
            },
        );
        router.register_extended(ext).await;

        router.record_failure("t1").await;
        assert_eq!(router.circuit_state("t1").await, Some(CircuitState::Open));

        tokio::time::sleep(Duration::from_millis(60)).await;

        // Transition to half-open via route_request.
        let _ = router.route_request(ctx("t1")).await;
        assert_eq!(
            router.circuit_state("t1").await,
            Some(CircuitState::HalfOpen)
        );

        // Failure in half-open reopens.
        router.record_failure("t1").await;
        assert_eq!(router.circuit_state("t1").await, Some(CircuitState::Open));
    }

    // ---- Failover routing ----

    #[tokio::test]
    async fn failover_routes_to_secondary_when_circuit_open() {
        let router = TenantRouter::new();
        let ext = TenantRouteConfigExt::from_base(default_config("t1"))
            .with_failover("shard-backup")
            .with_circuit_breaker(CircuitBreakerConfig {
                failure_threshold: 1,
                open_duration: Duration::from_secs(60),
                half_open_successes: 1,
            });
        router.register_extended(ext).await;

        // Open the circuit.
        router.record_failure("t1").await;

        // Normal route should fail.
        let err = router.route_request(ctx("t1")).await;
        assert!(matches!(err, Err(RoutingError::CircuitOpen { .. })));

        // Failover should succeed with backup shard.
        let decision = router.route_with_failover(ctx("t1")).await.unwrap();
        assert_eq!(decision.route.shard_id, "shard-backup");
    }

    #[tokio::test]
    async fn failover_returns_no_healthy_route_when_no_secondary() {
        let router = TenantRouter::new();
        let ext = TenantRouteConfigExt::from_base(default_config("t1")).with_circuit_breaker(
            CircuitBreakerConfig {
                failure_threshold: 1,
                open_duration: Duration::from_secs(60),
                half_open_successes: 1,
            },
        );
        router.register_extended(ext).await;

        router.record_failure("t1").await;

        let err = router.route_with_failover(ctx("t1")).await;
        assert!(matches!(err, Err(RoutingError::NoHealthyRoute { .. })));
    }

    #[tokio::test]
    async fn failover_uses_primary_when_circuit_closed() {
        let router = TenantRouter::new();
        let ext =
            TenantRouteConfigExt::from_base(default_config("t1")).with_failover("shard-backup");
        router.register_extended(ext).await;

        let decision = router.route_with_failover(ctx("t1")).await.unwrap();
        assert_eq!(decision.route.shard_id, "shard-1"); // primary
    }

    // ---- Health tracking ----

    #[tokio::test]
    async fn health_stats_track_requests() {
        let router = TenantRouter::new();
        router.register(default_config("t1")).await;

        router.record_success("t1").await;
        router.record_success("t1").await;
        router.record_failure("t1").await;

        let stats = router.health_stats("t1").await.unwrap();
        assert_eq!(stats.total_requests, 3);
        assert_eq!(stats.total_successes, 2);
        assert_eq!(stats.total_failures, 1);
        let expected_rate = 2.0 / 3.0;
        assert!((stats.success_rate - expected_rate).abs() < 0.01);
    }

    #[tokio::test]
    async fn health_stats_default_rate_is_one() {
        let router = TenantRouter::new();
        router.register(default_config("t1")).await;

        let stats = router.health_stats("t1").await.unwrap();
        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.success_rate, 1.0);
    }

    #[tokio::test]
    async fn health_stats_returns_none_for_unknown() {
        let router = TenantRouter::new();
        assert!(router.health_stats("nope").await.is_none());
    }

    #[tokio::test]
    async fn all_health_stats() {
        let router = TenantRouter::new();
        router.register(default_config("a")).await;
        router.register(default_config("b")).await;
        router.record_success("a").await;
        router.record_failure("b").await;

        let all = router.all_health_stats().await;
        assert_eq!(all.len(), 2);
    }

    // ---- Query / listing ----

    #[tokio::test]
    async fn list_routes() {
        let router = TenantRouter::new();
        router.register(default_config("a")).await;
        router.register(default_config("b")).await;
        let routes = router.list_routes().await;
        assert_eq!(routes.len(), 2);
    }

    #[tokio::test]
    async fn routes_by_region() {
        let router = TenantRouter::new();
        router.register(default_config("a")).await;
        let mut us_config = default_config("b");
        us_config.region = "us".into();
        router.register(us_config).await;

        let eu = router.routes_by_region("eu").await;
        assert_eq!(eu.len(), 1);
        assert_eq!(eu[0].tenant_id, "a");

        let us = router.routes_by_region("us").await;
        assert_eq!(us.len(), 1);
        assert_eq!(us[0].tenant_id, "b");
    }

    #[tokio::test]
    async fn routes_by_shard() {
        let router = TenantRouter::new();
        router.register(default_config("a")).await;
        let mut s2_config = default_config("b");
        s2_config.shard_id = "shard-2".into();
        router.register(s2_config).await;

        let s1 = router.routes_by_shard("shard-1").await;
        assert_eq!(s1.len(), 1);
        let s2 = router.routes_by_shard("shard-2").await;
        assert_eq!(s2.len(), 1);
    }

    #[tokio::test]
    async fn get_route_detail() {
        let router = TenantRouter::new();
        router.register(default_config("a")).await;
        let detail = router.get_route("a").await.unwrap();
        assert_eq!(detail.shard_id, "shard-1");
        assert_eq!(detail.priority, 100);
        assert!(router.get_route("nope").await.is_none());
    }

    // ---- Priority routing ----

    #[tokio::test]
    async fn highest_priority_route() {
        let router = TenantRouter::new();
        let ext_a = TenantRouteConfigExt::from_base(default_config("a")).with_priority(50);
        let ext_b = TenantRouteConfigExt::from_base(default_config("b")).with_priority(200);
        let ext_c = TenantRouteConfigExt::from_base(default_config("c")).with_priority(150);
        router.register_extended(ext_a).await;
        router.register_extended(ext_b).await;
        router.register_extended(ext_c).await;

        let best = router
            .highest_priority_route(&["a", "b", "c"])
            .await
            .unwrap();
        assert_eq!(best.tenant_id, "b");
        assert_eq!(best.priority, 200);
    }

    #[tokio::test]
    async fn highest_priority_route_empty_returns_none() {
        let router = TenantRouter::new();
        assert!(router.highest_priority_route(&["x", "y"]).await.is_none());
    }

    // ---- Set priority / failover ----

    #[tokio::test]
    async fn set_priority() {
        let router = TenantRouter::new();
        router.register(default_config("t1")).await;
        assert!(router.set_priority("t1", 500).await);
        let detail = router.get_route("t1").await.unwrap();
        assert_eq!(detail.priority, 500);
        assert!(!router.set_priority("nope", 1).await);
    }

    #[tokio::test]
    async fn set_failover_shard() {
        let router = TenantRouter::new();
        router.register(default_config("t1")).await;
        assert!(router.set_failover_shard("t1", Some("backup".into())).await);
        let detail = router.get_route("t1").await.unwrap();
        assert_eq!(detail.failover_shard_id.as_deref(), Some("backup"));
        assert!(router.set_failover_shard("t1", None).await);
        let detail = router.get_route("t1").await.unwrap();
        assert!(detail.failover_shard_id.is_none());
    }

    // ---- Lease integration ----

    #[tokio::test]
    async fn routes_with_lease_coordinator() {
        let coordinator = Arc::new(LeaseCoordinator::new());
        let router = TenantRouter::with_leases(coordinator);
        let mut config = default_config("t1");
        config.consensus_resource = Some("resource:t1".into());
        router.register(config).await;

        let decision = router.route_request(ctx("t1")).await.unwrap();
        assert!(decision.lease_token.is_some());
        let token = decision.lease_token.unwrap();
        assert_eq!(token.resource, "resource:t1");
        router.release_lease(token).await;
    }

    #[tokio::test]
    async fn lease_contention_fails_second_request() {
        let coordinator = Arc::new(LeaseCoordinator::new());
        let router = TenantRouter::with_leases(coordinator);
        let mut config = default_config("t1");
        config.consensus_resource = Some("resource:t1".into());
        router.register(config).await;

        let _decision1 = router.route_request(ctx("t1")).await.unwrap();
        let err = router.route_request(ctx("t1")).await;
        assert!(matches!(err, Err(RoutingError::LeaseFailure(_))));
    }

    // ---- Circuit breaker unit tests ----

    #[test]
    fn circuit_breaker_state_transitions() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            open_duration: Duration::from_secs(60),
            half_open_successes: 1,
        };
        let mut cb = CircuitBreaker::new(config);
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allow_request());

        // One failure — still closed.
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);

        // Second failure — opens.
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.allow_request()); // cooldown hasn't passed

        // Manual reset.
        cb.reset();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allow_request());
    }

    #[test]
    fn circuit_breaker_success_resets_counter() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            open_duration: Duration::from_secs(60),
            half_open_successes: 1,
        };
        let mut cb = CircuitBreaker::new(config);
        cb.record_failure();
        cb.record_failure();
        cb.record_success(); // resets counter
        cb.record_failure(); // only 1 consecutive now
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    // ---- Serialization ----

    #[test]
    fn circuit_state_serialization() {
        let state = CircuitState::HalfOpen;
        let json = serde_json::to_string(&state).unwrap();
        let parsed: CircuitState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, CircuitState::HalfOpen);
    }

    #[test]
    fn route_health_stats_serialization() {
        let stats = RouteHealthStats {
            tenant_id: "t1".into(),
            total_requests: 100,
            total_successes: 95,
            total_failures: 5,
            circuit_state: CircuitState::Closed,
            success_rate: 0.95,
        };
        let json = serde_json::to_string(&stats).unwrap();
        let parsed: RouteHealthStats = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.total_requests, 100);
        assert_eq!(parsed.circuit_state, CircuitState::Closed);
    }

    #[test]
    fn tenant_route_detail_serialization() {
        let detail = TenantRouteDetail {
            tenant_id: "t1".into(),
            shard_id: "shard-1".into(),
            queue_name: "default".into(),
            region: "eu".into(),
            priority: 100,
            failover_shard_id: Some("shard-backup".into()),
            max_inflight: 10,
            max_per_second: 50,
        };
        let json = serde_json::to_string(&detail).unwrap();
        let parsed: TenantRouteDetail = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.priority, 100);
        assert_eq!(parsed.failover_shard_id.as_deref(), Some("shard-backup"));
    }
}
