#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ────────────────────────────────────────────────────────────────────────────
// Chat Restrictions (existing)
// ────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatRestriction {
    pub user_id: i64,
    pub reason: String,
    pub created_at_unix: i64,
    pub expires_at_unix: i64,
}

#[derive(Default)]
pub struct ChatRestrictionStore {
    restrictions: Vec<ChatRestriction>,
}

impl ChatRestrictionStore {
    pub fn with_seed() -> Self {
        Self {
            restrictions: vec![
                ChatRestriction {
                    user_id: 21,
                    reason: "spam".to_string(),
                    created_at_unix: 1_739_426_000,
                    expires_at_unix: 1_739_427_000,
                },
                ChatRestriction {
                    user_id: 22,
                    reason: "abuse".to_string(),
                    created_at_unix: 1_739_426_000,
                    expires_at_unix: 1_999_999_999,
                },
            ],
        }
    }

    pub fn add(&mut self, restriction: ChatRestriction) {
        self.restrictions.push(restriction);
    }

    pub fn list(&self) -> Vec<ChatRestriction> {
        self.restrictions.clone()
    }

    pub fn cleanup_expired(&mut self, now_unix: i64) -> usize {
        let before = self.restrictions.len();
        self.restrictions
            .retain(|item| item.expires_at_unix > now_unix);
        before.saturating_sub(self.restrictions.len())
    }

    /// Returns true if the given user has an active (non-expired) restriction.
    pub fn is_restricted(&self, user_id: i64, now_unix: i64) -> bool {
        self.restrictions
            .iter()
            .any(|r| r.user_id == user_id && r.expires_at_unix > now_unix)
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Chat Room System
// ────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatRoomType {
    Global,
    Alliance,
    Private,
    System,
    Trade,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRoom {
    pub id: i64,
    pub name: String,
    pub room_type: ChatRoomType,
    pub created_by: i64,
    pub created_at: String,
    pub max_members: usize,
    pub is_archived: bool,
}

#[derive(Default)]
pub struct ChatRoomStore {
    rooms: HashMap<i64, ChatRoom>,
    next_id: i64,
}

impl ChatRoomStore {
    pub fn create_room(
        &mut self,
        name: String,
        room_type: ChatRoomType,
        created_by: i64,
        created_at: String,
        max_members: usize,
    ) -> ChatRoom {
        self.next_id += 1;
        let room = ChatRoom {
            id: self.next_id,
            name,
            room_type,
            created_by,
            created_at,
            max_members,
            is_archived: false,
        };
        self.rooms.insert(room.id, room.clone());
        room
    }

    pub fn get_room(&self, id: i64) -> Option<&ChatRoom> {
        self.rooms.get(&id)
    }

    pub fn list_rooms(&self) -> Vec<ChatRoom> {
        self.rooms.values().cloned().collect()
    }

    pub fn list_rooms_by_type(&self, room_type: &ChatRoomType) -> Vec<ChatRoom> {
        self.rooms
            .values()
            .filter(|r| &r.room_type == room_type)
            .cloned()
            .collect()
    }

    pub fn archive_room(&mut self, id: i64) -> bool {
        if let Some(room) = self.rooms.get_mut(&id) {
            room.is_archived = true;
            true
        } else {
            false
        }
    }

    pub fn delete_room(&mut self, id: i64) -> bool {
        self.rooms.remove(&id).is_some()
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Chat Messages
// ────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageType {
    Text,
    System,
    Emote,
    Trade,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: i64,
    pub room_id: i64,
    pub sender_id: i64,
    pub sender_name: String,
    pub content: String,
    pub message_type: MessageType,
    pub created_at: String,
    pub edited_at: Option<String>,
    pub is_deleted: bool,
}

#[derive(Default)]
pub struct ChatMessageStore {
    messages: HashMap<i64, ChatMessage>,
    next_id: i64,
}

impl ChatMessageStore {
    pub fn send_message(
        &mut self,
        room_id: i64,
        sender_id: i64,
        sender_name: String,
        content: String,
        message_type: MessageType,
        created_at: String,
    ) -> ChatMessage {
        self.next_id += 1;
        let msg = ChatMessage {
            id: self.next_id,
            room_id,
            sender_id,
            sender_name,
            content,
            message_type,
            created_at,
            edited_at: None,
            is_deleted: false,
        };
        self.messages.insert(msg.id, msg.clone());
        msg
    }

    pub fn get_message(&self, id: i64) -> Option<&ChatMessage> {
        self.messages.get(&id)
    }

    /// Get messages for a room with pagination (offset + limit), ordered by id ascending.
    pub fn get_messages(&self, room_id: i64, offset: usize, limit: usize) -> Vec<ChatMessage> {
        let mut msgs: Vec<&ChatMessage> = self
            .messages
            .values()
            .filter(|m| m.room_id == room_id && !m.is_deleted)
            .collect();
        msgs.sort_by_key(|m| m.id);
        msgs.into_iter().skip(offset).take(limit).cloned().collect()
    }

    /// Get the most recent N messages for a room, ordered by id ascending.
    pub fn get_recent_messages(&self, room_id: i64, count: usize) -> Vec<ChatMessage> {
        let mut msgs: Vec<&ChatMessage> = self
            .messages
            .values()
            .filter(|m| m.room_id == room_id && !m.is_deleted)
            .collect();
        msgs.sort_by_key(|m| m.id);
        let start = msgs.len().saturating_sub(count);
        msgs[start..].iter().cloned().cloned().collect()
    }

    pub fn edit_message(&mut self, id: i64, new_content: String, edited_at: String) -> bool {
        if let Some(msg) = self.messages.get_mut(&id) {
            if msg.is_deleted {
                return false;
            }
            msg.content = new_content;
            msg.edited_at = Some(edited_at);
            true
        } else {
            false
        }
    }

    /// Soft-delete a message (marks as deleted, clears content).
    pub fn delete_message(&mut self, id: i64) -> bool {
        if let Some(msg) = self.messages.get_mut(&id) {
            msg.is_deleted = true;
            msg.content = String::new();
            true
        } else {
            false
        }
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Chat Membership
// ────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemberRole {
    Owner,
    Moderator,
    Member,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMember {
    pub user_id: i64,
    pub room_id: i64,
    pub role: MemberRole,
    pub joined_at: String,
    pub last_read_message_id: Option<i64>,
}

#[derive(Default)]
pub struct ChatMembershipStore {
    /// Key: (user_id, room_id)
    members: HashMap<(i64, i64), ChatMember>,
}

impl ChatMembershipStore {
    pub fn join_room(
        &mut self,
        user_id: i64,
        room_id: i64,
        role: MemberRole,
        joined_at: String,
    ) -> bool {
        let key = (user_id, room_id);
        if self.members.contains_key(&key) {
            return false; // already a member
        }
        self.members.insert(
            key,
            ChatMember {
                user_id,
                room_id,
                role,
                joined_at,
                last_read_message_id: None,
            },
        );
        true
    }

    pub fn leave_room(&mut self, user_id: i64, room_id: i64) -> bool {
        self.members.remove(&(user_id, room_id)).is_some()
    }

    pub fn get_members(&self, room_id: i64) -> Vec<ChatMember> {
        self.members
            .values()
            .filter(|m| m.room_id == room_id)
            .cloned()
            .collect()
    }

    pub fn get_user_rooms(&self, user_id: i64) -> Vec<ChatMember> {
        self.members
            .values()
            .filter(|m| m.user_id == user_id)
            .cloned()
            .collect()
    }

    pub fn update_role(&mut self, user_id: i64, room_id: i64, new_role: MemberRole) -> bool {
        if let Some(member) = self.members.get_mut(&(user_id, room_id)) {
            member.role = new_role;
            true
        } else {
            false
        }
    }

    pub fn update_last_read(&mut self, user_id: i64, room_id: i64, message_id: i64) -> bool {
        if let Some(member) = self.members.get_mut(&(user_id, room_id)) {
            member.last_read_message_id = Some(message_id);
            true
        } else {
            false
        }
    }

    pub fn is_member(&self, user_id: i64, room_id: i64) -> bool {
        self.members.contains_key(&(user_id, room_id))
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Moderation
// ────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModerationAction {
    Mute,
    Kick,
    Ban,
    Warn,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModerationRecord {
    pub id: i64,
    pub room_id: i64,
    pub target_user_id: i64,
    pub moderator_id: i64,
    pub action: ModerationAction,
    pub reason: String,
    pub created_at: String,
    pub expires_at: Option<String>,
}

#[derive(Default)]
pub struct ModerationStore {
    records: HashMap<i64, ModerationRecord>,
    next_id: i64,
}

impl ModerationStore {
    pub fn add_action(
        &mut self,
        room_id: i64,
        target_user_id: i64,
        moderator_id: i64,
        action: ModerationAction,
        reason: String,
        created_at: String,
        expires_at: Option<String>,
    ) -> ModerationRecord {
        self.next_id += 1;
        let record = ModerationRecord {
            id: self.next_id,
            room_id,
            target_user_id,
            moderator_id,
            action,
            reason,
            created_at,
            expires_at,
        };
        self.records.insert(record.id, record.clone());
        record
    }

    pub fn get_user_actions(&self, target_user_id: i64) -> Vec<ModerationRecord> {
        self.records
            .values()
            .filter(|r| r.target_user_id == target_user_id)
            .cloned()
            .collect()
    }

    /// Check if a user is muted in a room. `now` is an ISO 8601 string for comparison.
    pub fn is_muted(&self, user_id: i64, room_id: i64, now: &str) -> bool {
        self.records.values().any(|r| {
            r.target_user_id == user_id
                && r.room_id == room_id
                && r.action == ModerationAction::Mute
                && match &r.expires_at {
                    Some(exp) => exp.as_str() > now,
                    None => true, // permanent
                }
        })
    }

    /// Check if a user is banned from a room. `now` is an ISO 8601 string for comparison.
    pub fn is_banned(&self, user_id: i64, room_id: i64, now: &str) -> bool {
        self.records.values().any(|r| {
            r.target_user_id == user_id
                && r.room_id == room_id
                && r.action == ModerationAction::Ban
                && match &r.expires_at {
                    Some(exp) => exp.as_str() > now,
                    None => true,
                }
        })
    }

    /// Remove records whose `expires_at` is in the past. Returns the number removed.
    pub fn cleanup_expired(&mut self, now: &str) -> usize {
        let before = self.records.len();
        self.records.retain(|_, r| match &r.expires_at {
            Some(exp) => exp.as_str() > now,
            None => true, // permanent records are never cleaned up
        });
        before.saturating_sub(self.records.len())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Profanity Filter
// ────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfanityFilter {
    pub banned_words: Vec<String>,
}

impl Default for ProfanityFilter {
    fn default() -> Self {
        Self {
            banned_words: vec![
                "badword".to_string(),
                "offensive".to_string(),
                "slur".to_string(),
                "profanity".to_string(),
                "vulgar".to_string(),
            ],
        }
    }
}

impl ProfanityFilter {
    /// Replace banned words with asterisks (case-insensitive, whole-word).
    pub fn filter_message(&self, content: &str) -> String {
        let mut result = content.to_string();
        for word in &self.banned_words {
            let replacement = "*".repeat(word.len());
            // Case-insensitive replacement
            let lower = result.to_lowercase();
            let pattern = word.to_lowercase();
            let mut output = String::with_capacity(result.len());
            let mut search_start = 0;
            while let Some(pos) = lower[search_start..].find(&pattern) {
                let abs_pos = search_start + pos;
                // Check word boundaries
                let before_ok =
                    abs_pos == 0 || !result.as_bytes()[abs_pos - 1].is_ascii_alphanumeric();
                let after_pos = abs_pos + pattern.len();
                let after_ok = after_pos >= result.len()
                    || !result.as_bytes()[after_pos].is_ascii_alphanumeric();
                if before_ok && after_ok {
                    output.push_str(&result[search_start..abs_pos]);
                    output.push_str(&replacement);
                    search_start = after_pos;
                } else {
                    output.push_str(&result[search_start..abs_pos + 1]);
                    search_start = abs_pos + 1;
                }
            }
            output.push_str(&result[search_start..]);
            result = output;
        }
        result
    }

    pub fn contains_profanity(&self, content: &str) -> bool {
        let lower = content.to_lowercase();
        for word in &self.banned_words {
            let pattern = word.to_lowercase();
            let mut search_start = 0;
            while let Some(pos) = lower[search_start..].find(&pattern) {
                let abs_pos = search_start + pos;
                let before_ok =
                    abs_pos == 0 || !content.as_bytes()[abs_pos - 1].is_ascii_alphanumeric();
                let after_pos = abs_pos + pattern.len();
                let after_ok = after_pos >= content.len()
                    || !content.as_bytes()[after_pos].is_ascii_alphanumeric();
                if before_ok && after_ok {
                    return true;
                }
                search_start = abs_pos + 1;
            }
        }
        false
    }

    pub fn add_word(&mut self, word: &str) {
        let w = word.to_lowercase();
        if !self.banned_words.contains(&w) {
            self.banned_words.push(w);
        }
    }

    pub fn remove_word(&mut self, word: &str) {
        let w = word.to_lowercase();
        self.banned_words.retain(|bw| bw != &w);
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Rate Limiter
// ────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRateLimiter {
    pub max_messages_per_window: usize,
    pub window_seconds: i64,
    /// Per-user timestamp list of recent messages (unix seconds).
    timestamps: HashMap<i64, Vec<i64>>,
}

impl ChatRateLimiter {
    pub fn new(max_messages_per_window: usize, window_seconds: i64) -> Self {
        Self {
            max_messages_per_window,
            window_seconds,
            timestamps: HashMap::new(),
        }
    }

    /// Returns `true` if the user is allowed to send a message at `now_unix`.
    /// Records the timestamp if allowed.
    pub fn check_rate(&mut self, user_id: i64, now_unix: i64) -> bool {
        let window_start = now_unix - self.window_seconds;
        let entries = self.timestamps.entry(user_id).or_default();
        // Prune old timestamps
        entries.retain(|&t| t > window_start);
        if entries.len() >= self.max_messages_per_window {
            false
        } else {
            entries.push(now_unix);
            true
        }
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── ChatRestrictionStore ──────────────────────────────────────────

    #[test]
    fn restriction_cleanup_expired_removes_only_expired_entries() {
        let mut store = ChatRestrictionStore::with_seed();
        let removed = store.cleanup_expired(1_739_427_500);
        assert_eq!(removed, 1);
        assert_eq!(store.list().len(), 1);
        assert_eq!(store.list()[0].user_id, 22);
    }

    #[test]
    fn restriction_add_then_cleanup_keeps_future_restriction() {
        let mut store = ChatRestrictionStore::default();
        store.add(ChatRestriction {
            user_id: 3,
            reason: "flood".to_string(),
            created_at_unix: 1_700_000_000,
            expires_at_unix: 1_800_000_000,
        });
        let removed = store.cleanup_expired(1_750_000_000);
        assert_eq!(removed, 0);
        assert_eq!(store.list().len(), 1);
    }

    #[test]
    fn restriction_is_restricted() {
        let store = ChatRestrictionStore::with_seed();
        assert!(store.is_restricted(22, 1_739_427_500));
        assert!(!store.is_restricted(21, 1_739_427_500)); // expired
        assert!(!store.is_restricted(99, 1_739_427_500)); // not present
    }

    // ── ChatRoomStore ─────────────────────────────────────────────────

    #[test]
    fn room_create_and_get() {
        let mut store = ChatRoomStore::default();
        let room = store.create_room(
            "General".into(),
            ChatRoomType::Global,
            1,
            "2025-01-01T00:00:00Z".into(),
            100,
        );
        assert_eq!(room.id, 1);
        assert_eq!(room.name, "General");
        let fetched = store.get_room(1).unwrap();
        assert_eq!(fetched.name, "General");
    }

    #[test]
    fn room_list_and_list_by_type() {
        let mut store = ChatRoomStore::default();
        store.create_room(
            "G1".into(),
            ChatRoomType::Global,
            1,
            "2025-01-01T00:00:00Z".into(),
            50,
        );
        store.create_room(
            "T1".into(),
            ChatRoomType::Trade,
            1,
            "2025-01-01T00:00:00Z".into(),
            50,
        );
        store.create_room(
            "G2".into(),
            ChatRoomType::Global,
            2,
            "2025-01-01T00:00:00Z".into(),
            50,
        );
        assert_eq!(store.list_rooms().len(), 3);
        assert_eq!(store.list_rooms_by_type(&ChatRoomType::Global).len(), 2);
        assert_eq!(store.list_rooms_by_type(&ChatRoomType::Trade).len(), 1);
        assert_eq!(store.list_rooms_by_type(&ChatRoomType::Private).len(), 0);
    }

    #[test]
    fn room_archive() {
        let mut store = ChatRoomStore::default();
        store.create_room(
            "R".into(),
            ChatRoomType::Alliance,
            1,
            "2025-01-01T00:00:00Z".into(),
            20,
        );
        assert!(store.archive_room(1));
        assert!(store.get_room(1).unwrap().is_archived);
        assert!(!store.archive_room(999));
    }

    #[test]
    fn room_delete() {
        let mut store = ChatRoomStore::default();
        store.create_room(
            "R".into(),
            ChatRoomType::System,
            1,
            "2025-01-01T00:00:00Z".into(),
            10,
        );
        assert!(store.delete_room(1));
        assert!(store.get_room(1).is_none());
        assert!(!store.delete_room(1));
    }

    #[test]
    fn room_auto_increment_ids() {
        let mut store = ChatRoomStore::default();
        let r1 = store.create_room("A".into(), ChatRoomType::Global, 1, "t".into(), 10);
        let r2 = store.create_room("B".into(), ChatRoomType::Global, 1, "t".into(), 10);
        assert_eq!(r1.id, 1);
        assert_eq!(r2.id, 2);
    }

    // ── ChatMessageStore ──────────────────────────────────────────────

    #[test]
    fn message_send_and_get() {
        let mut store = ChatMessageStore::default();
        let msg = store.send_message(
            1,
            10,
            "Alice".into(),
            "Hello".into(),
            MessageType::Text,
            "2025-01-01T00:00:00Z".into(),
        );
        assert_eq!(msg.id, 1);
        assert_eq!(msg.content, "Hello");
        let fetched = store.get_message(1).unwrap();
        assert_eq!(fetched.sender_name, "Alice");
    }

    #[test]
    fn message_pagination() {
        let mut store = ChatMessageStore::default();
        for i in 0..20 {
            store.send_message(
                1,
                10,
                "User".into(),
                format!("msg{i}"),
                MessageType::Text,
                "t".into(),
            );
        }
        let page = store.get_messages(1, 5, 5);
        assert_eq!(page.len(), 5);
        assert_eq!(page[0].content, "msg5");
        assert_eq!(page[4].content, "msg9");
    }

    #[test]
    fn message_get_messages_different_room() {
        let mut store = ChatMessageStore::default();
        store.send_message(
            1,
            10,
            "U".into(),
            "r1".into(),
            MessageType::Text,
            "t".into(),
        );
        store.send_message(
            2,
            10,
            "U".into(),
            "r2".into(),
            MessageType::Text,
            "t".into(),
        );
        assert_eq!(store.get_messages(1, 0, 100).len(), 1);
        assert_eq!(store.get_messages(2, 0, 100).len(), 1);
    }

    #[test]
    fn message_recent() {
        let mut store = ChatMessageStore::default();
        for i in 0..10 {
            store.send_message(
                1,
                10,
                "U".into(),
                format!("msg{i}"),
                MessageType::Text,
                "t".into(),
            );
        }
        let recent = store.get_recent_messages(1, 3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].content, "msg7");
        assert_eq!(recent[2].content, "msg9");
    }

    #[test]
    fn message_edit() {
        let mut store = ChatMessageStore::default();
        store.send_message(
            1,
            10,
            "U".into(),
            "original".into(),
            MessageType::Text,
            "t".into(),
        );
        assert!(store.edit_message(1, "edited".into(), "2025-01-02T00:00:00Z".into()));
        let msg = store.get_message(1).unwrap();
        assert_eq!(msg.content, "edited");
        assert_eq!(msg.edited_at.as_deref(), Some("2025-01-02T00:00:00Z"));
    }

    #[test]
    fn message_edit_nonexistent() {
        let mut store = ChatMessageStore::default();
        assert!(!store.edit_message(999, "x".into(), "t".into()));
    }

    #[test]
    fn message_soft_delete() {
        let mut store = ChatMessageStore::default();
        store.send_message(
            1,
            10,
            "U".into(),
            "hello".into(),
            MessageType::Text,
            "t".into(),
        );
        assert!(store.delete_message(1));
        let msg = store.get_message(1).unwrap();
        assert!(msg.is_deleted);
        assert!(msg.content.is_empty());
        // Deleted messages are excluded from listings
        assert_eq!(store.get_messages(1, 0, 100).len(), 0);
    }

    #[test]
    fn message_cannot_edit_deleted() {
        let mut store = ChatMessageStore::default();
        store.send_message(
            1,
            10,
            "U".into(),
            "hi".into(),
            MessageType::Text,
            "t".into(),
        );
        store.delete_message(1);
        assert!(!store.edit_message(1, "new".into(), "t".into()));
    }

    #[test]
    fn message_delete_nonexistent() {
        let mut store = ChatMessageStore::default();
        assert!(!store.delete_message(999));
    }

    // ── ChatMembershipStore ───────────────────────────────────────────

    #[test]
    fn membership_join_and_is_member() {
        let mut store = ChatMembershipStore::default();
        assert!(store.join_room(1, 100, MemberRole::Member, "2025-01-01T00:00:00Z".into()));
        assert!(store.is_member(1, 100));
        assert!(!store.is_member(1, 200));
    }

    #[test]
    fn membership_join_duplicate() {
        let mut store = ChatMembershipStore::default();
        store.join_room(1, 100, MemberRole::Member, "t".into());
        assert!(!store.join_room(1, 100, MemberRole::Owner, "t".into()));
    }

    #[test]
    fn membership_leave() {
        let mut store = ChatMembershipStore::default();
        store.join_room(1, 100, MemberRole::Member, "t".into());
        assert!(store.leave_room(1, 100));
        assert!(!store.is_member(1, 100));
        assert!(!store.leave_room(1, 100)); // already left
    }

    #[test]
    fn membership_get_members() {
        let mut store = ChatMembershipStore::default();
        store.join_room(1, 100, MemberRole::Owner, "t".into());
        store.join_room(2, 100, MemberRole::Member, "t".into());
        store.join_room(3, 200, MemberRole::Member, "t".into());
        assert_eq!(store.get_members(100).len(), 2);
        assert_eq!(store.get_members(200).len(), 1);
    }

    #[test]
    fn membership_get_user_rooms() {
        let mut store = ChatMembershipStore::default();
        store.join_room(1, 100, MemberRole::Member, "t".into());
        store.join_room(1, 200, MemberRole::Owner, "t".into());
        let rooms = store.get_user_rooms(1);
        assert_eq!(rooms.len(), 2);
    }

    #[test]
    fn membership_update_role() {
        let mut store = ChatMembershipStore::default();
        store.join_room(1, 100, MemberRole::Member, "t".into());
        assert!(store.update_role(1, 100, MemberRole::Moderator));
        let members = store.get_members(100);
        assert_eq!(members[0].role, MemberRole::Moderator);
    }

    #[test]
    fn membership_update_role_nonexistent() {
        let mut store = ChatMembershipStore::default();
        assert!(!store.update_role(99, 100, MemberRole::Owner));
    }

    #[test]
    fn membership_update_last_read() {
        let mut store = ChatMembershipStore::default();
        store.join_room(1, 100, MemberRole::Member, "t".into());
        assert!(store.update_last_read(1, 100, 42));
        let members = store.get_members(100);
        assert_eq!(members[0].last_read_message_id, Some(42));
    }

    #[test]
    fn membership_update_last_read_nonexistent() {
        let mut store = ChatMembershipStore::default();
        assert!(!store.update_last_read(1, 100, 5));
    }

    // ── ModerationStore ───────────────────────────────────────────────

    #[test]
    fn moderation_add_and_get_actions() {
        let mut store = ModerationStore::default();
        store.add_action(
            1,
            10,
            99,
            ModerationAction::Warn,
            "spamming".into(),
            "2025-01-01T00:00:00Z".into(),
            None,
        );
        store.add_action(
            1,
            10,
            99,
            ModerationAction::Mute,
            "continued spam".into(),
            "2025-01-02T00:00:00Z".into(),
            Some("2025-01-03T00:00:00Z".into()),
        );
        let actions = store.get_user_actions(10);
        assert_eq!(actions.len(), 2);
    }

    #[test]
    fn moderation_is_muted() {
        let mut store = ModerationStore::default();
        store.add_action(
            1,
            10,
            99,
            ModerationAction::Mute,
            "x".into(),
            "2025-01-01T00:00:00Z".into(),
            Some("2025-06-01T00:00:00Z".into()),
        );
        assert!(store.is_muted(10, 1, "2025-03-01T00:00:00Z"));
        assert!(!store.is_muted(10, 1, "2025-07-01T00:00:00Z")); // expired
        assert!(!store.is_muted(10, 2, "2025-03-01T00:00:00Z")); // different room
    }

    #[test]
    fn moderation_is_muted_permanent() {
        let mut store = ModerationStore::default();
        store.add_action(
            1,
            10,
            99,
            ModerationAction::Mute,
            "perm".into(),
            "2025-01-01T00:00:00Z".into(),
            None,
        );
        assert!(store.is_muted(10, 1, "2099-01-01T00:00:00Z"));
    }

    #[test]
    fn moderation_is_banned() {
        let mut store = ModerationStore::default();
        store.add_action(
            1,
            10,
            99,
            ModerationAction::Ban,
            "toxic".into(),
            "2025-01-01T00:00:00Z".into(),
            Some("2025-12-31T00:00:00Z".into()),
        );
        assert!(store.is_banned(10, 1, "2025-06-01T00:00:00Z"));
        assert!(!store.is_banned(10, 1, "2026-01-01T00:00:00Z"));
    }

    #[test]
    fn moderation_is_banned_not_banned() {
        let mut store = ModerationStore::default();
        store.add_action(
            1,
            10,
            99,
            ModerationAction::Warn,
            "x".into(),
            "t".into(),
            None,
        );
        assert!(!store.is_banned(10, 1, "2025-01-01T00:00:00Z"));
    }

    #[test]
    fn moderation_cleanup_expired() {
        let mut store = ModerationStore::default();
        store.add_action(
            1,
            10,
            99,
            ModerationAction::Mute,
            "a".into(),
            "2025-01-01T00:00:00Z".into(),
            Some("2025-02-01T00:00:00Z".into()),
        );
        store.add_action(
            1,
            11,
            99,
            ModerationAction::Ban,
            "b".into(),
            "2025-01-01T00:00:00Z".into(),
            Some("2025-12-01T00:00:00Z".into()),
        );
        store.add_action(
            1,
            12,
            99,
            ModerationAction::Warn,
            "c".into(),
            "2025-01-01T00:00:00Z".into(),
            None,
        ); // permanent
        let removed = store.cleanup_expired("2025-06-01T00:00:00Z");
        assert_eq!(removed, 1);
        assert_eq!(store.get_user_actions(10).len(), 0);
        assert_eq!(store.get_user_actions(11).len(), 1);
        assert_eq!(store.get_user_actions(12).len(), 1);
    }

    // ── ProfanityFilter ───────────────────────────────────────────────

    #[test]
    fn profanity_default_has_words() {
        let filter = ProfanityFilter::default();
        assert!(!filter.banned_words.is_empty());
    }

    #[test]
    fn profanity_filter_message() {
        let filter = ProfanityFilter::default();
        let result = filter.filter_message("This is a badword in a sentence");
        assert_eq!(result, "This is a ******* in a sentence");
    }

    #[test]
    fn profanity_filter_case_insensitive() {
        let filter = ProfanityFilter::default();
        let result = filter.filter_message("BADWORD here");
        assert_eq!(result, "******* here");
    }

    #[test]
    fn profanity_contains_profanity() {
        let filter = ProfanityFilter::default();
        assert!(filter.contains_profanity("contains badword here"));
        assert!(!filter.contains_profanity("this is clean text"));
    }

    #[test]
    fn profanity_contains_profanity_case_insensitive() {
        let filter = ProfanityFilter::default();
        assert!(filter.contains_profanity("contains OFFENSIVE stuff"));
    }

    #[test]
    fn profanity_whole_word_only() {
        let filter = ProfanityFilter::default();
        // "badword" is in the list, but "badwording" should not be matched
        assert!(!filter.contains_profanity("badwording is not matched"));
        assert!(filter.contains_profanity("badword is matched"));
    }

    #[test]
    fn profanity_add_word() {
        let mut filter = ProfanityFilter::default();
        filter.add_word("newbad");
        assert!(filter.contains_profanity("this is newbad"));
    }

    #[test]
    fn profanity_add_duplicate_word() {
        let mut filter = ProfanityFilter::default();
        let before = filter.banned_words.len();
        filter.add_word("badword");
        assert_eq!(filter.banned_words.len(), before);
    }

    #[test]
    fn profanity_remove_word() {
        let mut filter = ProfanityFilter::default();
        filter.remove_word("badword");
        assert!(!filter.contains_profanity("badword is now ok"));
    }

    #[test]
    fn profanity_filter_multiple_words() {
        let filter = ProfanityFilter::default();
        let result = filter.filter_message("badword and offensive together");
        assert!(result.contains("*******"));
        assert!(result.contains("*********"));
        assert!(!result.contains("badword"));
        assert!(!result.contains("offensive"));
    }

    #[test]
    fn profanity_no_match_clean_text() {
        let filter = ProfanityFilter::default();
        let text = "Hello commander, good game!";
        assert_eq!(filter.filter_message(text), text);
    }

    // ── ChatRateLimiter ───────────────────────────────────────────────

    #[test]
    fn rate_limiter_allows_under_limit() {
        let mut limiter = ChatRateLimiter::new(5, 60);
        for i in 0..5 {
            assert!(limiter.check_rate(1, 1000 + i));
        }
    }

    #[test]
    fn rate_limiter_blocks_over_limit() {
        let mut limiter = ChatRateLimiter::new(3, 60);
        assert!(limiter.check_rate(1, 1000));
        assert!(limiter.check_rate(1, 1001));
        assert!(limiter.check_rate(1, 1002));
        assert!(!limiter.check_rate(1, 1003));
    }

    #[test]
    fn rate_limiter_window_expiry() {
        let mut limiter = ChatRateLimiter::new(2, 10);
        assert!(limiter.check_rate(1, 100));
        assert!(limiter.check_rate(1, 101));
        assert!(!limiter.check_rate(1, 102)); // blocked
        assert!(limiter.check_rate(1, 111)); // old timestamps expired
    }

    #[test]
    fn rate_limiter_independent_users() {
        let mut limiter = ChatRateLimiter::new(1, 60);
        assert!(limiter.check_rate(1, 1000));
        assert!(!limiter.check_rate(1, 1001)); // user 1 blocked
        assert!(limiter.check_rate(2, 1001)); // user 2 still ok
    }

    // ── Serialization ─────────────────────────────────────────────────

    #[test]
    fn chat_room_type_serialization() {
        let rt = ChatRoomType::Alliance;
        let json = serde_json::to_string(&rt).unwrap();
        let deserialized: ChatRoomType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, rt);
    }

    #[test]
    fn chat_message_serialization() {
        let msg = ChatMessage {
            id: 1,
            room_id: 2,
            sender_id: 3,
            sender_name: "Alice".into(),
            content: "Hello".into(),
            message_type: MessageType::Text,
            created_at: "2025-01-01T00:00:00Z".into(),
            edited_at: None,
            is_deleted: false,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: ChatMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, msg.id);
        assert_eq!(deserialized.content, msg.content);
    }

    #[test]
    fn chat_room_serialization() {
        let room = ChatRoom {
            id: 1,
            name: "Global".into(),
            room_type: ChatRoomType::Global,
            created_by: 1,
            created_at: "2025-01-01T00:00:00Z".into(),
            max_members: 100,
            is_archived: false,
        };
        let json = serde_json::to_string(&room).unwrap();
        let deserialized: ChatRoom = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "Global");
    }

    #[test]
    fn member_role_serialization() {
        let role = MemberRole::Moderator;
        let json = serde_json::to_string(&role).unwrap();
        let deserialized: MemberRole = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, role);
    }

    #[test]
    fn moderation_action_serialization() {
        let action = ModerationAction::Ban;
        let json = serde_json::to_string(&action).unwrap();
        let deserialized: ModerationAction = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, action);
    }

    #[test]
    fn profanity_filter_serialization() {
        let filter = ProfanityFilter::default();
        let json = serde_json::to_string(&filter).unwrap();
        let deserialized: ProfanityFilter = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.banned_words, filter.banned_words);
    }

    #[test]
    fn rate_limiter_new_configuration() {
        let limiter = ChatRateLimiter::new(10, 30);
        assert_eq!(limiter.max_messages_per_window, 10);
        assert_eq!(limiter.window_seconds, 30);
    }

    #[test]
    fn chat_restriction_serialization() {
        let restriction = ChatRestriction {
            user_id: 5,
            reason: "test".into(),
            created_at_unix: 1_000_000,
            expires_at_unix: 2_000_000,
        };
        let json = serde_json::to_string(&restriction).unwrap();
        assert!(json.contains("userId")); // camelCase
        let deserialized: ChatRestriction = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.user_id, 5);
    }

    #[test]
    fn message_type_variants_serialization() {
        for mt in [
            MessageType::Text,
            MessageType::System,
            MessageType::Emote,
            MessageType::Trade,
        ] {
            let json = serde_json::to_string(&mt).unwrap();
            let back: MessageType = serde_json::from_str(&json).unwrap();
            assert_eq!(back, mt);
        }
    }
}
