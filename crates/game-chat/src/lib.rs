#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

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
}

#[cfg(test)]
mod tests {
    use super::{ChatRestriction, ChatRestrictionStore};

    #[test]
    fn cleanup_expired_removes_only_expired_entries() {
        let mut store = ChatRestrictionStore::with_seed();
        let removed = store.cleanup_expired(1_739_427_500);
        assert_eq!(removed, 1);
        assert_eq!(store.list().len(), 1);
        assert_eq!(store.list()[0].user_id, 22);
    }

    #[test]
    fn add_then_cleanup_keeps_future_restriction() {
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
}
