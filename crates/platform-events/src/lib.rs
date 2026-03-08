//! Core building blocks for the platform-events crate.
//!
//! Provides event types, an in-memory event bus, persistent event store with
//! replay/filtering, subscription management, and a dead-letter queue — all
//! designed for the Universus OGame-inspired MMO strategy game.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

// ---------------------------------------------------------------------------
// GameEventType
// ---------------------------------------------------------------------------

/// Every domain event the game can emit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum GameEventType {
    FleetDispatched,
    FleetArrived,
    CombatResolved,
    BuildingStarted,
    BuildingCompleted,
    ResearchStarted,
    ResearchCompleted,
    ShipyardStarted,
    ShipyardCompleted,
    TradeCreated,
    TradeCompleted,
    AllianceAction,
    PlayerLogin,
    PlayerLogout,
    ResourceUpdate,
    PlanetColonized,
    MoonCreated,
    EspionageReport,
    AchievementUnlocked,
    SystemAnnouncement,
}

impl fmt::Display for GameEventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            GameEventType::FleetDispatched => "FleetDispatched",
            GameEventType::FleetArrived => "FleetArrived",
            GameEventType::CombatResolved => "CombatResolved",
            GameEventType::BuildingStarted => "BuildingStarted",
            GameEventType::BuildingCompleted => "BuildingCompleted",
            GameEventType::ResearchStarted => "ResearchStarted",
            GameEventType::ResearchCompleted => "ResearchCompleted",
            GameEventType::ShipyardStarted => "ShipyardStarted",
            GameEventType::ShipyardCompleted => "ShipyardCompleted",
            GameEventType::TradeCreated => "TradeCreated",
            GameEventType::TradeCompleted => "TradeCompleted",
            GameEventType::AllianceAction => "AllianceAction",
            GameEventType::PlayerLogin => "PlayerLogin",
            GameEventType::PlayerLogout => "PlayerLogout",
            GameEventType::ResourceUpdate => "ResourceUpdate",
            GameEventType::PlanetColonized => "PlanetColonized",
            GameEventType::MoonCreated => "MoonCreated",
            GameEventType::EspionageReport => "EspionageReport",
            GameEventType::AchievementUnlocked => "AchievementUnlocked",
            GameEventType::SystemAnnouncement => "SystemAnnouncement",
        };
        write!(f, "{}", label)
    }
}

// ---------------------------------------------------------------------------
// EventEnvelope  (original — kept unchanged for backwards compat)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EventEnvelope {
    pub event_type: String,
    pub payload: serde_json::Value,
    pub emitted_at_unix: i64,
}

// ---------------------------------------------------------------------------
// RichEventEnvelope
// ---------------------------------------------------------------------------

/// Extended envelope that carries tracing / sequencing metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RichEventEnvelope {
    pub event_type: String,
    pub payload: serde_json::Value,
    pub emitted_at_unix: i64,
    pub source: Option<String>,
    pub correlation_id: Option<String>,
    pub sequence: Option<u64>,
}

impl From<EventEnvelope> for RichEventEnvelope {
    fn from(env: EventEnvelope) -> Self {
        Self {
            event_type: env.event_type,
            payload: env.payload,
            emitted_at_unix: env.emitted_at_unix,
            source: None,
            correlation_id: None,
            sequence: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Subscription / SubscriptionStore
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Subscription {
    pub id: String,
    pub event_types: Vec<GameEventType>,
    pub callback_url: Option<String>,
    pub created_at: String,
}

/// Manages active subscriptions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionStore {
    pub subscriptions: Vec<Subscription>,
}

impl Default for SubscriptionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SubscriptionStore {
    pub fn new() -> Self {
        Self {
            subscriptions: Vec::new(),
        }
    }

    pub fn add(&mut self, subscription: Subscription) {
        self.subscriptions.push(subscription);
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.subscriptions.len();
        self.subscriptions.retain(|s| s.id != id);
        self.subscriptions.len() < before
    }

    pub fn list(&self) -> &[Subscription] {
        &self.subscriptions
    }

    pub fn get(&self, id: &str) -> Option<&Subscription> {
        self.subscriptions.iter().find(|s| s.id == id)
    }

    pub fn list_by_event_type(&self, event_type: &GameEventType) -> Vec<&Subscription> {
        self.subscriptions
            .iter()
            .filter(|s| s.event_types.contains(event_type))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// EventStore  (replay / history)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredEvent {
    sequence: u64,
    event: EventEnvelope,
}

/// Append-only event log with replay and query capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventStore {
    events: Vec<StoredEvent>,
    next_sequence: u64,
}

impl Default for EventStore {
    fn default() -> Self {
        Self::new()
    }
}

impl EventStore {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            next_sequence: 1,
        }
    }

    /// Append an event and return its sequence number.
    pub fn append(&mut self, event: EventEnvelope) -> u64 {
        let seq = self.next_sequence;
        self.events.push(StoredEvent {
            sequence: seq,
            event,
        });
        self.next_sequence += 1;
        seq
    }

    /// Replay events starting from (and including) `from_sequence`, returning up
    /// to `limit` entries.
    pub fn replay(&self, from_sequence: u64, limit: usize) -> Vec<(u64, &EventEnvelope)> {
        self.events
            .iter()
            .filter(|se| se.sequence >= from_sequence)
            .take(limit)
            .map(|se| (se.sequence, &se.event))
            .collect()
    }

    /// Replay events of a specific type starting from `from_sequence`.
    pub fn replay_by_type(
        &self,
        event_type: &str,
        from_sequence: u64,
        limit: usize,
    ) -> Vec<(u64, &EventEnvelope)> {
        self.events
            .iter()
            .filter(|se| se.sequence >= from_sequence && se.event.event_type == event_type)
            .take(limit)
            .map(|se| (se.sequence, &se.event))
            .collect()
    }

    /// The last assigned sequence number, or 0 if the store is empty.
    pub fn latest_sequence(&self) -> u64 {
        self.events.last().map(|se| se.sequence).unwrap_or(0)
    }

    /// All events whose `emitted_at_unix` is ≥ `since_unix`.
    pub fn events_since(&self, since_unix: i64) -> Vec<&EventEnvelope> {
        self.events
            .iter()
            .filter(|se| se.event.emitted_at_unix >= since_unix)
            .map(|se| &se.event)
            .collect()
    }

    pub fn count(&self) -> usize {
        self.events.len()
    }

    /// Flexible query driven by an `EventFilter`.
    pub fn query(&self, filter: &EventFilter) -> Vec<&EventEnvelope> {
        let limit = filter.limit.unwrap_or(usize::MAX);
        self.events
            .iter()
            .filter(|se| {
                if let Some(ref types) = filter.event_types {
                    if !types.contains(&se.event.event_type) {
                        return false;
                    }
                }
                if let Some(since) = filter.since_unix {
                    if se.event.emitted_at_unix < since {
                        return false;
                    }
                }
                if let Some(until) = filter.until_unix {
                    if se.event.emitted_at_unix > until {
                        return false;
                    }
                }
                if let Some(ref src) = filter.source {
                    // EventEnvelope doesn't carry source — match against payload
                    // field "source" if present, otherwise skip this entry.
                    if let Some(payload_src) =
                        se.event.payload.get("source").and_then(|v| v.as_str())
                    {
                        if payload_src != src.as_str() {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
                true
            })
            .take(limit)
            .map(|se| &se.event)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// EventFilter
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct EventFilter {
    pub event_types: Option<Vec<String>>,
    pub since_unix: Option<i64>,
    pub until_unix: Option<i64>,
    pub source: Option<String>,
    pub limit: Option<usize>,
}

// ---------------------------------------------------------------------------
// EventBus  (in-memory, sync)
// ---------------------------------------------------------------------------

/// Simple synchronous in-memory event bus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventBus {
    pub events: Vec<EventEnvelope>,
    pub subscribers: HashMap<String, Vec<GameEventType>>,
    next_subscriber_id: u64,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            subscribers: HashMap::new(),
            next_subscriber_id: 1,
        }
    }

    /// Publish an event: stores it and returns a list of subscriber IDs that
    /// matched the event type.
    pub fn publish(&mut self, event: EventEnvelope) -> Vec<String> {
        let matched: Vec<String> = self
            .subscribers
            .iter()
            .filter(|(_, types)| types.iter().any(|t| t.to_string() == event.event_type))
            .map(|(id, _)| id.clone())
            .collect();
        self.events.push(event);
        matched
    }

    /// Subscribe to one or more event types. Returns a `Subscription` handle.
    pub fn subscribe(&mut self, event_types: Vec<GameEventType>) -> Subscription {
        let id = format!("sub-{}", self.next_subscriber_id);
        self.next_subscriber_id += 1;
        self.subscribers.insert(id.clone(), event_types.clone());
        Subscription {
            id,
            event_types,
            callback_url: None,
            created_at: iso_now(),
        }
    }

    pub fn unsubscribe(&mut self, subscription_id: &str) {
        self.subscribers.remove(subscription_id);
    }

    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }
}

// ---------------------------------------------------------------------------
// Dead Letter Queue
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeadLetterEntry {
    pub event: EventEnvelope,
    pub error: String,
    pub attempt: u32,
    pub enqueued_at: String,
}

/// Queue of events that could not be delivered.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeadLetterQueue {
    pub entries: Vec<DeadLetterEntry>,
}

impl DeadLetterQueue {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn enqueue(&mut self, event: EventEnvelope, error: String, attempt: u32) {
        self.entries.push(DeadLetterEntry {
            event,
            error,
            attempt,
            enqueued_at: iso_now(),
        });
    }

    pub fn dequeue(&mut self) -> Option<DeadLetterEntry> {
        if self.entries.is_empty() {
            None
        } else {
            Some(self.entries.remove(0))
        }
    }

    pub fn peek(&self) -> Option<&DeadLetterEntry> {
        self.entries.first()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Drain all entries for retry, returning them in order.
    pub fn retry_all(&mut self) -> Vec<DeadLetterEntry> {
        std::mem::take(&mut self.entries)
    }
}

// ---------------------------------------------------------------------------
// Original helper functions (preserved)
// ---------------------------------------------------------------------------

pub fn build_event<T: Serialize>(event_type: &str, payload: &T) -> EventEnvelope {
    EventEnvelope {
        event_type: event_type.to_string(),
        payload: serde_json::to_value(payload).unwrap_or_else(|_| serde_json::json!({})),
        emitted_at_unix: unix_timestamp(),
    }
}

pub fn build_publish_payload(channel: &str, event: &EventEnvelope) -> serde_json::Value {
    serde_json::json!({
        "channel": channel,
        "event": serde_json::to_string(event).unwrap_or_else(|_| "{}".to_string())
    })
}

pub async fn publish_http(
    base_url: &str,
    channel: &str,
    event: &EventEnvelope,
) -> Result<u16, String> {
    let url = format!("{}/api/realtime/publish", base_url.trim_end_matches('/'));
    let body = build_publish_payload(channel, event);
    let response = reqwest::Client::new()
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    Ok(response.status().as_u16())
}

/// Returns the crate name for a basic compile-time sanity check.
pub const fn crate_name() -> &'static str {
    "platform-events"
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn unix_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn iso_now() -> String {
    let secs = unix_timestamp();
    // Produce a minimal ISO-8601 string from a unix timestamp.
    // We avoid pulling in `chrono` — this is good enough for in-process use.
    format!("{}Z", secs)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- helpers --

    fn make_envelope(event_type: &str, ts: i64) -> EventEnvelope {
        EventEnvelope {
            event_type: event_type.to_string(),
            payload: serde_json::json!({"test": true}),
            emitted_at_unix: ts,
        }
    }

    fn make_envelope_with_source(event_type: &str, ts: i64, source: &str) -> EventEnvelope {
        EventEnvelope {
            event_type: event_type.to_string(),
            payload: serde_json::json!({"test": true, "source": source}),
            emitted_at_unix: ts,
        }
    }

    // ============================ crate_name ============================

    #[test]
    fn crate_name_returns_expected() {
        assert_eq!(crate_name(), "platform-events");
    }

    // ====================== build_event / payload =======================

    #[test]
    fn build_event_sets_event_type() {
        let ev = build_event("FleetDispatched", &serde_json::json!({"fleet":1}));
        assert_eq!(ev.event_type, "FleetDispatched");
    }

    #[test]
    fn build_event_sets_payload() {
        let ev = build_event("FleetDispatched", &serde_json::json!({"fleet_id":42}));
        assert_eq!(ev.payload["fleet_id"], 42);
    }

    #[test]
    fn build_event_has_nonzero_timestamp() {
        let ev = build_event("x", &serde_json::json!({}));
        assert!(ev.emitted_at_unix > 0);
    }

    #[test]
    fn build_publish_payload_contains_channel_and_event() {
        let event = build_event("scheduler.tick", &serde_json::json!({"job":"fleet"}));
        let payload = build_publish_payload("ops.scheduler", &event);
        assert_eq!(payload["channel"], "ops.scheduler");
        assert!(payload["event"]
            .as_str()
            .unwrap()
            .contains("scheduler.tick"));
    }

    // ======================== GameEventType =============================

    #[test]
    fn game_event_type_display() {
        assert_eq!(
            GameEventType::FleetDispatched.to_string(),
            "FleetDispatched"
        );
        assert_eq!(GameEventType::MoonCreated.to_string(), "MoonCreated");
        assert_eq!(
            GameEventType::SystemAnnouncement.to_string(),
            "SystemAnnouncement"
        );
    }

    #[test]
    fn game_event_type_clone_eq() {
        let a = GameEventType::CombatResolved;
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn game_event_type_serialize_roundtrip() {
        let original = GameEventType::ResearchCompleted;
        let json = serde_json::to_string(&original).unwrap();
        let back: GameEventType = serde_json::from_str(&json).unwrap();
        assert_eq!(original, back);
    }

    #[test]
    fn game_event_type_hash_works() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(GameEventType::PlayerLogin);
        set.insert(GameEventType::PlayerLogin);
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn all_game_event_types_display_unique() {
        let all = vec![
            GameEventType::FleetDispatched,
            GameEventType::FleetArrived,
            GameEventType::CombatResolved,
            GameEventType::BuildingStarted,
            GameEventType::BuildingCompleted,
            GameEventType::ResearchStarted,
            GameEventType::ResearchCompleted,
            GameEventType::ShipyardStarted,
            GameEventType::ShipyardCompleted,
            GameEventType::TradeCreated,
            GameEventType::TradeCompleted,
            GameEventType::AllianceAction,
            GameEventType::PlayerLogin,
            GameEventType::PlayerLogout,
            GameEventType::ResourceUpdate,
            GameEventType::PlanetColonized,
            GameEventType::MoonCreated,
            GameEventType::EspionageReport,
            GameEventType::AchievementUnlocked,
            GameEventType::SystemAnnouncement,
        ];
        use std::collections::HashSet;
        let strs: HashSet<String> = all.iter().map(|e| e.to_string()).collect();
        assert_eq!(strs.len(), 20);
    }

    // ==================== RichEventEnvelope =============================

    #[test]
    fn rich_envelope_from_event_envelope() {
        let env = make_envelope("FleetDispatched", 100);
        let rich: RichEventEnvelope = env.into();
        assert_eq!(rich.event_type, "FleetDispatched");
        assert_eq!(rich.emitted_at_unix, 100);
        assert!(rich.source.is_none());
        assert!(rich.correlation_id.is_none());
        assert!(rich.sequence.is_none());
    }

    #[test]
    fn rich_envelope_serde_roundtrip() {
        let rich = RichEventEnvelope {
            event_type: "CombatResolved".into(),
            payload: serde_json::json!({"winner": "p1"}),
            emitted_at_unix: 999,
            source: Some("combat-service".into()),
            correlation_id: Some("abc-123".into()),
            sequence: Some(42),
        };
        let json = serde_json::to_string(&rich).unwrap();
        let back: RichEventEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(rich, back);
    }

    // ======================== EventBus ==================================

    #[test]
    fn event_bus_new_is_empty() {
        let bus = EventBus::new();
        assert_eq!(bus.event_count(), 0);
    }

    #[test]
    fn event_bus_publish_increments_count() {
        let mut bus = EventBus::new();
        bus.publish(make_envelope("FleetDispatched", 1));
        bus.publish(make_envelope("FleetArrived", 2));
        assert_eq!(bus.event_count(), 2);
    }

    #[test]
    fn event_bus_subscribe_returns_subscription() {
        let mut bus = EventBus::new();
        let sub = bus.subscribe(vec![GameEventType::FleetDispatched]);
        assert!(sub.id.starts_with("sub-"));
        assert_eq!(sub.event_types, vec![GameEventType::FleetDispatched]);
    }

    #[test]
    fn event_bus_publish_notifies_matching_subscribers() {
        let mut bus = EventBus::new();
        let sub = bus.subscribe(vec![GameEventType::FleetDispatched]);
        let matched = bus.publish(make_envelope("FleetDispatched", 1));
        assert!(matched.contains(&sub.id));
    }

    #[test]
    fn event_bus_publish_does_not_notify_non_matching() {
        let mut bus = EventBus::new();
        let sub = bus.subscribe(vec![GameEventType::CombatResolved]);
        let matched = bus.publish(make_envelope("FleetDispatched", 1));
        assert!(!matched.contains(&sub.id));
    }

    #[test]
    fn event_bus_unsubscribe_removes_subscriber() {
        let mut bus = EventBus::new();
        let sub = bus.subscribe(vec![GameEventType::FleetDispatched]);
        bus.unsubscribe(&sub.id);
        let matched = bus.publish(make_envelope("FleetDispatched", 1));
        assert!(matched.is_empty());
    }

    #[test]
    fn event_bus_clear_empties_events() {
        let mut bus = EventBus::new();
        bus.publish(make_envelope("A", 1));
        bus.publish(make_envelope("B", 2));
        bus.clear();
        assert_eq!(bus.event_count(), 0);
    }

    #[test]
    fn event_bus_multiple_subscribers() {
        let mut bus = EventBus::new();
        let s1 = bus.subscribe(vec![GameEventType::FleetDispatched]);
        let s2 = bus.subscribe(vec![
            GameEventType::FleetDispatched,
            GameEventType::CombatResolved,
        ]);
        let matched = bus.publish(make_envelope("FleetDispatched", 1));
        assert!(matched.contains(&s1.id));
        assert!(matched.contains(&s2.id));
    }

    #[test]
    fn event_bus_default_is_new() {
        let bus = EventBus::default();
        assert_eq!(bus.event_count(), 0);
        assert!(bus.subscribers.is_empty());
    }

    // ==================== SubscriptionStore =============================

    #[test]
    fn subscription_store_add_and_list() {
        let mut store = SubscriptionStore::new();
        store.add(Subscription {
            id: "s1".into(),
            event_types: vec![GameEventType::PlayerLogin],
            callback_url: None,
            created_at: "0Z".into(),
        });
        assert_eq!(store.list().len(), 1);
    }

    #[test]
    fn subscription_store_get() {
        let mut store = SubscriptionStore::new();
        store.add(Subscription {
            id: "s1".into(),
            event_types: vec![],
            callback_url: Some("http://example.com".into()),
            created_at: "0Z".into(),
        });
        let found = store.get("s1").unwrap();
        assert_eq!(found.callback_url.as_deref(), Some("http://example.com"));
    }

    #[test]
    fn subscription_store_remove() {
        let mut store = SubscriptionStore::new();
        store.add(Subscription {
            id: "s1".into(),
            event_types: vec![],
            callback_url: None,
            created_at: "0Z".into(),
        });
        assert!(store.remove("s1"));
        assert!(store.list().is_empty());
    }

    #[test]
    fn subscription_store_remove_nonexistent_returns_false() {
        let mut store = SubscriptionStore::new();
        assert!(!store.remove("nope"));
    }

    #[test]
    fn subscription_store_list_by_event_type() {
        let mut store = SubscriptionStore::new();
        store.add(Subscription {
            id: "a".into(),
            event_types: vec![GameEventType::TradeCreated, GameEventType::TradeCompleted],
            callback_url: None,
            created_at: "0Z".into(),
        });
        store.add(Subscription {
            id: "b".into(),
            event_types: vec![GameEventType::TradeCreated],
            callback_url: None,
            created_at: "0Z".into(),
        });
        store.add(Subscription {
            id: "c".into(),
            event_types: vec![GameEventType::CombatResolved],
            callback_url: None,
            created_at: "0Z".into(),
        });
        let matches = store.list_by_event_type(&GameEventType::TradeCreated);
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn subscription_store_default() {
        let store = SubscriptionStore::default();
        assert!(store.list().is_empty());
    }

    // ======================== EventStore ================================

    #[test]
    fn event_store_append_returns_incrementing_sequences() {
        let mut store = EventStore::new();
        assert_eq!(store.append(make_envelope("A", 1)), 1);
        assert_eq!(store.append(make_envelope("B", 2)), 2);
        assert_eq!(store.append(make_envelope("C", 3)), 3);
    }

    #[test]
    fn event_store_count() {
        let mut store = EventStore::new();
        assert_eq!(store.count(), 0);
        store.append(make_envelope("A", 1));
        assert_eq!(store.count(), 1);
    }

    #[test]
    fn event_store_latest_sequence_empty() {
        let store = EventStore::new();
        assert_eq!(store.latest_sequence(), 0);
    }

    #[test]
    fn event_store_latest_sequence_after_appends() {
        let mut store = EventStore::new();
        store.append(make_envelope("A", 1));
        store.append(make_envelope("B", 2));
        assert_eq!(store.latest_sequence(), 2);
    }

    #[test]
    fn event_store_replay_from_start() {
        let mut store = EventStore::new();
        store.append(make_envelope("A", 1));
        store.append(make_envelope("B", 2));
        store.append(make_envelope("C", 3));
        let results = store.replay(1, 10);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].0, 1);
        assert_eq!(results[2].0, 3);
    }

    #[test]
    fn event_store_replay_with_offset() {
        let mut store = EventStore::new();
        store.append(make_envelope("A", 1));
        store.append(make_envelope("B", 2));
        store.append(make_envelope("C", 3));
        let results = store.replay(2, 10);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].1.event_type, "B");
    }

    #[test]
    fn event_store_replay_with_limit() {
        let mut store = EventStore::new();
        for i in 0..10 {
            store.append(make_envelope("X", i));
        }
        let results = store.replay(1, 3);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn event_store_replay_by_type() {
        let mut store = EventStore::new();
        store.append(make_envelope("FleetDispatched", 1));
        store.append(make_envelope("CombatResolved", 2));
        store.append(make_envelope("FleetDispatched", 3));
        let results = store.replay_by_type("FleetDispatched", 1, 100);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn event_store_replay_by_type_respects_sequence() {
        let mut store = EventStore::new();
        store.append(make_envelope("FleetDispatched", 1));
        store.append(make_envelope("FleetDispatched", 2));
        store.append(make_envelope("FleetDispatched", 3));
        let results = store.replay_by_type("FleetDispatched", 3, 100);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 3);
    }

    #[test]
    fn event_store_events_since() {
        let mut store = EventStore::new();
        store.append(make_envelope("A", 100));
        store.append(make_envelope("B", 200));
        store.append(make_envelope("C", 300));
        let results = store.events_since(200);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn event_store_default() {
        let store = EventStore::default();
        assert_eq!(store.count(), 0);
        assert_eq!(store.latest_sequence(), 0);
    }

    // ======================== EventFilter / query ========================

    #[test]
    fn event_filter_default_is_empty() {
        let filter = EventFilter::default();
        assert!(filter.event_types.is_none());
        assert!(filter.since_unix.is_none());
        assert!(filter.until_unix.is_none());
        assert!(filter.source.is_none());
        assert!(filter.limit.is_none());
    }

    #[test]
    fn event_store_query_no_filter_returns_all() {
        let mut store = EventStore::new();
        store.append(make_envelope("A", 1));
        store.append(make_envelope("B", 2));
        let results = store.query(&EventFilter::default());
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn event_store_query_by_event_types() {
        let mut store = EventStore::new();
        store.append(make_envelope("FleetDispatched", 1));
        store.append(make_envelope("CombatResolved", 2));
        store.append(make_envelope("FleetDispatched", 3));
        let filter = EventFilter {
            event_types: Some(vec!["FleetDispatched".into()]),
            ..Default::default()
        };
        let results = store.query(&filter);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn event_store_query_by_time_range() {
        let mut store = EventStore::new();
        store.append(make_envelope("A", 100));
        store.append(make_envelope("B", 200));
        store.append(make_envelope("C", 300));
        let filter = EventFilter {
            since_unix: Some(150),
            until_unix: Some(250),
            ..Default::default()
        };
        let results = store.query(&filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].event_type, "B");
    }

    #[test]
    fn event_store_query_by_source() {
        let mut store = EventStore::new();
        store.append(make_envelope_with_source("A", 1, "fleet-svc"));
        store.append(make_envelope_with_source("B", 2, "combat-svc"));
        store.append(make_envelope("C", 3)); // no source in payload
        let filter = EventFilter {
            source: Some("fleet-svc".into()),
            ..Default::default()
        };
        let results = store.query(&filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].event_type, "A");
    }

    #[test]
    fn event_store_query_with_limit() {
        let mut store = EventStore::new();
        for i in 0..10 {
            store.append(make_envelope("X", i));
        }
        let filter = EventFilter {
            limit: Some(3),
            ..Default::default()
        };
        let results = store.query(&filter);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn event_store_query_combined_filters() {
        let mut store = EventStore::new();
        store.append(make_envelope("FleetDispatched", 100));
        store.append(make_envelope("FleetDispatched", 200));
        store.append(make_envelope("CombatResolved", 200));
        store.append(make_envelope("FleetDispatched", 300));
        let filter = EventFilter {
            event_types: Some(vec!["FleetDispatched".into()]),
            since_unix: Some(150),
            limit: Some(1),
            ..Default::default()
        };
        let results = store.query(&filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].emitted_at_unix, 200);
    }

    // ===================== DeadLetterQueue ==============================

    #[test]
    fn dlq_new_is_empty() {
        let dlq = DeadLetterQueue::new();
        assert!(dlq.is_empty());
        assert_eq!(dlq.len(), 0);
    }

    #[test]
    fn dlq_enqueue_increments_len() {
        let mut dlq = DeadLetterQueue::new();
        dlq.enqueue(make_envelope("A", 1), "timeout".into(), 1);
        assert_eq!(dlq.len(), 1);
        assert!(!dlq.is_empty());
    }

    #[test]
    fn dlq_peek_returns_first() {
        let mut dlq = DeadLetterQueue::new();
        dlq.enqueue(make_envelope("A", 1), "err1".into(), 1);
        dlq.enqueue(make_envelope("B", 2), "err2".into(), 2);
        let peeked = dlq.peek().unwrap();
        assert_eq!(peeked.event.event_type, "A");
        assert_eq!(dlq.len(), 2); // peek does not consume
    }

    #[test]
    fn dlq_dequeue_removes_first() {
        let mut dlq = DeadLetterQueue::new();
        dlq.enqueue(make_envelope("A", 1), "e".into(), 1);
        dlq.enqueue(make_envelope("B", 2), "e".into(), 1);
        let entry = dlq.dequeue().unwrap();
        assert_eq!(entry.event.event_type, "A");
        assert_eq!(dlq.len(), 1);
    }

    #[test]
    fn dlq_dequeue_empty_returns_none() {
        let mut dlq = DeadLetterQueue::new();
        assert!(dlq.dequeue().is_none());
    }

    #[test]
    fn dlq_retry_all_drains() {
        let mut dlq = DeadLetterQueue::new();
        dlq.enqueue(make_envelope("A", 1), "e".into(), 1);
        dlq.enqueue(make_envelope("B", 2), "e".into(), 2);
        let entries = dlq.retry_all();
        assert_eq!(entries.len(), 2);
        assert!(dlq.is_empty());
    }

    #[test]
    fn dlq_entry_has_attempt_and_error() {
        let mut dlq = DeadLetterQueue::new();
        dlq.enqueue(make_envelope("X", 1), "connection refused".into(), 3);
        let entry = dlq.peek().unwrap();
        assert_eq!(entry.attempt, 3);
        assert_eq!(entry.error, "connection refused");
    }

    #[test]
    fn dlq_default_is_empty() {
        let dlq = DeadLetterQueue::default();
        assert!(dlq.is_empty());
    }

    // ==================== EventEnvelope serde ============================

    #[test]
    fn event_envelope_serde_roundtrip() {
        let env = make_envelope("FleetDispatched", 1234);
        let json = serde_json::to_string(&env).unwrap();
        let back: EventEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(env, back);
    }

    #[test]
    fn event_envelope_uses_camel_case() {
        let env = make_envelope("X", 10);
        let json = serde_json::to_string(&env).unwrap();
        assert!(json.contains("eventType"));
        assert!(json.contains("emittedAtUnix"));
    }
}
