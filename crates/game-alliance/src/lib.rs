#![forbid(unsafe_code)]

use std::collections::HashMap;

use serde::Serialize;

pub fn crate_name() -> &'static str {
    "game-alliance"
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub enum AllianceRole {
    Leader,
    Officer,
    Member,
    Applicant,
}

#[derive(Clone, Debug, Serialize)]
pub struct Alliance {
    pub id: i64,
    pub name: String,
    pub tag: String,
    pub description: String,
    pub leader_id: i64,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct AllianceMember {
    pub user_id: i64,
    pub alliance_id: i64,
    pub role: AllianceRole,
    pub joined_at: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub enum DiplomacyRelation {
    Neutral,
    Alliance,
    War,
    Nap(String),
}

#[derive(Clone, Debug, Serialize)]
pub struct DiplomacyEntry {
    pub alliance_id: i64,
    pub target_alliance_id: i64,
    pub relation: DiplomacyRelation,
}

pub struct AllianceStore {
    alliances: HashMap<i64, Alliance>,
    /// Keyed by alliance_id, then user_id.
    members: HashMap<i64, HashMap<i64, AllianceMember>>,
    /// Keyed by (alliance_id, target_alliance_id).
    diplomacy: HashMap<(i64, i64), DiplomacyEntry>,
    next_id: i64,
}

impl AllianceStore {
    pub fn new() -> Self {
        let mut store = Self {
            alliances: HashMap::new(),
            members: HashMap::new(),
            diplomacy: HashMap::new(),
            next_id: 1,
        };
        store.create_alliance(
            "Galactic Order".to_string(),
            "GO".to_string(),
            "The founding alliance.".to_string(),
            1,
        );
        store
    }

    pub fn create_alliance(
        &mut self,
        name: String,
        tag: String,
        description: String,
        leader_id: i64,
    ) -> Alliance {
        let id = self.next_id;
        self.next_id += 1;
        let alliance = Alliance {
            id,
            name,
            tag,
            description,
            leader_id,
            created_at: now_timestamp(),
        };
        self.alliances.insert(id, alliance.clone());
        // Automatically add the leader as a member.
        self.members.entry(id).or_default().insert(
            leader_id,
            AllianceMember {
                user_id: leader_id,
                alliance_id: id,
                role: AllianceRole::Leader,
                joined_at: now_timestamp(),
            },
        );
        alliance
    }

    pub fn get_alliance(&self, id: i64) -> Option<Alliance> {
        self.alliances.get(&id).cloned()
    }

    pub fn list_alliances(&self) -> Vec<Alliance> {
        let mut list: Vec<Alliance> = self.alliances.values().cloned().collect();
        list.sort_by_key(|a| a.id);
        list
    }

    pub fn add_member(&mut self, alliance_id: i64, user_id: i64, role: AllianceRole) -> bool {
        if !self.alliances.contains_key(&alliance_id) {
            return false;
        }
        let roster = self.members.entry(alliance_id).or_default();
        if roster.contains_key(&user_id) {
            return false;
        }
        roster.insert(
            user_id,
            AllianceMember {
                user_id,
                alliance_id,
                role,
                joined_at: now_timestamp(),
            },
        );
        true
    }

    pub fn remove_member(&mut self, alliance_id: i64, user_id: i64) -> bool {
        self.members
            .get_mut(&alliance_id)
            .map_or(false, |roster| roster.remove(&user_id).is_some())
    }

    pub fn list_members(&self, alliance_id: i64) -> Vec<AllianceMember> {
        self.members.get(&alliance_id).map_or_else(Vec::new, |roster| {
            let mut list: Vec<AllianceMember> = roster.values().cloned().collect();
            list.sort_by_key(|m| m.user_id);
            list
        })
    }

    /// Promotes a member one tier: Member → Officer → Leader.
    /// When a member is promoted to Leader the previous leader is demoted to Officer.
    pub fn promote_member(&mut self, alliance_id: i64, user_id: i64) -> bool {
        let roster = match self.members.get_mut(&alliance_id) {
            Some(r) => r,
            None => return false,
        };
        let member = match roster.get_mut(&user_id) {
            Some(m) => m,
            None => return false,
        };
        match member.role {
            AllianceRole::Member => {
                member.role = AllianceRole::Officer;
                true
            }
            AllianceRole::Officer => {
                member.role = AllianceRole::Leader;
                // Demote the previous leader to Officer.
                if let Some(alliance) = self.alliances.get_mut(&alliance_id) {
                    let old_leader_id = alliance.leader_id;
                    alliance.leader_id = user_id;
                    if old_leader_id != user_id {
                        if let Some(old_leader) = roster.get_mut(&old_leader_id) {
                            old_leader.role = AllianceRole::Officer;
                        }
                    }
                }
                true
            }
            _ => false,
        }
    }

    pub fn set_diplomacy(
        &mut self,
        alliance_id: i64,
        target_id: i64,
        relation: DiplomacyRelation,
    ) {
        let entry = DiplomacyEntry {
            alliance_id,
            target_alliance_id: target_id,
            relation,
        };
        self.diplomacy.insert((alliance_id, target_id), entry);
    }

    pub fn get_diplomacy(&self, alliance_id: i64, target_id: i64) -> DiplomacyRelation {
        self.diplomacy
            .get(&(alliance_id, target_id))
            .map_or(DiplomacyRelation::Neutral, |e| e.relation.clone())
    }
}

impl Default for AllianceStore {
    fn default() -> Self {
        Self::new()
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

    #[test]
    fn crate_name_returns_expected() {
        assert_eq!(crate_name(), "game-alliance");
    }

    #[test]
    fn new_store_seeds_default_alliance() {
        let store = AllianceStore::new();
        let alliances = store.list_alliances();
        assert_eq!(alliances.len(), 1);
        assert_eq!(alliances[0].name, "Galactic Order");
        assert_eq!(alliances[0].tag, "GO");
        assert_eq!(alliances[0].leader_id, 1);
    }

    #[test]
    fn create_and_get_alliance() {
        let mut store = AllianceStore::new();
        let a = store.create_alliance(
            "Star Legion".to_string(),
            "SL".to_string(),
            "A mighty legion.".to_string(),
            42,
        );
        assert_eq!(a.id, 2);
        let fetched = store.get_alliance(2).unwrap();
        assert_eq!(fetched.name, "Star Legion");
        assert!(store.get_alliance(999).is_none());
    }

    #[test]
    fn leader_auto_added_on_create() {
        let mut store = AllianceStore::new();
        store.create_alliance("X".to_string(), "X".to_string(), "X".to_string(), 55);
        let members = store.list_members(2);
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].user_id, 55);
        assert_eq!(members[0].role, AllianceRole::Leader);
    }

    #[test]
    fn add_and_remove_member() {
        let mut store = AllianceStore::new();
        assert!(store.add_member(1, 10, AllianceRole::Member));
        // Duplicate add returns false.
        assert!(!store.add_member(1, 10, AllianceRole::Member));
        assert_eq!(store.list_members(1).len(), 2);
        assert!(store.remove_member(1, 10));
        assert!(!store.remove_member(1, 10));
        // Adding to non-existent alliance returns false.
        assert!(!store.add_member(999, 10, AllianceRole::Member));
    }

    #[test]
    fn promote_member_to_officer() {
        let mut store = AllianceStore::new();
        store.add_member(1, 10, AllianceRole::Member);
        assert!(store.promote_member(1, 10));
        let members = store.list_members(1);
        let m = members.iter().find(|m| m.user_id == 10).unwrap();
        assert_eq!(m.role, AllianceRole::Officer);
    }

    #[test]
    fn promote_officer_to_leader_demotes_old_leader() {
        let mut store = AllianceStore::new();
        store.add_member(1, 10, AllianceRole::Officer);
        assert!(store.promote_member(1, 10));
        let members = store.list_members(1);
        let new_leader = members.iter().find(|m| m.user_id == 10).unwrap();
        assert_eq!(new_leader.role, AllianceRole::Leader);
        let old_leader = members.iter().find(|m| m.user_id == 1).unwrap();
        assert_eq!(old_leader.role, AllianceRole::Officer);
        assert_eq!(store.get_alliance(1).unwrap().leader_id, 10);
    }

    #[test]
    fn promote_applicant_fails() {
        let mut store = AllianceStore::new();
        store.add_member(1, 20, AllianceRole::Applicant);
        assert!(!store.promote_member(1, 20));
    }

    #[test]
    fn diplomacy_defaults_to_neutral() {
        let store = AllianceStore::new();
        assert_eq!(store.get_diplomacy(1, 99), DiplomacyRelation::Neutral);
    }

    #[test]
    fn set_and_get_diplomacy() {
        let mut store = AllianceStore::new();
        store.create_alliance("B".to_string(), "B".to_string(), "B".to_string(), 2);
        store.set_diplomacy(1, 2, DiplomacyRelation::War);
        assert_eq!(store.get_diplomacy(1, 2), DiplomacyRelation::War);
        assert_eq!(store.get_diplomacy(2, 1), DiplomacyRelation::Neutral);
        store.set_diplomacy(1, 2, DiplomacyRelation::Nap("2026-12-31".to_string()));
        assert_eq!(
            store.get_diplomacy(1, 2),
            DiplomacyRelation::Nap("2026-12-31".to_string())
        );
    }

    #[test]
    fn list_members_empty_for_unknown_alliance() {
        let store = AllianceStore::new();
        assert!(store.list_members(999).is_empty());
    }
}
