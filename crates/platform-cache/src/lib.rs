//! Core building blocks for the platform-cache crate.

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

struct CacheEntry<T> {
    value: T,
    expires_at_ms: Option<u128>,
}

impl<T> CacheEntry<T> {
    fn is_expired(&self, now: u128) -> bool {
        match self.expires_at_ms {
            Some(expires) => now >= expires,
            None => false,
        }
    }
}

pub struct Cache<T: Clone> {
    entries: HashMap<String, CacheEntry<T>>,
}

impl<T: Clone> Cache<T> {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<T> {
        let now = now_ms();
        if let Some(entry) = self.entries.get(key) {
            if entry.is_expired(now) {
                self.entries.remove(key);
                return None;
            }
            return Some(entry.value.clone());
        }
        None
    }

    pub fn set(&mut self, key: String, value: T, ttl_ms: Option<u128>) {
        let expires_at_ms = ttl_ms.map(|ttl| now_ms() + ttl);
        self.entries.insert(
            key,
            CacheEntry {
                value,
                expires_at_ms,
            },
        );
    }

    pub fn remove(&mut self, key: &str) -> bool {
        self.entries.remove(key).is_some()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn cleanup_expired(&mut self) -> usize {
        let now = now_ms();
        let before = self.entries.len();
        self.entries.retain(|_, entry| !entry.is_expired(now));
        before.saturating_sub(self.entries.len())
    }
}

impl<T: Clone> Default for Cache<T> {
    fn default() -> Self {
        Self::new()
    }
}

pub struct TenantCache<T: Clone> {
    tenants: HashMap<String, Cache<T>>,
}

impl<T: Clone> TenantCache<T> {
    pub fn new() -> Self {
        Self {
            tenants: HashMap::new(),
        }
    }

    pub fn get(&mut self, tenant: &str, key: &str) -> Option<T> {
        self.tenants.get_mut(tenant)?.get(key)
    }

    pub fn set(&mut self, tenant: &str, key: String, value: T, ttl_ms: Option<u128>) {
        self.tenants
            .entry(tenant.to_string())
            .or_default()
            .set(key, value, ttl_ms);
    }

    pub fn remove(&mut self, tenant: &str, key: &str) -> bool {
        match self.tenants.get_mut(tenant) {
            Some(cache) => cache.remove(key),
            None => false,
        }
    }

    pub fn clear_tenant(&mut self, tenant: &str) {
        self.tenants.remove(tenant);
    }

    pub fn cleanup_all_expired(&mut self) -> usize {
        self.tenants
            .values_mut()
            .map(|cache| cache.cleanup_expired())
            .sum()
    }
}

impl<T: Clone> Default for TenantCache<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns the crate name for a basic compile-time sanity check.
pub const fn crate_name() -> &'static str {
    "platform-cache"
}

#[cfg(test)]
mod tests {
    use super::{Cache, TenantCache};

    #[test]
    fn crate_name_returns_expected_value() {
        assert_eq!(super::crate_name(), "platform-cache");
    }

    // --- Cache tests ---

    #[test]
    fn new_cache_is_empty() {
        let cache: Cache<String> = Cache::new();
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn set_and_get_without_ttl() {
        let mut cache = Cache::new();
        cache.set("key1".to_string(), 42, None);
        assert_eq!(cache.get("key1"), Some(42));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn get_missing_key_returns_none() {
        let mut cache: Cache<i32> = Cache::new();
        assert_eq!(cache.get("missing"), None);
    }

    #[test]
    fn set_overwrites_existing_value() {
        let mut cache = Cache::new();
        cache.set("k".to_string(), "first".to_string(), None);
        cache.set("k".to_string(), "second".to_string(), None);
        assert_eq!(cache.get("k"), Some("second".to_string()));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn remove_existing_key_returns_true() {
        let mut cache = Cache::new();
        cache.set("a".to_string(), 1, None);
        assert!(cache.remove("a"));
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn remove_missing_key_returns_false() {
        let mut cache: Cache<i32> = Cache::new();
        assert!(!cache.remove("nope"));
    }

    #[test]
    fn clear_removes_all_entries() {
        let mut cache = Cache::new();
        cache.set("a".to_string(), 1, None);
        cache.set("b".to_string(), 2, None);
        cache.clear();
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn get_with_long_ttl_returns_value() {
        let mut cache = Cache::new();
        cache.set("k".to_string(), 99, Some(600_000));
        assert_eq!(cache.get("k"), Some(99));
    }

    #[test]
    fn get_expired_entry_returns_none_and_removes_it() {
        let mut cache = Cache::new();
        // TTL of 0 means it expires immediately
        cache.set("k".to_string(), 10, Some(0));
        std::thread::sleep(std::time::Duration::from_millis(1));
        assert_eq!(cache.get("k"), None);
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn cleanup_expired_removes_stale_entries() {
        let mut cache = Cache::new();
        cache.set("stale".to_string(), 1, Some(0));
        cache.set("fresh".to_string(), 2, None);
        std::thread::sleep(std::time::Duration::from_millis(1));
        let removed = cache.cleanup_expired();
        assert_eq!(removed, 1);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get("fresh"), Some(2));
    }

    #[test]
    fn cleanup_expired_on_empty_cache_returns_zero() {
        let mut cache: Cache<i32> = Cache::new();
        assert_eq!(cache.cleanup_expired(), 0);
    }

    #[test]
    fn default_creates_empty_cache() {
        let cache: Cache<i32> = Cache::default();
        assert_eq!(cache.len(), 0);
    }

    // --- TenantCache tests ---

    #[test]
    fn tenant_set_and_get() {
        let mut tc = TenantCache::new();
        tc.set("t1", "k1".to_string(), 100, None);
        assert_eq!(tc.get("t1", "k1"), Some(100));
    }

    #[test]
    fn tenant_get_wrong_tenant_returns_none() {
        let mut tc = TenantCache::new();
        tc.set("t1", "k".to_string(), 1, None);
        assert_eq!(tc.get("t2", "k"), None);
    }

    #[test]
    fn tenant_get_wrong_key_returns_none() {
        let mut tc = TenantCache::new();
        tc.set("t1", "k".to_string(), 1, None);
        assert_eq!(tc.get("t1", "other"), None);
    }

    #[test]
    fn tenant_remove_returns_true_for_existing() {
        let mut tc = TenantCache::new();
        tc.set("t1", "k".to_string(), 1, None);
        assert!(tc.remove("t1", "k"));
    }

    #[test]
    fn tenant_remove_returns_false_for_missing_tenant() {
        let mut tc: TenantCache<i32> = TenantCache::new();
        assert!(!tc.remove("ghost", "k"));
    }

    #[test]
    fn clear_tenant_removes_all_keys_for_tenant() {
        let mut tc = TenantCache::new();
        tc.set("t1", "a".to_string(), 1, None);
        tc.set("t1", "b".to_string(), 2, None);
        tc.set("t2", "a".to_string(), 3, None);
        tc.clear_tenant("t1");
        assert_eq!(tc.get("t1", "a"), None);
        assert_eq!(tc.get("t2", "a"), Some(3));
    }

    #[test]
    fn cleanup_all_expired_across_tenants() {
        let mut tc = TenantCache::new();
        tc.set("t1", "stale".to_string(), 1, Some(0));
        tc.set("t2", "stale".to_string(), 2, Some(0));
        tc.set("t2", "fresh".to_string(), 3, None);
        std::thread::sleep(std::time::Duration::from_millis(1));
        let removed = tc.cleanup_all_expired();
        assert_eq!(removed, 2);
        assert_eq!(tc.get("t2", "fresh"), Some(3));
    }

    #[test]
    fn tenant_default_creates_empty() {
        let mut tc: TenantCache<i32> = TenantCache::default();
        assert_eq!(tc.get("any", "any"), None);
        assert_eq!(tc.cleanup_all_expired(), 0);
    }
}
