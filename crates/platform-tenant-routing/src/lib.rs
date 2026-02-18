//! Tenant-routing and quota helpers that keep multi-tenant workers and HTTP gateways safe.
//! This crate builds on `platform-tenancy`/`platform-consensus` to map tenants to shards,
//! enforce per-tenant quotas, and tie routing decisions to consensus leases when shared resources
//! require arbitration.

use anyhow::{bail, Result};
use platform_consensus::{LeaseCoordinator, LeaseToken};
use platform_tenancy::{TenantContext, TenantGuard};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{OwnedSemaphorePermit, RwLock, Semaphore};
use tokio::time::{Duration, Instant};

#[derive(Debug, Error)]
pub enum RoutingError {
    #[error("tenant {0} is unknown")]
    UnknownTenant(String),

    #[error("quota exceeded for tenant {tenant_id}")]
    QuotaExceeded { tenant_id: String },

    #[error("lease acquisition failed: {0}")]
    LeaseFailure(anyhow::Error),
}

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

#[derive(Clone)]
struct TenantRoute {
    config: TenantRouteConfig,
    quota: Arc<TenantQuotaState>,
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
}

/// Snapshot of `TenantRoute` exposed to inbound services so they can scope telemetry/logs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantRouteSummary {
    pub tenant_id: String,
    pub region: String,
    pub queue_name: String,
    pub shard_id: String,
}

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

    /// Register a tenant route into the router. Re-registering an existing tenant replaces the route.
    pub async fn register(&self, config: TenantRouteConfig) {
        let route = TenantRoute {
            quota: Arc::new(TenantQuotaState::new(
                config.max_inflight,
                config.max_per_second,
            )),
            config,
        };
        let mut state = self.state.write().await;
        state.routes.insert(route.config.tenant_id.clone(), route);
    }

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
        })
    }

    /// Releases a lease that was previously returned during routing.
    pub async fn release_lease(&self, token: LeaseToken) {
        if let Some(coordinator) = &self.lease_coordinator {
            coordinator.release(&token.resource, &token.owner).await;
        }
    }
}

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

/// The decision returned by `TenantRouter::route_request`.
pub struct TenantRoutingDecision {
    pub guard: TenantGuard,
    pub route: TenantRouteSummary,
    pub lease_token: Option<LeaseToken>,
    _permit: TenantQuotaPermit,
}

impl TenantRoutingDecision {
    /// Returns the tenant ID associated with this decision.
    pub fn tenant_id(&self) -> &'_ str {
        &self.route.tenant_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Duration;

    fn default_context(id: &str) -> TenantContext {
        TenantContext {
            tenant_id: id.to_string(),
            tenant_name: Some("Tenant".into()),
            access_level: platform_tenancy::TenantAccessLevel::Worker,
        }
    }

    #[tokio::test]
    async fn registers_and_routes_tenant() {
        let router = TenantRouter::new();
        router
            .register(TenantRouteConfig {
                tenant_id: "tenant-a".into(),
                shard_id: "shard-1".into(),
                queue_name: "default".into(),
                region: "eu".into(),
                max_inflight: 2,
                max_per_second: 5,
                consensus_resource: None,
                lease_ttl: Duration::from_secs(1),
            })
            .await;

        let decision = router
            .route_request(default_context("tenant-a"))
            .await
            .expect("routing succeeds");
        assert_eq!(decision.route.queue_name, "default");
    }

    #[tokio::test]
    async fn unknown_tenant_errors() {
        let router = TenantRouter::new();
        let err = router.route_request(default_context("ghost")).await;
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

        let _ = router
            .route_request(default_context("burst"))
            .await
            .unwrap();
        let err = router.route_request(default_context("burst")).await;
        assert!(matches!(err, Err(RoutingError::QuotaExceeded { .. })));
    }
}
