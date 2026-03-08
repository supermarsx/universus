//! Tenant-aware context and routing helpers shared across the Rust services.
//! This crate surfaces a `TenantContext` guard that can be mounted into Axum routers
//! and passed through worker queues so requests/work loops can carry a tenant identifier.
//!
//! Additionally provides a full multi-tenancy system including:
//! - Tenant registry with plans, statuses, and settings
//! - Tenant isolation configuration (shared/dedicated/hybrid)
//! - Resource quota tracking and enforcement
//! - Tenant resolution from request headers and host mappings
//! - Audit logging for tenant operations

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Original types (kept exactly as-is)
// ---------------------------------------------------------------------------

/// Inline placeholder for tenant metadata and per-request scope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantContext {
    pub tenant_id: String,
    pub tenant_name: Option<String>,
    pub access_level: TenantAccessLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TenantAccessLevel {
    Admin,
    ReadOnly,
    Worker,
}

/// Shared guard with minimal locking for the Axum router.
#[derive(Clone)]
pub struct TenantGuard {
    context: Arc<TenantContext>,
}

impl TenantGuard {
    pub fn new(context: TenantContext) -> Self {
        Self {
            context: Arc::new(context),
        }
    }

    pub fn context(&self) -> &TenantContext {
        &self.context
    }
}

// ---------------------------------------------------------------------------
// Tenant Registry
// ---------------------------------------------------------------------------

/// Billing/feature plan for a tenant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TenantPlan {
    Free,
    Basic,
    Premium,
    Enterprise,
}

/// Lifecycle status of a tenant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TenantStatus {
    Active,
    Suspended,
    Archived,
    PendingSetup,
}

/// Per-tenant configuration knobs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TenantSettings {
    pub custom_domain: Option<String>,
    pub branding_color: Option<String>,
    pub max_storage_mb: u64,
    pub features_enabled: Vec<String>,
}

/// Core tenant record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tenant {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub plan: TenantPlan,
    pub status: TenantStatus,
    pub created_at: String,
    pub updated_at: String,
    pub settings: TenantSettings,
    pub max_universes: u32,
    pub max_players_per_universe: u32,
}

/// In-memory tenant store keyed by tenant id.
#[derive(Debug, Clone, Default)]
pub struct TenantRegistry {
    tenants: HashMap<String, Tenant>,
}

impl TenantRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new tenant. Rejects duplicate id or slug.
    pub fn register(&mut self, tenant: Tenant) -> Result<(), String> {
        if self.tenants.contains_key(&tenant.id) {
            return Err(format!("Tenant with id '{}' already exists", tenant.id));
        }
        if self.tenants.values().any(|t| t.slug == tenant.slug) {
            return Err(format!("Tenant with slug '{}' already exists", tenant.slug));
        }
        self.tenants.insert(tenant.id.clone(), tenant);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&Tenant> {
        self.tenants.get(id)
    }

    pub fn get_by_slug(&self, slug: &str) -> Option<&Tenant> {
        self.tenants.values().find(|t| t.slug == slug)
    }

    pub fn list(&self) -> Vec<&Tenant> {
        self.tenants.values().collect()
    }

    pub fn list_by_status(&self, status: &TenantStatus) -> Vec<&Tenant> {
        self.tenants
            .values()
            .filter(|t| t.status == *status)
            .collect()
    }

    pub fn update_status(&mut self, id: &str, status: TenantStatus) -> bool {
        if let Some(tenant) = self.tenants.get_mut(id) {
            tenant.status = status;
            true
        } else {
            false
        }
    }

    pub fn update_plan(&mut self, id: &str, plan: TenantPlan) -> bool {
        if let Some(tenant) = self.tenants.get_mut(id) {
            tenant.plan = plan;
            true
        } else {
            false
        }
    }

    pub fn update_settings(&mut self, id: &str, settings: TenantSettings) -> bool {
        if let Some(tenant) = self.tenants.get_mut(id) {
            tenant.settings = settings;
            true
        } else {
            false
        }
    }

    pub fn delete(&mut self, id: &str) -> bool {
        self.tenants.remove(id).is_some()
    }

    pub fn count(&self) -> usize {
        self.tenants.len()
    }
}

// ---------------------------------------------------------------------------
// Tenant Isolation
// ---------------------------------------------------------------------------

/// How a tenant's data is isolated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IsolationLevel {
    Shared,
    Dedicated,
    Hybrid,
}

/// Isolation configuration for a single tenant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantIsolation {
    pub tenant_id: String,
    pub isolation_level: IsolationLevel,
    pub database_schema: Option<String>,
    pub cache_prefix: String,
    pub event_namespace: String,
}

/// In-memory store for tenant isolation configs.
#[derive(Debug, Clone, Default)]
pub struct IsolationRegistry {
    isolations: HashMap<String, TenantIsolation>,
}

impl IsolationRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, isolation: TenantIsolation) -> Result<(), String> {
        if self.isolations.contains_key(&isolation.tenant_id) {
            return Err(format!(
                "Isolation for tenant '{}' already registered",
                isolation.tenant_id
            ));
        }
        self.isolations
            .insert(isolation.tenant_id.clone(), isolation);
        Ok(())
    }

    pub fn get(&self, tenant_id: &str) -> Option<&TenantIsolation> {
        self.isolations.get(tenant_id)
    }

    /// Returns the custom cache prefix if registered, otherwise `"tenant:{id}:"`.
    pub fn get_cache_prefix(&self, tenant_id: &str) -> String {
        self.isolations
            .get(tenant_id)
            .map(|i| i.cache_prefix.clone())
            .unwrap_or_else(|| format!("tenant:{tenant_id}:"))
    }

    pub fn get_event_namespace(&self, tenant_id: &str) -> String {
        self.isolations
            .get(tenant_id)
            .map(|i| i.event_namespace.clone())
            .unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// Resource Quotas
// ---------------------------------------------------------------------------

/// Resource quota definition and current usage counters for a tenant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceQuota {
    pub tenant_id: String,
    pub max_api_calls_per_hour: u64,
    pub max_storage_bytes: u64,
    pub max_concurrent_connections: u32,
    pub current_api_calls: u64,
    pub current_storage_bytes: u64,
    pub current_connections: u32,
}

/// In-memory quota store.
#[derive(Debug, Clone, Default)]
pub struct QuotaRegistry {
    quotas: HashMap<String, ResourceQuota>,
}

impl QuotaRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_quota(&mut self, quota: ResourceQuota) {
        self.quotas.insert(quota.tenant_id.clone(), quota);
    }

    pub fn get_quota(&self, tenant_id: &str) -> Option<&ResourceQuota> {
        self.quotas.get(tenant_id)
    }

    /// Returns `true` if the tenant is under the API call limit.
    pub fn check_api_limit(&self, tenant_id: &str) -> bool {
        self.quotas
            .get(tenant_id)
            .map(|q| q.current_api_calls < q.max_api_calls_per_hour)
            .unwrap_or(false)
    }

    /// Increment API call counter; returns `true` if still under limit after increment.
    pub fn increment_api_calls(&mut self, tenant_id: &str) -> bool {
        if let Some(q) = self.quotas.get_mut(tenant_id) {
            q.current_api_calls += 1;
            q.current_api_calls <= q.max_api_calls_per_hour
        } else {
            false
        }
    }

    pub fn reset_api_calls(&mut self, tenant_id: &str) {
        if let Some(q) = self.quotas.get_mut(tenant_id) {
            q.current_api_calls = 0;
        }
    }

    pub fn update_storage(&mut self, tenant_id: &str, bytes: u64) {
        if let Some(q) = self.quotas.get_mut(tenant_id) {
            q.current_storage_bytes = bytes;
        }
    }

    /// Increment connection count; returns `true` if still under limit after increment.
    pub fn increment_connections(&mut self, tenant_id: &str) -> bool {
        if let Some(q) = self.quotas.get_mut(tenant_id) {
            q.current_connections += 1;
            q.current_connections <= q.max_concurrent_connections
        } else {
            false
        }
    }

    pub fn decrement_connections(&mut self, tenant_id: &str) {
        if let Some(q) = self.quotas.get_mut(tenant_id) {
            if q.current_connections > 0 {
                q.current_connections -= 1;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tenant Resolution
// ---------------------------------------------------------------------------

/// Resolves a tenant id from incoming request headers or host names.
#[derive(Debug, Clone)]
pub struct TenantResolver {
    host_map: HashMap<String, String>,
    header_name: String,
}

impl TenantResolver {
    pub fn new(header_name: &str) -> Self {
        Self {
            host_map: HashMap::new(),
            header_name: header_name.to_string(),
        }
    }

    pub fn add_host_mapping(&mut self, host: &str, tenant_id: &str) {
        self.host_map
            .insert(host.to_string(), tenant_id.to_string());
    }

    /// Resolve directly from a header value (the value *is* the tenant id).
    pub fn resolve_from_header(&self, header_value: &str) -> Option<String> {
        if header_value.is_empty() {
            None
        } else {
            Some(header_value.to_string())
        }
    }

    /// Resolve via host → tenant_id mapping.
    pub fn resolve_from_host(&self, host: &str) -> Option<String> {
        self.host_map.get(host).cloned()
    }

    /// Try header first, then host. Returns `Err` if neither resolves.
    pub fn resolve(&self, host: Option<&str>, header: Option<&str>) -> Result<String, String> {
        if let Some(h) = header {
            if let Some(tid) = self.resolve_from_header(h) {
                return Ok(tid);
            }
        }
        if let Some(h) = host {
            if let Some(tid) = self.resolve_from_host(h) {
                return Ok(tid);
            }
        }
        Err("Could not resolve tenant from header or host".to_string())
    }

    /// Returns the configured header name.
    pub fn header_name(&self) -> &str {
        &self.header_name
    }
}

// ---------------------------------------------------------------------------
// Audit Log
// ---------------------------------------------------------------------------

/// A single audit log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantAuditEntry {
    pub id: u64,
    pub tenant_id: String,
    pub action: String,
    pub actor: String,
    pub details: Option<String>,
    pub timestamp: String,
}

/// Append-only in-memory audit log for tenant operations.
#[derive(Debug, Clone, Default)]
pub struct TenantAuditLog {
    entries: Vec<TenantAuditEntry>,
    next_id: u64,
}

impl TenantAuditLog {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_id: 1,
        }
    }

    /// Record an audit entry. Returns the assigned entry id.
    pub fn record(
        &mut self,
        tenant_id: &str,
        action: &str,
        actor: &str,
        details: Option<&str>,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.entries.push(TenantAuditEntry {
            id,
            tenant_id: tenant_id.to_string(),
            action: action.to_string(),
            actor: actor.to_string(),
            details: details.map(|d| d.to_string()),
            timestamp: "2026-01-01T00:00:00Z".to_string(), // placeholder ISO 8601
        });
        id
    }

    /// Most-recent-first entries for a given tenant, capped at `limit`.
    pub fn get_entries(&self, tenant_id: &str, limit: usize) -> Vec<&TenantAuditEntry> {
        self.entries
            .iter()
            .rev()
            .filter(|e| e.tenant_id == tenant_id)
            .take(limit)
            .collect()
    }

    /// Most-recent-first entries across all tenants, capped at `limit`.
    pub fn get_all_entries(&self, limit: usize) -> Vec<&TenantAuditEntry> {
        self.entries.iter().rev().take(limit).collect()
    }

    pub fn count(&self, tenant_id: &str) -> usize {
        self.entries
            .iter()
            .filter(|e| e.tenant_id == tenant_id)
            .count()
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- helpers --

    fn sample_settings() -> TenantSettings {
        TenantSettings {
            custom_domain: None,
            branding_color: Some("#FF5500".into()),
            max_storage_mb: 1024,
            features_enabled: vec!["fleet_dispatch".into()],
        }
    }

    fn sample_tenant(id: &str, slug: &str) -> Tenant {
        Tenant {
            id: id.into(),
            name: format!("Tenant {id}"),
            slug: slug.into(),
            plan: TenantPlan::Free,
            status: TenantStatus::Active,
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
            settings: sample_settings(),
            max_universes: 5,
            max_players_per_universe: 1000,
        }
    }

    fn sample_quota(tenant_id: &str) -> ResourceQuota {
        ResourceQuota {
            tenant_id: tenant_id.into(),
            max_api_calls_per_hour: 100,
            max_storage_bytes: 1_000_000,
            max_concurrent_connections: 10,
            current_api_calls: 0,
            current_storage_bytes: 0,
            current_connections: 0,
        }
    }

    // -----------------------------------------------------------------------
    // Original tests (kept as-is)
    // -----------------------------------------------------------------------

    #[test]
    fn guard_context_is_cloneable() {
        let tenant = TenantContext {
            tenant_id: "t1".into(),
            tenant_name: Some("Tenant One".into()),
            access_level: TenantAccessLevel::Admin,
        };
        let guard = TenantGuard::new(tenant);
        let clone = guard.clone();
        assert_eq!(clone.context().tenant_id, "t1");
    }

    // -----------------------------------------------------------------------
    // TenantRegistry
    // -----------------------------------------------------------------------

    #[test]
    fn registry_register_and_get() {
        let mut reg = TenantRegistry::new();
        reg.register(sample_tenant("t1", "alpha")).unwrap();
        assert!(reg.get("t1").is_some());
        assert_eq!(reg.get("t1").unwrap().slug, "alpha");
    }

    #[test]
    fn registry_rejects_duplicate_id() {
        let mut reg = TenantRegistry::new();
        reg.register(sample_tenant("t1", "alpha")).unwrap();
        let res = reg.register(sample_tenant("t1", "beta"));
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("id"));
    }

    #[test]
    fn registry_rejects_duplicate_slug() {
        let mut reg = TenantRegistry::new();
        reg.register(sample_tenant("t1", "alpha")).unwrap();
        let res = reg.register(sample_tenant("t2", "alpha"));
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("slug"));
    }

    #[test]
    fn registry_get_by_slug() {
        let mut reg = TenantRegistry::new();
        reg.register(sample_tenant("t1", "alpha")).unwrap();
        assert!(reg.get_by_slug("alpha").is_some());
        assert!(reg.get_by_slug("nope").is_none());
    }

    #[test]
    fn registry_list_and_count() {
        let mut reg = TenantRegistry::new();
        assert_eq!(reg.count(), 0);
        reg.register(sample_tenant("t1", "a")).unwrap();
        reg.register(sample_tenant("t2", "b")).unwrap();
        assert_eq!(reg.count(), 2);
        assert_eq!(reg.list().len(), 2);
    }

    #[test]
    fn registry_list_by_status() {
        let mut reg = TenantRegistry::new();
        reg.register(sample_tenant("t1", "a")).unwrap();
        let mut suspended = sample_tenant("t2", "b");
        suspended.status = TenantStatus::Suspended;
        reg.register(suspended).unwrap();

        assert_eq!(reg.list_by_status(&TenantStatus::Active).len(), 1);
        assert_eq!(reg.list_by_status(&TenantStatus::Suspended).len(), 1);
        assert_eq!(reg.list_by_status(&TenantStatus::Archived).len(), 0);
    }

    #[test]
    fn registry_update_status() {
        let mut reg = TenantRegistry::new();
        reg.register(sample_tenant("t1", "a")).unwrap();
        assert!(reg.update_status("t1", TenantStatus::Suspended));
        assert_eq!(reg.get("t1").unwrap().status, TenantStatus::Suspended);
        assert!(!reg.update_status("nope", TenantStatus::Active));
    }

    #[test]
    fn registry_update_plan() {
        let mut reg = TenantRegistry::new();
        reg.register(sample_tenant("t1", "a")).unwrap();
        assert!(reg.update_plan("t1", TenantPlan::Enterprise));
        assert_eq!(reg.get("t1").unwrap().plan, TenantPlan::Enterprise);
        assert!(!reg.update_plan("nope", TenantPlan::Basic));
    }

    #[test]
    fn registry_update_settings() {
        let mut reg = TenantRegistry::new();
        reg.register(sample_tenant("t1", "a")).unwrap();
        let new_settings = TenantSettings {
            custom_domain: Some("example.com".into()),
            branding_color: None,
            max_storage_mb: 2048,
            features_enabled: vec![],
        };
        assert!(reg.update_settings("t1", new_settings.clone()));
        assert_eq!(reg.get("t1").unwrap().settings, new_settings);
        assert!(!reg.update_settings("nope", sample_settings()));
    }

    #[test]
    fn registry_delete() {
        let mut reg = TenantRegistry::new();
        reg.register(sample_tenant("t1", "a")).unwrap();
        assert!(reg.delete("t1"));
        assert!(!reg.delete("t1"));
        assert_eq!(reg.count(), 0);
    }

    #[test]
    fn registry_get_nonexistent() {
        let reg = TenantRegistry::new();
        assert!(reg.get("nope").is_none());
    }

    // -----------------------------------------------------------------------
    // Tenant Isolation
    // -----------------------------------------------------------------------

    fn sample_isolation(tenant_id: &str) -> TenantIsolation {
        TenantIsolation {
            tenant_id: tenant_id.into(),
            isolation_level: IsolationLevel::Shared,
            database_schema: Some(format!("schema_{tenant_id}")),
            cache_prefix: format!("cache:{tenant_id}:"),
            event_namespace: format!("events.{tenant_id}"),
        }
    }

    #[test]
    fn isolation_register_and_get() {
        let mut reg = IsolationRegistry::new();
        reg.register(sample_isolation("t1")).unwrap();
        assert!(reg.get("t1").is_some());
        assert_eq!(
            reg.get("t1").unwrap().isolation_level,
            IsolationLevel::Shared
        );
    }

    #[test]
    fn isolation_rejects_duplicate() {
        let mut reg = IsolationRegistry::new();
        reg.register(sample_isolation("t1")).unwrap();
        assert!(reg.register(sample_isolation("t1")).is_err());
    }

    #[test]
    fn isolation_cache_prefix_registered() {
        let mut reg = IsolationRegistry::new();
        reg.register(sample_isolation("t1")).unwrap();
        assert_eq!(reg.get_cache_prefix("t1"), "cache:t1:");
    }

    #[test]
    fn isolation_cache_prefix_default() {
        let reg = IsolationRegistry::new();
        assert_eq!(reg.get_cache_prefix("t1"), "tenant:t1:");
    }

    #[test]
    fn isolation_event_namespace() {
        let mut reg = IsolationRegistry::new();
        reg.register(sample_isolation("t1")).unwrap();
        assert_eq!(reg.get_event_namespace("t1"), "events.t1");
    }

    #[test]
    fn isolation_event_namespace_default() {
        let reg = IsolationRegistry::new();
        assert_eq!(reg.get_event_namespace("unknown"), "");
    }

    // -----------------------------------------------------------------------
    // Resource Quotas
    // -----------------------------------------------------------------------

    #[test]
    fn quota_set_and_get() {
        let mut reg = QuotaRegistry::new();
        reg.set_quota(sample_quota("t1"));
        assert!(reg.get_quota("t1").is_some());
        assert!(reg.get_quota("nope").is_none());
    }

    #[test]
    fn quota_check_api_limit_under() {
        let mut reg = QuotaRegistry::new();
        reg.set_quota(sample_quota("t1"));
        assert!(reg.check_api_limit("t1"));
    }

    #[test]
    fn quota_check_api_limit_at_max() {
        let mut reg = QuotaRegistry::new();
        let mut q = sample_quota("t1");
        q.current_api_calls = 100;
        reg.set_quota(q);
        assert!(!reg.check_api_limit("t1"));
    }

    #[test]
    fn quota_check_api_limit_unknown_tenant() {
        let reg = QuotaRegistry::new();
        assert!(!reg.check_api_limit("nope"));
    }

    #[test]
    fn quota_increment_api_calls() {
        let mut reg = QuotaRegistry::new();
        reg.set_quota(sample_quota("t1"));
        assert!(reg.increment_api_calls("t1"));
        assert_eq!(reg.get_quota("t1").unwrap().current_api_calls, 1);
    }

    #[test]
    fn quota_increment_api_calls_exceeds() {
        let mut reg = QuotaRegistry::new();
        let mut q = sample_quota("t1");
        q.current_api_calls = 100;
        reg.set_quota(q);
        assert!(!reg.increment_api_calls("t1"));
    }

    #[test]
    fn quota_increment_api_calls_unknown() {
        let mut reg = QuotaRegistry::new();
        assert!(!reg.increment_api_calls("nope"));
    }

    #[test]
    fn quota_reset_api_calls() {
        let mut reg = QuotaRegistry::new();
        let mut q = sample_quota("t1");
        q.current_api_calls = 50;
        reg.set_quota(q);
        reg.reset_api_calls("t1");
        assert_eq!(reg.get_quota("t1").unwrap().current_api_calls, 0);
    }

    #[test]
    fn quota_update_storage() {
        let mut reg = QuotaRegistry::new();
        reg.set_quota(sample_quota("t1"));
        reg.update_storage("t1", 500_000);
        assert_eq!(reg.get_quota("t1").unwrap().current_storage_bytes, 500_000);
    }

    #[test]
    fn quota_increment_connections() {
        let mut reg = QuotaRegistry::new();
        reg.set_quota(sample_quota("t1"));
        assert!(reg.increment_connections("t1"));
        assert_eq!(reg.get_quota("t1").unwrap().current_connections, 1);
    }

    #[test]
    fn quota_increment_connections_exceeds() {
        let mut reg = QuotaRegistry::new();
        let mut q = sample_quota("t1");
        q.current_connections = 10;
        reg.set_quota(q);
        assert!(!reg.increment_connections("t1"));
    }

    #[test]
    fn quota_decrement_connections() {
        let mut reg = QuotaRegistry::new();
        let mut q = sample_quota("t1");
        q.current_connections = 3;
        reg.set_quota(q);
        reg.decrement_connections("t1");
        assert_eq!(reg.get_quota("t1").unwrap().current_connections, 2);
    }

    #[test]
    fn quota_decrement_connections_floor() {
        let mut reg = QuotaRegistry::new();
        reg.set_quota(sample_quota("t1"));
        reg.decrement_connections("t1");
        assert_eq!(reg.get_quota("t1").unwrap().current_connections, 0);
    }

    // -----------------------------------------------------------------------
    // Tenant Resolver
    // -----------------------------------------------------------------------

    #[test]
    fn resolver_new_header_name() {
        let r = TenantResolver::new("X-Tenant-Id");
        assert_eq!(r.header_name(), "X-Tenant-Id");
    }

    #[test]
    fn resolver_from_header() {
        let r = TenantResolver::new("X-Tenant-Id");
        assert_eq!(r.resolve_from_header("t1"), Some("t1".into()));
    }

    #[test]
    fn resolver_from_header_empty() {
        let r = TenantResolver::new("X-Tenant-Id");
        assert_eq!(r.resolve_from_header(""), None);
    }

    #[test]
    fn resolver_from_host() {
        let mut r = TenantResolver::new("X-Tenant-Id");
        r.add_host_mapping("alpha.example.com", "t1");
        assert_eq!(r.resolve_from_host("alpha.example.com"), Some("t1".into()));
        assert_eq!(r.resolve_from_host("unknown.com"), None);
    }

    #[test]
    fn resolver_resolve_header_first() {
        let mut r = TenantResolver::new("X-Tenant-Id");
        r.add_host_mapping("alpha.example.com", "host-tenant");
        let result = r.resolve(Some("alpha.example.com"), Some("header-tenant"));
        assert_eq!(result.unwrap(), "header-tenant");
    }

    #[test]
    fn resolver_resolve_falls_back_to_host() {
        let mut r = TenantResolver::new("X-Tenant-Id");
        r.add_host_mapping("alpha.example.com", "host-tenant");
        let result = r.resolve(Some("alpha.example.com"), None);
        assert_eq!(result.unwrap(), "host-tenant");
    }

    #[test]
    fn resolver_resolve_neither() {
        let r = TenantResolver::new("X-Tenant-Id");
        assert!(r.resolve(None, None).is_err());
    }

    #[test]
    fn resolver_resolve_empty_header_falls_back() {
        let mut r = TenantResolver::new("X-Tenant-Id");
        r.add_host_mapping("alpha.example.com", "host-tenant");
        let result = r.resolve(Some("alpha.example.com"), Some(""));
        assert_eq!(result.unwrap(), "host-tenant");
    }

    #[test]
    fn resolver_resolve_unknown_host_no_header() {
        let r = TenantResolver::new("X-Tenant-Id");
        assert!(r.resolve(Some("unknown.com"), None).is_err());
    }

    // -----------------------------------------------------------------------
    // Audit Log
    // -----------------------------------------------------------------------

    #[test]
    fn audit_record_returns_incrementing_ids() {
        let mut log = TenantAuditLog::new();
        let id1 = log.record("t1", "create", "admin", None);
        let id2 = log.record("t1", "update", "admin", Some("changed plan"));
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }

    #[test]
    fn audit_get_entries_by_tenant() {
        let mut log = TenantAuditLog::new();
        log.record("t1", "create", "admin", None);
        log.record("t2", "create", "admin", None);
        log.record("t1", "update", "admin", None);
        let entries = log.get_entries("t1", 10);
        assert_eq!(entries.len(), 2);
        // most-recent first
        assert_eq!(entries[0].action, "update");
        assert_eq!(entries[1].action, "create");
    }

    #[test]
    fn audit_get_entries_respects_limit() {
        let mut log = TenantAuditLog::new();
        for i in 0..10 {
            log.record("t1", &format!("action_{i}"), "admin", None);
        }
        assert_eq!(log.get_entries("t1", 3).len(), 3);
    }

    #[test]
    fn audit_get_all_entries() {
        let mut log = TenantAuditLog::new();
        log.record("t1", "a", "admin", None);
        log.record("t2", "b", "admin", None);
        let all = log.get_all_entries(10);
        assert_eq!(all.len(), 2);
        // most-recent first
        assert_eq!(all[0].tenant_id, "t2");
    }

    #[test]
    fn audit_get_all_entries_respects_limit() {
        let mut log = TenantAuditLog::new();
        for i in 0..20 {
            log.record("t1", &format!("a{i}"), "admin", None);
        }
        assert_eq!(log.get_all_entries(5).len(), 5);
    }

    #[test]
    fn audit_count() {
        let mut log = TenantAuditLog::new();
        log.record("t1", "a", "admin", None);
        log.record("t1", "b", "admin", None);
        log.record("t2", "c", "admin", None);
        assert_eq!(log.count("t1"), 2);
        assert_eq!(log.count("t2"), 1);
        assert_eq!(log.count("t3"), 0);
    }

    #[test]
    fn audit_details_stored() {
        let mut log = TenantAuditLog::new();
        log.record("t1", "update", "admin", Some("plan changed to premium"));
        let entries = log.get_entries("t1", 1);
        assert_eq!(
            entries[0].details.as_deref(),
            Some("plan changed to premium")
        );
    }

    #[test]
    fn audit_timestamp_is_populated() {
        let mut log = TenantAuditLog::new();
        log.record("t1", "create", "admin", None);
        let entries = log.get_entries("t1", 1);
        assert!(!entries[0].timestamp.is_empty());
    }

    // -----------------------------------------------------------------------
    // Serialization round-trips
    // -----------------------------------------------------------------------

    #[test]
    fn tenant_serde_roundtrip() {
        let t = sample_tenant("t1", "alpha");
        let json = serde_json::to_string(&t).unwrap();
        let t2: Tenant = serde_json::from_str(&json).unwrap();
        assert_eq!(t2.id, "t1");
        assert_eq!(t2.slug, "alpha");
    }

    #[test]
    fn tenant_settings_serde_roundtrip() {
        let s = sample_settings();
        let json = serde_json::to_string(&s).unwrap();
        let s2: TenantSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(s, s2);
    }

    #[test]
    fn isolation_serde_roundtrip() {
        let iso = sample_isolation("t1");
        let json = serde_json::to_string(&iso).unwrap();
        let iso2: TenantIsolation = serde_json::from_str(&json).unwrap();
        assert_eq!(iso2.tenant_id, "t1");
    }

    #[test]
    fn resource_quota_serde_roundtrip() {
        let q = sample_quota("t1");
        let json = serde_json::to_string(&q).unwrap();
        let q2: ResourceQuota = serde_json::from_str(&json).unwrap();
        assert_eq!(q2.tenant_id, "t1");
        assert_eq!(q2.max_api_calls_per_hour, 100);
    }

    #[test]
    fn audit_entry_serde_roundtrip() {
        let entry = TenantAuditEntry {
            id: 1,
            tenant_id: "t1".into(),
            action: "create".into(),
            actor: "admin".into(),
            details: Some("initial setup".into()),
            timestamp: "2026-01-01T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let entry2: TenantAuditEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry2.id, 1);
        assert_eq!(entry2.details.as_deref(), Some("initial setup"));
    }

    #[test]
    fn tenant_context_serde_roundtrip() {
        let ctx = TenantContext {
            tenant_id: "t1".into(),
            tenant_name: Some("Test".into()),
            access_level: TenantAccessLevel::Worker,
        };
        let json = serde_json::to_string(&ctx).unwrap();
        let ctx2: TenantContext = serde_json::from_str(&json).unwrap();
        assert_eq!(ctx2.tenant_id, "t1");
    }

    // -----------------------------------------------------------------------
    // Edge-case / integration-style tests
    // -----------------------------------------------------------------------

    #[test]
    fn quota_overwrite_replaces() {
        let mut reg = QuotaRegistry::new();
        reg.set_quota(sample_quota("t1"));
        let mut q2 = sample_quota("t1");
        q2.max_api_calls_per_hour = 999;
        reg.set_quota(q2);
        assert_eq!(reg.get_quota("t1").unwrap().max_api_calls_per_hour, 999);
    }

    #[test]
    fn registry_delete_then_re_register() {
        let mut reg = TenantRegistry::new();
        reg.register(sample_tenant("t1", "alpha")).unwrap();
        reg.delete("t1");
        reg.register(sample_tenant("t1", "alpha")).unwrap();
        assert_eq!(reg.count(), 1);
    }

    #[test]
    fn resolver_multiple_hosts_same_tenant() {
        let mut r = TenantResolver::new("X-Tenant-Id");
        r.add_host_mapping("a.example.com", "t1");
        r.add_host_mapping("b.example.com", "t1");
        assert_eq!(r.resolve_from_host("a.example.com"), Some("t1".into()));
        assert_eq!(r.resolve_from_host("b.example.com"), Some("t1".into()));
    }

    #[test]
    fn isolation_dedicated_level() {
        let mut reg = IsolationRegistry::new();
        let mut iso = sample_isolation("t1");
        iso.isolation_level = IsolationLevel::Dedicated;
        reg.register(iso).unwrap();
        assert_eq!(
            reg.get("t1").unwrap().isolation_level,
            IsolationLevel::Dedicated
        );
    }
}
