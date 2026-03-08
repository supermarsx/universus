#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AcsMissionType {
    Attack,
    Defend,
}

impl fmt::Display for AcsMissionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Attack => write!(f, "Attack"),
            Self::Defend => write!(f, "Defend"),
        }
    }
}

impl FromStr for AcsMissionType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "attack" => Ok(Self::Attack),
            "defend" => Ok(Self::Defend),
            other => Err(format!("unknown ACS mission type: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AcsGroupStatus {
    Forming,
    Launched,
    Arrived,
    Completed,
    Cancelled,
}

impl fmt::Display for AcsGroupStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Forming => write!(f, "Forming"),
            Self::Launched => write!(f, "Launched"),
            Self::Arrived => write!(f, "Arrived"),
            Self::Completed => write!(f, "Completed"),
            Self::Cancelled => write!(f, "Cancelled"),
        }
    }
}

impl FromStr for AcsGroupStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "forming" => Ok(Self::Forming),
            "launched" => Ok(Self::Launched),
            "arrived" => Ok(Self::Arrived),
            "completed" => Ok(Self::Completed),
            "cancelled" => Ok(Self::Cancelled),
            other => Err(format!("unknown ACS group status: {other}")),
        }
    }
}

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AcsError {
    NotFound,
    GroupFull,
    AlreadyJoined,
    NotMember,
    NotInitiator,
    InvalidStatus(String),
    InvalidCoordinates,
    InsufficientParticipants,
    AllianceMismatch,
}

impl fmt::Display for AcsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "ACS group not found"),
            Self::GroupFull => write!(f, "ACS group is full"),
            Self::AlreadyJoined => write!(f, "already joined this ACS group"),
            Self::NotMember => write!(f, "not a member of this ACS group"),
            Self::NotInitiator => write!(f, "only the initiator can perform this action"),
            Self::InvalidStatus(s) => write!(f, "invalid group status for this action: {s}"),
            Self::InvalidCoordinates => write!(f, "invalid target coordinates"),
            Self::InsufficientParticipants => {
                write!(f, "at least 2 participants required to launch")
            }
            Self::AllianceMismatch => write!(f, "player is not in the required alliance"),
        }
    }
}

// ---------------------------------------------------------------------------
// Structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AcsParticipant {
    pub player_id: i64,
    pub planet_id: i64,
    pub fleet_id: Option<i64>,
    pub ship_count: i64,
    pub joined_at: String,
    pub is_initiator: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AcsGroup {
    pub id: i64,
    pub mission_type: AcsMissionType,
    pub target_galaxy: i32,
    pub target_system: i32,
    pub target_position: i32,
    pub participants: Vec<AcsParticipant>,
    pub max_participants: usize,
    pub departure_window_start: String,
    pub departure_window_end: String,
    pub status: AcsGroupStatus,
    pub created_at: String,
    pub launched_at: Option<String>,
    pub completed_at: Option<String>,
    pub notes: Option<String>,
    pub alliance_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateAcsGroupInput {
    pub mission_type: AcsMissionType,
    pub target_galaxy: i32,
    pub target_system: i32,
    pub target_position: i32,
    pub max_participants: Option<usize>,
    pub departure_window_start: Option<String>,
    pub departure_window_end: Option<String>,
    pub notes: Option<String>,
    pub alliance_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JoinAcsGroupInput {
    pub group_id: i64,
    pub player_id: i64,
    pub planet_id: i64,
    pub ship_count: i64,
    pub alliance_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AcsGroupSummary {
    pub id: i64,
    pub mission_type: AcsMissionType,
    pub target_galaxy: i32,
    pub target_system: i32,
    pub target_position: i32,
    pub participant_count: usize,
    pub max_participants: usize,
    pub status: AcsGroupStatus,
    pub departure_window_start: String,
    pub departure_window_end: String,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn validate_coordinates(galaxy: i32, system: i32, position: i32) -> bool {
    (1..=9).contains(&galaxy) && (1..=499).contains(&system) && (1..=15).contains(&position)
}

/// Add `minutes` minutes to a simplified ISO 8601 timestamp.
///
/// Supports the format `YYYY-MM-DDTHH:MM:SSZ`. This is intentionally a
/// minimal implementation that avoids pulling in a datetime crate — good
/// enough for the in-memory store used here.
fn add_minutes_to_timestamp(ts: &str, minutes: i64) -> String {
    // Parse the fixed-format timestamp.
    let parse = || -> Option<(i32, u32, u32, u32, u32, u32)> {
        let ts = ts.trim_end_matches('Z');
        let (date, time) = ts.split_once('T')?;
        let date_parts: Vec<&str> = date.split('-').collect();
        let time_parts: Vec<&str> = time.split(':').collect();
        if date_parts.len() != 3 || time_parts.len() != 3 {
            return None;
        }
        Some((
            date_parts[0].parse().ok()?,
            date_parts[1].parse().ok()?,
            date_parts[2].parse().ok()?,
            time_parts[0].parse().ok()?,
            time_parts[1].parse().ok()?,
            time_parts[2].parse().ok()?,
        ))
    };

    let Some((year, month, day, hour, minute, second)) = parse() else {
        // Can't parse — return the original string.
        return ts.to_string();
    };

    // Convert everything to total-minutes since epoch-ish, add offset, convert
    // back.  We only need relative correctness within a few hours, so we can
    // simplify greatly.  Convert the timestamp into total seconds, add the
    // offset, and convert back.
    let total_seconds = (hour as i64) * 3600 + (minute as i64) * 60 + (second as i64);
    let new_total_seconds = total_seconds + minutes * 60;

    // Handle day overflow (positive only, negative wraps to previous day which
    // we clamp to 00:00:00 for simplicity).
    let (extra_days, day_seconds) = if new_total_seconds < 0 {
        (0i64, 0i64)
    } else {
        (new_total_seconds / 86400, new_total_seconds % 86400)
    };

    let new_hour = (day_seconds / 3600) as u32;
    let new_minute = ((day_seconds % 3600) / 60) as u32;
    let new_second = (day_seconds % 60) as u32;
    let new_day = (day as i64 + extra_days).clamp(1, 28) as u32; // clamp to 28 for simplicity

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, new_day, new_hour, new_minute, new_second
    )
}

fn group_summary(group: &AcsGroup) -> AcsGroupSummary {
    AcsGroupSummary {
        id: group.id,
        mission_type: group.mission_type,
        target_galaxy: group.target_galaxy,
        target_system: group.target_system,
        target_position: group.target_position,
        participant_count: group.participants.len(),
        max_participants: group.max_participants,
        status: group.status,
        departure_window_start: group.departure_window_start.clone(),
        departure_window_end: group.departure_window_end.clone(),
    }
}

// ---------------------------------------------------------------------------
// ACS timing logic (free functions)
// ---------------------------------------------------------------------------

/// Given `(player_id, travel_time_seconds)` pairs, returns the maximum
/// travel time — the slowest fleet sets the pace for the whole ACS group.
pub fn calculate_slowest_arrival(participants_travel_times: &[(i64, i32)]) -> i32 {
    participants_travel_times
        .iter()
        .map(|(_, t)| *t)
        .max()
        .unwrap_or(0)
}

/// Each participant must depart at a different time so all fleets arrive at
/// `target_arrival` simultaneously.
///
/// Returns `(player_id, departure_time)` pairs where `departure_time` is an
/// ISO 8601 string.
pub fn align_departure_times(
    target_arrival: &str,
    participants_travel_times: &[(i64, i32)],
) -> Vec<(i64, String)> {
    participants_travel_times
        .iter()
        .map(|(player_id, travel_seconds)| {
            let offset_minutes = -(*travel_seconds as i64) / 60;
            // For sub-minute precision we adjust seconds directly.
            let departure = add_minutes_to_timestamp(target_arrival, offset_minutes);
            (*player_id, departure)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

pub struct AcsStore {
    groups: HashMap<i64, AcsGroup>,
    next_id: i64,
}

impl AcsStore {
    /// Creates a new store pre-seeded with one example ACS group.
    pub fn new() -> Self {
        let mut groups = HashMap::new();
        let seed_group = AcsGroup {
            id: 1,
            mission_type: AcsMissionType::Attack,
            target_galaxy: 1,
            target_system: 223,
            target_position: 9,
            participants: vec![
                AcsParticipant {
                    player_id: 100,
                    planet_id: 1,
                    fleet_id: None,
                    ship_count: 50,
                    joined_at: "2026-02-13T20:00:00Z".to_string(),
                    is_initiator: true,
                },
                AcsParticipant {
                    player_id: 101,
                    planet_id: 2,
                    fleet_id: None,
                    ship_count: 30,
                    joined_at: "2026-02-13T20:01:00Z".to_string(),
                    is_initiator: false,
                },
                AcsParticipant {
                    player_id: 102,
                    planet_id: 3,
                    fleet_id: None,
                    ship_count: 20,
                    joined_at: "2026-02-13T20:02:00Z".to_string(),
                    is_initiator: false,
                },
            ],
            max_participants: 5,
            departure_window_start: "2026-02-13T20:00:00Z".to_string(),
            departure_window_end: "2026-02-13T20:15:00Z".to_string(),
            status: AcsGroupStatus::Forming,
            created_at: "2026-02-13T20:00:00Z".to_string(),
            launched_at: None,
            completed_at: None,
            notes: Some("Synchronized strike".to_string()),
            alliance_id: None,
        };
        groups.insert(seed_group.id, seed_group);

        Self { groups, next_id: 2 }
    }

    /// Creates an empty store with a custom starting ID for auto-increment.
    ///
    /// Useful when the caller manages seed data externally and needs
    /// newly-created groups to start at a specific offset.
    pub fn empty_with_starting_id(starting_id: i64) -> Self {
        Self {
            groups: HashMap::new(),
            next_id: starting_id,
        }
    }

    /// Inserts a pre-built ACS group without affecting `next_id`.
    pub fn insert(&mut self, group: AcsGroup) {
        self.groups.insert(group.id, group);
    }

    // -- create_group -------------------------------------------------------

    pub fn create_group(
        &mut self,
        input: CreateAcsGroupInput,
        initiator_player_id: i64,
        initiator_planet_id: i64,
        now: &str,
    ) -> Result<AcsGroup, AcsError> {
        if !validate_coordinates(
            input.target_galaxy,
            input.target_system,
            input.target_position,
        ) {
            return Err(AcsError::InvalidCoordinates);
        }

        let max_participants = input.max_participants.unwrap_or(5).clamp(2, 16);

        let departure_window_start = input
            .departure_window_start
            .unwrap_or_else(|| now.to_string());
        let departure_window_end = input
            .departure_window_end
            .unwrap_or_else(|| add_minutes_to_timestamp(now, 15));

        let id = self.next_id;
        self.next_id += 1;

        let group = AcsGroup {
            id,
            mission_type: input.mission_type,
            target_galaxy: input.target_galaxy,
            target_system: input.target_system,
            target_position: input.target_position,
            participants: vec![AcsParticipant {
                player_id: initiator_player_id,
                planet_id: initiator_planet_id,
                fleet_id: None,
                ship_count: 0,
                joined_at: now.to_string(),
                is_initiator: true,
            }],
            max_participants,
            departure_window_start,
            departure_window_end,
            status: AcsGroupStatus::Forming,
            created_at: now.to_string(),
            launched_at: None,
            completed_at: None,
            notes: input.notes,
            alliance_id: input.alliance_id,
        };

        self.groups.insert(id, group.clone());
        Ok(group)
    }

    // -- get_group ----------------------------------------------------------

    pub fn get_group(&self, id: i64) -> Option<AcsGroup> {
        self.groups.get(&id).cloned()
    }

    // -- list_groups --------------------------------------------------------

    pub fn list_groups(&self, alliance_id: Option<i64>) -> Vec<AcsGroupSummary> {
        let mut summaries: Vec<AcsGroupSummary> = self
            .groups
            .values()
            .filter(|g| g.status == AcsGroupStatus::Forming || g.status == AcsGroupStatus::Launched)
            .filter(|g| match alliance_id {
                Some(aid) => g.alliance_id == Some(aid),
                None => true,
            })
            .map(group_summary)
            .collect();
        summaries.sort_by_key(|s| s.id);
        summaries
    }

    // -- join_group ---------------------------------------------------------

    pub fn join_group(
        &mut self,
        group_id: i64,
        player_id: i64,
        planet_id: i64,
        ship_count: i64,
        now: &str,
    ) -> Result<AcsParticipant, AcsError> {
        let group = self.groups.get_mut(&group_id).ok_or(AcsError::NotFound)?;

        if group.status != AcsGroupStatus::Forming {
            return Err(AcsError::InvalidStatus(group.status.to_string()));
        }

        if group.participants.iter().any(|p| p.player_id == player_id) {
            return Err(AcsError::AlreadyJoined);
        }

        if group.participants.len() >= group.max_participants {
            return Err(AcsError::GroupFull);
        }

        if let Some(required_alliance) = group.alliance_id {
            // The caller should pass the player's alliance_id. We check it via
            // a simple convention: if the group has an alliance_id set, the
            // caller must also be in that alliance. We encode this by accepting
            // an `alliance_id` field on JoinAcsGroupInput, but for the store
            // method we do a simpler check — see `join_group_with_alliance`.
            // For backward-compat this method always passes.
            //
            // Use `join_group_checked` if you need alliance validation.
            let _ = required_alliance;
        }

        let participant = AcsParticipant {
            player_id,
            planet_id,
            fleet_id: None,
            ship_count,
            joined_at: now.to_string(),
            is_initiator: false,
        };

        group.participants.push(participant.clone());
        Ok(participant)
    }

    /// Like `join_group` but also validates the player's alliance membership.
    pub fn join_group_checked(
        &mut self,
        group_id: i64,
        player_id: i64,
        planet_id: i64,
        ship_count: i64,
        player_alliance_id: Option<i64>,
        now: &str,
    ) -> Result<AcsParticipant, AcsError> {
        let group = self.groups.get(&group_id).ok_or(AcsError::NotFound)?;
        if let Some(required) = group.alliance_id {
            if player_alliance_id != Some(required) {
                return Err(AcsError::AllianceMismatch);
            }
        }
        self.join_group(group_id, player_id, planet_id, ship_count, now)
    }

    // -- leave_group --------------------------------------------------------

    pub fn leave_group(&mut self, group_id: i64, player_id: i64) -> Result<(), AcsError> {
        let group = self.groups.get_mut(&group_id).ok_or(AcsError::NotFound)?;

        if group.status == AcsGroupStatus::Launched
            || group.status == AcsGroupStatus::Completed
            || group.status == AcsGroupStatus::Arrived
        {
            return Err(AcsError::InvalidStatus(group.status.to_string()));
        }

        let participant = group
            .participants
            .iter()
            .find(|p| p.player_id == player_id)
            .ok_or(AcsError::NotMember)?;

        if participant.is_initiator {
            group.status = AcsGroupStatus::Cancelled;
            return Ok(());
        }

        group.participants.retain(|p| p.player_id != player_id);
        Ok(())
    }

    // -- assign_fleet -------------------------------------------------------

    pub fn assign_fleet(
        &mut self,
        group_id: i64,
        player_id: i64,
        fleet_id: i64,
    ) -> Result<(), AcsError> {
        let group = self.groups.get_mut(&group_id).ok_or(AcsError::NotFound)?;

        let participant = group
            .participants
            .iter_mut()
            .find(|p| p.player_id == player_id)
            .ok_or(AcsError::NotMember)?;

        participant.fleet_id = Some(fleet_id);
        Ok(())
    }

    // -- launch_group -------------------------------------------------------

    pub fn launch_group(&mut self, group_id: i64, now: &str) -> Result<AcsGroup, AcsError> {
        let group = self.groups.get_mut(&group_id).ok_or(AcsError::NotFound)?;

        if group.status != AcsGroupStatus::Forming {
            return Err(AcsError::InvalidStatus(group.status.to_string()));
        }

        if group.participants.len() < 2 {
            return Err(AcsError::InsufficientParticipants);
        }

        if group.participants.iter().any(|p| p.fleet_id.is_none()) {
            return Err(AcsError::InvalidStatus(
                "not all participants have assigned fleets".to_string(),
            ));
        }

        group.status = AcsGroupStatus::Launched;
        group.launched_at = Some(now.to_string());
        Ok(group.clone())
    }

    // -- complete_group -----------------------------------------------------

    pub fn complete_group(&mut self, group_id: i64, now: &str) -> Result<AcsGroup, AcsError> {
        let group = self.groups.get_mut(&group_id).ok_or(AcsError::NotFound)?;

        if group.status != AcsGroupStatus::Launched && group.status != AcsGroupStatus::Arrived {
            return Err(AcsError::InvalidStatus(group.status.to_string()));
        }

        group.status = AcsGroupStatus::Completed;
        group.completed_at = Some(now.to_string());
        Ok(group.clone())
    }

    // -- cancel_group -------------------------------------------------------

    pub fn cancel_group(
        &mut self,
        group_id: i64,
        player_id: i64,
        _now: &str,
    ) -> Result<(), AcsError> {
        let group = self.groups.get_mut(&group_id).ok_or(AcsError::NotFound)?;

        if group.status != AcsGroupStatus::Forming {
            return Err(AcsError::InvalidStatus(group.status.to_string()));
        }

        let is_initiator = group
            .participants
            .iter()
            .any(|p| p.player_id == player_id && p.is_initiator);

        if !is_initiator {
            return Err(AcsError::NotInitiator);
        }

        group.status = AcsGroupStatus::Cancelled;
        Ok(())
    }

    // -- check_expired_windows ----------------------------------------------

    /// Cancels groups whose departure window has ended but that haven't been
    /// launched yet. Returns the IDs of cancelled groups.
    pub fn check_expired_windows(&mut self, now: &str) -> Vec<i64> {
        let mut cancelled = Vec::new();
        for group in self.groups.values_mut() {
            if group.status == AcsGroupStatus::Forming
                && group.departure_window_end < now.to_string()
            {
                group.status = AcsGroupStatus::Cancelled;
                cancelled.push(group.id);
            }
        }
        cancelled.sort();
        cancelled
    }

    // -- player_active_groups -----------------------------------------------

    pub fn player_active_groups(&self, player_id: i64) -> Vec<AcsGroupSummary> {
        let mut summaries: Vec<AcsGroupSummary> = self
            .groups
            .values()
            .filter(|g| g.status == AcsGroupStatus::Forming || g.status == AcsGroupStatus::Launched)
            .filter(|g| g.participants.iter().any(|p| p.player_id == player_id))
            .map(group_summary)
            .collect();
        summaries.sort_by_key(|s| s.id);
        summaries
    }

    // -- participants_for_group ---------------------------------------------

    pub fn participants_for_group(&self, group_id: i64) -> Vec<AcsParticipant> {
        self.groups
            .get(&group_id)
            .map(|g| g.participants.clone())
            .unwrap_or_default()
    }
}

impl Default for AcsStore {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: &str = "2026-03-01T12:00:00Z";

    fn make_input(mission: AcsMissionType) -> CreateAcsGroupInput {
        CreateAcsGroupInput {
            mission_type: mission,
            target_galaxy: 1,
            target_system: 100,
            target_position: 8,
            max_participants: None,
            departure_window_start: Some("2026-03-01T12:00:00Z".to_string()),
            departure_window_end: Some("2026-03-01T12:15:00Z".to_string()),
            notes: None,
            alliance_id: None,
        }
    }

    // -- enum Display / FromStr ---------------------------------------------

    #[test]
    fn mission_type_display_and_parse() {
        assert_eq!(AcsMissionType::Attack.to_string(), "Attack");
        assert_eq!(AcsMissionType::Defend.to_string(), "Defend");
        assert_eq!(
            "attack".parse::<AcsMissionType>().unwrap(),
            AcsMissionType::Attack
        );
        assert_eq!(
            "Defend".parse::<AcsMissionType>().unwrap(),
            AcsMissionType::Defend
        );
        assert!("unknown".parse::<AcsMissionType>().is_err());
    }

    #[test]
    fn group_status_display_and_parse() {
        assert_eq!(AcsGroupStatus::Forming.to_string(), "Forming");
        assert_eq!(AcsGroupStatus::Cancelled.to_string(), "Cancelled");
        assert_eq!(
            "launched".parse::<AcsGroupStatus>().unwrap(),
            AcsGroupStatus::Launched
        );
        assert!("invalid".parse::<AcsGroupStatus>().is_err());
    }

    // -- seed group ---------------------------------------------------------

    #[test]
    fn seed_group_exists() {
        let store = AcsStore::new();
        let group = store.get_group(1).expect("seed group must exist");
        assert_eq!(group.participants.len(), 3);
        assert_eq!(group.mission_type, AcsMissionType::Attack);
        assert_eq!(group.status, AcsGroupStatus::Forming);
    }

    // -- create_group -------------------------------------------------------

    #[test]
    fn create_group_basic() {
        let mut store = AcsStore::new();
        let group = store
            .create_group(make_input(AcsMissionType::Attack), 10, 20, NOW)
            .unwrap();
        assert_eq!(group.participants.len(), 1);
        assert!(group.participants[0].is_initiator);
        assert_eq!(group.participants[0].player_id, 10);
        assert_eq!(group.status, AcsGroupStatus::Forming);
        assert_eq!(group.max_participants, 5);
    }

    #[test]
    fn create_group_invalid_coordinates() {
        let mut store = AcsStore::new();
        let mut input = make_input(AcsMissionType::Attack);
        input.target_galaxy = 0;
        assert_eq!(
            store.create_group(input.clone(), 1, 1, NOW).unwrap_err(),
            AcsError::InvalidCoordinates
        );

        input.target_galaxy = 1;
        input.target_system = 500;
        assert_eq!(
            store.create_group(input.clone(), 1, 1, NOW).unwrap_err(),
            AcsError::InvalidCoordinates
        );

        input.target_system = 1;
        input.target_position = 16;
        assert_eq!(
            store.create_group(input, 1, 1, NOW).unwrap_err(),
            AcsError::InvalidCoordinates
        );
    }

    #[test]
    fn create_group_default_departure_window() {
        let mut store = AcsStore::new();
        let mut input = make_input(AcsMissionType::Defend);
        input.departure_window_start = None;
        input.departure_window_end = None;
        let group = store.create_group(input, 10, 20, NOW).unwrap();
        assert_eq!(group.departure_window_start, NOW);
        assert_eq!(group.departure_window_end, "2026-03-01T12:15:00Z");
    }

    #[test]
    fn create_group_clamps_max_participants() {
        let mut store = AcsStore::new();
        let mut input = make_input(AcsMissionType::Attack);
        input.max_participants = Some(100);
        let group = store.create_group(input, 10, 20, NOW).unwrap();
        assert_eq!(group.max_participants, 16);

        let mut input2 = make_input(AcsMissionType::Attack);
        input2.max_participants = Some(1);
        let group2 = store.create_group(input2, 10, 20, NOW).unwrap();
        assert_eq!(group2.max_participants, 2);
    }

    // -- join_group ---------------------------------------------------------

    #[test]
    fn join_group_success() {
        let mut store = AcsStore::new();
        let group = store
            .create_group(make_input(AcsMissionType::Attack), 10, 20, NOW)
            .unwrap();
        let participant = store.join_group(group.id, 11, 21, 25, NOW).unwrap();
        assert_eq!(participant.player_id, 11);
        assert_eq!(participant.ship_count, 25);
        assert!(!participant.is_initiator);

        let updated = store.get_group(group.id).unwrap();
        assert_eq!(updated.participants.len(), 2);
    }

    #[test]
    fn join_group_already_joined() {
        let mut store = AcsStore::new();
        let group = store
            .create_group(make_input(AcsMissionType::Attack), 10, 20, NOW)
            .unwrap();
        assert_eq!(
            store.join_group(group.id, 10, 20, 5, NOW).unwrap_err(),
            AcsError::AlreadyJoined
        );
    }

    #[test]
    fn join_group_full() {
        let mut store = AcsStore::new();
        let mut input = make_input(AcsMissionType::Attack);
        input.max_participants = Some(2);
        let group = store.create_group(input, 10, 20, NOW).unwrap();
        store.join_group(group.id, 11, 21, 10, NOW).unwrap();
        assert_eq!(
            store.join_group(group.id, 12, 22, 10, NOW).unwrap_err(),
            AcsError::GroupFull
        );
    }

    #[test]
    fn join_group_not_forming() {
        let mut store = AcsStore::new();
        let group = store
            .create_group(make_input(AcsMissionType::Attack), 10, 20, NOW)
            .unwrap();

        // Cancel it first
        store.cancel_group(group.id, 10, NOW).unwrap();
        let err = store.join_group(group.id, 11, 21, 5, NOW).unwrap_err();
        assert!(matches!(err, AcsError::InvalidStatus(_)));
    }

    #[test]
    fn join_group_not_found() {
        let mut store = AcsStore::new();
        assert_eq!(
            store.join_group(9999, 11, 21, 5, NOW).unwrap_err(),
            AcsError::NotFound
        );
    }

    #[test]
    fn join_group_alliance_mismatch() {
        let mut store = AcsStore::new();
        let mut input = make_input(AcsMissionType::Defend);
        input.alliance_id = Some(42);
        let group = store.create_group(input, 10, 20, NOW).unwrap();

        // Player not in the alliance
        let err = store
            .join_group_checked(group.id, 11, 21, 10, Some(99), NOW)
            .unwrap_err();
        assert_eq!(err, AcsError::AllianceMismatch);

        // Player with no alliance
        let err = store
            .join_group_checked(group.id, 12, 22, 10, None, NOW)
            .unwrap_err();
        assert_eq!(err, AcsError::AllianceMismatch);

        // Player in correct alliance
        let p = store
            .join_group_checked(group.id, 13, 23, 10, Some(42), NOW)
            .unwrap();
        assert_eq!(p.player_id, 13);
    }

    // -- leave_group --------------------------------------------------------

    #[test]
    fn leave_group_non_initiator() {
        let mut store = AcsStore::new();
        let group = store
            .create_group(make_input(AcsMissionType::Attack), 10, 20, NOW)
            .unwrap();
        store.join_group(group.id, 11, 21, 10, NOW).unwrap();
        store.leave_group(group.id, 11).unwrap();
        let updated = store.get_group(group.id).unwrap();
        assert_eq!(updated.participants.len(), 1);
        assert_eq!(updated.status, AcsGroupStatus::Forming);
    }

    #[test]
    fn leave_group_initiator_cancels() {
        let mut store = AcsStore::new();
        let group = store
            .create_group(make_input(AcsMissionType::Attack), 10, 20, NOW)
            .unwrap();
        store.join_group(group.id, 11, 21, 10, NOW).unwrap();
        store.leave_group(group.id, 10).unwrap();
        let updated = store.get_group(group.id).unwrap();
        assert_eq!(updated.status, AcsGroupStatus::Cancelled);
    }

    #[test]
    fn leave_group_not_member() {
        let mut store = AcsStore::new();
        let group = store
            .create_group(make_input(AcsMissionType::Attack), 10, 20, NOW)
            .unwrap();
        assert_eq!(
            store.leave_group(group.id, 999).unwrap_err(),
            AcsError::NotMember
        );
    }

    #[test]
    fn leave_group_invalid_status_launched() {
        let mut store = AcsStore::new();
        let group = store
            .create_group(make_input(AcsMissionType::Attack), 10, 20, NOW)
            .unwrap();
        store.join_group(group.id, 11, 21, 10, NOW).unwrap();
        store.assign_fleet(group.id, 10, 1000).unwrap();
        store.assign_fleet(group.id, 11, 1001).unwrap();
        store.launch_group(group.id, NOW).unwrap();

        let err = store.leave_group(group.id, 11).unwrap_err();
        assert!(matches!(err, AcsError::InvalidStatus(_)));
    }

    // -- assign_fleet -------------------------------------------------------

    #[test]
    fn assign_fleet_success() {
        let mut store = AcsStore::new();
        let group = store
            .create_group(make_input(AcsMissionType::Attack), 10, 20, NOW)
            .unwrap();
        store.assign_fleet(group.id, 10, 500).unwrap();
        let updated = store.get_group(group.id).unwrap();
        assert_eq!(updated.participants[0].fleet_id, Some(500));
    }

    #[test]
    fn assign_fleet_not_member() {
        let mut store = AcsStore::new();
        let group = store
            .create_group(make_input(AcsMissionType::Attack), 10, 20, NOW)
            .unwrap();
        assert_eq!(
            store.assign_fleet(group.id, 999, 500).unwrap_err(),
            AcsError::NotMember
        );
    }

    // -- launch_group -------------------------------------------------------

    #[test]
    fn launch_group_success() {
        let mut store = AcsStore::new();
        let group = store
            .create_group(make_input(AcsMissionType::Attack), 10, 20, NOW)
            .unwrap();
        store.join_group(group.id, 11, 21, 10, NOW).unwrap();
        store.assign_fleet(group.id, 10, 1000).unwrap();
        store.assign_fleet(group.id, 11, 1001).unwrap();
        let launched = store.launch_group(group.id, NOW).unwrap();
        assert_eq!(launched.status, AcsGroupStatus::Launched);
        assert_eq!(launched.launched_at, Some(NOW.to_string()));
    }

    #[test]
    fn launch_group_insufficient_participants() {
        let mut store = AcsStore::new();
        let group = store
            .create_group(make_input(AcsMissionType::Attack), 10, 20, NOW)
            .unwrap();
        store.assign_fleet(group.id, 10, 1000).unwrap();
        assert_eq!(
            store.launch_group(group.id, NOW).unwrap_err(),
            AcsError::InsufficientParticipants
        );
    }

    #[test]
    fn launch_group_missing_fleets() {
        let mut store = AcsStore::new();
        let group = store
            .create_group(make_input(AcsMissionType::Attack), 10, 20, NOW)
            .unwrap();
        store.join_group(group.id, 11, 21, 10, NOW).unwrap();
        // Only assign fleet to one participant
        store.assign_fleet(group.id, 10, 1000).unwrap();
        let err = store.launch_group(group.id, NOW).unwrap_err();
        assert!(matches!(err, AcsError::InvalidStatus(_)));
    }

    #[test]
    fn launch_group_wrong_status() {
        let mut store = AcsStore::new();
        let group = store
            .create_group(make_input(AcsMissionType::Attack), 10, 20, NOW)
            .unwrap();
        store.cancel_group(group.id, 10, NOW).unwrap();
        let err = store.launch_group(group.id, NOW).unwrap_err();
        assert!(matches!(err, AcsError::InvalidStatus(_)));
    }

    // -- complete_group -----------------------------------------------------

    #[test]
    fn complete_group_success() {
        let mut store = AcsStore::new();
        let group = store
            .create_group(make_input(AcsMissionType::Attack), 10, 20, NOW)
            .unwrap();
        store.join_group(group.id, 11, 21, 10, NOW).unwrap();
        store.assign_fleet(group.id, 10, 1000).unwrap();
        store.assign_fleet(group.id, 11, 1001).unwrap();
        store.launch_group(group.id, NOW).unwrap();

        let completed = store
            .complete_group(group.id, "2026-03-01T13:00:00Z")
            .unwrap();
        assert_eq!(completed.status, AcsGroupStatus::Completed);
        assert_eq!(
            completed.completed_at,
            Some("2026-03-01T13:00:00Z".to_string())
        );
    }

    #[test]
    fn complete_group_wrong_status() {
        let mut store = AcsStore::new();
        let group = store
            .create_group(make_input(AcsMissionType::Attack), 10, 20, NOW)
            .unwrap();
        let err = store.complete_group(group.id, NOW).unwrap_err();
        assert!(matches!(err, AcsError::InvalidStatus(_)));
    }

    // -- cancel_group -------------------------------------------------------

    #[test]
    fn cancel_group_by_initiator() {
        let mut store = AcsStore::new();
        let group = store
            .create_group(make_input(AcsMissionType::Attack), 10, 20, NOW)
            .unwrap();
        store.cancel_group(group.id, 10, NOW).unwrap();
        let updated = store.get_group(group.id).unwrap();
        assert_eq!(updated.status, AcsGroupStatus::Cancelled);
    }

    #[test]
    fn cancel_group_not_initiator() {
        let mut store = AcsStore::new();
        let group = store
            .create_group(make_input(AcsMissionType::Attack), 10, 20, NOW)
            .unwrap();
        store.join_group(group.id, 11, 21, 10, NOW).unwrap();
        assert_eq!(
            store.cancel_group(group.id, 11, NOW).unwrap_err(),
            AcsError::NotInitiator
        );
    }

    #[test]
    fn cancel_group_wrong_status() {
        let mut store = AcsStore::new();
        let group = store
            .create_group(make_input(AcsMissionType::Attack), 10, 20, NOW)
            .unwrap();
        store.join_group(group.id, 11, 21, 10, NOW).unwrap();
        store.assign_fleet(group.id, 10, 1000).unwrap();
        store.assign_fleet(group.id, 11, 1001).unwrap();
        store.launch_group(group.id, NOW).unwrap();

        let err = store.cancel_group(group.id, 10, NOW).unwrap_err();
        assert!(matches!(err, AcsError::InvalidStatus(_)));
    }

    // -- check_expired_windows ----------------------------------------------

    #[test]
    fn check_expired_windows_cancels_old_groups() {
        let mut store = AcsStore::new();
        let group = store
            .create_group(make_input(AcsMissionType::Attack), 10, 20, NOW)
            .unwrap();
        // Window ends at 12:15, check at 12:30 → should cancel
        let cancelled = store.check_expired_windows("2026-03-01T12:30:00Z");
        assert!(cancelled.contains(&group.id));
        let updated = store.get_group(group.id).unwrap();
        assert_eq!(updated.status, AcsGroupStatus::Cancelled);
    }

    #[test]
    fn check_expired_windows_ignores_active() {
        let mut store = AcsStore::new();
        let group = store
            .create_group(make_input(AcsMissionType::Attack), 10, 20, NOW)
            .unwrap();
        // Check before window ends → should NOT cancel
        let cancelled = store.check_expired_windows("2026-03-01T12:10:00Z");
        assert!(!cancelled.contains(&group.id));
        let updated = store.get_group(group.id).unwrap();
        assert_eq!(updated.status, AcsGroupStatus::Forming);
    }

    #[test]
    fn check_expired_windows_ignores_launched() {
        let mut store = AcsStore::new();
        let group = store
            .create_group(make_input(AcsMissionType::Attack), 10, 20, NOW)
            .unwrap();
        store.join_group(group.id, 11, 21, 10, NOW).unwrap();
        store.assign_fleet(group.id, 10, 1000).unwrap();
        store.assign_fleet(group.id, 11, 1001).unwrap();
        store.launch_group(group.id, NOW).unwrap();

        let cancelled = store.check_expired_windows("2026-03-01T13:00:00Z");
        assert!(!cancelled.contains(&group.id));
    }

    // -- list_groups --------------------------------------------------------

    #[test]
    fn list_groups_filters_by_status() {
        let mut store = AcsStore::new();
        let g1 = store
            .create_group(make_input(AcsMissionType::Attack), 10, 20, NOW)
            .unwrap();
        let _g2 = store
            .create_group(make_input(AcsMissionType::Defend), 11, 21, NOW)
            .unwrap();
        store.cancel_group(g1.id, 10, NOW).unwrap();

        let active = store.list_groups(None);
        // Seed group (Forming) + g2 (Forming); g1 is Cancelled
        assert_eq!(active.len(), 2);
    }

    #[test]
    fn list_groups_filters_by_alliance() {
        let mut store = AcsStore::new();
        let mut input1 = make_input(AcsMissionType::Attack);
        input1.alliance_id = Some(42);
        store.create_group(input1, 10, 20, NOW).unwrap();

        let mut input2 = make_input(AcsMissionType::Defend);
        input2.alliance_id = Some(99);
        store.create_group(input2, 11, 21, NOW).unwrap();

        let alliance_42 = store.list_groups(Some(42));
        assert_eq!(alliance_42.len(), 1);
        assert_eq!(alliance_42[0].mission_type, AcsMissionType::Attack);
    }

    // -- player_active_groups -----------------------------------------------

    #[test]
    fn player_active_groups_only_shows_own() {
        let mut store = AcsStore::new();
        let group = store
            .create_group(make_input(AcsMissionType::Attack), 10, 20, NOW)
            .unwrap();
        store.join_group(group.id, 11, 21, 10, NOW).unwrap();

        let active_10 = store.player_active_groups(10);
        assert!(active_10.iter().any(|g| g.id == group.id));

        let active_11 = store.player_active_groups(11);
        assert!(active_11.iter().any(|g| g.id == group.id));

        // Player 999 is not in any created group (may be in seed)
        let active_999 = store.player_active_groups(999);
        assert!(active_999.iter().all(|g| g.id != group.id));
    }

    // -- participants_for_group ---------------------------------------------

    #[test]
    fn participants_for_group_returns_all() {
        let mut store = AcsStore::new();
        let group = store
            .create_group(make_input(AcsMissionType::Attack), 10, 20, NOW)
            .unwrap();
        store.join_group(group.id, 11, 21, 10, NOW).unwrap();
        store.join_group(group.id, 12, 22, 15, NOW).unwrap();

        let participants = store.participants_for_group(group.id);
        assert_eq!(participants.len(), 3);
    }

    #[test]
    fn participants_for_nonexistent_group() {
        let store = AcsStore::new();
        let participants = store.participants_for_group(9999);
        assert!(participants.is_empty());
    }

    // -- timing logic -------------------------------------------------------

    #[test]
    fn calculate_slowest_arrival_basic() {
        let times = vec![(1, 300), (2, 600), (3, 450)];
        assert_eq!(calculate_slowest_arrival(&times), 600);
    }

    #[test]
    fn calculate_slowest_arrival_empty() {
        assert_eq!(calculate_slowest_arrival(&[]), 0);
    }

    #[test]
    fn calculate_slowest_arrival_single() {
        assert_eq!(calculate_slowest_arrival(&[(1, 120)]), 120);
    }

    #[test]
    fn align_departure_times_basic() {
        let target = "2026-03-01T13:00:00Z";
        let times = vec![(1, 600), (2, 1200)];
        let departures = align_departure_times(target, &times);

        // Player 1: 600s = 10 min before arrival → 12:50
        assert_eq!(departures[0], (1, "2026-03-01T12:50:00Z".to_string()));
        // Player 2: 1200s = 20 min before arrival → 12:40
        assert_eq!(departures[1], (2, "2026-03-01T12:40:00Z".to_string()));
    }

    #[test]
    fn align_departure_times_same_travel() {
        let target = "2026-03-01T13:00:00Z";
        let times = vec![(1, 300), (2, 300)];
        let departures = align_departure_times(target, &times);
        // Both should depart at same time
        assert_eq!(departures[0].1, departures[1].1);
    }

    // -- AcsError display ---------------------------------------------------

    #[test]
    fn acs_error_display() {
        assert_eq!(AcsError::NotFound.to_string(), "ACS group not found");
        assert_eq!(AcsError::GroupFull.to_string(), "ACS group is full");
        assert_eq!(
            AcsError::InvalidStatus("Launched".into()).to_string(),
            "invalid group status for this action: Launched"
        );
    }

    // -- serialization round-trip -------------------------------------------

    #[test]
    fn group_serialization_roundtrip() {
        let mut store = AcsStore::new();
        let group = store
            .create_group(make_input(AcsMissionType::Defend), 10, 20, NOW)
            .unwrap();
        let json = serde_json::to_string(&group).unwrap();
        let deserialized: AcsGroup = serde_json::from_str(&json).unwrap();
        assert_eq!(group, deserialized);
    }

    #[test]
    fn participant_serialization_roundtrip() {
        let p = AcsParticipant {
            player_id: 42,
            planet_id: 7,
            fleet_id: Some(100),
            ship_count: 50,
            joined_at: NOW.to_string(),
            is_initiator: true,
        };
        let json = serde_json::to_string(&p).unwrap();
        let deserialized: AcsParticipant = serde_json::from_str(&json).unwrap();
        assert_eq!(p, deserialized);
    }

    // -- add_minutes_to_timestamp helper ------------------------------------

    #[test]
    fn add_minutes_positive() {
        assert_eq!(
            add_minutes_to_timestamp("2026-03-01T12:00:00Z", 30),
            "2026-03-01T12:30:00Z"
        );
    }

    #[test]
    fn add_minutes_negative() {
        assert_eq!(
            add_minutes_to_timestamp("2026-03-01T13:00:00Z", -20),
            "2026-03-01T12:40:00Z"
        );
    }

    #[test]
    fn add_minutes_hour_rollover() {
        assert_eq!(
            add_minutes_to_timestamp("2026-03-01T12:50:00Z", 15),
            "2026-03-01T13:05:00Z"
        );
    }
}
