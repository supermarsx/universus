#![forbid(unsafe_code)]

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Notification Category
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationCategory {
    Fleet,
    Combat,
    Research,
    Building,
    Alliance,
    Trade,
    System,
    Achievement,
    Espionage,
}

impl NotificationCategory {
    /// Parse a string into a category (case-insensitive).  Falls back to
    /// `System` for unrecognised values.
    pub fn from_str_lossy(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "fleet" => Self::Fleet,
            "combat" => Self::Combat,
            "research" => Self::Research,
            "building" => Self::Building,
            "alliance" => Self::Alliance,
            "trade" => Self::Trade,
            "system" => Self::System,
            "achievement" => Self::Achievement,
            "espionage" => Self::Espionage,
            _ => Self::System,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Fleet => "fleet",
            Self::Combat => "combat",
            Self::Research => "research",
            Self::Building => "building",
            Self::Alliance => "alliance",
            Self::Trade => "trade",
            Self::System => "system",
            Self::Achievement => "achievement",
            Self::Espionage => "espionage",
        }
    }
}

impl From<&str> for NotificationCategory {
    fn from(s: &str) -> Self {
        Self::from_str_lossy(s)
    }
}

impl From<String> for NotificationCategory {
    fn from(s: String) -> Self {
        Self::from_str_lossy(&s)
    }
}

impl std::fmt::Display for NotificationCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// Notification Priority
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationPriority {
    Low,
    Medium,
    High,
    Critical,
}

impl NotificationPriority {
    /// Convert a numeric priority (0-255) into an enum variant.
    pub fn from_u8(value: u8) -> Self {
        match value {
            0..=1 => Self::Low,
            2..=3 => Self::Medium,
            4..=5 => Self::High,
            _ => Self::Critical,
        }
    }

    pub fn to_u8(self) -> u8 {
        match self {
            Self::Low => 1,
            Self::Medium => 2,
            Self::High => 4,
            Self::Critical => 6,
        }
    }
}

impl From<u8> for NotificationPriority {
    fn from(value: u8) -> Self {
        Self::from_u8(value)
    }
}

impl From<NotificationPriority> for u8 {
    fn from(value: NotificationPriority) -> Self {
        value.to_u8()
    }
}

impl std::fmt::Display for NotificationPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        };
        f.write_str(label)
    }
}

// ---------------------------------------------------------------------------
// Notification Channels
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationChannel {
    InApp,
    Push,
    Email,
    WebSocket,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelConfig {
    pub channel: NotificationChannel,
    pub enabled: bool,
}

// ---------------------------------------------------------------------------
// Core notification types – backwards compatible
// ---------------------------------------------------------------------------

/// A notification record.  The `category` and `priority` fields are kept as
/// `String` / `u8` so that the existing API gateway serialisation continues to
/// work unchanged.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    pub id: i64,
    pub user_id: i64,
    pub title: String,
    pub message: String,
    pub category: String,
    pub priority: u8,
    pub is_read: bool,
    pub created_at: String,
    pub read_at: Option<String>,
}

impl Notification {
    /// Parse `category` into a typed enum.
    pub fn category_enum(&self) -> NotificationCategory {
        NotificationCategory::from_str_lossy(&self.category)
    }

    /// Parse `priority` into a typed enum.
    pub fn priority_enum(&self) -> NotificationPriority {
        NotificationPriority::from_u8(self.priority)
    }
}

/// Input for creating a new notification.  Keeps `String` / `u8` for
/// backwards compatibility with the API gateway.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewNotification {
    pub title: String,
    pub message: String,
    pub category: String,
    pub priority: u8,
}

// ---------------------------------------------------------------------------
// User Preferences
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationPreferences {
    pub user_id: i64,
    pub muted_categories: Vec<NotificationCategory>,
    pub min_priority: NotificationPriority,
    pub channels: Vec<ChannelConfig>,
    pub quiet_hours_start: Option<u32>,
    pub quiet_hours_end: Option<u32>,
}

impl NotificationPreferences {
    pub fn default_for(user_id: i64) -> Self {
        Self {
            user_id,
            muted_categories: Vec::new(),
            min_priority: NotificationPriority::Low,
            channels: vec![
                ChannelConfig {
                    channel: NotificationChannel::InApp,
                    enabled: true,
                },
                ChannelConfig {
                    channel: NotificationChannel::WebSocket,
                    enabled: true,
                },
                ChannelConfig {
                    channel: NotificationChannel::Push,
                    enabled: false,
                },
                ChannelConfig {
                    channel: NotificationChannel::Email,
                    enabled: false,
                },
            ],
            quiet_hours_start: None,
            quiet_hours_end: None,
        }
    }
}

#[derive(Default)]
pub struct PreferencesStore {
    prefs: HashMap<i64, NotificationPreferences>,
}

impl PreferencesStore {
    pub fn get_preferences(&self, user_id: i64) -> NotificationPreferences {
        self.prefs
            .get(&user_id)
            .cloned()
            .unwrap_or_else(|| NotificationPreferences::default_for(user_id))
    }

    pub fn set_preferences(&mut self, prefs: NotificationPreferences) {
        self.prefs.insert(prefs.user_id, prefs);
    }

    /// Determines whether a notification should be delivered based on the
    /// user's preferences (muted categories, minimum priority, quiet hours).
    ///
    /// `current_hour` is an optional externally-supplied hour (0-23) used for
    /// quiet-hours checking; when `None` quiet hours are ignored.
    pub fn should_deliver(
        &self,
        user_id: i64,
        category: &NotificationCategory,
        priority: &NotificationPriority,
    ) -> bool {
        self.should_deliver_at(user_id, category, priority, None)
    }

    pub fn should_deliver_at(
        &self,
        user_id: i64,
        category: &NotificationCategory,
        priority: &NotificationPriority,
        current_hour: Option<u32>,
    ) -> bool {
        let prefs = self.get_preferences(user_id);

        // Muted category?
        if prefs.muted_categories.contains(category) {
            return false;
        }

        // Below minimum priority?
        if *priority < prefs.min_priority {
            return false;
        }

        // Quiet hours?
        if let (Some(start), Some(end), Some(hour)) =
            (prefs.quiet_hours_start, prefs.quiet_hours_end, current_hour)
        {
            let in_quiet = if start <= end {
                hour >= start && hour < end
            } else {
                // wraps midnight, e.g. 22..06
                hour >= start || hour < end
            };
            if in_quiet {
                return false;
            }
        }

        true
    }
}

// ---------------------------------------------------------------------------
// Delivery Tracking
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryStatus {
    Pending,
    Delivered,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryRecord {
    pub notification_id: i64,
    pub channel: NotificationChannel,
    pub status: DeliveryStatus,
    pub attempted_at: String,
    pub delivered_at: Option<String>,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct DeliveryStore {
    records: Vec<DeliveryRecord>,
}

impl DeliveryStore {
    pub fn record_attempt(&mut self, notification_id: i64, channel: NotificationChannel) {
        self.records.push(DeliveryRecord {
            notification_id,
            channel,
            status: DeliveryStatus::Pending,
            attempted_at: now_timestamp(),
            delivered_at: None,
            error: None,
        });
    }

    pub fn mark_delivered(&mut self, notification_id: i64, channel: &NotificationChannel) -> bool {
        for record in self.records.iter_mut().rev() {
            if record.notification_id == notification_id
                && record.channel == *channel
                && record.status == DeliveryStatus::Pending
            {
                record.status = DeliveryStatus::Delivered;
                record.delivered_at = Some(now_timestamp());
                return true;
            }
        }
        false
    }

    pub fn mark_failed(
        &mut self,
        notification_id: i64,
        channel: &NotificationChannel,
        error: String,
    ) -> bool {
        for record in self.records.iter_mut().rev() {
            if record.notification_id == notification_id
                && record.channel == *channel
                && record.status == DeliveryStatus::Pending
            {
                record.status = DeliveryStatus::Failed;
                record.error = Some(error);
                return true;
            }
        }
        false
    }

    pub fn get_delivery_status(&self, notification_id: i64) -> Vec<&DeliveryRecord> {
        self.records
            .iter()
            .filter(|r| r.notification_id == notification_id)
            .collect()
    }

    pub fn pending_deliveries(&self) -> Vec<&DeliveryRecord> {
        self.records
            .iter()
            .filter(|r| r.status == DeliveryStatus::Pending)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Notification Templates
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationTemplate {
    pub id: String,
    pub category: NotificationCategory,
    pub title_template: String,
    pub message_template: String,
}

pub struct TemplateStore {
    templates: HashMap<String, NotificationTemplate>,
}

impl Default for TemplateStore {
    fn default() -> Self {
        let mut store = Self {
            templates: HashMap::new(),
        };
        store.seed_defaults();
        store
    }
}

impl TemplateStore {
    pub fn add_template(&mut self, template: NotificationTemplate) {
        self.templates.insert(template.id.clone(), template);
    }

    pub fn get_template(&self, template_id: &str) -> Option<&NotificationTemplate> {
        self.templates.get(template_id)
    }

    /// Render a template by replacing `{placeholder}` tokens in the title and
    /// message templates.  Returns `None` if the template id is unknown.
    pub fn render(
        &self,
        template_id: &str,
        params: &HashMap<String, String>,
    ) -> Option<(String, String)> {
        let tmpl = self.templates.get(template_id)?;
        Some((
            Self::replace_placeholders(&tmpl.title_template, params),
            Self::replace_placeholders(&tmpl.message_template, params),
        ))
    }

    fn replace_placeholders(template: &str, params: &HashMap<String, String>) -> String {
        let mut result = template.to_string();
        for (key, value) in params {
            let placeholder = format!("{{{key}}}");
            result = result.replace(&placeholder, value);
        }
        result
    }

    fn seed_defaults(&mut self) {
        let defaults = vec![
            NotificationTemplate {
                id: "fleet_arrived".to_string(),
                category: NotificationCategory::Fleet,
                title_template: "Fleet Arrived".to_string(),
                message_template: "Your fleet has arrived at {destination}.".to_string(),
            },
            NotificationTemplate {
                id: "combat_report".to_string(),
                category: NotificationCategory::Combat,
                title_template: "Combat Report".to_string(),
                message_template: "Battle at {location}: {outcome}.".to_string(),
            },
            NotificationTemplate {
                id: "research_complete".to_string(),
                category: NotificationCategory::Research,
                title_template: "Research Complete".to_string(),
                message_template: "{technology} has reached level {level}.".to_string(),
            },
            NotificationTemplate {
                id: "building_complete".to_string(),
                category: NotificationCategory::Building,
                title_template: "Building Complete".to_string(),
                message_template: "{building} has been upgraded to level {level}.".to_string(),
            },
            NotificationTemplate {
                id: "under_attack".to_string(),
                category: NotificationCategory::Combat,
                title_template: "Under Attack!".to_string(),
                message_template: "Your planet at {location} is under attack by {attacker}!"
                    .to_string(),
            },
            NotificationTemplate {
                id: "alliance_invite".to_string(),
                category: NotificationCategory::Alliance,
                title_template: "Alliance Invitation".to_string(),
                message_template: "You have been invited to join {alliance}.".to_string(),
            },
        ];
        for tmpl in defaults {
            self.templates.insert(tmpl.id.clone(), tmpl);
        }
    }
}

// ---------------------------------------------------------------------------
// Notification Store – backwards compatible API + new batch operations
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct NotificationStore {
    next_id: i64,
    by_user: HashMap<i64, Vec<Notification>>,
}

impl NotificationStore {
    // -- original API (unchanged signatures) --------------------------------

    pub fn list_user_notifications(
        &mut self,
        user_id: i64,
        unread_only: bool,
        limit: usize,
    ) -> Vec<Notification> {
        self.ensure_seed(user_id);
        let max_items = limit.max(1);
        self.by_user
            .get(&user_id)
            .map(|items| {
                let mut result: Vec<Notification> = items
                    .iter()
                    .filter(|item| !unread_only || !item.is_read)
                    .cloned()
                    .collect();
                result.sort_by_key(|item| item.id);
                result.reverse();
                result.truncate(max_items);
                result
            })
            .unwrap_or_default()
    }

    pub fn unread_count(&mut self, user_id: i64) -> usize {
        self.ensure_seed(user_id);
        self.by_user
            .get(&user_id)
            .map(|items| items.iter().filter(|item| !item.is_read).count())
            .unwrap_or(0)
    }

    pub fn create_notification(&mut self, user_id: i64, input: NewNotification) -> Notification {
        self.ensure_seed(user_id);
        self.next_id += 1;
        let notification = Notification {
            id: self.next_id,
            user_id,
            title: input.title,
            message: input.message,
            category: input.category,
            priority: input.priority,
            is_read: false,
            created_at: now_timestamp(),
            read_at: None,
        };
        self.by_user
            .entry(user_id)
            .or_default()
            .push(notification.clone());
        notification
    }

    pub fn mark_read(&mut self, user_id: i64, notification_id: i64) -> bool {
        self.ensure_seed(user_id);
        if let Some(items) = self.by_user.get_mut(&user_id) {
            for item in items.iter_mut() {
                if item.id == notification_id {
                    if !item.is_read {
                        item.is_read = true;
                        item.read_at = Some(now_timestamp());
                    }
                    return true;
                }
            }
        }
        false
    }

    pub fn mark_all_read(&mut self, user_id: i64) -> usize {
        self.ensure_seed(user_id);
        let mut updated = 0usize;
        if let Some(items) = self.by_user.get_mut(&user_id) {
            for item in items.iter_mut().filter(|entry| !entry.is_read) {
                item.is_read = true;
                item.read_at = Some(now_timestamp());
                updated += 1;
            }
        }
        updated
    }

    // -- new batch operations -----------------------------------------------

    /// Delete a single notification.  Returns `true` if found and removed.
    pub fn delete_notification(&mut self, user_id: i64, notification_id: i64) -> bool {
        if let Some(items) = self.by_user.get_mut(&user_id) {
            let before = items.len();
            items.retain(|n| n.id != notification_id);
            return items.len() < before;
        }
        false
    }

    /// Delete all read notifications for a user.  Returns the number removed.
    pub fn delete_all_read(&mut self, user_id: i64) -> usize {
        if let Some(items) = self.by_user.get_mut(&user_id) {
            let before = items.len();
            items.retain(|n| !n.is_read);
            return before - items.len();
        }
        0
    }

    /// List notifications for a user filtered by category.
    pub fn get_by_category(
        &mut self,
        user_id: i64,
        category: &NotificationCategory,
        limit: usize,
    ) -> Vec<Notification> {
        self.ensure_seed(user_id);
        let cat_str = category.as_str();
        let max_items = limit.max(1);
        self.by_user
            .get(&user_id)
            .map(|items| {
                let mut result: Vec<Notification> = items
                    .iter()
                    .filter(|n| n.category == cat_str)
                    .cloned()
                    .collect();
                result.sort_by_key(|item| item.id);
                result.reverse();
                result.truncate(max_items);
                result
            })
            .unwrap_or_default()
    }

    /// Count notifications per category for a user.
    pub fn count_by_category(&mut self, user_id: i64) -> HashMap<NotificationCategory, usize> {
        self.ensure_seed(user_id);
        let mut counts: HashMap<NotificationCategory, usize> = HashMap::new();
        if let Some(items) = self.by_user.get(&user_id) {
            for item in items {
                let cat = NotificationCategory::from_str_lossy(&item.category);
                *counts.entry(cat).or_insert(0) += 1;
            }
        }
        counts
    }

    // -- seed data ----------------------------------------------------------

    fn ensure_seed(&mut self, user_id: i64) {
        if self.by_user.contains_key(&user_id) {
            return;
        }
        self.next_id = self.next_id.max(2);
        self.by_user.insert(
            user_id,
            vec![
                Notification {
                    id: 1,
                    user_id,
                    title: "Fleet Arrived".to_string(),
                    message: "Your expedition returned safely.".to_string(),
                    category: "fleet".to_string(),
                    priority: 2,
                    is_read: false,
                    created_at: "2026-02-14T00:00:00Z".to_string(),
                    read_at: None,
                },
                Notification {
                    id: 2,
                    user_id,
                    title: "Research Complete".to_string(),
                    message: "Energy Technology reached the next level.".to_string(),
                    category: "research".to_string(),
                    priority: 1,
                    is_read: true,
                    created_at: "2026-02-13T23:30:00Z".to_string(),
                    read_at: Some("2026-02-13T23:45:00Z".to_string()),
                },
            ],
        );
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn now_timestamp() -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("unix:{ts}")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // ===== NotificationCategory ============================================

    #[test]
    fn category_from_str_known() {
        assert_eq!(
            NotificationCategory::from_str_lossy("fleet"),
            NotificationCategory::Fleet
        );
        assert_eq!(
            NotificationCategory::from_str_lossy("combat"),
            NotificationCategory::Combat
        );
        assert_eq!(
            NotificationCategory::from_str_lossy("research"),
            NotificationCategory::Research
        );
        assert_eq!(
            NotificationCategory::from_str_lossy("building"),
            NotificationCategory::Building
        );
        assert_eq!(
            NotificationCategory::from_str_lossy("alliance"),
            NotificationCategory::Alliance
        );
        assert_eq!(
            NotificationCategory::from_str_lossy("trade"),
            NotificationCategory::Trade
        );
        assert_eq!(
            NotificationCategory::from_str_lossy("system"),
            NotificationCategory::System
        );
        assert_eq!(
            NotificationCategory::from_str_lossy("achievement"),
            NotificationCategory::Achievement
        );
        assert_eq!(
            NotificationCategory::from_str_lossy("espionage"),
            NotificationCategory::Espionage
        );
    }

    #[test]
    fn category_from_str_unknown_defaults_system() {
        assert_eq!(
            NotificationCategory::from_str_lossy("xyz"),
            NotificationCategory::System
        );
    }

    #[test]
    fn category_case_insensitive() {
        assert_eq!(
            NotificationCategory::from_str_lossy("Fleet"),
            NotificationCategory::Fleet
        );
        assert_eq!(
            NotificationCategory::from_str_lossy("COMBAT"),
            NotificationCategory::Combat
        );
    }

    #[test]
    fn category_display() {
        assert_eq!(NotificationCategory::Fleet.to_string(), "fleet");
        assert_eq!(NotificationCategory::Espionage.to_string(), "espionage");
    }

    #[test]
    fn category_serde_roundtrip() {
        let cat = NotificationCategory::Research;
        let json = serde_json::to_string(&cat).unwrap();
        assert_eq!(json, "\"research\"");
        let back: NotificationCategory = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cat);
    }

    #[test]
    fn category_from_string_impl() {
        let cat: NotificationCategory = "alliance".to_string().into();
        assert_eq!(cat, NotificationCategory::Alliance);
    }

    // ===== NotificationPriority ============================================

    #[test]
    fn priority_from_u8_low() {
        assert_eq!(NotificationPriority::from_u8(0), NotificationPriority::Low);
        assert_eq!(NotificationPriority::from_u8(1), NotificationPriority::Low);
    }

    #[test]
    fn priority_from_u8_medium() {
        assert_eq!(
            NotificationPriority::from_u8(2),
            NotificationPriority::Medium
        );
        assert_eq!(
            NotificationPriority::from_u8(3),
            NotificationPriority::Medium
        );
    }

    #[test]
    fn priority_from_u8_high() {
        assert_eq!(NotificationPriority::from_u8(4), NotificationPriority::High);
        assert_eq!(NotificationPriority::from_u8(5), NotificationPriority::High);
    }

    #[test]
    fn priority_from_u8_critical() {
        assert_eq!(
            NotificationPriority::from_u8(6),
            NotificationPriority::Critical
        );
        assert_eq!(
            NotificationPriority::from_u8(255),
            NotificationPriority::Critical
        );
    }

    #[test]
    fn priority_ordering() {
        assert!(NotificationPriority::Low < NotificationPriority::Medium);
        assert!(NotificationPriority::Medium < NotificationPriority::High);
        assert!(NotificationPriority::High < NotificationPriority::Critical);
    }

    #[test]
    fn priority_serde_roundtrip() {
        let pri = NotificationPriority::High;
        let json = serde_json::to_string(&pri).unwrap();
        assert_eq!(json, "\"high\"");
        let back: NotificationPriority = serde_json::from_str(&json).unwrap();
        assert_eq!(back, pri);
    }

    #[test]
    fn priority_to_u8_roundtrip() {
        let p = NotificationPriority::Medium;
        let n: u8 = p.into();
        assert_eq!(n, 2);
    }

    #[test]
    fn priority_display() {
        assert_eq!(NotificationPriority::Critical.to_string(), "critical");
    }

    // ===== NotificationChannel & ChannelConfig =============================

    #[test]
    fn channel_serde_roundtrip() {
        let ch = NotificationChannel::WebSocket;
        let json = serde_json::to_string(&ch).unwrap();
        assert_eq!(json, "\"web_socket\"");
        let back: NotificationChannel = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ch);
    }

    #[test]
    fn channel_config_serde() {
        let cfg = ChannelConfig {
            channel: NotificationChannel::Email,
            enabled: true,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: ChannelConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cfg);
    }

    // ===== Notification (backwards compat) =================================

    #[test]
    fn notification_category_enum_helper() {
        let n = Notification {
            id: 1,
            user_id: 1,
            title: "T".into(),
            message: "M".into(),
            category: "fleet".into(),
            priority: 4,
            is_read: false,
            created_at: "2026-01-01T00:00:00Z".into(),
            read_at: None,
        };
        assert_eq!(n.category_enum(), NotificationCategory::Fleet);
        assert_eq!(n.priority_enum(), NotificationPriority::High);
    }

    #[test]
    fn notification_serde_camel_case() {
        let n = Notification {
            id: 42,
            user_id: 7,
            title: "Hello".into(),
            message: "World".into(),
            category: "combat".into(),
            priority: 2,
            is_read: false,
            created_at: "2026-01-01T00:00:00Z".into(),
            read_at: None,
        };
        let json = serde_json::to_string(&n).unwrap();
        assert!(json.contains("\"userId\""));
        assert!(json.contains("\"isRead\""));
        assert!(json.contains("\"createdAt\""));
    }

    // ===== NotificationStore original API ==================================

    #[test]
    fn seeded_user_has_unread_count() {
        let mut store = NotificationStore::default();
        assert_eq!(store.unread_count(7), 1);
    }

    #[test]
    fn create_notification_increases_unread_count() {
        let mut store = NotificationStore::default();
        let before = store.unread_count(9);
        let _ = store.create_notification(
            9,
            NewNotification {
                title: "Under Attack".to_string(),
                message: "Enemy fleet detected.".to_string(),
                category: "combat".to_string(),
                priority: 5,
            },
        );
        assert_eq!(store.unread_count(9), before + 1);
    }

    #[test]
    fn mark_read_updates_unread_count() {
        let mut store = NotificationStore::default();
        let listed = store.list_user_notifications(11, true, 50);
        let unread_id = listed.first().expect("seed unread notification").id;
        assert!(store.mark_read(11, unread_id));
        assert_eq!(store.unread_count(11), 0);
    }

    #[test]
    fn mark_read_returns_false_for_unknown_id() {
        let mut store = NotificationStore::default();
        store.ensure_seed(1);
        assert!(!store.mark_read(1, 9999));
    }

    #[test]
    fn mark_all_read_returns_count() {
        let mut store = NotificationStore::default();
        let _ = store.create_notification(
            20,
            NewNotification {
                title: "A".into(),
                message: "B".into(),
                category: "fleet".into(),
                priority: 1,
            },
        );
        // seed has 1 unread + we added 1 = 2 unread
        let updated = store.mark_all_read(20);
        assert_eq!(updated, 2);
        assert_eq!(store.unread_count(20), 0);
    }

    #[test]
    fn list_with_limit() {
        let mut store = NotificationStore::default();
        let all = store.list_user_notifications(1, false, 100);
        assert_eq!(all.len(), 2); // seed data
        let limited = store.list_user_notifications(1, false, 1);
        assert_eq!(limited.len(), 1);
    }

    #[test]
    fn list_unread_only() {
        let mut store = NotificationStore::default();
        let unread = store.list_user_notifications(1, true, 100);
        assert_eq!(unread.len(), 1);
        assert!(!unread[0].is_read);
    }

    #[test]
    fn list_sorted_descending_by_id() {
        let mut store = NotificationStore::default();
        let all = store.list_user_notifications(1, false, 100);
        assert!(all[0].id > all[1].id);
    }

    // ===== Batch operations ================================================

    #[test]
    fn delete_notification_success() {
        let mut store = NotificationStore::default();
        store.ensure_seed(1);
        assert!(store.delete_notification(1, 1));
        let remaining = store.list_user_notifications(1, false, 100);
        assert_eq!(remaining.len(), 1);
    }

    #[test]
    fn delete_notification_not_found() {
        let mut store = NotificationStore::default();
        store.ensure_seed(1);
        assert!(!store.delete_notification(1, 999));
    }

    #[test]
    fn delete_notification_wrong_user() {
        let mut store = NotificationStore::default();
        // user 999 has no data and no seed triggered
        assert!(!store.delete_notification(999, 1));
    }

    #[test]
    fn delete_all_read() {
        let mut store = NotificationStore::default();
        store.ensure_seed(1);
        // seed has 1 read notification (id 2)
        let removed = store.delete_all_read(1);
        assert_eq!(removed, 1);
        let remaining = store.list_user_notifications(1, false, 100);
        assert_eq!(remaining.len(), 1);
        assert!(!remaining[0].is_read);
    }

    #[test]
    fn delete_all_read_none_read() {
        let mut store = NotificationStore::default();
        // create a fresh user with only unread
        store.create_notification(
            50,
            NewNotification {
                title: "T".into(),
                message: "M".into(),
                category: "fleet".into(),
                priority: 1,
            },
        );
        // seed has 1 read, we added 1 unread
        let removed = store.delete_all_read(50);
        assert_eq!(removed, 1); // just the seed read one
    }

    #[test]
    fn get_by_category() {
        let mut store = NotificationStore::default();
        store.ensure_seed(1);
        let fleet = store.get_by_category(1, &NotificationCategory::Fleet, 100);
        assert_eq!(fleet.len(), 1);
        assert_eq!(fleet[0].category, "fleet");

        let research = store.get_by_category(1, &NotificationCategory::Research, 100);
        assert_eq!(research.len(), 1);
    }

    #[test]
    fn get_by_category_empty() {
        let mut store = NotificationStore::default();
        let combat = store.get_by_category(1, &NotificationCategory::Combat, 100);
        assert!(combat.is_empty());
    }

    #[test]
    fn get_by_category_with_limit() {
        let mut store = NotificationStore::default();
        for i in 0..5 {
            store.create_notification(
                30,
                NewNotification {
                    title: format!("Fleet {i}"),
                    message: "msg".into(),
                    category: "fleet".into(),
                    priority: 1,
                },
            );
        }
        // seed has 1 fleet + 5 created = 6
        let fleet = store.get_by_category(30, &NotificationCategory::Fleet, 3);
        assert_eq!(fleet.len(), 3);
    }

    #[test]
    fn count_by_category() {
        let mut store = NotificationStore::default();
        store.ensure_seed(1);
        let counts = store.count_by_category(1);
        assert_eq!(*counts.get(&NotificationCategory::Fleet).unwrap_or(&0), 1);
        assert_eq!(
            *counts.get(&NotificationCategory::Research).unwrap_or(&0),
            1
        );
        assert_eq!(*counts.get(&NotificationCategory::Combat).unwrap_or(&0), 0);
    }

    // ===== Preferences Store ===============================================

    #[test]
    fn default_preferences() {
        let store = PreferencesStore::default();
        let prefs = store.get_preferences(1);
        assert_eq!(prefs.user_id, 1);
        assert!(prefs.muted_categories.is_empty());
        assert_eq!(prefs.min_priority, NotificationPriority::Low);
    }

    #[test]
    fn set_and_get_preferences() {
        let mut store = PreferencesStore::default();
        let mut prefs = NotificationPreferences::default_for(5);
        prefs.muted_categories.push(NotificationCategory::Trade);
        prefs.min_priority = NotificationPriority::High;
        store.set_preferences(prefs);

        let loaded = store.get_preferences(5);
        assert_eq!(loaded.muted_categories, vec![NotificationCategory::Trade]);
        assert_eq!(loaded.min_priority, NotificationPriority::High);
    }

    #[test]
    fn should_deliver_allows_default() {
        let store = PreferencesStore::default();
        assert!(store.should_deliver(1, &NotificationCategory::Fleet, &NotificationPriority::Low,));
    }

    #[test]
    fn should_deliver_blocked_by_muted_category() {
        let mut store = PreferencesStore::default();
        let mut prefs = NotificationPreferences::default_for(1);
        prefs.muted_categories.push(NotificationCategory::Fleet);
        store.set_preferences(prefs);

        assert!(!store.should_deliver(
            1,
            &NotificationCategory::Fleet,
            &NotificationPriority::Critical,
        ));
    }

    #[test]
    fn should_deliver_blocked_by_min_priority() {
        let mut store = PreferencesStore::default();
        let mut prefs = NotificationPreferences::default_for(1);
        prefs.min_priority = NotificationPriority::High;
        store.set_preferences(prefs);

        assert!(!store.should_deliver(
            1,
            &NotificationCategory::Fleet,
            &NotificationPriority::Medium,
        ));
        assert!(store.should_deliver(1, &NotificationCategory::Fleet, &NotificationPriority::High,));
    }

    #[test]
    fn should_deliver_quiet_hours_simple() {
        let mut store = PreferencesStore::default();
        let mut prefs = NotificationPreferences::default_for(1);
        prefs.quiet_hours_start = Some(22);
        prefs.quiet_hours_end = Some(6);
        store.set_preferences(prefs);

        // at 23:00 -> in quiet hours
        assert!(!store.should_deliver_at(
            1,
            &NotificationCategory::Fleet,
            &NotificationPriority::Low,
            Some(23),
        ));

        // at 3:00 -> in quiet hours (wraps midnight)
        assert!(!store.should_deliver_at(
            1,
            &NotificationCategory::Fleet,
            &NotificationPriority::Low,
            Some(3),
        ));

        // at 12:00 -> outside quiet hours
        assert!(store.should_deliver_at(
            1,
            &NotificationCategory::Fleet,
            &NotificationPriority::Low,
            Some(12),
        ));
    }

    #[test]
    fn should_deliver_quiet_hours_no_wrap() {
        let mut store = PreferencesStore::default();
        let mut prefs = NotificationPreferences::default_for(1);
        prefs.quiet_hours_start = Some(9);
        prefs.quiet_hours_end = Some(17);
        store.set_preferences(prefs);

        assert!(!store.should_deliver_at(
            1,
            &NotificationCategory::Fleet,
            &NotificationPriority::Low,
            Some(12),
        ));
        assert!(store.should_deliver_at(
            1,
            &NotificationCategory::Fleet,
            &NotificationPriority::Low,
            Some(18),
        ));
    }

    #[test]
    fn should_deliver_no_current_hour_skips_quiet() {
        let mut store = PreferencesStore::default();
        let mut prefs = NotificationPreferences::default_for(1);
        prefs.quiet_hours_start = Some(0);
        prefs.quiet_hours_end = Some(23);
        store.set_preferences(prefs);

        // without current_hour, quiet hours are ignored
        assert!(store.should_deliver(1, &NotificationCategory::Fleet, &NotificationPriority::Low,));
    }

    // ===== Delivery Store ==================================================

    #[test]
    fn delivery_record_attempt() {
        let mut store = DeliveryStore::default();
        store.record_attempt(1, NotificationChannel::InApp);
        let pending = store.pending_deliveries();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].notification_id, 1);
        assert_eq!(pending[0].status, DeliveryStatus::Pending);
    }

    #[test]
    fn delivery_mark_delivered() {
        let mut store = DeliveryStore::default();
        store.record_attempt(1, NotificationChannel::InApp);
        assert!(store.mark_delivered(1, &NotificationChannel::InApp));
        assert!(store.pending_deliveries().is_empty());

        let records = store.get_delivery_status(1);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].status, DeliveryStatus::Delivered);
        assert!(records[0].delivered_at.is_some());
    }

    #[test]
    fn delivery_mark_failed() {
        let mut store = DeliveryStore::default();
        store.record_attempt(1, NotificationChannel::Push);
        assert!(store.mark_failed(1, &NotificationChannel::Push, "timeout".into()));

        let records = store.get_delivery_status(1);
        assert_eq!(records[0].status, DeliveryStatus::Failed);
        assert_eq!(records[0].error.as_deref(), Some("timeout"));
    }

    #[test]
    fn delivery_mark_nonexistent_returns_false() {
        let mut store = DeliveryStore::default();
        assert!(!store.mark_delivered(999, &NotificationChannel::InApp));
        assert!(!store.mark_failed(999, &NotificationChannel::InApp, "err".into()));
    }

    #[test]
    fn delivery_multiple_channels() {
        let mut store = DeliveryStore::default();
        store.record_attempt(1, NotificationChannel::InApp);
        store.record_attempt(1, NotificationChannel::Email);
        store.mark_delivered(1, &NotificationChannel::InApp);

        let pending = store.pending_deliveries();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].channel, NotificationChannel::Email);

        let all = store.get_delivery_status(1);
        assert_eq!(all.len(), 2);
    }

    // ===== Template Store ==================================================

    #[test]
    fn template_store_has_defaults() {
        let store = TemplateStore::default();
        assert!(store.get_template("fleet_arrived").is_some());
        assert!(store.get_template("combat_report").is_some());
        assert!(store.get_template("research_complete").is_some());
        assert!(store.get_template("building_complete").is_some());
        assert!(store.get_template("under_attack").is_some());
        assert!(store.get_template("alliance_invite").is_some());
    }

    #[test]
    fn template_render_fleet_arrived() {
        let store = TemplateStore::default();
        let mut params = HashMap::new();
        params.insert("destination".to_string(), "Planet [2:45:7]".to_string());
        let (title, message) = store.render("fleet_arrived", &params).unwrap();
        assert_eq!(title, "Fleet Arrived");
        assert_eq!(message, "Your fleet has arrived at Planet [2:45:7].");
    }

    #[test]
    fn template_render_combat_report() {
        let store = TemplateStore::default();
        let mut params = HashMap::new();
        params.insert("location".to_string(), "[1:200:3]".to_string());
        params.insert("outcome".to_string(), "Victory".to_string());
        let (_, message) = store.render("combat_report", &params).unwrap();
        assert_eq!(message, "Battle at [1:200:3]: Victory.");
    }

    #[test]
    fn template_render_research_complete() {
        let store = TemplateStore::default();
        let mut params = HashMap::new();
        params.insert("technology".to_string(), "Laser Technology".to_string());
        params.insert("level".to_string(), "5".to_string());
        let (title, message) = store.render("research_complete", &params).unwrap();
        assert_eq!(title, "Research Complete");
        assert_eq!(message, "Laser Technology has reached level 5.");
    }

    #[test]
    fn template_render_building_complete() {
        let store = TemplateStore::default();
        let mut params = HashMap::new();
        params.insert("building".to_string(), "Metal Mine".to_string());
        params.insert("level".to_string(), "12".to_string());
        let (_, message) = store.render("building_complete", &params).unwrap();
        assert_eq!(message, "Metal Mine has been upgraded to level 12.");
    }

    #[test]
    fn template_render_under_attack() {
        let store = TemplateStore::default();
        let mut params = HashMap::new();
        params.insert("location".to_string(), "[3:100:5]".to_string());
        params.insert("attacker".to_string(), "DarkFleet".to_string());
        let (title, msg) = store.render("under_attack", &params).unwrap();
        assert_eq!(title, "Under Attack!");
        assert!(msg.contains("DarkFleet"));
    }

    #[test]
    fn template_render_alliance_invite() {
        let store = TemplateStore::default();
        let mut params = HashMap::new();
        params.insert("alliance".to_string(), "StarForge".to_string());
        let (_, msg) = store.render("alliance_invite", &params).unwrap();
        assert_eq!(msg, "You have been invited to join StarForge.");
    }

    #[test]
    fn template_render_unknown_returns_none() {
        let store = TemplateStore::default();
        let params = HashMap::new();
        assert!(store.render("nonexistent", &params).is_none());
    }

    #[test]
    fn template_add_custom() {
        let mut store = TemplateStore::default();
        store.add_template(NotificationTemplate {
            id: "custom_event".to_string(),
            category: NotificationCategory::System,
            title_template: "Custom: {name}".to_string(),
            message_template: "Event {name} occurred at {time}.".to_string(),
        });
        let mut params = HashMap::new();
        params.insert("name".to_string(), "Maintenance".to_string());
        params.insert("time".to_string(), "03:00 UTC".to_string());
        let (title, msg) = store.render("custom_event", &params).unwrap();
        assert_eq!(title, "Custom: Maintenance");
        assert_eq!(msg, "Event Maintenance occurred at 03:00 UTC.");
    }

    #[test]
    fn template_render_missing_placeholder_left_as_is() {
        let store = TemplateStore::default();
        let params = HashMap::new(); // no params at all
        let (_, msg) = store.render("fleet_arrived", &params).unwrap();
        assert_eq!(msg, "Your fleet has arrived at {destination}.");
    }

    #[test]
    fn template_categories_correct() {
        let store = TemplateStore::default();
        assert_eq!(
            store.get_template("fleet_arrived").unwrap().category,
            NotificationCategory::Fleet
        );
        assert_eq!(
            store.get_template("combat_report").unwrap().category,
            NotificationCategory::Combat
        );
        assert_eq!(
            store.get_template("research_complete").unwrap().category,
            NotificationCategory::Research
        );
        assert_eq!(
            store.get_template("building_complete").unwrap().category,
            NotificationCategory::Building
        );
        assert_eq!(
            store.get_template("under_attack").unwrap().category,
            NotificationCategory::Combat
        );
        assert_eq!(
            store.get_template("alliance_invite").unwrap().category,
            NotificationCategory::Alliance
        );
    }

    // ===== Integration / cross-module ======================================

    #[test]
    fn new_notification_serde_roundtrip() {
        let input = NewNotification {
            title: "Test".into(),
            message: "Hello".into(),
            category: "fleet".into(),
            priority: 3,
        };
        let json = serde_json::to_string(&input).unwrap();
        let back: NewNotification = serde_json::from_str(&json).unwrap();
        assert_eq!(back.title, "Test");
        assert_eq!(back.priority, 3);
    }

    #[test]
    fn notification_json_category_stays_string() {
        let mut store = NotificationStore::default();
        let n = store.create_notification(
            1,
            NewNotification {
                title: "T".into(),
                message: "M".into(),
                category: "espionage".into(),
                priority: 6,
            },
        );
        let json = serde_json::to_string(&n).unwrap();
        // category must appear as a plain string, not as an enum wrapper
        assert!(json.contains("\"espionage\""));
    }

    #[test]
    fn preferences_default_channels() {
        let prefs = NotificationPreferences::default_for(1);
        assert_eq!(prefs.channels.len(), 4);
        let in_app = prefs
            .channels
            .iter()
            .find(|c| c.channel == NotificationChannel::InApp)
            .unwrap();
        assert!(in_app.enabled);
        let email = prefs
            .channels
            .iter()
            .find(|c| c.channel == NotificationChannel::Email)
            .unwrap();
        assert!(!email.enabled);
    }

    #[test]
    fn delivery_status_serde() {
        let s = DeliveryStatus::Pending;
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, "\"pending\"");
    }

    #[test]
    fn delivery_record_serde() {
        let rec = DeliveryRecord {
            notification_id: 1,
            channel: NotificationChannel::Push,
            status: DeliveryStatus::Delivered,
            attempted_at: "2026-01-01T00:00:00Z".into(),
            delivered_at: Some("2026-01-01T00:00:01Z".into()),
            error: None,
        };
        let json = serde_json::to_string(&rec).unwrap();
        let back: DeliveryRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(back.notification_id, 1);
        assert_eq!(back.status, DeliveryStatus::Delivered);
    }

    #[test]
    fn preferences_serde_roundtrip() {
        let prefs = NotificationPreferences {
            user_id: 42,
            muted_categories: vec![NotificationCategory::Trade, NotificationCategory::Alliance],
            min_priority: NotificationPriority::Medium,
            channels: vec![ChannelConfig {
                channel: NotificationChannel::InApp,
                enabled: true,
            }],
            quiet_hours_start: Some(22),
            quiet_hours_end: Some(7),
        };
        let json = serde_json::to_string(&prefs).unwrap();
        let back: NotificationPreferences = serde_json::from_str(&json).unwrap();
        assert_eq!(back.user_id, 42);
        assert_eq!(back.muted_categories.len(), 2);
        assert_eq!(back.quiet_hours_start, Some(22));
    }

    #[test]
    fn count_by_category_after_creates() {
        let mut store = NotificationStore::default();
        for _ in 0..3 {
            store.create_notification(
                70,
                NewNotification {
                    title: "T".into(),
                    message: "M".into(),
                    category: "combat".into(),
                    priority: 5,
                },
            );
        }
        let counts = store.count_by_category(70);
        // seed: 1 fleet + 1 research + 3 combat
        assert_eq!(*counts.get(&NotificationCategory::Combat).unwrap(), 3);
        assert_eq!(*counts.get(&NotificationCategory::Fleet).unwrap(), 1);
        assert_eq!(*counts.get(&NotificationCategory::Research).unwrap(), 1);
    }
}
