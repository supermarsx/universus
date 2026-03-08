#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// MessageType
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    Private,
    Alliance,
    CombatReport,
    EspionageReport,
    ExpeditionLog,
    MissileAttack,
    Transport,
    System,
}

impl fmt::Display for MessageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            MessageType::Private => "Private",
            MessageType::Alliance => "Alliance",
            MessageType::CombatReport => "CombatReport",
            MessageType::EspionageReport => "EspionageReport",
            MessageType::ExpeditionLog => "ExpeditionLog",
            MessageType::MissileAttack => "MissileAttack",
            MessageType::Transport => "Transport",
            MessageType::System => "System",
        };
        write!(f, "{s}")
    }
}

impl FromStr for MessageType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Private" => Ok(MessageType::Private),
            "Alliance" => Ok(MessageType::Alliance),
            "CombatReport" => Ok(MessageType::CombatReport),
            "EspionageReport" => Ok(MessageType::EspionageReport),
            "ExpeditionLog" => Ok(MessageType::ExpeditionLog),
            "MissileAttack" => Ok(MessageType::MissileAttack),
            "Transport" => Ok(MessageType::Transport),
            "System" => Ok(MessageType::System),
            other => Err(format!("unknown message type: {other}")),
        }
    }
}

/// All eight variants for iteration in tests / bulk operations.
#[cfg(test)]
const ALL_MESSAGE_TYPES: [MessageType; 8] = [
    MessageType::Private,
    MessageType::Alliance,
    MessageType::CombatReport,
    MessageType::EspionageReport,
    MessageType::ExpeditionLog,
    MessageType::MissileAttack,
    MessageType::Transport,
    MessageType::System,
];

// ---------------------------------------------------------------------------
// MessageError
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageError {
    NotFound,
    Unauthorized,
    EmptySubject,
    EmptyBody,
    RecipientRequired,
    SelfMessage,
}

impl fmt::Display for MessageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            MessageError::NotFound => "message not found",
            MessageError::Unauthorized => "unauthorized access to message",
            MessageError::EmptySubject => "subject must not be empty",
            MessageError::EmptyBody => "body must not be empty",
            MessageError::RecipientRequired => "recipient is required",
            MessageError::SelfMessage => "cannot send a message to yourself",
        };
        write!(f, "{s}")
    }
}

// ---------------------------------------------------------------------------
// Message
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: i64,
    pub sender_id: Option<i64>,
    pub recipient_id: i64,
    pub message_type: MessageType,
    pub subject: String,
    pub body: String,
    pub is_read: bool,
    pub is_archived: bool,
    pub is_deleted: bool,
    pub metadata: Option<Value>,
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// MessageThread
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MessageThread {
    pub thread_id: String,
    pub participants: Vec<i64>,
    pub subject: String,
    pub message_count: i32,
    pub last_message_at: String,
}

// ---------------------------------------------------------------------------
// SpamGuard
// ---------------------------------------------------------------------------

/// Per-sender rate limiter: max 10 messages per 60-second window.
#[derive(Debug, Clone)]
pub struct SpamGuard {
    /// Maps sender_id -> list of send-timestamps (unix seconds).
    sends: HashMap<i64, Vec<i64>>,
}

const SPAM_WINDOW_SECS: i64 = 60;
const SPAM_MAX_PER_WINDOW: usize = 10;

impl SpamGuard {
    pub fn new() -> Self {
        Self {
            sends: HashMap::new(),
        }
    }

    /// Returns `true` if the sender is allowed to send another message at `now`.
    pub fn can_send(&self, sender_id: i64, now: i64) -> bool {
        match self.sends.get(&sender_id) {
            None => true,
            Some(timestamps) => {
                let recent = timestamps
                    .iter()
                    .filter(|&&ts| ts > now - SPAM_WINDOW_SECS)
                    .count();
                recent < SPAM_MAX_PER_WINDOW
            }
        }
    }

    /// Record that `sender_id` sent a message at `now`.
    pub fn record_send(&mut self, sender_id: i64, now: i64) {
        self.sends.entry(sender_id).or_default().push(now);
    }

    /// Remove all entries older than 1 minute relative to `now`.
    pub fn cleanup_expired(&mut self, now: i64) {
        let cutoff = now - SPAM_WINDOW_SECS;
        self.sends.retain(|_sender, timestamps| {
            timestamps.retain(|&ts| ts > cutoff);
            !timestamps.is_empty()
        });
    }
}

impl Default for SpamGuard {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// MessageStore
// ---------------------------------------------------------------------------

pub struct MessageStore {
    next_id: i64,
    messages: HashMap<i64, Message>,
}

impl MessageStore {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            messages: HashMap::new(),
        }
    }

    // -- helpers ------------------------------------------------------------

    fn alloc_id(&mut self) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn now_timestamp() -> String {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        format!("unix:{ts}")
    }

    // -- public API ---------------------------------------------------------

    /// Send a message after validating inputs. Returns the new message id.
    pub fn send_message(
        &mut self,
        sender_id: Option<i64>,
        recipient_id: i64,
        message_type: MessageType,
        subject: &str,
        body: &str,
        metadata: Option<Value>,
    ) -> Result<i64, MessageError> {
        if recipient_id == 0 {
            return Err(MessageError::RecipientRequired);
        }
        if subject.trim().is_empty() {
            return Err(MessageError::EmptySubject);
        }
        if body.trim().is_empty() {
            return Err(MessageError::EmptyBody);
        }
        if let Some(sid) = sender_id {
            if sid == recipient_id {
                return Err(MessageError::SelfMessage);
            }
        }

        let id = self.alloc_id();
        let msg = Message {
            id,
            sender_id,
            recipient_id,
            message_type,
            subject: subject.to_string(),
            body: body.to_string(),
            is_read: false,
            is_archived: false,
            is_deleted: false,
            metadata,
            created_at: Self::now_timestamp(),
        };
        self.messages.insert(id, msg);
        Ok(id)
    }

    /// Retrieve a message. Only the sender or recipient may access it.
    pub fn get_message(&self, message_id: i64, user_id: i64) -> Result<Message, MessageError> {
        let msg = self
            .messages
            .get(&message_id)
            .ok_or(MessageError::NotFound)?;
        let is_sender = msg.sender_id == Some(user_id);
        let is_recipient = msg.recipient_id == user_id;
        if !is_sender && !is_recipient {
            return Err(MessageError::Unauthorized);
        }
        Ok(msg.clone())
    }

    /// List inbox messages for a user, newest first.
    /// Only non-deleted messages where user is recipient.
    /// Optionally filter by message type.
    pub fn list_inbox(
        &self,
        user_id: i64,
        message_type: Option<MessageType>,
        offset: usize,
        limit: usize,
    ) -> Vec<Message> {
        let mut results: Vec<&Message> = self
            .messages
            .values()
            .filter(|m| {
                m.recipient_id == user_id
                    && !m.is_deleted
                    && message_type.map_or(true, |mt| m.message_type == mt)
            })
            .collect();
        // newest first (higher id = newer)
        results.sort_by(|a, b| b.id.cmp(&a.id));
        results
            .into_iter()
            .skip(offset)
            .take(limit)
            .cloned()
            .collect()
    }

    /// List sent messages for a user, newest first.
    pub fn list_sent(&self, user_id: i64, offset: usize, limit: usize) -> Vec<Message> {
        let mut results: Vec<&Message> = self
            .messages
            .values()
            .filter(|m| m.sender_id == Some(user_id) && !m.is_deleted)
            .collect();
        results.sort_by(|a, b| b.id.cmp(&a.id));
        results
            .into_iter()
            .skip(offset)
            .take(limit)
            .cloned()
            .collect()
    }

    /// Mark a single message as read. Only the recipient may do this.
    pub fn mark_read(&mut self, message_id: i64, user_id: i64) -> Result<(), MessageError> {
        let msg = self
            .messages
            .get_mut(&message_id)
            .ok_or(MessageError::NotFound)?;
        if msg.recipient_id != user_id {
            return Err(MessageError::Unauthorized);
        }
        msg.is_read = true;
        Ok(())
    }

    /// Mark all inbox messages as read for a user, optionally filtered by type.
    /// Returns the number of messages updated.
    pub fn mark_all_read(&mut self, user_id: i64, message_type: Option<MessageType>) -> usize {
        let mut count = 0usize;
        for msg in self.messages.values_mut() {
            if msg.recipient_id == user_id
                && !msg.is_read
                && !msg.is_deleted
                && message_type.map_or(true, |mt| msg.message_type == mt)
            {
                msg.is_read = true;
                count += 1;
            }
        }
        count
    }

    /// Archive a message. Only the recipient may do this.
    pub fn archive_message(&mut self, message_id: i64, user_id: i64) -> Result<(), MessageError> {
        let msg = self
            .messages
            .get_mut(&message_id)
            .ok_or(MessageError::NotFound)?;
        if msg.recipient_id != user_id {
            return Err(MessageError::Unauthorized);
        }
        msg.is_archived = true;
        Ok(())
    }

    /// Soft-delete a message. Only the sender or recipient may do this.
    pub fn delete_message(&mut self, message_id: i64, user_id: i64) -> Result<(), MessageError> {
        let msg = self
            .messages
            .get_mut(&message_id)
            .ok_or(MessageError::NotFound)?;
        let is_sender = msg.sender_id == Some(user_id);
        let is_recipient = msg.recipient_id == user_id;
        if !is_sender && !is_recipient {
            return Err(MessageError::Unauthorized);
        }
        msg.is_deleted = true;
        Ok(())
    }

    /// Count unread, non-deleted inbox messages for a user.
    pub fn unread_count(&self, user_id: i64) -> usize {
        self.messages
            .values()
            .filter(|m| m.recipient_id == user_id && !m.is_read && !m.is_deleted)
            .count()
    }

    /// Count unread, non-deleted inbox messages grouped by message type.
    pub fn unread_count_by_type(&self, user_id: i64) -> HashMap<MessageType, usize> {
        let mut counts: HashMap<MessageType, usize> = HashMap::new();
        for msg in self.messages.values() {
            if msg.recipient_id == user_id && !msg.is_read && !msg.is_deleted {
                *counts.entry(msg.message_type).or_insert(0) += 1;
            }
        }
        counts
    }

    /// Convenience: send a system message (no sender). Returns the message id.
    pub fn send_system_message(&mut self, recipient_id: i64, subject: &str, body: &str) -> i64 {
        self.send_message(None, recipient_id, MessageType::System, subject, body, None)
            .expect("system messages should always pass validation")
    }

    /// Send a combat report to both attacker and defender.
    /// Returns `(attacker_msg_id, defender_msg_id)`.
    pub fn send_combat_report(
        &mut self,
        attacker_id: i64,
        defender_id: i64,
        report: &str,
        metadata: Value,
    ) -> (i64, i64) {
        let attacker_msg = self
            .send_message(
                None,
                attacker_id,
                MessageType::CombatReport,
                "Combat Report",
                report,
                Some(metadata.clone()),
            )
            .expect("combat report should pass validation");

        let defender_msg = self
            .send_message(
                None,
                defender_id,
                MessageType::CombatReport,
                "Combat Report",
                report,
                Some(metadata),
            )
            .expect("combat report should pass validation");

        (attacker_msg, defender_msg)
    }

    /// Send an espionage report to both spy owner and target.
    /// Returns `(spy_msg_id, target_msg_id)`.
    pub fn send_espionage_report(
        &mut self,
        spy_id: i64,
        target_id: i64,
        report: &str,
        metadata: Value,
    ) -> (i64, i64) {
        let spy_msg = self
            .send_message(
                None,
                spy_id,
                MessageType::EspionageReport,
                "Espionage Report",
                report,
                Some(metadata.clone()),
            )
            .expect("espionage report should pass validation");

        let target_msg = self
            .send_message(
                None,
                target_id,
                MessageType::EspionageReport,
                "Espionage Report",
                report,
                Some(metadata),
            )
            .expect("espionage report should pass validation");

        (spy_msg, target_msg)
    }

    /// Bulk soft-delete all non-deleted messages of a given type for a user.
    /// Returns the number of messages deleted.
    pub fn bulk_delete_by_type(&mut self, user_id: i64, message_type: MessageType) -> usize {
        let mut count = 0usize;
        for msg in self.messages.values_mut() {
            if msg.recipient_id == user_id && msg.message_type == message_type && !msg.is_deleted {
                msg.is_deleted = true;
                count += 1;
            }
        }
        count
    }

    /// Hard-remove messages whose `created_at` timestamp is older than `days` days.
    /// Only works with the `unix:<seconds>` timestamp format used by this crate.
    /// Returns the number of messages removed.
    pub fn cleanup_old_messages(&mut self, days: i64) -> usize {
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let cutoff = now_secs - (days * 86_400);
        let before = self.messages.len();
        self.messages.retain(|_id, msg| {
            parse_unix_timestamp(&msg.created_at).map_or(true, |ts| ts >= cutoff)
        });
        before.saturating_sub(self.messages.len())
    }
}

impl Default for MessageStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse our `unix:<seconds>` timestamp format.
fn parse_unix_timestamp(s: &str) -> Option<i64> {
    s.strip_prefix("unix:")
        .and_then(|rest| rest.parse::<i64>().ok())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // -- MessageType Display / FromStr --------------------------------------

    #[test]
    fn message_type_display_round_trip() {
        for mt in ALL_MESSAGE_TYPES {
            let s = mt.to_string();
            let parsed: MessageType = s.parse().unwrap();
            assert_eq!(parsed, mt);
        }
    }

    #[test]
    fn message_type_from_str_invalid() {
        assert!("Bogus".parse::<MessageType>().is_err());
    }

    // -- send / receive -----------------------------------------------------

    #[test]
    fn send_and_get_message() {
        let mut store = MessageStore::new();
        let id = store
            .send_message(Some(1), 2, MessageType::Private, "Hello", "World", None)
            .unwrap();
        assert_eq!(id, 1);

        let msg = store.get_message(id, 2).unwrap();
        assert_eq!(msg.sender_id, Some(1));
        assert_eq!(msg.recipient_id, 2);
        assert_eq!(msg.subject, "Hello");
        assert!(!msg.is_read);
    }

    #[test]
    fn get_message_unauthorized() {
        let mut store = MessageStore::new();
        let id = store
            .send_message(Some(1), 2, MessageType::Private, "Secret", "Data", None)
            .unwrap();
        assert_eq!(store.get_message(id, 3), Err(MessageError::Unauthorized));
    }

    #[test]
    fn send_message_validation_errors() {
        let mut store = MessageStore::new();

        assert_eq!(
            store.send_message(Some(1), 0, MessageType::Private, "Hi", "Body", None),
            Err(MessageError::RecipientRequired)
        );
        assert_eq!(
            store.send_message(Some(1), 2, MessageType::Private, "", "Body", None),
            Err(MessageError::EmptySubject)
        );
        assert_eq!(
            store.send_message(Some(1), 2, MessageType::Private, "   ", "Body", None),
            Err(MessageError::EmptySubject)
        );
        assert_eq!(
            store.send_message(Some(1), 2, MessageType::Private, "Hi", "", None),
            Err(MessageError::EmptyBody)
        );
        assert_eq!(
            store.send_message(Some(1), 1, MessageType::Private, "Hi", "Body", None),
            Err(MessageError::SelfMessage)
        );
    }

    // -- inbox / sent -------------------------------------------------------

    #[test]
    fn list_inbox_returns_newest_first_with_type_filter() {
        let mut store = MessageStore::new();
        store
            .send_message(Some(1), 2, MessageType::Private, "A", "a", None)
            .unwrap();
        store
            .send_message(Some(1), 2, MessageType::Alliance, "B", "b", None)
            .unwrap();
        store
            .send_message(Some(1), 2, MessageType::Private, "C", "c", None)
            .unwrap();

        let all = store.list_inbox(2, None, 0, 100);
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].subject, "C"); // newest first

        let private_only = store.list_inbox(2, Some(MessageType::Private), 0, 100);
        assert_eq!(private_only.len(), 2);
        assert_eq!(private_only[0].subject, "C");
        assert_eq!(private_only[1].subject, "A");
    }

    #[test]
    fn list_inbox_pagination() {
        let mut store = MessageStore::new();
        for i in 0..5 {
            store
                .send_message(
                    Some(1),
                    2,
                    MessageType::Private,
                    &format!("Msg {i}"),
                    "body",
                    None,
                )
                .unwrap();
        }

        let page1 = store.list_inbox(2, None, 0, 2);
        assert_eq!(page1.len(), 2);
        assert_eq!(page1[0].subject, "Msg 4");
        assert_eq!(page1[1].subject, "Msg 3");

        let page2 = store.list_inbox(2, None, 2, 2);
        assert_eq!(page2.len(), 2);
        assert_eq!(page2[0].subject, "Msg 2");
    }

    #[test]
    fn list_sent_messages() {
        let mut store = MessageStore::new();
        store
            .send_message(Some(1), 2, MessageType::Private, "Out", "going", None)
            .unwrap();
        store
            .send_message(Some(3), 2, MessageType::Private, "Other", "sender", None)
            .unwrap();

        let sent = store.list_sent(1, 0, 100);
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].subject, "Out");
    }

    // -- read / archive / delete --------------------------------------------

    #[test]
    fn mark_read_and_unread_count() {
        let mut store = MessageStore::new();
        let id1 = store
            .send_message(Some(1), 2, MessageType::Private, "A", "a", None)
            .unwrap();
        let _id2 = store
            .send_message(Some(1), 2, MessageType::Private, "B", "b", None)
            .unwrap();

        assert_eq!(store.unread_count(2), 2);

        store.mark_read(id1, 2).unwrap();
        assert_eq!(store.unread_count(2), 1);

        // Non-recipient cannot mark read
        let id3 = store
            .send_message(Some(1), 2, MessageType::Private, "C", "c", None)
            .unwrap();
        assert_eq!(store.mark_read(id3, 1), Err(MessageError::Unauthorized));
    }

    #[test]
    fn mark_all_read_with_type_filter() {
        let mut store = MessageStore::new();
        store
            .send_message(Some(1), 2, MessageType::Private, "A", "a", None)
            .unwrap();
        store
            .send_message(Some(1), 2, MessageType::Alliance, "B", "b", None)
            .unwrap();
        store
            .send_message(Some(1), 2, MessageType::Private, "C", "c", None)
            .unwrap();

        let marked = store.mark_all_read(2, Some(MessageType::Private));
        assert_eq!(marked, 2);
        assert_eq!(store.unread_count(2), 1); // alliance msg still unread
    }

    #[test]
    fn archive_message() {
        let mut store = MessageStore::new();
        let id = store
            .send_message(Some(1), 2, MessageType::Private, "A", "a", None)
            .unwrap();

        store.archive_message(id, 2).unwrap();
        let msg = store.get_message(id, 2).unwrap();
        assert!(msg.is_archived);

        // Non-recipient cannot archive
        assert_eq!(
            store.archive_message(id, 1),
            Err(MessageError::Unauthorized)
        );
    }

    #[test]
    fn delete_message_soft_deletes_and_hides_from_inbox() {
        let mut store = MessageStore::new();
        let id = store
            .send_message(Some(1), 2, MessageType::Private, "Del", "ete", None)
            .unwrap();
        store
            .send_message(Some(1), 2, MessageType::Private, "Keep", "me", None)
            .unwrap();

        store.delete_message(id, 2).unwrap();

        // Should not appear in inbox
        let inbox = store.list_inbox(2, None, 0, 100);
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].subject, "Keep");

        // Deleted messages do not count as unread
        assert_eq!(store.unread_count(2), 1);
    }

    // -- combat / espionage reports -----------------------------------------

    #[test]
    fn combat_report_sent_to_both_players() {
        let mut store = MessageStore::new();
        let meta = json!({"attacker_losses": 100, "defender_losses": 200});
        let (a_id, d_id) = store.send_combat_report(1, 2, "Battle at [2:45:8]", meta.clone());

        let a_msg = store.get_message(a_id, 1).unwrap();
        assert_eq!(a_msg.message_type, MessageType::CombatReport);
        assert_eq!(a_msg.recipient_id, 1);
        assert_eq!(a_msg.metadata, Some(meta.clone()));

        let d_msg = store.get_message(d_id, 2).unwrap();
        assert_eq!(d_msg.message_type, MessageType::CombatReport);
        assert_eq!(d_msg.recipient_id, 2);
    }

    #[test]
    fn espionage_report_sent_to_both_players() {
        let mut store = MessageStore::new();
        let meta = json!({"resources": {"metal": 5000}});
        let (s_id, t_id) =
            store.send_espionage_report(1, 2, "Spy report on [3:100:7]", meta.clone());

        let spy_msg = store.get_message(s_id, 1).unwrap();
        assert_eq!(spy_msg.message_type, MessageType::EspionageReport);

        let target_msg = store.get_message(t_id, 2).unwrap();
        assert_eq!(target_msg.message_type, MessageType::EspionageReport);
    }

    // -- unread count by type -----------------------------------------------

    #[test]
    fn unread_count_by_type_groups_correctly() {
        let mut store = MessageStore::new();
        store
            .send_message(Some(1), 2, MessageType::Private, "A", "a", None)
            .unwrap();
        store
            .send_message(Some(1), 2, MessageType::Private, "B", "b", None)
            .unwrap();
        store
            .send_message(Some(1), 2, MessageType::Alliance, "C", "c", None)
            .unwrap();
        store.send_system_message(2, "Welcome", "Hello commander");

        let counts = store.unread_count_by_type(2);
        assert_eq!(counts.get(&MessageType::Private), Some(&2));
        assert_eq!(counts.get(&MessageType::Alliance), Some(&1));
        assert_eq!(counts.get(&MessageType::System), Some(&1));
        assert_eq!(counts.get(&MessageType::CombatReport), None);
    }

    // -- system messages ----------------------------------------------------

    #[test]
    fn send_system_message_has_no_sender() {
        let mut store = MessageStore::new();
        let id = store.send_system_message(5, "Server Maintenance", "Downtime at 03:00 UTC");

        let msg = store.get_message(id, 5).unwrap();
        assert_eq!(msg.sender_id, None);
        assert_eq!(msg.message_type, MessageType::System);
    }

    // -- bulk delete --------------------------------------------------------

    #[test]
    fn bulk_delete_by_type_only_deletes_matching() {
        let mut store = MessageStore::new();
        store
            .send_message(Some(1), 2, MessageType::Private, "A", "a", None)
            .unwrap();
        store
            .send_message(Some(1), 2, MessageType::Alliance, "B", "b", None)
            .unwrap();
        store
            .send_message(Some(1), 2, MessageType::Private, "C", "c", None)
            .unwrap();

        let deleted = store.bulk_delete_by_type(2, MessageType::Private);
        assert_eq!(deleted, 2);

        let inbox = store.list_inbox(2, None, 0, 100);
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].message_type, MessageType::Alliance);
    }

    // -- cleanup_old_messages -----------------------------------------------

    #[test]
    fn cleanup_old_messages_removes_expired() {
        let mut store = MessageStore::new();

        // Manually insert a message with a very old timestamp
        let old_id = store.alloc_id();
        store.messages.insert(
            old_id,
            Message {
                id: old_id,
                sender_id: Some(1),
                recipient_id: 2,
                message_type: MessageType::Private,
                subject: "Old".to_string(),
                body: "Ancient message".to_string(),
                is_read: true,
                is_archived: false,
                is_deleted: false,
                metadata: None,
                created_at: "unix:1000000".to_string(), // ~1970
            },
        );

        // Send a fresh one through the normal API
        store
            .send_message(Some(1), 2, MessageType::Private, "New", "Recent", None)
            .unwrap();

        let removed = store.cleanup_old_messages(30);
        assert_eq!(removed, 1);
        assert_eq!(store.messages.len(), 1);
    }

    // -- SpamGuard ----------------------------------------------------------

    #[test]
    fn spam_guard_allows_up_to_limit() {
        let mut guard = SpamGuard::new();
        let now = 1_000_000;

        for i in 0..10 {
            assert!(guard.can_send(1, now + i));
            guard.record_send(1, now + i);
        }
        // 11th message should be blocked
        assert!(!guard.can_send(1, now + 10));
    }

    #[test]
    fn spam_guard_resets_after_window() {
        let mut guard = SpamGuard::new();
        let now = 1_000_000;

        for _ in 0..10 {
            guard.record_send(1, now);
        }
        assert!(!guard.can_send(1, now));

        // After the 60-second window, sending is allowed again
        assert!(guard.can_send(1, now + 61));
    }

    #[test]
    fn spam_guard_cleanup_expired() {
        let mut guard = SpamGuard::new();
        guard.record_send(1, 100);
        guard.record_send(2, 200);

        guard.cleanup_expired(200);
        // sender 1's entry at t=100 is expired at t=200 (100 <= 200 - 60)
        assert!(guard.sends.get(&1).is_none() || guard.sends[&1].is_empty());
        // sender 2 at t=200 is still within window at t=200
        assert!(guard.can_send(2, 200));
    }

    #[test]
    fn spam_guard_independent_senders() {
        let mut guard = SpamGuard::new();
        let now = 1_000_000;

        for _ in 0..10 {
            guard.record_send(1, now);
        }
        // Sender 1 blocked, but sender 2 unaffected
        assert!(!guard.can_send(1, now));
        assert!(guard.can_send(2, now));
    }

    // -- MessageThread struct -----------------------------------------------

    #[test]
    fn message_thread_serde_round_trip() {
        let thread = MessageThread {
            thread_id: "thread-42".to_string(),
            participants: vec![1, 2],
            subject: "Alliance Discussion".to_string(),
            message_count: 5,
            last_message_at: "unix:1700000000".to_string(),
        };
        let json = serde_json::to_string(&thread).unwrap();
        let parsed: MessageThread = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, thread);
    }

    // -- edge cases ---------------------------------------------------------

    #[test]
    fn get_nonexistent_message_returns_not_found() {
        let store = MessageStore::new();
        assert_eq!(store.get_message(999, 1), Err(MessageError::NotFound));
    }

    #[test]
    fn delete_nonexistent_message_returns_not_found() {
        let mut store = MessageStore::new();
        assert_eq!(store.delete_message(999, 1), Err(MessageError::NotFound));
    }

    #[test]
    fn message_with_metadata_preserved() {
        let mut store = MessageStore::new();
        let meta = json!({"fleet_id": 42, "coordinates": [2, 45, 8]});
        let id = store
            .send_message(
                Some(1),
                2,
                MessageType::Transport,
                "Delivery",
                "Resources sent",
                Some(meta.clone()),
            )
            .unwrap();

        let msg = store.get_message(id, 2).unwrap();
        assert_eq!(msg.metadata, Some(meta));
    }

    #[test]
    fn mark_all_read_without_type_filter() {
        let mut store = MessageStore::new();
        store
            .send_message(Some(1), 2, MessageType::Private, "A", "a", None)
            .unwrap();
        store
            .send_message(Some(1), 2, MessageType::Alliance, "B", "b", None)
            .unwrap();
        store.send_system_message(2, "Sys", "msg");

        let marked = store.mark_all_read(2, None);
        assert_eq!(marked, 3);
        assert_eq!(store.unread_count(2), 0);
    }
}
