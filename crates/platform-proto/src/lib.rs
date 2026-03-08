//! Protocol and message types for inter-service communication in Universus.
//!
//! This crate defines the canonical wire types used across API gateways,
//! event buses, worker queues, WebSocket connections, and health endpoints.

#![forbid(unsafe_code)]

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// API Protocol Types
// ---------------------------------------------------------------------------

/// Generic API request wrapper.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiRequest<T> {
    pub request_id: String,
    pub timestamp: i64,
    pub payload: T,
}

/// Generic API response wrapper.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub request_id: String,
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<ApiError>,
    pub timestamp: i64,
}

/// Structured API error returned inside [`ApiResponse`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl<T> ApiResponse<T> {
    /// Build a successful response carrying `data`.
    pub fn ok(request_id: impl Into<String>, data: T) -> Self {
        Self {
            request_id: request_id.into(),
            success: true,
            data: Some(data),
            error: None,
            timestamp: unix_timestamp(),
        }
    }

    /// Build an error response.
    pub fn err(
        request_id: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            success: false,
            data: None,
            error: Some(ApiError {
                code: code.into(),
                message: message.into(),
                details: None,
            }),
            timestamp: unix_timestamp(),
        }
    }
}

// ---------------------------------------------------------------------------
// Event Protocol Types
// ---------------------------------------------------------------------------

/// All game event variants exchanged through the event bus.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum GameEvent {
    PlayerRegistered {
        player_id: i64,
        username: String,
    },
    PlanetColonized {
        player_id: i64,
        planet_id: i64,
        coordinates: String,
    },
    FleetDispatched {
        fleet_id: i64,
        origin: String,
        destination: String,
        mission: String,
    },
    FleetArrived {
        fleet_id: i64,
        destination: String,
    },
    FleetReturned {
        fleet_id: i64,
        origin: String,
    },
    CombatResolved {
        battle_id: i64,
        attacker_id: i64,
        defender_id: i64,
        winner: String,
    },
    ResearchCompleted {
        player_id: i64,
        research: String,
        level: i32,
    },
    BuildingCompleted {
        planet_id: i64,
        building: String,
        level: i32,
    },
    ShipConstructed {
        planet_id: i64,
        ship_type: String,
        quantity: i32,
    },
    AllianceCreated {
        alliance_id: i64,
        name: String,
        founder_id: i64,
    },
    AllianceDisbanded {
        alliance_id: i64,
    },
    MessageSent {
        message_id: i64,
        sender_id: i64,
        recipient_id: i64,
    },
    TradeCompleted {
        trade_id: i64,
        buyer_id: i64,
        seller_id: i64,
    },
    MoonCreated {
        moon_id: i64,
        planet_id: i64,
    },
    MoonDestroyed {
        moon_id: i64,
    },
    UniverseCreated {
        universe_id: i64,
        name: String,
    },
    PlayerBanned {
        player_id: i64,
        reason: String,
    },
    PlayerUnbanned {
        player_id: i64,
    },
}

/// Envelope that carries a [`GameEvent`] together with routing metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameEventEnvelope {
    pub event_id: String,
    pub event_type: String,
    pub universe_id: Option<i64>,
    pub player_id: Option<i64>,
    pub timestamp: i64,
    pub payload: GameEvent,
    pub metadata: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// Worker Message Types
// ---------------------------------------------------------------------------

/// Categories of background tasks processed by workers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WorkerTaskType {
    ProcessFleet,
    ProcessCombat,
    UpdateResources,
    SendNotification,
    SendEmail,
    SendSms,
    CalculateLeaderboard,
    CleanupExpired,
    ProcessScheduledTask,
    RunMigration,
}

/// A task dispatched to a worker queue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerTask {
    pub task_id: String,
    pub task_type: WorkerTaskType,
    pub payload: serde_json::Value,
    pub priority: i32,
    pub created_at: i64,
    pub max_retries: i32,
    pub retry_count: i32,
}

/// Result reported back by a worker after processing a task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerResult {
    pub task_id: String,
    pub success: bool,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    pub duration_ms: i64,
}

// ---------------------------------------------------------------------------
// Realtime Protocol Types (WebSocket)
// ---------------------------------------------------------------------------

/// Messages pushed to clients over WebSocket connections.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RealtimeMessage {
    ResourceUpdate {
        planet_id: i64,
        resources: serde_json::Value,
    },
    FleetUpdate {
        fleet_id: i64,
        status: String,
    },
    CombatAlert {
        battle_id: i64,
        coordinates: String,
    },
    MessageNotification {
        message_id: i64,
        subject: String,
    },
    BuildingProgress {
        planet_id: i64,
        building: String,
        progress_percent: f64,
    },
    ResearchProgress {
        research: String,
        progress_percent: f64,
    },
    ServerAnnouncement {
        message: String,
    },
}

/// Envelope that wraps a [`RealtimeMessage`] with routing information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RealtimeEnvelope {
    pub message_type: String,
    pub target_user_id: Option<i64>,
    pub target_universe_id: Option<i64>,
    pub broadcast: bool,
    pub payload: RealtimeMessage,
    pub timestamp: i64,
}

// ---------------------------------------------------------------------------
// Health / Status Protocol
// ---------------------------------------------------------------------------

/// Aggregate health status of a service.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ServiceStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Individual health-check entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthCheck {
    pub name: String,
    pub status: ServiceStatus,
    pub message: Option<String>,
    pub latency_ms: Option<i64>,
}

/// Top-level service health report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServiceHealth {
    pub service_name: String,
    pub status: ServiceStatus,
    pub version: String,
    pub uptime_seconds: i64,
    pub checks: Vec<HealthCheck>,
}

// ---------------------------------------------------------------------------
// Pagination Protocol
// ---------------------------------------------------------------------------

/// Pagination parameters sent by clients.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageRequest {
    pub offset: i64,
    pub limit: i64,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}

/// Paginated response wrapper.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageResponse<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub offset: i64,
    pub limit: i64,
    pub has_next: bool,
}

impl<T> PageResponse<T> {
    /// Build a [`PageResponse`] from a full items vec, total count, and pagination params.
    pub fn from_vec(items: Vec<T>, total: i64, offset: i64, limit: i64) -> Self {
        let has_next = (offset + limit) < total;
        Self {
            items,
            total,
            offset,
            limit,
            has_next,
        }
    }
}

// ---------------------------------------------------------------------------
// Serialization Helpers
// ---------------------------------------------------------------------------

/// Serialize a value to a JSON string.
pub fn to_json<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string(value).map_err(|e| e.to_string())
}

/// Deserialize a value from a JSON string.
pub fn from_json<T: DeserializeOwned>(json: &str) -> Result<T, String> {
    serde_json::from_str(json).map_err(|e| e.to_string())
}

/// Serialize a value to JSON bytes.
pub fn to_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    serde_json::to_vec(value).map_err(|e| e.to_string())
}

/// Deserialize a value from JSON bytes.
pub fn from_bytes<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, String> {
    serde_json::from_slice(bytes).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn unix_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- API protocol -------------------------------------------------------

    #[test]
    fn api_request_round_trip() {
        let req = ApiRequest {
            request_id: "req-1".into(),
            timestamp: 1700000000,
            payload: serde_json::json!({"action": "login"}),
        };
        let json = to_json(&req).unwrap();
        let decoded: ApiRequest<serde_json::Value> = from_json(&json).unwrap();
        assert_eq!(req, decoded);
    }

    #[test]
    fn api_response_ok_constructor() {
        let resp = ApiResponse::ok("req-2", "hello");
        assert!(resp.success);
        assert_eq!(resp.data, Some("hello"));
        assert!(resp.error.is_none());
        assert_eq!(resp.request_id, "req-2");
    }

    #[test]
    fn api_response_err_constructor() {
        let resp: ApiResponse<()> = ApiResponse::err("req-3", "NOT_FOUND", "resource missing");
        assert!(!resp.success);
        assert!(resp.data.is_none());
        let err = resp.error.as_ref().unwrap();
        assert_eq!(err.code, "NOT_FOUND");
        assert_eq!(err.message, "resource missing");
    }

    #[test]
    fn api_response_serialization_round_trip() {
        let resp = ApiResponse::ok("req-4", vec![1, 2, 3]);
        let json = to_json(&resp).unwrap();
        let decoded: ApiResponse<Vec<i32>> = from_json(&json).unwrap();
        assert_eq!(resp.data, decoded.data);
        assert_eq!(resp.success, decoded.success);
    }

    // -- Game events --------------------------------------------------------

    #[test]
    fn game_event_tagged_serialization() {
        let event = GameEvent::PlayerRegistered {
            player_id: 42,
            username: "commander".into(),
        };
        let json = to_json(&event).unwrap();
        assert!(json.contains("\"type\":\"PlayerRegistered\""));
        let decoded: GameEvent = from_json(&json).unwrap();
        assert_eq!(event, decoded);
    }

    #[test]
    fn game_event_envelope_round_trip() {
        let mut metadata = HashMap::new();
        metadata.insert("source".into(), "core-engine".into());
        let envelope = GameEventEnvelope {
            event_id: "evt-1".into(),
            event_type: "FleetDispatched".into(),
            universe_id: Some(1),
            player_id: Some(10),
            timestamp: 1700000000,
            payload: GameEvent::FleetDispatched {
                fleet_id: 99,
                origin: "1:2:3".into(),
                destination: "4:5:6".into(),
                mission: "attack".into(),
            },
            metadata,
        };
        let bytes = to_bytes(&envelope).unwrap();
        let decoded: GameEventEnvelope = from_bytes(&bytes).unwrap();
        assert_eq!(envelope, decoded);
    }

    #[test]
    fn game_event_all_variants_serialize() {
        // Ensure every variant can at least be serialised without panic.
        let variants: Vec<GameEvent> = vec![
            GameEvent::PlayerRegistered {
                player_id: 1,
                username: "u".into(),
            },
            GameEvent::PlanetColonized {
                player_id: 1,
                planet_id: 2,
                coordinates: "1:1:1".into(),
            },
            GameEvent::FleetDispatched {
                fleet_id: 1,
                origin: "a".into(),
                destination: "b".into(),
                mission: "spy".into(),
            },
            GameEvent::FleetArrived {
                fleet_id: 1,
                destination: "b".into(),
            },
            GameEvent::FleetReturned {
                fleet_id: 1,
                origin: "a".into(),
            },
            GameEvent::CombatResolved {
                battle_id: 1,
                attacker_id: 2,
                defender_id: 3,
                winner: "attacker".into(),
            },
            GameEvent::ResearchCompleted {
                player_id: 1,
                research: "laser".into(),
                level: 5,
            },
            GameEvent::BuildingCompleted {
                planet_id: 1,
                building: "factory".into(),
                level: 3,
            },
            GameEvent::ShipConstructed {
                planet_id: 1,
                ship_type: "cruiser".into(),
                quantity: 10,
            },
            GameEvent::AllianceCreated {
                alliance_id: 1,
                name: "ally".into(),
                founder_id: 2,
            },
            GameEvent::AllianceDisbanded { alliance_id: 1 },
            GameEvent::MessageSent {
                message_id: 1,
                sender_id: 2,
                recipient_id: 3,
            },
            GameEvent::TradeCompleted {
                trade_id: 1,
                buyer_id: 2,
                seller_id: 3,
            },
            GameEvent::MoonCreated {
                moon_id: 1,
                planet_id: 2,
            },
            GameEvent::MoonDestroyed { moon_id: 1 },
            GameEvent::UniverseCreated {
                universe_id: 1,
                name: "uni".into(),
            },
            GameEvent::PlayerBanned {
                player_id: 1,
                reason: "cheating".into(),
            },
            GameEvent::PlayerUnbanned { player_id: 1 },
        ];
        for variant in &variants {
            let json = to_json(variant).unwrap();
            let decoded: GameEvent = from_json(&json).unwrap();
            assert_eq!(variant, &decoded);
        }
    }

    // -- Worker tasks -------------------------------------------------------

    #[test]
    fn worker_task_round_trip() {
        let task = WorkerTask {
            task_id: "task-1".into(),
            task_type: WorkerTaskType::ProcessFleet,
            payload: serde_json::json!({"fleet_id": 7}),
            priority: 5,
            created_at: 1700000000,
            max_retries: 3,
            retry_count: 0,
        };
        let json = to_json(&task).unwrap();
        let decoded: WorkerTask = from_json(&json).unwrap();
        assert_eq!(task, decoded);
    }

    #[test]
    fn worker_result_success_and_failure() {
        let ok = WorkerResult {
            task_id: "task-2".into(),
            success: true,
            result: Some(serde_json::json!({"processed": true})),
            error: None,
            duration_ms: 42,
        };
        let err = WorkerResult {
            task_id: "task-3".into(),
            success: false,
            result: None,
            error: Some("timeout".into()),
            duration_ms: 5000,
        };
        let ok_json = to_json(&ok).unwrap();
        let err_json = to_json(&err).unwrap();
        assert_eq!(ok, from_json::<WorkerResult>(&ok_json).unwrap());
        assert_eq!(err, from_json::<WorkerResult>(&err_json).unwrap());
    }

    // -- Realtime messages --------------------------------------------------

    #[test]
    fn realtime_message_tagged_serialization() {
        let msg = RealtimeMessage::CombatAlert {
            battle_id: 55,
            coordinates: "3:120:7".into(),
        };
        let json = to_json(&msg).unwrap();
        assert!(json.contains("\"type\":\"CombatAlert\""));
        let decoded: RealtimeMessage = from_json(&json).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn realtime_envelope_round_trip() {
        let envelope = RealtimeEnvelope {
            message_type: "ServerAnnouncement".into(),
            target_user_id: None,
            target_universe_id: None,
            broadcast: true,
            payload: RealtimeMessage::ServerAnnouncement {
                message: "maintenance in 10 min".into(),
            },
            timestamp: 1700000000,
        };
        let bytes = to_bytes(&envelope).unwrap();
        let decoded: RealtimeEnvelope = from_bytes(&bytes).unwrap();
        assert_eq!(envelope, decoded);
    }

    // -- Health / status ----------------------------------------------------

    #[test]
    fn service_health_round_trip() {
        let health = ServiceHealth {
            service_name: "core-engine".into(),
            status: ServiceStatus::Healthy,
            version: "0.1.0".into(),
            uptime_seconds: 3600,
            checks: vec![
                HealthCheck {
                    name: "database".into(),
                    status: ServiceStatus::Healthy,
                    message: None,
                    latency_ms: Some(2),
                },
                HealthCheck {
                    name: "redis".into(),
                    status: ServiceStatus::Degraded,
                    message: Some("high latency".into()),
                    latency_ms: Some(150),
                },
            ],
        };
        let json = to_json(&health).unwrap();
        let decoded: ServiceHealth = from_json(&json).unwrap();
        assert_eq!(health, decoded);
    }

    // -- Pagination ---------------------------------------------------------

    #[test]
    fn page_response_from_vec_has_next() {
        let page = PageResponse::from_vec(vec!["a", "b"], 5, 0, 2);
        assert!(page.has_next);
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.total, 5);
    }

    #[test]
    fn page_response_from_vec_no_next() {
        let page = PageResponse::from_vec(vec![1, 2, 3], 3, 0, 10);
        assert!(!page.has_next);
    }

    #[test]
    fn page_request_round_trip() {
        let req = PageRequest {
            offset: 20,
            limit: 10,
            sort_by: Some("score".into()),
            sort_order: Some("desc".into()),
        };
        let json = to_json(&req).unwrap();
        let decoded: PageRequest = from_json(&json).unwrap();
        assert_eq!(req, decoded);
    }

    // -- Serialization helpers ----------------------------------------------

    #[test]
    fn bytes_round_trip() {
        let value = serde_json::json!({"key": "value", "num": 42});
        let bytes = to_bytes(&value).unwrap();
        let decoded: serde_json::Value = from_bytes(&bytes).unwrap();
        assert_eq!(value, decoded);
    }

    #[test]
    fn from_json_returns_error_on_invalid_input() {
        let result = from_json::<ApiError>("not valid json!!!");
        assert!(result.is_err());
    }

    #[test]
    fn from_bytes_returns_error_on_invalid_input() {
        let result = from_bytes::<ApiError>(b"garbage");
        assert!(result.is_err());
    }
}
