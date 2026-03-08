//! Core building blocks for the platform-proto crate.
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::fmt;

/// Returns the crate name for a basic compile-time sanity check.
pub const fn crate_name() -> &'static str {
    "platform-proto"
}

/// Health snapshot reported by a running service instance.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServiceHealth {
    pub service: String,
    pub status: String,
    pub uptime_secs: u64,
    pub version: String,
}

/// A bundle of the four core resources in the game economy.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct ResourceBundle {
    pub metal: i64,
    pub crystal: i64,
    pub deuterium: i64,
    pub energy: i64,
}

impl ResourceBundle {
    /// Sum of all four resource amounts.
    pub fn total(&self) -> i64 {
        self.metal + self.crystal + self.deuterium + self.energy
    }
}

/// Galaxy / system / position triplet that uniquely locates a planet.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Coordinates {
    pub galaxy: i32,
    pub system: i32,
    pub position: i32,
}

impl fmt::Display for Coordinates {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.galaxy, self.system, self.position)
    }
}

/// Lightweight identity envelope for a player.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlayerIdentity {
    pub id: i64,
    pub username: String,
    pub alliance_id: Option<i64>,
}

/// Generic success / failure envelope returned by service actions.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ActionResult {
    pub success: bool,
    pub message: String,
    pub code: u32,
}

impl ActionResult {
    /// Build a successful result with the given message.
    pub fn ok(msg: impl Into<String>) -> Self {
        Self {
            success: true,
            message: msg.into(),
            code: 0,
        }
    }

    /// Build a failure result with the given message and error code.
    pub fn err(msg: impl Into<String>, code: u32) -> Self {
        Self {
            success: false,
            message: msg.into(),
            code,
        }
    }
}

/// A unit of work dispatched to a background worker.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WorkerTask {
    pub task_id: String,
    pub task_type: String,
    pub tenant_id: String,
    pub payload: String,
    pub created_at_unix: i64,
}

/// Outcome reported by a worker after processing a [`WorkerTask`].
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WorkerTaskResult {
    pub task_id: String,
    pub success: bool,
    pub output: String,
    pub duration_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_returns_expected() {
        assert_eq!(crate_name(), "platform-proto");
    }

    #[test]
    fn service_health_roundtrip() {
        let h = ServiceHealth {
            service: "auth".into(),
            status: "healthy".into(),
            uptime_secs: 3600,
            version: "0.1.0".into(),
        };
        let json = serde_json::to_string(&h).unwrap();
        let h2: ServiceHealth = serde_json::from_str(&json).unwrap();
        assert_eq!(h2.service, "auth");
        assert_eq!(h2.uptime_secs, 3600);
    }

    #[test]
    fn resource_bundle_default_is_zero() {
        let r = ResourceBundle::default();
        assert_eq!(r.metal, 0);
        assert_eq!(r.crystal, 0);
        assert_eq!(r.deuterium, 0);
        assert_eq!(r.energy, 0);
        assert_eq!(r.total(), 0);
    }

    #[test]
    fn resource_bundle_total() {
        let r = ResourceBundle {
            metal: 100,
            crystal: 200,
            deuterium: 50,
            energy: -10,
        };
        assert_eq!(r.total(), 340);
    }

    #[test]
    fn resource_bundle_equality() {
        let a = ResourceBundle { metal: 1, crystal: 2, deuterium: 3, energy: 4 };
        let b = ResourceBundle { metal: 1, crystal: 2, deuterium: 3, energy: 4 };
        assert_eq!(a, b);
    }

    #[test]
    fn coordinates_display() {
        let c = Coordinates { galaxy: 1, system: 42, position: 7 };
        assert_eq!(c.to_string(), "1:42:7");
    }

    #[test]
    fn coordinates_eq_and_hash() {
        use std::collections::HashSet;
        let a = Coordinates { galaxy: 1, system: 2, position: 3 };
        let b = Coordinates { galaxy: 1, system: 2, position: 3 };
        assert_eq!(a, b);

        let mut set = HashSet::new();
        set.insert(a);
        assert!(set.contains(&b));
    }

    #[test]
    fn coordinates_roundtrip() {
        let c = Coordinates { galaxy: 5, system: 100, position: 12 };
        let json = serde_json::to_string(&c).unwrap();
        let c2: Coordinates = serde_json::from_str(&json).unwrap();
        assert_eq!(c, c2);
    }

    #[test]
    fn player_identity_optional_alliance() {
        let p = PlayerIdentity { id: 42, username: "hero".into(), alliance_id: None };
        let json = serde_json::to_string(&p).unwrap();
        let p2: PlayerIdentity = serde_json::from_str(&json).unwrap();
        assert_eq!(p2.id, 42);
        assert!(p2.alliance_id.is_none());

        let p3 = PlayerIdentity { id: 7, username: "ally".into(), alliance_id: Some(99) };
        let json3 = serde_json::to_string(&p3).unwrap();
        let p4: PlayerIdentity = serde_json::from_str(&json3).unwrap();
        assert_eq!(p4.alliance_id, Some(99));
    }

    #[test]
    fn action_result_ok() {
        let r = ActionResult::ok("done");
        assert!(r.success);
        assert_eq!(r.message, "done");
        assert_eq!(r.code, 0);
    }

    #[test]
    fn action_result_err() {
        let r = ActionResult::err("bad input", 400);
        assert!(!r.success);
        assert_eq!(r.message, "bad input");
        assert_eq!(r.code, 400);
    }

    #[test]
    fn action_result_roundtrip() {
        let r = ActionResult::err("not found", 404);
        let json = serde_json::to_string(&r).unwrap();
        let r2: ActionResult = serde_json::from_str(&json).unwrap();
        assert!(!r2.success);
        assert_eq!(r2.code, 404);
    }

    #[test]
    fn worker_task_roundtrip() {
        let t = WorkerTask {
            task_id: "t-1".into(),
            task_type: "build".into(),
            tenant_id: "tenant-a".into(),
            payload: r#"{"level":5}"#.into(),
            created_at_unix: 1_700_000_000,
        };
        let json = serde_json::to_string(&t).unwrap();
        let t2: WorkerTask = serde_json::from_str(&json).unwrap();
        assert_eq!(t2.task_id, "t-1");
        assert_eq!(t2.created_at_unix, 1_700_000_000);
    }

    #[test]
    fn worker_task_result_roundtrip() {
        let r = WorkerTaskResult {
            task_id: "t-1".into(),
            success: true,
            output: "ok".into(),
            duration_ms: 250,
        };
        let json = serde_json::to_string(&r).unwrap();
        let r2: WorkerTaskResult = serde_json::from_str(&json).unwrap();
        assert!(r2.success);
        assert_eq!(r2.duration_ms, 250);
    }
}
