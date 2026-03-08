#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

pub fn crate_name() -> &'static str {
    "game-messaging"
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: i64,
    pub sender_id: i64,
    pub recipient_id: i64,
    pub subject: String,
    pub body: String,
    pub is_read: bool,
    pub created_at: String,
    pub read_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewMessage {
    pub sender_id: i64,
    pub recipient_id: i64,
    pub subject: String,
    pub body: String,
}

#[derive(Default)]
pub struct MessageStore {
    next_id: i64,
    messages: Vec<Message>,
}

impl MessageStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn send_message(&mut self, input: NewMessage) -> Message {
        self.next_id += 1;
        let message = Message {
            id: self.next_id,
            sender_id: input.sender_id,
            recipient_id: input.recipient_id,
            subject: input.subject,
            body: input.body,
            is_read: false,
            created_at: now_timestamp(),
            read_at: None,
        };
        self.messages.push(message.clone());
        message
    }

    pub fn get_inbox(&self, user_id: i64, unread_only: bool, limit: usize) -> Vec<Message> {
        let max_items = limit.max(1);
        let mut result: Vec<Message> = self
            .messages
            .iter()
            .filter(|m| m.recipient_id == user_id && (!unread_only || !m.is_read))
            .cloned()
            .collect();
        result.sort_by(|a, b| b.id.cmp(&a.id));
        result.truncate(max_items);
        result
    }

    pub fn get_outbox(&self, user_id: i64, limit: usize) -> Vec<Message> {
        let max_items = limit.max(1);
        let mut result: Vec<Message> = self
            .messages
            .iter()
            .filter(|m| m.sender_id == user_id)
            .cloned()
            .collect();
        result.sort_by(|a, b| b.id.cmp(&a.id));
        result.truncate(max_items);
        result
    }

    pub fn get_message(&self, message_id: i64, user_id: i64) -> Option<Message> {
        self.messages
            .iter()
            .find(|m| m.id == message_id && (m.sender_id == user_id || m.recipient_id == user_id))
            .cloned()
    }

    pub fn mark_read(&mut self, message_id: i64, user_id: i64) -> bool {
        for m in self.messages.iter_mut() {
            if m.id == message_id && m.recipient_id == user_id {
                if !m.is_read {
                    m.is_read = true;
                    m.read_at = Some(now_timestamp());
                }
                return true;
            }
        }
        false
    }

    pub fn delete_message(&mut self, message_id: i64, user_id: i64) -> bool {
        let len_before = self.messages.len();
        self.messages
            .retain(|m| !(m.id == message_id && (m.sender_id == user_id || m.recipient_id == user_id)));
        self.messages.len() < len_before
    }

    pub fn unread_count(&self, user_id: i64) -> usize {
        self.messages
            .iter()
            .filter(|m| m.recipient_id == user_id && !m.is_read)
            .count()
    }
}

fn now_timestamp() -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("unix:{ts}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_store_with_messages() -> MessageStore {
        let mut store = MessageStore::new();
        store.send_message(NewMessage {
            sender_id: 1,
            recipient_id: 2,
            subject: "Hello".to_string(),
            body: "Hi there!".to_string(),
        });
        store.send_message(NewMessage {
            sender_id: 1,
            recipient_id: 2,
            subject: "Follow-up".to_string(),
            body: "Any update?".to_string(),
        });
        store.send_message(NewMessage {
            sender_id: 2,
            recipient_id: 1,
            subject: "Reply".to_string(),
            body: "Got it!".to_string(),
        });
        store
    }

    #[test]
    fn crate_name_returns_expected() {
        assert_eq!(crate_name(), "game-messaging");
    }

    #[test]
    fn send_message_assigns_incremental_ids() {
        let mut store = MessageStore::new();
        let m1 = store.send_message(NewMessage {
            sender_id: 1,
            recipient_id: 2,
            subject: "First".to_string(),
            body: "Body 1".to_string(),
        });
        let m2 = store.send_message(NewMessage {
            sender_id: 1,
            recipient_id: 2,
            subject: "Second".to_string(),
            body: "Body 2".to_string(),
        });
        assert_eq!(m1.id, 1);
        assert_eq!(m2.id, 2);
        assert!(!m1.is_read);
        assert!(m1.read_at.is_none());
    }

    #[test]
    fn get_inbox_returns_recipient_messages_sorted_desc() {
        let store = make_store_with_messages();
        let inbox = store.get_inbox(2, false, 50);
        assert_eq!(inbox.len(), 2);
        assert!(inbox[0].id > inbox[1].id);
        assert_eq!(inbox[0].subject, "Follow-up");
    }

    #[test]
    fn get_inbox_unread_only_filters_read_messages() {
        let mut store = make_store_with_messages();
        store.mark_read(1, 2);
        let inbox = store.get_inbox(2, true, 50);
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].id, 2);
    }

    #[test]
    fn get_inbox_respects_limit() {
        let store = make_store_with_messages();
        let inbox = store.get_inbox(2, false, 1);
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].id, 2);
    }

    #[test]
    fn get_outbox_returns_sender_messages_sorted_desc() {
        let store = make_store_with_messages();
        let outbox = store.get_outbox(1, 50);
        assert_eq!(outbox.len(), 2);
        assert!(outbox[0].id > outbox[1].id);
    }

    #[test]
    fn get_message_returns_for_sender_or_recipient() {
        let store = make_store_with_messages();
        assert!(store.get_message(1, 1).is_some()); // sender
        assert!(store.get_message(1, 2).is_some()); // recipient
        assert!(store.get_message(1, 99).is_none()); // unrelated user
    }

    #[test]
    fn mark_read_sets_read_flag_for_recipient_only() {
        let mut store = make_store_with_messages();
        assert!(!store.mark_read(1, 1)); // sender cannot mark read
        assert!(store.mark_read(1, 2)); // recipient can
        let msg = store.get_message(1, 2).unwrap();
        assert!(msg.is_read);
        assert!(msg.read_at.is_some());
    }

    #[test]
    fn mark_read_is_idempotent() {
        let mut store = make_store_with_messages();
        assert!(store.mark_read(1, 2));
        let first_read_at = store.get_message(1, 2).unwrap().read_at.clone();
        assert!(store.mark_read(1, 2));
        let second_read_at = store.get_message(1, 2).unwrap().read_at;
        assert_eq!(first_read_at, second_read_at);
    }

    #[test]
    fn delete_message_removes_for_authorized_user() {
        let mut store = make_store_with_messages();
        assert!(!store.delete_message(1, 99)); // unrelated user
        assert!(store.delete_message(1, 1)); // sender deletes
        assert!(store.get_message(1, 1).is_none());
        assert!(store.get_message(1, 2).is_none());
    }

    #[test]
    fn unread_count_tracks_correctly() {
        let mut store = make_store_with_messages();
        assert_eq!(store.unread_count(2), 2);
        store.mark_read(1, 2);
        assert_eq!(store.unread_count(2), 1);
        assert_eq!(store.unread_count(1), 1); // message 3 is unread for user 1
    }

    #[test]
    fn delete_reduces_unread_count() {
        let mut store = make_store_with_messages();
        assert_eq!(store.unread_count(2), 2);
        store.delete_message(1, 2);
        assert_eq!(store.unread_count(2), 1);
    }
}
