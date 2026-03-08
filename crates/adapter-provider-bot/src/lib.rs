#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BotConfig {
    pub bot_id: String,
    pub name: String,
    pub token: String,
    pub webhook_url: Option<String>,
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct BotEvent {
    pub event_type: String,
    pub payload: String,
    pub timestamp_unix: i64,
}

/// Placeholder bot provider adapter.
pub struct BotProviderAdapter {
    bots: HashMap<String, BotConfig>,
    events: HashMap<String, Vec<BotEvent>>,
}

impl BotProviderAdapter {
    pub fn new() -> Self {
        Self {
            bots: HashMap::new(),
            events: HashMap::new(),
        }
    }

    /// Registers a bot. Returns `false` if `bot_id` already exists.
    pub fn register_bot(&mut self, config: BotConfig) -> bool {
        if self.bots.contains_key(&config.bot_id) {
            return false;
        }
        self.bots.insert(config.bot_id.clone(), config);
        true
    }

    /// Unregisters a bot by id. Returns `false` if not found.
    pub fn unregister_bot(&mut self, bot_id: &str) -> bool {
        self.bots.remove(bot_id).is_some()
    }

    pub fn get_bot(&self, bot_id: &str) -> Option<BotConfig> {
        self.bots.get(bot_id).cloned()
    }

    pub fn list_bots(&self) -> Vec<BotConfig> {
        self.bots.values().cloned().collect()
    }

    /// Sets or clears the webhook URL for a bot. Returns `false` if bot not found.
    pub fn set_webhook(&mut self, bot_id: &str, url: Option<String>) -> bool {
        match self.bots.get_mut(bot_id) {
            Some(bot) => {
                bot.webhook_url = url;
                true
            }
            None => false,
        }
    }

    pub fn log_event(&mut self, bot_id: &str, event_type: &str, payload: &str) {
        let event = BotEvent {
            event_type: event_type.to_string(),
            payload: payload.to_string(),
            timestamp_unix: 0, // deterministic for testing; real impl would use system time
        };
        self.events
            .entry(bot_id.to_string())
            .or_default()
            .push(event);
    }

    pub fn get_events(&self, bot_id: &str, limit: usize) -> Vec<BotEvent> {
        match self.events.get(bot_id) {
            Some(events) => events.iter().rev().take(limit).cloned().collect(),
            None => Vec::new(),
        }
    }
}

impl Default for BotProviderAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config(id: &str) -> BotConfig {
        BotConfig {
            bot_id: id.to_string(),
            name: format!("Bot {id}"),
            token: "tok_secret".to_string(),
            webhook_url: None,
            enabled: true,
        }
    }

    #[test]
    fn test_register_and_get_bot() {
        let mut adapter = BotProviderAdapter::new();
        assert!(adapter.register_bot(sample_config("b1")));
        let bot = adapter.get_bot("b1").unwrap();
        assert_eq!(bot.name, "Bot b1");
    }

    #[test]
    fn test_duplicate_registration_fails() {
        let mut adapter = BotProviderAdapter::new();
        assert!(adapter.register_bot(sample_config("b1")));
        assert!(!adapter.register_bot(sample_config("b1")));
    }

    #[test]
    fn test_unregister_bot() {
        let mut adapter = BotProviderAdapter::new();
        adapter.register_bot(sample_config("b1"));
        assert!(adapter.unregister_bot("b1"));
        assert!(!adapter.unregister_bot("b1"));
        assert!(adapter.get_bot("b1").is_none());
    }

    #[test]
    fn test_list_bots() {
        let mut adapter = BotProviderAdapter::new();
        adapter.register_bot(sample_config("b1"));
        adapter.register_bot(sample_config("b2"));
        assert_eq!(adapter.list_bots().len(), 2);
    }

    #[test]
    fn test_set_webhook() {
        let mut adapter = BotProviderAdapter::new();
        adapter.register_bot(sample_config("b1"));
        assert!(adapter.set_webhook("b1", Some("https://example.com".to_string())));
        assert_eq!(
            adapter.get_bot("b1").unwrap().webhook_url.as_deref(),
            Some("https://example.com")
        );
        assert!(!adapter.set_webhook("missing", None));
    }

    #[test]
    fn test_log_and_get_events() {
        let mut adapter = BotProviderAdapter::new();
        adapter.log_event("b1", "message", "hello");
        adapter.log_event("b1", "command", "/start");
        adapter.log_event("b1", "callback", "btn_1");
        let events = adapter.get_events("b1", 2);
        assert_eq!(events.len(), 2);
        // Most recent first
        assert_eq!(events[0].event_type, "callback");
    }

    #[test]
    fn test_get_events_empty() {
        let adapter = BotProviderAdapter::new();
        assert!(adapter.get_events("nonexistent", 10).is_empty());
    }
}
