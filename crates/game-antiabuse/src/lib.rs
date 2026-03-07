#![forbid(unsafe_code)]

use serde::Serialize;
use std::collections::HashMap;

pub fn crate_name() -> &'static str {
    "game-antiabuse"
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub enum ViolationType {
    MultiAccounting,
    BotUsage,
    ExploitAbuse,
    RateLimitExceeded,
    ChatSpam,
    PushAbuse,
}

#[derive(Clone, Debug, Serialize)]
pub struct Violation {
    pub id: i64,
    pub user_id: i64,
    pub violation_type: ViolationType,
    pub description: String,
    pub detected_at_unix: i64,
    pub resolved: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Restriction {
    pub user_id: i64,
    pub reason: String,
    pub restriction_type: String,
    pub created_at_unix: i64,
    pub expires_at_unix: i64,
}

// ---------------------------------------------------------------------------
// RateLimiter
// ---------------------------------------------------------------------------

pub struct RateLimiter {
    max_actions: usize,
    window_ms: u128,
    actions: HashMap<i64, Vec<u128>>,
}

impl RateLimiter {
    pub fn new(max_actions: usize, window_ms: u128) -> Self {
        Self {
            max_actions,
            window_ms,
            actions: HashMap::new(),
        }
    }

    /// Returns `true` if the action is allowed, `false` if rate-limited.
    pub fn check(&mut self, user_id: i64, now_ms: u128) -> bool {
        let timestamps = self.actions.entry(user_id).or_default();
        let cutoff = now_ms.saturating_sub(self.window_ms);
        timestamps.retain(|&ts| ts > cutoff);

        if timestamps.len() >= self.max_actions {
            return false;
        }
        timestamps.push(now_ms);
        true
    }
}

// ---------------------------------------------------------------------------
// AbuseStore
// ---------------------------------------------------------------------------

pub struct AbuseStore {
    violations: Vec<Violation>,
    restrictions: Vec<Restriction>,
    next_violation_id: i64,
}

impl AbuseStore {
    pub fn new() -> Self {
        Self {
            violations: Vec::new(),
            restrictions: Vec::new(),
            next_violation_id: 1,
        }
    }

    pub fn report_violation(
        &mut self,
        user_id: i64,
        violation_type: ViolationType,
        description: String,
    ) -> Violation {
        let violation = Violation {
            id: self.next_violation_id,
            user_id,
            violation_type,
            description,
            detected_at_unix: 0,
            resolved: false,
        };
        self.next_violation_id += 1;
        self.violations.push(violation.clone());
        violation
    }

    pub fn list_violations(&self, user_id: i64) -> Vec<Violation> {
        self.violations
            .iter()
            .filter(|v| v.user_id == user_id)
            .cloned()
            .collect()
    }

    pub fn resolve_violation(&mut self, violation_id: i64) -> bool {
        if let Some(v) = self.violations.iter_mut().find(|v| v.id == violation_id) {
            v.resolved = true;
            return true;
        }
        false
    }

    pub fn add_restriction(&mut self, restriction: Restriction) {
        self.restrictions.push(restriction);
    }

    pub fn list_active_restrictions(&self, user_id: i64, now_unix: i64) -> Vec<Restriction> {
        self.restrictions
            .iter()
            .filter(|r| r.user_id == user_id && r.expires_at_unix > now_unix)
            .cloned()
            .collect()
    }

    pub fn cleanup_expired_restrictions(&mut self, now_unix: i64) -> usize {
        let before = self.restrictions.len();
        self.restrictions
            .retain(|r| r.expires_at_unix > now_unix);
        before.saturating_sub(self.restrictions.len())
    }

    pub fn is_restricted(&self, user_id: i64, now_unix: i64) -> bool {
        self.restrictions
            .iter()
            .any(|r| r.user_id == user_id && r.expires_at_unix > now_unix)
    }
}

impl Default for AbuseStore {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_returns_expected() {
        assert_eq!(crate_name(), "game-antiabuse");
    }

    #[test]
    fn report_and_list_violations() {
        let mut store = AbuseStore::new();
        store.report_violation(1, ViolationType::BotUsage, "automated clicks".into());
        store.report_violation(1, ViolationType::ChatSpam, "flooding chat".into());
        store.report_violation(2, ViolationType::ExploitAbuse, "dupe glitch".into());

        let v1 = store.list_violations(1);
        assert_eq!(v1.len(), 2);
        assert_eq!(v1[0].violation_type, ViolationType::BotUsage);

        let v2 = store.list_violations(2);
        assert_eq!(v2.len(), 1);
    }

    #[test]
    fn resolve_violation_marks_resolved() {
        let mut store = AbuseStore::new();
        let v = store.report_violation(1, ViolationType::PushAbuse, "push spam".into());
        assert!(!v.resolved);

        assert!(store.resolve_violation(v.id));
        let listed = store.list_violations(1);
        assert!(listed[0].resolved);
    }

    #[test]
    fn resolve_nonexistent_violation_returns_false() {
        let mut store = AbuseStore::new();
        assert!(!store.resolve_violation(999));
    }

    #[test]
    fn add_and_list_active_restrictions() {
        let mut store = AbuseStore::new();
        store.add_restriction(Restriction {
            user_id: 1,
            reason: "bot".into(),
            restriction_type: "ban".into(),
            created_at_unix: 1000,
            expires_at_unix: 2000,
        });
        store.add_restriction(Restriction {
            user_id: 1,
            reason: "spam".into(),
            restriction_type: "mute".into(),
            created_at_unix: 1000,
            expires_at_unix: 1500,
        });

        let active = store.list_active_restrictions(1, 1600);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].reason, "bot");
    }

    #[test]
    fn cleanup_expired_restrictions_removes_old() {
        let mut store = AbuseStore::new();
        store.add_restriction(Restriction {
            user_id: 1,
            reason: "old".into(),
            restriction_type: "ban".into(),
            created_at_unix: 100,
            expires_at_unix: 200,
        });
        store.add_restriction(Restriction {
            user_id: 2,
            reason: "still active".into(),
            restriction_type: "mute".into(),
            created_at_unix: 100,
            expires_at_unix: 5000,
        });

        let removed = store.cleanup_expired_restrictions(300);
        assert_eq!(removed, 1);
        assert!(!store.is_restricted(1, 300));
        assert!(store.is_restricted(2, 300));
    }

    #[test]
    fn is_restricted_returns_false_when_no_restrictions() {
        let store = AbuseStore::new();
        assert!(!store.is_restricted(42, 1000));
    }

    #[test]
    fn rate_limiter_allows_within_limit() {
        let mut rl = RateLimiter::new(3, 1000);
        assert!(rl.check(1, 100));
        assert!(rl.check(1, 200));
        assert!(rl.check(1, 300));
        // Fourth action within window → blocked
        assert!(!rl.check(1, 400));
    }

    #[test]
    fn rate_limiter_resets_after_window() {
        let mut rl = RateLimiter::new(2, 1000);
        assert!(rl.check(1, 100));
        assert!(rl.check(1, 200));
        assert!(!rl.check(1, 300));
        // After the window elapses, actions are allowed again
        assert!(rl.check(1, 1200));
    }

    #[test]
    fn rate_limiter_tracks_users_independently() {
        let mut rl = RateLimiter::new(1, 1000);
        assert!(rl.check(1, 100));
        assert!(!rl.check(1, 200));
        // Different user is unaffected
        assert!(rl.check(2, 200));
    }

    #[test]
    fn violation_ids_are_unique() {
        let mut store = AbuseStore::new();
        let v1 = store.report_violation(1, ViolationType::MultiAccounting, "alt".into());
        let v2 = store.report_violation(1, ViolationType::RateLimitExceeded, "flood".into());
        assert_ne!(v1.id, v2.id);
    }
}
