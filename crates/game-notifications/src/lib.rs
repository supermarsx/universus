#![forbid(unsafe_code)]

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewNotification {
    pub title: String,
    pub message: String,
    pub category: String,
    pub priority: u8,
}

#[derive(Default)]
pub struct NotificationStore {
    next_id: i64,
    by_user: HashMap<i64, Vec<Notification>>,
}

impl NotificationStore {
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

fn now_timestamp() -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("unix:{ts}")
}

#[cfg(test)]
mod tests {
    use super::{NewNotification, NotificationStore};

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
}
