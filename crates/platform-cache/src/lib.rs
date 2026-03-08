//! Caching abstraction layer for the Universus platform.
//!
//! Provides an in-memory cache with TTL expiration, LRU/FIFO eviction,
//! typed wrappers via serde, two-level caching, and game-specific key builders.

#![forbid(unsafe_code)]

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// EvictionPolicy
// ---------------------------------------------------------------------------

/// Strategy used when the cache exceeds `max_entries`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvictionPolicy {
    /// Least-recently-used entry is evicted first.
    Lru,
    /// First-in, first-out — oldest inserted entry is evicted.
    Fifo,
    /// Entries closest to expiration are evicted first.
    Ttl,
}

impl fmt::Display for EvictionPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvictionPolicy::Lru => write!(f, "LRU"),
            EvictionPolicy::Fifo => write!(f, "FIFO"),
            EvictionPolicy::Ttl => write!(f, "TTL"),
        }
    }
}

impl FromStr for EvictionPolicy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_uppercase().as_str() {
            "LRU" => Ok(EvictionPolicy::Lru),
            "FIFO" => Ok(EvictionPolicy::Fifo),
            "TTL" => Ok(EvictionPolicy::Ttl),
            other => Err(format!("unknown eviction policy: {other}")),
        }
    }
}

// ---------------------------------------------------------------------------
// CacheConfig
// ---------------------------------------------------------------------------

/// Configuration for a cache instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub default_ttl_seconds: i64,
    pub max_entries: usize,
    pub eviction_policy: EvictionPolicy,
    pub prefix: String,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            default_ttl_seconds: 300,
            max_entries: 10_000,
            eviction_policy: EvictionPolicy::Lru,
            prefix: String::new(),
        }
    }
}

impl CacheConfig {
    /// Build a `CacheConfig` from environment variables.
    ///
    /// | Variable | Default |
    /// |---|---|
    /// | `CACHE_DEFAULT_TTL` | `300` |
    /// | `CACHE_MAX_ENTRIES` | `10000` |
    /// | `CACHE_EVICTION_POLICY` | `LRU` |
    /// | `CACHE_PREFIX` | `""` |
    pub fn from_env() -> Self {
        let default_ttl_seconds = std::env::var("CACHE_DEFAULT_TTL")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(300);

        let max_entries = std::env::var("CACHE_MAX_ENTRIES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10_000);

        let eviction_policy = std::env::var("CACHE_EVICTION_POLICY")
            .ok()
            .and_then(|v| EvictionPolicy::from_str(&v).ok())
            .unwrap_or(EvictionPolicy::Lru);

        let prefix = std::env::var("CACHE_PREFIX").unwrap_or_default();

        Self {
            default_ttl_seconds,
            max_entries,
            eviction_policy,
            prefix,
        }
    }
}

// ---------------------------------------------------------------------------
// CacheStats
// ---------------------------------------------------------------------------

/// Runtime statistics for an `InMemoryCache`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub expired: u64,
    pub current_size: usize,
    pub max_size: usize,
}

// ---------------------------------------------------------------------------
// CacheEntry (internal)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct CacheEntry {
    value: String,
    inserted_at: i64,
    ttl_seconds: Option<i64>,
    last_accessed: i64,
    /// Monotonic counter for insertion order (FIFO).
    insert_seq: u64,
    /// Monotonic counter for access order (LRU).
    access_seq: u64,
}

impl CacheEntry {
    fn is_expired(&self, now: i64) -> bool {
        match self.ttl_seconds {
            Some(ttl) => now >= self.inserted_at + ttl,
            None => false,
        }
    }

    fn remaining_ttl(&self, now: i64) -> Option<i64> {
        self.ttl_seconds.map(|ttl| {
            let remaining = (self.inserted_at + ttl) - now;
            if remaining < 0 {
                0
            } else {
                remaining
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Cache trait
// ---------------------------------------------------------------------------

/// Abstraction over any key-value cache backend.
pub trait Cache: Send + Sync {
    fn get(&self, key: &str) -> Option<String>;
    fn set(&self, key: &str, value: &str, ttl_seconds: Option<i64>);
    fn delete(&self, key: &str) -> bool;
    fn exists(&self, key: &str) -> bool;
    fn ttl(&self, key: &str) -> Option<i64>;
    fn keys(&self, pattern: &str) -> Vec<String>;
    fn flush(&self);
    fn size(&self) -> usize;
}

// ---------------------------------------------------------------------------
// Glob matching
// ---------------------------------------------------------------------------

/// Simple glob-style pattern matching.
///
/// `*` matches zero or more characters and `?` matches exactly one character.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    glob_match_inner(&p, &t)
}

fn glob_match_inner(pattern: &[char], text: &[char]) -> bool {
    let mut pi = 0;
    let mut ti = 0;
    let mut star_pi = usize::MAX; // position in pattern after last '*'
    let mut star_ti = 0; // position in text when last '*' was hit

    while ti < text.len() {
        if pi < pattern.len() && (pattern[pi] == '?' || pattern[pi] == text[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < pattern.len() && pattern[pi] == '*' {
            star_pi = pi + 1;
            star_ti = ti;
            pi += 1;
        } else if star_pi != usize::MAX {
            pi = star_pi;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }

    // consume trailing stars
    while pi < pattern.len() && pattern[pi] == '*' {
        pi += 1;
    }

    pi == pattern.len()
}

// ---------------------------------------------------------------------------
// InMemoryCache internals
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct InMemoryCacheInner {
    entries: HashMap<String, CacheEntry>,
    config: CacheConfig,
    hits: u64,
    misses: u64,
    evictions: u64,
    expired: u64,
    /// Monotonic counter incremented on every insert.
    next_insert_seq: u64,
    /// Monotonic counter incremented on every access.
    next_access_seq: u64,
}

impl InMemoryCacheInner {
    fn prefixed_key(&self, key: &str) -> String {
        if self.config.prefix.is_empty() {
            key.to_string()
        } else {
            format!("{}:{}", self.config.prefix, key)
        }
    }

    /// Evict one entry according to the configured policy. Returns `true` if an
    /// entry was actually removed.
    fn evict_one(&mut self) -> bool {
        if self.entries.is_empty() {
            return false;
        }

        let victim = match self.config.eviction_policy {
            EvictionPolicy::Lru => self
                .entries
                .iter()
                .min_by_key(|(_, e)| e.access_seq)
                .map(|(k, _)| k.clone()),
            EvictionPolicy::Fifo => self
                .entries
                .iter()
                .min_by_key(|(_, e)| e.insert_seq)
                .map(|(k, _)| k.clone()),
            EvictionPolicy::Ttl => self
                .entries
                .iter()
                .filter_map(|(k, e)| e.remaining_ttl(now_secs()).map(|r| (k.clone(), r)))
                .min_by_key(|(_, r)| *r)
                .map(|(k, _)| k),
        };

        if let Some(key) = victim {
            self.entries.remove(&key);
            self.evictions += 1;
            true
        } else {
            false
        }
    }

    fn cleanup_expired_inner(&mut self) -> usize {
        let now = now_secs();
        let expired_keys: Vec<String> = self
            .entries
            .iter()
            .filter(|(_, e)| e.is_expired(now))
            .map(|(k, _)| k.clone())
            .collect();
        let count = expired_keys.len();
        for k in &expired_keys {
            self.entries.remove(k);
        }
        self.expired += count as u64;
        count
    }
}

// ---------------------------------------------------------------------------
// InMemoryCache
// ---------------------------------------------------------------------------

/// Thread-safe in-memory cache with TTL expiration and configurable eviction.
#[derive(Debug, Clone)]
pub struct InMemoryCache {
    inner: Arc<Mutex<InMemoryCacheInner>>,
}

impl InMemoryCache {
    pub fn new(config: CacheConfig) -> Self {
        Self {
            inner: Arc::new(Mutex::new(InMemoryCacheInner {
                entries: HashMap::new(),
                config,
                hits: 0,
                misses: 0,
                evictions: 0,
                expired: 0,
                next_insert_seq: 0,
                next_access_seq: 0,
            })),
        }
    }

    /// Proactively remove all expired entries. Returns the number removed.
    pub fn cleanup_expired(&self) -> usize {
        let mut inner = self.inner.lock().unwrap();
        inner.cleanup_expired_inner()
    }

    /// Snapshot of runtime statistics.
    pub fn stats(&self) -> CacheStats {
        let inner = self.inner.lock().unwrap();
        CacheStats {
            hits: inner.hits,
            misses: inner.misses,
            evictions: inner.evictions,
            expired: inner.expired,
            current_size: inner.entries.len(),
            max_size: inner.config.max_entries,
        }
    }
}

impl Cache for InMemoryCache {
    fn get(&self, key: &str) -> Option<String> {
        let mut inner = self.inner.lock().unwrap();
        let full_key = inner.prefixed_key(key);
        let now = now_secs();

        // Check entry; handle lazy expiry.
        match inner.entries.get(&full_key) {
            None => {
                inner.misses += 1;
                None
            }
            Some(entry) if entry.is_expired(now) => {
                inner.entries.remove(&full_key);
                inner.expired += 1;
                inner.misses += 1;
                None
            }
            Some(entry) => {
                let value = entry.value.clone();
                let seq = inner.next_access_seq;
                inner.next_access_seq += 1;
                // Update last_accessed for LRU.
                if let Some(e) = inner.entries.get_mut(&full_key) {
                    e.last_accessed = now;
                    e.access_seq = seq;
                }
                inner.hits += 1;
                Some(value)
            }
        }
    }

    fn set(&self, key: &str, value: &str, ttl_seconds: Option<i64>) {
        let mut inner = self.inner.lock().unwrap();
        let full_key = inner.prefixed_key(key);
        let now = now_secs();

        let ttl = ttl_seconds.or(Some(inner.config.default_ttl_seconds));

        // If the key already exists we just overwrite (no eviction needed).
        if !inner.entries.contains_key(&full_key) && inner.entries.len() >= inner.config.max_entries
        {
            inner.evict_one();
        }

        let insert_seq = inner.next_insert_seq;
        inner.next_insert_seq += 1;
        let access_seq = inner.next_access_seq;
        inner.next_access_seq += 1;

        inner.entries.insert(
            full_key,
            CacheEntry {
                value: value.to_string(),
                inserted_at: now,
                ttl_seconds: ttl,
                last_accessed: now,
                insert_seq,
                access_seq,
            },
        );
    }

    fn delete(&self, key: &str) -> bool {
        let mut inner = self.inner.lock().unwrap();
        let full_key = inner.prefixed_key(key);
        inner.entries.remove(&full_key).is_some()
    }

    fn exists(&self, key: &str) -> bool {
        let inner = self.inner.lock().unwrap();
        let full_key = inner.prefixed_key(key);
        let now = now_secs();
        match inner.entries.get(&full_key) {
            Some(e) => !e.is_expired(now),
            None => false,
        }
    }

    fn ttl(&self, key: &str) -> Option<i64> {
        let inner = self.inner.lock().unwrap();
        let full_key = inner.prefixed_key(key);
        let now = now_secs();
        let entry = inner.entries.get(&full_key)?;
        if entry.is_expired(now) {
            return None;
        }
        entry.remaining_ttl(now)
    }

    fn keys(&self, pattern: &str) -> Vec<String> {
        let inner = self.inner.lock().unwrap();
        let now = now_secs();
        inner
            .entries
            .iter()
            .filter(|(_, e)| !e.is_expired(now))
            .map(|(k, _)| k.clone())
            .filter(|k| glob_match(pattern, k))
            .collect()
    }

    fn flush(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.entries.clear();
        inner.hits = 0;
        inner.misses = 0;
        inner.evictions = 0;
        inner.expired = 0;
    }

    fn size(&self) -> usize {
        let inner = self.inner.lock().unwrap();
        let now = now_secs();
        inner
            .entries
            .iter()
            .filter(|(_, e)| !e.is_expired(now))
            .count()
    }
}

// ---------------------------------------------------------------------------
// TypedCache
// ---------------------------------------------------------------------------

/// Convenience wrapper that serialises / deserialises values via serde_json.
#[derive(Debug, Clone)]
pub struct TypedCache<C: Cache> {
    pub inner: C,
}

impl<C: Cache> TypedCache<C> {
    pub fn new(cache: C) -> Self {
        Self { inner: cache }
    }

    pub fn get_typed<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let raw = self.inner.get(key)?;
        serde_json::from_str(&raw).ok()
    }

    pub fn set_typed<T: Serialize>(&self, key: &str, value: &T, ttl: Option<i64>) {
        if let Ok(json) = serde_json::to_string(value) {
            self.inner.set(key, &json, ttl);
        }
    }
}

// ---------------------------------------------------------------------------
// TwoLevelCache
// ---------------------------------------------------------------------------

/// Multi-level cache: a fast, small L1 backed by a larger L2.
///
/// `get` checks L1 first; on L1 miss / L2 hit the value is promoted to L1.
/// `set` writes to both levels.
#[derive(Debug, Clone)]
pub struct TwoLevelCache {
    pub l1: InMemoryCache,
    pub l2: InMemoryCache,
}

impl TwoLevelCache {
    pub fn new(l1: InMemoryCache, l2: InMemoryCache) -> Self {
        Self { l1, l2 }
    }
}

impl Cache for TwoLevelCache {
    fn get(&self, key: &str) -> Option<String> {
        // Try L1.
        if let Some(v) = self.l1.get(key) {
            return Some(v);
        }

        // Try L2 and promote on hit.
        if let Some(v) = self.l2.get(key) {
            let ttl = self.l2.ttl(key);
            self.l1.set(key, &v, ttl);
            return Some(v);
        }

        None
    }

    fn set(&self, key: &str, value: &str, ttl_seconds: Option<i64>) {
        self.l1.set(key, value, ttl_seconds);
        self.l2.set(key, value, ttl_seconds);
    }

    fn delete(&self, key: &str) -> bool {
        let a = self.l1.delete(key);
        let b = self.l2.delete(key);
        a || b
    }

    fn exists(&self, key: &str) -> bool {
        self.l1.exists(key) || self.l2.exists(key)
    }

    fn ttl(&self, key: &str) -> Option<i64> {
        self.l1.ttl(key).or_else(|| self.l2.ttl(key))
    }

    fn keys(&self, pattern: &str) -> Vec<String> {
        let mut all = self.l1.keys(pattern);
        for k in self.l2.keys(pattern) {
            if !all.contains(&k) {
                all.push(k);
            }
        }
        all
    }

    fn flush(&self) {
        self.l1.flush();
        self.l2.flush();
    }

    fn size(&self) -> usize {
        // The logical size is the L2 size (superset), but callers may want
        // the combined deduplicated count. We take the simpler approach of
        // returning L2 size since L1 is a hot subset of L2.
        self.l2.size()
    }
}

// ---------------------------------------------------------------------------
// Key builders
// ---------------------------------------------------------------------------

pub fn player_key(player_id: i64) -> String {
    format!("player:{player_id}")
}

pub fn planet_key(planet_id: i64) -> String {
    format!("planet:{planet_id}")
}

pub fn universe_key(universe_id: i64) -> String {
    format!("universe:{universe_id}")
}

pub fn leaderboard_key(category: &str) -> String {
    format!("leaderboard:{category}")
}

pub fn session_key(session_id: &str) -> String {
    format!("session:{session_id}")
}

pub fn rate_limit_key(player_id: i64, action: &str) -> String {
    format!("ratelimit:{player_id}:{action}")
}

/// Returns the crate name for a basic compile-time sanity check.
pub const fn crate_name() -> &'static str {
    "platform-cache"
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn default_cache() -> InMemoryCache {
        InMemoryCache::new(CacheConfig {
            default_ttl_seconds: 300,
            max_entries: 100,
            eviction_policy: EvictionPolicy::Lru,
            prefix: String::new(),
        })
    }

    // -- basic get / set / delete -------------------------------------------

    #[test]
    fn get_set_roundtrip() {
        let cache = default_cache();
        cache.set("k1", "hello", None);
        assert_eq!(cache.get("k1"), Some("hello".to_string()));
    }

    #[test]
    fn get_missing_key_returns_none() {
        let cache = default_cache();
        assert_eq!(cache.get("nonexistent"), None);
    }

    #[test]
    fn delete_existing_key() {
        let cache = default_cache();
        cache.set("k1", "v", None);
        assert!(cache.delete("k1"));
        assert_eq!(cache.get("k1"), None);
    }

    #[test]
    fn delete_missing_key_returns_false() {
        let cache = default_cache();
        assert!(!cache.delete("nope"));
    }

    #[test]
    fn exists_reflects_presence() {
        let cache = default_cache();
        assert!(!cache.exists("k"));
        cache.set("k", "v", None);
        assert!(cache.exists("k"));
    }

    // -- TTL ----------------------------------------------------------------

    #[test]
    fn ttl_expiration_lazy() {
        let cache = InMemoryCache::new(CacheConfig {
            default_ttl_seconds: 300,
            max_entries: 100,
            eviction_policy: EvictionPolicy::Lru,
            prefix: String::new(),
        });

        // Insert with a zero-second TTL — should expire immediately on next access.
        cache.set("ephemeral", "gone", Some(0));
        // After a set with ttl=0, inserted_at == now and expiry == inserted_at + 0 == now,
        // so `now >= inserted_at + ttl` is true immediately.
        assert_eq!(cache.get("ephemeral"), None);
    }

    #[test]
    fn ttl_returns_remaining_seconds() {
        let cache = default_cache();
        cache.set("k", "v", Some(600));
        let remaining = cache.ttl("k").unwrap();
        // Should be close to 600 (might lose a second due to timing).
        assert!(remaining >= 598 && remaining <= 600);
    }

    #[test]
    fn ttl_returns_none_for_missing() {
        let cache = default_cache();
        assert!(cache.ttl("nope").is_none());
    }

    // -- LRU eviction -------------------------------------------------------

    #[test]
    fn lru_eviction_when_max_entries_exceeded() {
        let cache = InMemoryCache::new(CacheConfig {
            default_ttl_seconds: 300,
            max_entries: 3,
            eviction_policy: EvictionPolicy::Lru,
            prefix: String::new(),
        });

        cache.set("a", "1", None);
        cache.set("b", "2", None);
        cache.set("c", "3", None);

        // Access "a" so it is recently used.
        let _ = cache.get("a");

        // Insert "d" — should evict the least recently used ("b").
        cache.set("d", "4", None);

        assert!(cache.get("a").is_some(), "a should survive (recently used)");
        assert!(cache.get("b").is_none(), "b should have been evicted");
        assert!(cache.get("c").is_some(), "c should survive");
        assert!(cache.get("d").is_some(), "d was just inserted");
    }

    #[test]
    fn fifo_eviction_removes_oldest_insert() {
        let cache = InMemoryCache::new(CacheConfig {
            default_ttl_seconds: 300,
            max_entries: 2,
            eviction_policy: EvictionPolicy::Fifo,
            prefix: String::new(),
        });

        cache.set("first", "1", None);
        cache.set("second", "2", None);

        // Even if we access "first", FIFO ignores access time.
        let _ = cache.get("first");

        cache.set("third", "3", None);
        assert!(
            cache.get("first").is_none(),
            "first should be evicted (FIFO)"
        );
        assert!(cache.get("second").is_some());
        assert!(cache.get("third").is_some());
    }

    // -- cleanup_expired ----------------------------------------------------

    #[test]
    fn cleanup_expired_removes_stale_entries() {
        let cache = default_cache();
        cache.set("alive", "yes", Some(9999));
        cache.set("dead", "no", Some(0));

        let removed = cache.cleanup_expired();
        assert_eq!(removed, 1);
        assert!(cache.get("alive").is_some());
        assert!(cache.get("dead").is_none());
    }

    // -- stats --------------------------------------------------------------

    #[test]
    fn stats_track_hits_and_misses() {
        let cache = default_cache();
        cache.set("k", "v", None);

        let _ = cache.get("k"); // hit
        let _ = cache.get("k"); // hit
        let _ = cache.get("miss"); // miss

        let stats = cache.stats();
        assert_eq!(stats.hits, 2);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.current_size, 1);
        assert_eq!(stats.max_size, 100);
    }

    #[test]
    fn stats_track_evictions() {
        let cache = InMemoryCache::new(CacheConfig {
            default_ttl_seconds: 300,
            max_entries: 1,
            eviction_policy: EvictionPolicy::Lru,
            prefix: String::new(),
        });

        cache.set("a", "1", None);
        cache.set("b", "2", None); // evicts "a"

        let stats = cache.stats();
        assert_eq!(stats.evictions, 1);
    }

    // -- flush & size -------------------------------------------------------

    #[test]
    fn flush_clears_everything() {
        let cache = default_cache();
        cache.set("a", "1", None);
        cache.set("b", "2", None);
        assert_eq!(cache.size(), 2);

        cache.flush();
        assert_eq!(cache.size(), 0);
        assert_eq!(cache.get("a"), None);
    }

    // -- prefix -------------------------------------------------------------

    #[test]
    fn prefix_is_applied_to_keys() {
        let cache = InMemoryCache::new(CacheConfig {
            default_ttl_seconds: 300,
            max_entries: 100,
            eviction_policy: EvictionPolicy::Lru,
            prefix: "ns".to_string(),
        });

        cache.set("key", "val", None);

        // The stored key should be "ns:key".
        let keys = cache.keys("ns:*");
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0], "ns:key");

        // Access via the un-prefixed API key still works.
        assert_eq!(cache.get("key"), Some("val".to_string()));
    }

    // -- typed cache --------------------------------------------------------

    #[test]
    fn typed_cache_serde_roundtrip() {
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        struct Player {
            id: i64,
            name: String,
        }

        let cache = TypedCache::new(default_cache());
        let player = Player {
            id: 42,
            name: "Cosmo".to_string(),
        };

        cache.set_typed("player:42", &player, None);
        let loaded: Option<Player> = cache.get_typed("player:42");
        assert_eq!(loaded, Some(player));
    }

    #[test]
    fn typed_cache_returns_none_on_bad_json() {
        let cache = TypedCache::new(default_cache());
        cache.inner.set("bad", "not-json{{{", None);

        let result: Option<serde_json::Value> = cache.get_typed("bad");
        assert!(result.is_none());
    }

    // -- two-level cache ----------------------------------------------------

    #[test]
    fn two_level_cache_promotes_on_l2_hit() {
        let l1 = InMemoryCache::new(CacheConfig {
            default_ttl_seconds: 300,
            max_entries: 10,
            eviction_policy: EvictionPolicy::Lru,
            prefix: String::new(),
        });
        let l2 = InMemoryCache::new(CacheConfig {
            default_ttl_seconds: 300,
            max_entries: 100,
            eviction_policy: EvictionPolicy::Lru,
            prefix: String::new(),
        });
        let two = TwoLevelCache::new(l1.clone(), l2.clone());

        // Write only to L2 directly.
        l2.set("deep", "value", None);
        assert!(l1.get("deep").is_none());

        // Read through TwoLevelCache — should promote to L1.
        assert_eq!(two.get("deep"), Some("value".to_string()));
        assert_eq!(l1.get("deep"), Some("value".to_string()));
    }

    #[test]
    fn two_level_cache_set_writes_both() {
        let l1 = InMemoryCache::new(CacheConfig {
            default_ttl_seconds: 300,
            max_entries: 10,
            eviction_policy: EvictionPolicy::Lru,
            prefix: String::new(),
        });
        let l2 = InMemoryCache::new(CacheConfig {
            default_ttl_seconds: 300,
            max_entries: 100,
            eviction_policy: EvictionPolicy::Lru,
            prefix: String::new(),
        });
        let two = TwoLevelCache::new(l1.clone(), l2.clone());

        two.set("key", "val", None);
        assert_eq!(l1.get("key"), Some("val".to_string()));
        assert_eq!(l2.get("key"), Some("val".to_string()));
    }

    // -- glob matching ------------------------------------------------------

    #[test]
    fn glob_match_star() {
        assert!(glob_match("player:*", "player:42"));
        assert!(glob_match("player:*", "player:"));
        assert!(!glob_match("player:*", "planet:1"));
    }

    #[test]
    fn glob_match_question_mark() {
        assert!(glob_match("key?", "keyA"));
        assert!(!glob_match("key?", "keyAB"));
        assert!(!glob_match("key?", "key"));
    }

    #[test]
    fn glob_match_complex() {
        assert!(glob_match("*:42:*", "player:42:stats"));
        assert!(glob_match("rate*:?", "ratelimit:x"));
        assert!(!glob_match("rate*:?", "ratelimit:xy"));
    }

    // -- key builders -------------------------------------------------------

    #[test]
    fn key_builders_format_correctly() {
        assert_eq!(player_key(1), "player:1");
        assert_eq!(planet_key(2), "planet:2");
        assert_eq!(universe_key(3), "universe:3");
        assert_eq!(leaderboard_key("points"), "leaderboard:points");
        assert_eq!(session_key("abc-123"), "session:abc-123");
        assert_eq!(rate_limit_key(7, "attack"), "ratelimit:7:attack");
    }

    // -- EvictionPolicy Display / FromStr -----------------------------------

    #[test]
    fn eviction_policy_display_fromstr_roundtrip() {
        for policy in [
            EvictionPolicy::Lru,
            EvictionPolicy::Fifo,
            EvictionPolicy::Ttl,
        ] {
            let s = policy.to_string();
            let parsed: EvictionPolicy = s.parse().unwrap();
            assert_eq!(parsed, policy);
        }
        assert!("UNKNOWN".parse::<EvictionPolicy>().is_err());
    }

    // -- CacheConfig --------------------------------------------------------

    #[test]
    fn cache_config_default_values() {
        let cfg = CacheConfig::default();
        assert_eq!(cfg.default_ttl_seconds, 300);
        assert_eq!(cfg.max_entries, 10_000);
        assert_eq!(cfg.eviction_policy, EvictionPolicy::Lru);
        assert!(cfg.prefix.is_empty());
    }

    // -- keys() with pattern ------------------------------------------------

    #[test]
    fn keys_returns_matching_non_expired() {
        let cache = default_cache();
        cache.set("player:1", "a", None);
        cache.set("player:2", "b", None);
        cache.set("planet:1", "c", None);
        cache.set("expired", "d", Some(0)); // instantly expired

        let mut player_keys = cache.keys("player:*");
        player_keys.sort();
        assert_eq!(player_keys, vec!["player:1", "player:2"]);

        // The expired entry should not appear.
        let all = cache.keys("*");
        assert!(!all.contains(&"expired".to_string()));
    }

    // -- overwrite existing key doesn't increase size -----------------------

    #[test]
    fn overwrite_does_not_grow_size() {
        let cache = default_cache();
        cache.set("k", "v1", None);
        cache.set("k", "v2", None);
        assert_eq!(cache.size(), 1);
        assert_eq!(cache.get("k"), Some("v2".to_string()));
    }
}
