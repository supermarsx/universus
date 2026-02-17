//! Tenant-aware context and routing helpers shared across the Rust services.
//! This crate surfaces a `TenantContext` guard that can be mounted into Axum routers
//! and passed through worker queues so requests/work loops can carry a tenant identifier.

use serde::{Deserialize, Serialize};
use std::sync::Arc;

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
