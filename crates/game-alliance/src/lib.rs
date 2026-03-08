#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AllianceRole {
    Applicant = 0,
    Member = 1,
    Veteran = 2,
    Officer = 3,
    CoLeader = 4,
    Leader = 5,
}

impl AllianceRole {
    /// Returns a numeric authority level (higher = more authority).
    pub fn authority(self) -> u8 {
        self as u8
    }
}

impl fmt::Display for AllianceRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            AllianceRole::Leader => "Leader",
            AllianceRole::CoLeader => "CoLeader",
            AllianceRole::Officer => "Officer",
            AllianceRole::Veteran => "Veteran",
            AllianceRole::Member => "Member",
            AllianceRole::Applicant => "Applicant",
        };
        write!(f, "{s}")
    }
}

impl FromStr for AllianceRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "leader" => Ok(AllianceRole::Leader),
            "coleader" | "co_leader" | "co-leader" => Ok(AllianceRole::CoLeader),
            "officer" => Ok(AllianceRole::Officer),
            "veteran" => Ok(AllianceRole::Veteran),
            "member" => Ok(AllianceRole::Member),
            "applicant" => Ok(AllianceRole::Applicant),
            other => Err(format!("unknown alliance role: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ApplicationStatus {
    Pending,
    Accepted,
    Rejected,
    Cancelled,
}

impl fmt::Display for ApplicationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ApplicationStatus::Pending => "Pending",
            ApplicationStatus::Accepted => "Accepted",
            ApplicationStatus::Rejected => "Rejected",
            ApplicationStatus::Cancelled => "Cancelled",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DiplomacyRelation {
    War,
    Peace,
    Alliance,
    Trade,
}

impl fmt::Display for DiplomacyRelation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            DiplomacyRelation::War => "War",
            DiplomacyRelation::Peace => "Peace",
            DiplomacyRelation::Alliance => "Alliance",
            DiplomacyRelation::Trade => "Trade",
        };
        write!(f, "{s}")
    }
}

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AllianceError {
    NotFound,
    TagTaken,
    TagTooLong,
    TagTooShort,
    AlreadyInAlliance,
    NotAMember,
    InsufficientRank,
    AllianceFull,
    CannotRemoveLeader,
    ApplicationNotFound,
    DuplicateApplication,
    SelfDiplomacy,
}

impl fmt::Display for AllianceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            AllianceError::NotFound => "alliance not found",
            AllianceError::TagTaken => "alliance tag already taken",
            AllianceError::TagTooLong => "alliance tag too long (max 8 chars)",
            AllianceError::TagTooShort => "alliance tag too short (min 2 chars)",
            AllianceError::AlreadyInAlliance => "player is already in an alliance",
            AllianceError::NotAMember => "player is not a member of this alliance",
            AllianceError::InsufficientRank => "insufficient rank for this action",
            AllianceError::AllianceFull => "alliance has reached max members",
            AllianceError::CannotRemoveLeader => "cannot remove leader; transfer leadership first",
            AllianceError::ApplicationNotFound => "application not found",
            AllianceError::DuplicateApplication => "player already has a pending application",
            AllianceError::SelfDiplomacy => "cannot create diplomacy with own alliance",
        };
        write!(f, "{s}")
    }
}

// ---------------------------------------------------------------------------
// Data structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Alliance {
    pub id: i64,
    pub tag: String,
    pub name: String,
    pub founder_id: i64,
    pub description: Option<String>,
    pub logo_url: Option<String>,
    pub is_open: bool,
    pub member_count: i32,
    pub max_members: i32,
    pub score: i64,
    pub rank: Option<i32>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AllianceMember {
    pub player_id: i64,
    pub alliance_id: i64,
    pub role: AllianceRole,
    pub joined_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AllianceApplication {
    pub id: i64,
    pub player_id: i64,
    pub alliance_id: i64,
    pub message: Option<String>,
    pub status: ApplicationStatus,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiplomacyPact {
    pub id: i64,
    pub alliance_a_id: i64,
    pub alliance_b_id: i64,
    pub relation: DiplomacyRelation,
    pub proposed_by_id: i64,
    pub accepted: bool,
    pub created_at: String,
    pub expires_at: Option<String>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn now_timestamp() -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("unix:{ts}")
}

// ---------------------------------------------------------------------------
// AllianceStore — in-memory store
// ---------------------------------------------------------------------------

pub struct AllianceStore {
    alliances: HashMap<i64, Alliance>,
    /// (alliance_id, player_id) -> member
    members: HashMap<(i64, i64), AllianceMember>,
    /// player_id -> alliance_id  (quick lookup)
    player_alliance: HashMap<i64, i64>,
    applications: HashMap<i64, AllianceApplication>,
    pacts: HashMap<i64, DiplomacyPact>,
    next_alliance_id: i64,
    next_application_id: i64,
    next_pact_id: i64,
}

impl AllianceStore {
    pub fn new() -> Self {
        Self {
            alliances: HashMap::new(),
            members: HashMap::new(),
            player_alliance: HashMap::new(),
            applications: HashMap::new(),
            pacts: HashMap::new(),
            next_alliance_id: 1,
            next_application_id: 1,
            next_pact_id: 1,
        }
    }

    // -----------------------------------------------------------------------
    // Alliance CRUD
    // -----------------------------------------------------------------------

    pub fn create_alliance(
        &mut self,
        tag: &str,
        name: &str,
        founder_id: i64,
    ) -> Result<Alliance, AllianceError> {
        let tag = tag.trim().to_uppercase();
        if tag.len() < 2 {
            return Err(AllianceError::TagTooShort);
        }
        if tag.len() > 8 {
            return Err(AllianceError::TagTooLong);
        }
        if self.alliances.values().any(|a| a.tag == tag) {
            return Err(AllianceError::TagTaken);
        }
        if self.player_alliance.contains_key(&founder_id) {
            return Err(AllianceError::AlreadyInAlliance);
        }

        let id = self.next_alliance_id;
        self.next_alliance_id += 1;

        let now = now_timestamp();

        let alliance = Alliance {
            id,
            tag: tag.clone(),
            name: name.to_string(),
            founder_id,
            description: None,
            logo_url: None,
            is_open: false,
            member_count: 1,
            max_members: 50,
            score: 0,
            rank: None,
            created_at: now.clone(),
        };

        self.alliances.insert(id, alliance.clone());

        // Auto-add founder as Leader
        let member = AllianceMember {
            player_id: founder_id,
            alliance_id: id,
            role: AllianceRole::Leader,
            joined_at: now,
        };
        self.members.insert((id, founder_id), member);
        self.player_alliance.insert(founder_id, id);

        Ok(alliance)
    }

    pub fn disband_alliance(
        &mut self,
        alliance_id: i64,
        requester_id: i64,
    ) -> Result<(), AllianceError> {
        let alliance = self
            .alliances
            .get(&alliance_id)
            .ok_or(AllianceError::NotFound)?;
        if alliance.founder_id != requester_id {
            // Also check if they are current leader
            let member = self
                .members
                .get(&(alliance_id, requester_id))
                .ok_or(AllianceError::NotAMember)?;
            if member.role != AllianceRole::Leader {
                return Err(AllianceError::InsufficientRank);
            }
        }

        // Remove all members
        let member_keys: Vec<(i64, i64)> = self
            .members
            .keys()
            .filter(|(aid, _)| *aid == alliance_id)
            .copied()
            .collect();
        for key in member_keys {
            self.player_alliance.remove(&key.1);
            self.members.remove(&key);
        }

        // Remove pending applications
        let app_ids: Vec<i64> = self
            .applications
            .iter()
            .filter(|(_, app)| app.alliance_id == alliance_id)
            .map(|(id, _)| *id)
            .collect();
        for id in app_ids {
            self.applications.remove(&id);
        }

        // Remove diplomacy pacts
        let pact_ids: Vec<i64> = self
            .pacts
            .iter()
            .filter(|(_, p)| p.alliance_a_id == alliance_id || p.alliance_b_id == alliance_id)
            .map(|(id, _)| *id)
            .collect();
        for id in pact_ids {
            self.pacts.remove(&id);
        }

        self.alliances.remove(&alliance_id);
        Ok(())
    }

    pub fn get_alliance(&self, id: i64) -> Option<&Alliance> {
        self.alliances.get(&id)
    }

    pub fn find_alliance_by_tag(&self, tag: &str) -> Option<&Alliance> {
        let tag_upper = tag.trim().to_uppercase();
        self.alliances.values().find(|a| a.tag == tag_upper)
    }

    pub fn list_alliances(&self) -> Vec<&Alliance> {
        self.alliances.values().collect()
    }

    // -----------------------------------------------------------------------
    // Membership
    // -----------------------------------------------------------------------

    pub fn add_member(
        &mut self,
        alliance_id: i64,
        player_id: i64,
        role: AllianceRole,
    ) -> Result<(), AllianceError> {
        let alliance = self
            .alliances
            .get(&alliance_id)
            .ok_or(AllianceError::NotFound)?;
        if alliance.member_count >= alliance.max_members {
            return Err(AllianceError::AllianceFull);
        }
        if self.player_alliance.contains_key(&player_id) {
            return Err(AllianceError::AlreadyInAlliance);
        }

        let now = now_timestamp();
        let member = AllianceMember {
            player_id,
            alliance_id,
            role,
            joined_at: now,
        };
        self.members.insert((alliance_id, player_id), member);
        self.player_alliance.insert(player_id, alliance_id);

        if let Some(a) = self.alliances.get_mut(&alliance_id) {
            a.member_count += 1;
        }
        Ok(())
    }

    pub fn remove_member(&mut self, alliance_id: i64, player_id: i64) -> Result<(), AllianceError> {
        self.alliances
            .get(&alliance_id)
            .ok_or(AllianceError::NotFound)?;

        let member = self
            .members
            .get(&(alliance_id, player_id))
            .ok_or(AllianceError::NotAMember)?;

        if member.role == AllianceRole::Leader {
            return Err(AllianceError::CannotRemoveLeader);
        }

        self.members.remove(&(alliance_id, player_id));
        self.player_alliance.remove(&player_id);

        if let Some(a) = self.alliances.get_mut(&alliance_id) {
            a.member_count -= 1;
        }
        Ok(())
    }

    pub fn change_role(
        &mut self,
        alliance_id: i64,
        requester_id: i64,
        target_id: i64,
        new_role: AllianceRole,
    ) -> Result<(), AllianceError> {
        self.alliances
            .get(&alliance_id)
            .ok_or(AllianceError::NotFound)?;

        let requester = self
            .members
            .get(&(alliance_id, requester_id))
            .ok_or(AllianceError::NotAMember)?;
        let target = self
            .members
            .get(&(alliance_id, target_id))
            .ok_or(AllianceError::NotAMember)?;

        // Requester must have strictly higher authority than both the target's
        // current role and the new role being assigned.
        if requester.role.authority() <= target.role.authority() {
            return Err(AllianceError::InsufficientRank);
        }
        if requester.role.authority() <= new_role.authority() {
            return Err(AllianceError::InsufficientRank);
        }

        if let Some(m) = self.members.get_mut(&(alliance_id, target_id)) {
            m.role = new_role;
        }
        Ok(())
    }

    pub fn transfer_leadership(
        &mut self,
        alliance_id: i64,
        current_leader_id: i64,
        new_leader_id: i64,
    ) -> Result<(), AllianceError> {
        self.alliances
            .get(&alliance_id)
            .ok_or(AllianceError::NotFound)?;

        let current = self
            .members
            .get(&(alliance_id, current_leader_id))
            .ok_or(AllianceError::NotAMember)?;

        if current.role != AllianceRole::Leader {
            return Err(AllianceError::InsufficientRank);
        }

        self.members
            .get(&(alliance_id, new_leader_id))
            .ok_or(AllianceError::NotAMember)?;

        // Demote current leader to CoLeader
        if let Some(m) = self.members.get_mut(&(alliance_id, current_leader_id)) {
            m.role = AllianceRole::CoLeader;
        }
        // Promote new leader
        if let Some(m) = self.members.get_mut(&(alliance_id, new_leader_id)) {
            m.role = AllianceRole::Leader;
        }
        Ok(())
    }

    pub fn get_members(&self, alliance_id: i64) -> Vec<&AllianceMember> {
        self.members
            .iter()
            .filter(|((aid, _), _)| *aid == alliance_id)
            .map(|(_, m)| m)
            .collect()
    }

    pub fn get_player_alliance(&self, player_id: i64) -> Option<i64> {
        self.player_alliance.get(&player_id).copied()
    }

    // -----------------------------------------------------------------------
    // Applications
    // -----------------------------------------------------------------------

    pub fn apply(
        &mut self,
        player_id: i64,
        alliance_id: i64,
        message: Option<String>,
    ) -> Result<i64, AllianceError> {
        self.alliances
            .get(&alliance_id)
            .ok_or(AllianceError::NotFound)?;

        if self.player_alliance.contains_key(&player_id) {
            return Err(AllianceError::AlreadyInAlliance);
        }

        // Check for existing pending application from same player to same alliance
        let has_pending = self.applications.values().any(|app| {
            app.player_id == player_id
                && app.alliance_id == alliance_id
                && app.status == ApplicationStatus::Pending
        });
        if has_pending {
            return Err(AllianceError::DuplicateApplication);
        }

        let id = self.next_application_id;
        self.next_application_id += 1;

        let app = AllianceApplication {
            id,
            player_id,
            alliance_id,
            message,
            status: ApplicationStatus::Pending,
            created_at: now_timestamp(),
            resolved_at: None,
        };
        self.applications.insert(id, app);
        Ok(id)
    }

    pub fn accept_application(
        &mut self,
        application_id: i64,
        officer_id: i64,
    ) -> Result<(), AllianceError> {
        let app = self
            .applications
            .get(&application_id)
            .ok_or(AllianceError::ApplicationNotFound)?;

        if app.status != ApplicationStatus::Pending {
            return Err(AllianceError::ApplicationNotFound);
        }

        let alliance_id = app.alliance_id;
        let player_id = app.player_id;

        // Verify officer has sufficient rank (Officer+)
        let officer = self
            .members
            .get(&(alliance_id, officer_id))
            .ok_or(AllianceError::NotAMember)?;
        if officer.role.authority() < AllianceRole::Officer.authority() {
            return Err(AllianceError::InsufficientRank);
        }

        // Add the member
        self.add_member(alliance_id, player_id, AllianceRole::Member)?;

        // Update application
        if let Some(a) = self.applications.get_mut(&application_id) {
            a.status = ApplicationStatus::Accepted;
            a.resolved_at = Some(now_timestamp());
        }
        Ok(())
    }

    pub fn reject_application(
        &mut self,
        application_id: i64,
        officer_id: i64,
    ) -> Result<(), AllianceError> {
        let app = self
            .applications
            .get(&application_id)
            .ok_or(AllianceError::ApplicationNotFound)?;

        if app.status != ApplicationStatus::Pending {
            return Err(AllianceError::ApplicationNotFound);
        }

        let alliance_id = app.alliance_id;

        // Verify officer has sufficient rank (Officer+)
        let officer = self
            .members
            .get(&(alliance_id, officer_id))
            .ok_or(AllianceError::NotAMember)?;
        if officer.role.authority() < AllianceRole::Officer.authority() {
            return Err(AllianceError::InsufficientRank);
        }

        if let Some(a) = self.applications.get_mut(&application_id) {
            a.status = ApplicationStatus::Rejected;
            a.resolved_at = Some(now_timestamp());
        }
        Ok(())
    }

    pub fn list_applications(&self, alliance_id: i64) -> Vec<&AllianceApplication> {
        self.applications
            .values()
            .filter(|app| app.alliance_id == alliance_id)
            .collect()
    }

    // -----------------------------------------------------------------------
    // Diplomacy
    // -----------------------------------------------------------------------

    pub fn propose_diplomacy(
        &mut self,
        alliance_a: i64,
        alliance_b: i64,
        relation: DiplomacyRelation,
        proposed_by: i64,
    ) -> Result<i64, AllianceError> {
        if alliance_a == alliance_b {
            return Err(AllianceError::SelfDiplomacy);
        }
        self.alliances
            .get(&alliance_a)
            .ok_or(AllianceError::NotFound)?;
        self.alliances
            .get(&alliance_b)
            .ok_or(AllianceError::NotFound)?;

        // Verify proposer is a member of alliance_a with at least Officer rank
        let proposer = self
            .members
            .get(&(alliance_a, proposed_by))
            .ok_or(AllianceError::NotAMember)?;
        if proposer.role.authority() < AllianceRole::Officer.authority() {
            return Err(AllianceError::InsufficientRank);
        }

        let id = self.next_pact_id;
        self.next_pact_id += 1;

        let pact = DiplomacyPact {
            id,
            alliance_a_id: alliance_a,
            alliance_b_id: alliance_b,
            relation,
            proposed_by_id: proposed_by,
            accepted: false,
            created_at: now_timestamp(),
            expires_at: None,
        };
        self.pacts.insert(id, pact);
        Ok(id)
    }

    pub fn accept_diplomacy(
        &mut self,
        pact_id: i64,
        acceptor_alliance_id: i64,
    ) -> Result<(), AllianceError> {
        let pact = self.pacts.get(&pact_id).ok_or(AllianceError::NotFound)?;

        // The acceptor must be alliance_b (the side that didn't propose)
        if pact.alliance_b_id != acceptor_alliance_id {
            return Err(AllianceError::InsufficientRank);
        }

        if let Some(p) = self.pacts.get_mut(&pact_id) {
            p.accepted = true;
        }
        Ok(())
    }

    pub fn list_diplomacy(&self, alliance_id: i64) -> Vec<&DiplomacyPact> {
        self.pacts
            .values()
            .filter(|p| p.alliance_a_id == alliance_id || p.alliance_b_id == alliance_id)
            .collect()
    }
}

impl Default for AllianceStore {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_store() -> AllianceStore {
        AllianceStore::new()
    }

    // -- Alliance creation --------------------------------------------------

    #[test]
    fn create_alliance_success() {
        let mut store = make_store();
        let alliance = store.create_alliance("TST", "Test Alliance", 1).unwrap();
        assert_eq!(alliance.tag, "TST");
        assert_eq!(alliance.name, "Test Alliance");
        assert_eq!(alliance.founder_id, 1);
        assert_eq!(alliance.member_count, 1);
        assert_eq!(alliance.max_members, 50);

        // Founder should be Leader member
        let members = store.get_members(alliance.id);
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].role, AllianceRole::Leader);
        assert_eq!(members[0].player_id, 1);
    }

    #[test]
    fn create_alliance_tag_too_short() {
        let mut store = make_store();
        let err = store.create_alliance("A", "Short Tag", 1).unwrap_err();
        assert_eq!(err, AllianceError::TagTooShort);
    }

    #[test]
    fn create_alliance_tag_too_long() {
        let mut store = make_store();
        let err = store
            .create_alliance("ABCDEFGHI", "Long Tag", 1)
            .unwrap_err();
        assert_eq!(err, AllianceError::TagTooLong);
    }

    #[test]
    fn create_alliance_duplicate_tag() {
        let mut store = make_store();
        store.create_alliance("DUP", "First", 1).unwrap();
        let err = store.create_alliance("dup", "Second", 2).unwrap_err();
        assert_eq!(err, AllianceError::TagTaken);
    }

    #[test]
    fn create_alliance_founder_already_in_alliance() {
        let mut store = make_store();
        store.create_alliance("AAA", "First", 1).unwrap();
        let err = store.create_alliance("BBB", "Second", 1).unwrap_err();
        assert_eq!(err, AllianceError::AlreadyInAlliance);
    }

    #[test]
    fn find_alliance_by_tag() {
        let mut store = make_store();
        store.create_alliance("XYZ", "XYZ Corp", 1).unwrap();
        let found = store.find_alliance_by_tag("xyz").unwrap();
        assert_eq!(found.name, "XYZ Corp");
    }

    #[test]
    fn list_alliances() {
        let mut store = make_store();
        store.create_alliance("AA", "Alpha", 1).unwrap();
        store.create_alliance("BB", "Beta", 2).unwrap();
        assert_eq!(store.list_alliances().len(), 2);
    }

    // -- Membership ---------------------------------------------------------

    #[test]
    fn add_and_remove_member() {
        let mut store = make_store();
        let alliance = store.create_alliance("MEM", "Members", 1).unwrap();
        store
            .add_member(alliance.id, 2, AllianceRole::Member)
            .unwrap();

        assert_eq!(store.get_members(alliance.id).len(), 2);
        assert_eq!(store.get_alliance(alliance.id).unwrap().member_count, 2);
        assert_eq!(store.get_player_alliance(2), Some(alliance.id));

        store.remove_member(alliance.id, 2).unwrap();
        assert_eq!(store.get_members(alliance.id).len(), 1);
        assert_eq!(store.get_alliance(alliance.id).unwrap().member_count, 1);
        assert_eq!(store.get_player_alliance(2), None);
    }

    #[test]
    fn cannot_remove_leader() {
        let mut store = make_store();
        let alliance = store.create_alliance("LDR", "Leader Test", 1).unwrap();
        let err = store.remove_member(alliance.id, 1).unwrap_err();
        assert_eq!(err, AllianceError::CannotRemoveLeader);
    }

    #[test]
    fn alliance_full_rejects_new_member() {
        let mut store = make_store();
        let alliance = store.create_alliance("FUL", "Full", 1).unwrap();
        // Manually set max_members to 1 (founder already in)
        store.alliances.get_mut(&alliance.id).unwrap().max_members = 1;

        let err = store
            .add_member(alliance.id, 2, AllianceRole::Member)
            .unwrap_err();
        assert_eq!(err, AllianceError::AllianceFull);
    }

    // -- Roles --------------------------------------------------------------

    #[test]
    fn change_role_success() {
        let mut store = make_store();
        let alliance = store.create_alliance("ROL", "Roles", 1).unwrap();
        store
            .add_member(alliance.id, 2, AllianceRole::Member)
            .unwrap();

        // Leader (1) promotes Member (2) to Officer
        store
            .change_role(alliance.id, 1, 2, AllianceRole::Officer)
            .unwrap();

        let members = store.get_members(alliance.id);
        let target = members.iter().find(|m| m.player_id == 2).unwrap();
        assert_eq!(target.role, AllianceRole::Officer);
    }

    #[test]
    fn change_role_insufficient_rank() {
        let mut store = make_store();
        let alliance = store.create_alliance("RNK", "Rank Test", 1).unwrap();
        store
            .add_member(alliance.id, 2, AllianceRole::Member)
            .unwrap();
        store
            .add_member(alliance.id, 3, AllianceRole::Member)
            .unwrap();

        // Member (2) cannot promote Member (3)
        let err = store
            .change_role(alliance.id, 2, 3, AllianceRole::Officer)
            .unwrap_err();
        assert_eq!(err, AllianceError::InsufficientRank);
    }

    // -- Leadership transfer ------------------------------------------------

    #[test]
    fn transfer_leadership() {
        let mut store = make_store();
        let alliance = store.create_alliance("TRF", "Transfer", 1).unwrap();
        store
            .add_member(alliance.id, 2, AllianceRole::Officer)
            .unwrap();

        store.transfer_leadership(alliance.id, 1, 2).unwrap();

        let members = store.get_members(alliance.id);
        let old_leader = members.iter().find(|m| m.player_id == 1).unwrap();
        let new_leader = members.iter().find(|m| m.player_id == 2).unwrap();
        assert_eq!(old_leader.role, AllianceRole::CoLeader);
        assert_eq!(new_leader.role, AllianceRole::Leader);
    }

    #[test]
    fn transfer_leadership_not_leader_fails() {
        let mut store = make_store();
        let alliance = store.create_alliance("TRF", "Transfer", 1).unwrap();
        store
            .add_member(alliance.id, 2, AllianceRole::Officer)
            .unwrap();

        let err = store.transfer_leadership(alliance.id, 2, 1).unwrap_err();
        assert_eq!(err, AllianceError::InsufficientRank);
    }

    // -- Applications -------------------------------------------------------

    #[test]
    fn application_lifecycle() {
        let mut store = make_store();
        let alliance = store.create_alliance("APP", "Applications", 1).unwrap();

        // Player 2 applies
        let app_id = store
            .apply(2, alliance.id, Some("Let me in!".to_string()))
            .unwrap();

        let apps = store.list_applications(alliance.id);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].status, ApplicationStatus::Pending);

        // Leader (1) accepts
        store.accept_application(app_id, 1).unwrap();

        let apps = store.list_applications(alliance.id);
        let resolved = apps.iter().find(|a| a.id == app_id).unwrap();
        assert_eq!(resolved.status, ApplicationStatus::Accepted);
        assert!(resolved.resolved_at.is_some());

        // Player 2 should now be a member
        assert_eq!(store.get_player_alliance(2), Some(alliance.id));
    }

    #[test]
    fn reject_application() {
        let mut store = make_store();
        let alliance = store.create_alliance("REJ", "Reject", 1).unwrap();

        let app_id = store.apply(2, alliance.id, None).unwrap();
        store.reject_application(app_id, 1).unwrap();

        let apps = store.list_applications(alliance.id);
        let rejected = apps.iter().find(|a| a.id == app_id).unwrap();
        assert_eq!(rejected.status, ApplicationStatus::Rejected);

        // Player 2 should NOT be a member
        assert_eq!(store.get_player_alliance(2), None);
    }

    #[test]
    fn duplicate_application_rejected() {
        let mut store = make_store();
        let alliance = store.create_alliance("DPA", "DupApp", 1).unwrap();

        store.apply(2, alliance.id, None).unwrap();
        let err = store.apply(2, alliance.id, None).unwrap_err();
        assert_eq!(err, AllianceError::DuplicateApplication);
    }

    #[test]
    fn application_insufficient_rank() {
        let mut store = make_store();
        let alliance = store.create_alliance("INS", "Insufficient", 1).unwrap();
        store
            .add_member(alliance.id, 3, AllianceRole::Member)
            .unwrap();

        let app_id = store.apply(2, alliance.id, None).unwrap();

        // Regular Member (3) cannot accept
        let err = store.accept_application(app_id, 3).unwrap_err();
        assert_eq!(err, AllianceError::InsufficientRank);
    }

    // -- Diplomacy ----------------------------------------------------------

    #[test]
    fn diplomacy_propose_and_accept() {
        let mut store = make_store();
        let a = store.create_alliance("AAA", "Alpha", 1).unwrap();
        let b = store.create_alliance("BBB", "Beta", 2).unwrap();

        let pact_id = store
            .propose_diplomacy(a.id, b.id, DiplomacyRelation::Peace, 1)
            .unwrap();

        let pacts = store.list_diplomacy(a.id);
        assert_eq!(pacts.len(), 1);
        assert!(!pacts[0].accepted);

        store.accept_diplomacy(pact_id, b.id).unwrap();

        let pact = store.pacts.get(&pact_id).unwrap();
        assert!(pact.accepted);
        assert_eq!(pact.relation, DiplomacyRelation::Peace);
    }

    #[test]
    fn self_diplomacy_rejected() {
        let mut store = make_store();
        let a = store.create_alliance("SLF", "Self", 1).unwrap();
        let err = store
            .propose_diplomacy(a.id, a.id, DiplomacyRelation::Trade, 1)
            .unwrap_err();
        assert_eq!(err, AllianceError::SelfDiplomacy);
    }

    // -- Disband ------------------------------------------------------------

    #[test]
    fn disband_alliance() {
        let mut store = make_store();
        let alliance = store.create_alliance("DIS", "Disband", 1).unwrap();
        store
            .add_member(alliance.id, 2, AllianceRole::Member)
            .unwrap();

        store.disband_alliance(alliance.id, 1).unwrap();

        assert!(store.get_alliance(alliance.id).is_none());
        assert_eq!(store.get_player_alliance(1), None);
        assert_eq!(store.get_player_alliance(2), None);
        assert!(store.get_members(alliance.id).is_empty());
    }

    #[test]
    fn disband_alliance_non_leader_fails() {
        let mut store = make_store();
        let alliance = store.create_alliance("DIS", "Disband", 1).unwrap();
        store
            .add_member(alliance.id, 2, AllianceRole::Officer)
            .unwrap();

        let err = store.disband_alliance(alliance.id, 2).unwrap_err();
        assert_eq!(err, AllianceError::InsufficientRank);
    }

    // -- Role enum ----------------------------------------------------------

    #[test]
    fn role_display_and_fromstr() {
        assert_eq!(AllianceRole::Leader.to_string(), "Leader");
        assert_eq!(AllianceRole::Applicant.to_string(), "Applicant");

        let parsed: AllianceRole = "officer".parse().unwrap();
        assert_eq!(parsed, AllianceRole::Officer);

        let parsed: AllianceRole = "co-leader".parse().unwrap();
        assert_eq!(parsed, AllianceRole::CoLeader);

        assert!("unknown".parse::<AllianceRole>().is_err());
    }

    #[test]
    fn role_authority_ordering() {
        assert!(AllianceRole::Leader.authority() > AllianceRole::CoLeader.authority());
        assert!(AllianceRole::CoLeader.authority() > AllianceRole::Officer.authority());
        assert!(AllianceRole::Officer.authority() > AllianceRole::Veteran.authority());
        assert!(AllianceRole::Veteran.authority() > AllianceRole::Member.authority());
        assert!(AllianceRole::Member.authority() > AllianceRole::Applicant.authority());
    }
}
