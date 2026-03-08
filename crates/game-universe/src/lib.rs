#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

// ---------------------------------------------------------------------------
// Universe Settings
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UniverseSettings {
    pub name: String,
    pub speed_factor: i32,
    pub fleet_speed_factor: i32,
    pub resource_multiplier: f64,
    pub debris_factor: f64,
    pub max_galaxies: i32,
    pub max_systems: i32,
    pub max_positions: i32,
    pub max_planets_per_player: i32,
    pub donut_galaxy: bool,
    pub donut_system: bool,
    pub noob_protection_points: i64,
    pub noob_protection_multiplier: f64,
    pub is_acs_enabled: bool,
    pub is_marketplace_enabled: bool,
}

// ---------------------------------------------------------------------------
// Default presets
// ---------------------------------------------------------------------------

pub fn default_settings() -> UniverseSettings {
    UniverseSettings {
        name: "Universe".to_string(),
        speed_factor: 1,
        fleet_speed_factor: 1,
        resource_multiplier: 1.0,
        debris_factor: 0.3,
        max_galaxies: 9,
        max_systems: 499,
        max_positions: 15,
        max_planets_per_player: 9,
        donut_galaxy: true,
        donut_system: true,
        noob_protection_points: 5000,
        noob_protection_multiplier: 5.0,
        is_acs_enabled: true,
        is_marketplace_enabled: true,
    }
}

pub fn speed_universe_settings() -> UniverseSettings {
    UniverseSettings {
        name: "Speed Universe".to_string(),
        speed_factor: 4,
        fleet_speed_factor: 4,
        resource_multiplier: 4.0,
        debris_factor: 0.3,
        max_galaxies: 5,
        max_systems: 499,
        max_positions: 15,
        max_planets_per_player: 12,
        donut_galaxy: true,
        donut_system: true,
        noob_protection_points: 50000,
        noob_protection_multiplier: 5.0,
        is_acs_enabled: true,
        is_marketplace_enabled: true,
    }
}

pub fn war_universe_settings() -> UniverseSettings {
    UniverseSettings {
        name: "War Universe".to_string(),
        speed_factor: 2,
        fleet_speed_factor: 4,
        resource_multiplier: 2.0,
        debris_factor: 0.7,
        max_galaxies: 3,
        max_systems: 499,
        max_positions: 15,
        max_planets_per_player: 9,
        donut_galaxy: false,
        donut_system: false,
        noob_protection_points: 1000,
        noob_protection_multiplier: 2.0,
        is_acs_enabled: true,
        is_marketplace_enabled: false,
    }
}

// ---------------------------------------------------------------------------
// Universe Status
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UniverseStatus {
    Creating,
    Online,
    Maintenance,
    Merging,
    Closed,
}

impl fmt::Display for UniverseStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UniverseStatus::Creating => write!(f, "Creating"),
            UniverseStatus::Online => write!(f, "Online"),
            UniverseStatus::Maintenance => write!(f, "Maintenance"),
            UniverseStatus::Merging => write!(f, "Merging"),
            UniverseStatus::Closed => write!(f, "Closed"),
        }
    }
}

// ---------------------------------------------------------------------------
// Universe
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Universe {
    pub id: i64,
    pub settings: UniverseSettings,
    pub status: UniverseStatus,
    pub player_count: i32,
    pub created_at: String,
    pub started_at: Option<String>,
    pub closed_at: Option<String>,
}

// ---------------------------------------------------------------------------
// Universe Error
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UniverseError {
    NotFound,
    InvalidTransition { from: String, to: String },
    AlreadyClosed,
    MergeSameUniverse,
}

impl fmt::Display for UniverseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UniverseError::NotFound => write!(f, "universe not found"),
            UniverseError::InvalidTransition { from, to } => {
                write!(f, "invalid status transition from {from} to {to}")
            }
            UniverseError::AlreadyClosed => write!(f, "universe is already closed"),
            UniverseError::MergeSameUniverse => {
                write!(f, "cannot merge a universe into itself")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Universe Manager
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct UniverseManager {
    universes: HashMap<i64, Universe>,
    next_id: i64,
}

impl UniverseManager {
    pub fn new() -> Self {
        Self {
            universes: HashMap::new(),
            next_id: 1,
        }
    }

    /// Creates a new empty manager with a custom starting ID.
    ///
    /// This is useful when integrating with systems that pre-seed universes
    /// at lower IDs and need newly-created universes to start at a higher
    /// offset (e.g. `with_starting_id(101)` after seeding IDs 1..=2).
    pub fn with_starting_id(starting_id: i64) -> Self {
        Self {
            universes: HashMap::new(),
            next_id: starting_id,
        }
    }

    /// Inserts a pre-built universe into the store without affecting `next_id`.
    ///
    /// Useful for seeding the manager with existing universes (e.g. loaded
    /// from a database) while keeping auto-increment IDs starting higher.
    pub fn insert(&mut self, universe: Universe) {
        self.universes.insert(universe.id, universe);
    }

    pub fn create_universe(&mut self, settings: UniverseSettings) -> Universe {
        let id = self.next_id;
        self.next_id += 1;

        let universe = Universe {
            id,
            settings,
            status: UniverseStatus::Creating,
            player_count: 0,
            created_at: current_timestamp(),
            started_at: None,
            closed_at: None,
        };

        self.universes.insert(id, universe.clone());
        universe
    }

    pub fn start_universe(&mut self, id: i64) -> Result<(), UniverseError> {
        let universe = self.universes.get_mut(&id).ok_or(UniverseError::NotFound)?;

        if universe.status != UniverseStatus::Creating {
            return Err(UniverseError::InvalidTransition {
                from: universe.status.to_string(),
                to: UniverseStatus::Online.to_string(),
            });
        }

        universe.status = UniverseStatus::Online;
        universe.started_at = Some(current_timestamp());
        Ok(())
    }

    pub fn set_maintenance(&mut self, id: i64) -> Result<(), UniverseError> {
        let universe = self.universes.get_mut(&id).ok_or(UniverseError::NotFound)?;

        if universe.status != UniverseStatus::Online {
            return Err(UniverseError::InvalidTransition {
                from: universe.status.to_string(),
                to: UniverseStatus::Maintenance.to_string(),
            });
        }

        universe.status = UniverseStatus::Maintenance;
        Ok(())
    }

    pub fn resume_universe(&mut self, id: i64) -> Result<(), UniverseError> {
        let universe = self.universes.get_mut(&id).ok_or(UniverseError::NotFound)?;

        if universe.status != UniverseStatus::Maintenance {
            return Err(UniverseError::InvalidTransition {
                from: universe.status.to_string(),
                to: UniverseStatus::Online.to_string(),
            });
        }

        universe.status = UniverseStatus::Online;
        Ok(())
    }

    pub fn close_universe(&mut self, id: i64) -> Result<(), UniverseError> {
        let universe = self.universes.get_mut(&id).ok_or(UniverseError::NotFound)?;

        if universe.status == UniverseStatus::Closed {
            return Err(UniverseError::AlreadyClosed);
        }

        universe.status = UniverseStatus::Closed;
        universe.closed_at = Some(current_timestamp());
        Ok(())
    }

    pub fn get_universe(&self, id: i64) -> Option<&Universe> {
        self.universes.get(&id)
    }

    pub fn list_universes(&self) -> Vec<&Universe> {
        let mut list: Vec<&Universe> = self.universes.values().collect();
        list.sort_by_key(|u| u.id);
        list
    }

    pub fn list_online_universes(&self) -> Vec<&Universe> {
        let mut list: Vec<&Universe> = self
            .universes
            .values()
            .filter(|u| u.status == UniverseStatus::Online)
            .collect();
        list.sort_by_key(|u| u.id);
        list
    }

    pub fn update_settings(
        &mut self,
        id: i64,
        settings: UniverseSettings,
    ) -> Result<(), UniverseError> {
        let universe = self.universes.get_mut(&id).ok_or(UniverseError::NotFound)?;

        if universe.status == UniverseStatus::Closed {
            return Err(UniverseError::AlreadyClosed);
        }

        universe.settings = settings;
        Ok(())
    }

    pub fn increment_player_count(&mut self, id: i64) -> Result<i32, UniverseError> {
        let universe = self.universes.get_mut(&id).ok_or(UniverseError::NotFound)?;

        if universe.status == UniverseStatus::Closed {
            return Err(UniverseError::AlreadyClosed);
        }

        universe.player_count += 1;
        Ok(universe.player_count)
    }

    pub fn decrement_player_count(&mut self, id: i64) -> Result<i32, UniverseError> {
        let universe = self.universes.get_mut(&id).ok_or(UniverseError::NotFound)?;

        if universe.status == UniverseStatus::Closed {
            return Err(UniverseError::AlreadyClosed);
        }

        if universe.player_count > 0 {
            universe.player_count -= 1;
        }

        Ok(universe.player_count)
    }

    pub fn merge_universes(&mut self, source_id: i64, target_id: i64) -> Result<(), UniverseError> {
        if source_id == target_id {
            return Err(UniverseError::MergeSameUniverse);
        }

        // Validate both exist.
        if !self.universes.contains_key(&source_id) {
            return Err(UniverseError::NotFound);
        }
        if !self.universes.contains_key(&target_id) {
            return Err(UniverseError::NotFound);
        }

        // Validate neither is closed.
        let source_status = self.universes[&source_id].status;
        let target_status = self.universes[&target_id].status;

        if source_status == UniverseStatus::Closed {
            return Err(UniverseError::AlreadyClosed);
        }
        if target_status == UniverseStatus::Closed {
            return Err(UniverseError::AlreadyClosed);
        }

        // Source transitions to Merging, then Closed.
        let source = self.universes.get_mut(&source_id).unwrap();
        source.status = UniverseStatus::Merging;
        source.status = UniverseStatus::Closed;
        source.closed_at = Some(current_timestamp());

        Ok(())
    }
}

impl Default for UniverseManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn current_timestamp() -> String {
    // Simple ISO-8601-ish placeholder; in production this would use chrono or
    // std::time, but we avoid extra dependencies for the game-universe crate.
    "2026-01-01T00:00:00Z".to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_manager_with_online_universe() -> (UniverseManager, i64) {
        let mut mgr = UniverseManager::new();
        let u = mgr.create_universe(default_settings());
        mgr.start_universe(u.id).unwrap();
        (mgr, u.id)
    }

    // -- Creation -----------------------------------------------------------

    #[test]
    fn test_create_universe_assigns_incremental_ids() {
        let mut mgr = UniverseManager::new();
        let u1 = mgr.create_universe(default_settings());
        let u2 = mgr.create_universe(speed_universe_settings());
        assert_eq!(u1.id, 1);
        assert_eq!(u2.id, 2);
        assert_eq!(u1.status, UniverseStatus::Creating);
        assert_eq!(u2.status, UniverseStatus::Creating);
    }

    #[test]
    fn test_create_universe_stores_settings() {
        let mut mgr = UniverseManager::new();
        let settings = war_universe_settings();
        let u = mgr.create_universe(settings.clone());
        assert_eq!(u.settings, settings);
        assert_eq!(u.player_count, 0);
        assert!(u.started_at.is_none());
        assert!(u.closed_at.is_none());
    }

    // -- State transitions --------------------------------------------------

    #[test]
    fn test_start_universe_creating_to_online() {
        let mut mgr = UniverseManager::new();
        let u = mgr.create_universe(default_settings());
        assert!(mgr.start_universe(u.id).is_ok());

        let fetched = mgr.get_universe(u.id).unwrap();
        assert_eq!(fetched.status, UniverseStatus::Online);
        assert!(fetched.started_at.is_some());
    }

    #[test]
    fn test_start_universe_invalid_from_online() {
        let (mut mgr, id) = make_manager_with_online_universe();
        let err = mgr.start_universe(id).unwrap_err();
        assert_eq!(
            err,
            UniverseError::InvalidTransition {
                from: "Online".to_string(),
                to: "Online".to_string(),
            }
        );
    }

    #[test]
    fn test_maintenance_and_resume_cycle() {
        let (mut mgr, id) = make_manager_with_online_universe();

        assert!(mgr.set_maintenance(id).is_ok());
        assert_eq!(
            mgr.get_universe(id).unwrap().status,
            UniverseStatus::Maintenance
        );

        assert!(mgr.resume_universe(id).is_ok());
        assert_eq!(mgr.get_universe(id).unwrap().status, UniverseStatus::Online);
    }

    #[test]
    fn test_set_maintenance_invalid_from_creating() {
        let mut mgr = UniverseManager::new();
        let u = mgr.create_universe(default_settings());
        let err = mgr.set_maintenance(u.id).unwrap_err();
        assert!(matches!(err, UniverseError::InvalidTransition { .. }));
    }

    #[test]
    fn test_resume_invalid_from_online() {
        let (mut mgr, id) = make_manager_with_online_universe();
        let err = mgr.resume_universe(id).unwrap_err();
        assert!(matches!(err, UniverseError::InvalidTransition { .. }));
    }

    #[test]
    fn test_close_universe_from_any_state() {
        let (mut mgr, id) = make_manager_with_online_universe();
        assert!(mgr.close_universe(id).is_ok());

        let fetched = mgr.get_universe(id).unwrap();
        assert_eq!(fetched.status, UniverseStatus::Closed);
        assert!(fetched.closed_at.is_some());
    }

    #[test]
    fn test_close_already_closed() {
        let (mut mgr, id) = make_manager_with_online_universe();
        mgr.close_universe(id).unwrap();
        let err = mgr.close_universe(id).unwrap_err();
        assert_eq!(err, UniverseError::AlreadyClosed);
    }

    // -- Queries ------------------------------------------------------------

    #[test]
    fn test_list_universes_and_online_filter() {
        let mut mgr = UniverseManager::new();
        let u1 = mgr.create_universe(default_settings());
        let u2 = mgr.create_universe(speed_universe_settings());
        let _u3 = mgr.create_universe(war_universe_settings());
        mgr.start_universe(u1.id).unwrap();
        mgr.start_universe(u2.id).unwrap();

        assert_eq!(mgr.list_universes().len(), 3);
        assert_eq!(mgr.list_online_universes().len(), 2);
    }

    #[test]
    fn test_get_universe_not_found() {
        let mgr = UniverseManager::new();
        assert!(mgr.get_universe(999).is_none());
    }

    // -- Settings update ----------------------------------------------------

    #[test]
    fn test_update_settings_success() {
        let (mut mgr, id) = make_manager_with_online_universe();
        let mut new_settings = speed_universe_settings();
        new_settings.name = "Renamed".to_string();
        assert!(mgr.update_settings(id, new_settings.clone()).is_ok());
        assert_eq!(mgr.get_universe(id).unwrap().settings.name, "Renamed");
    }

    #[test]
    fn test_update_settings_closed_fails() {
        let (mut mgr, id) = make_manager_with_online_universe();
        mgr.close_universe(id).unwrap();
        let err = mgr.update_settings(id, default_settings()).unwrap_err();
        assert_eq!(err, UniverseError::AlreadyClosed);
    }

    // -- Player count -------------------------------------------------------

    #[test]
    fn test_increment_and_decrement_player_count() {
        let (mut mgr, id) = make_manager_with_online_universe();

        assert_eq!(mgr.increment_player_count(id).unwrap(), 1);
        assert_eq!(mgr.increment_player_count(id).unwrap(), 2);
        assert_eq!(mgr.increment_player_count(id).unwrap(), 3);
        assert_eq!(mgr.decrement_player_count(id).unwrap(), 2);

        assert_eq!(mgr.get_universe(id).unwrap().player_count, 2);
    }

    #[test]
    fn test_decrement_does_not_go_below_zero() {
        let (mut mgr, id) = make_manager_with_online_universe();
        assert_eq!(mgr.decrement_player_count(id).unwrap(), 0);
        assert_eq!(mgr.get_universe(id).unwrap().player_count, 0);
    }

    #[test]
    fn test_player_count_closed_fails() {
        let (mut mgr, id) = make_manager_with_online_universe();
        mgr.close_universe(id).unwrap();
        assert_eq!(
            mgr.increment_player_count(id).unwrap_err(),
            UniverseError::AlreadyClosed
        );
        assert_eq!(
            mgr.decrement_player_count(id).unwrap_err(),
            UniverseError::AlreadyClosed
        );
    }

    // -- Merge --------------------------------------------------------------

    #[test]
    fn test_merge_universes_success() {
        let mut mgr = UniverseManager::new();
        let u1 = mgr.create_universe(default_settings());
        let u2 = mgr.create_universe(speed_universe_settings());
        mgr.start_universe(u1.id).unwrap();
        mgr.start_universe(u2.id).unwrap();

        assert!(mgr.merge_universes(u1.id, u2.id).is_ok());

        let source = mgr.get_universe(u1.id).unwrap();
        assert_eq!(source.status, UniverseStatus::Closed);
        assert!(source.closed_at.is_some());

        // Target remains online.
        let target = mgr.get_universe(u2.id).unwrap();
        assert_eq!(target.status, UniverseStatus::Online);
    }

    #[test]
    fn test_merge_same_universe_fails() {
        let (mut mgr, id) = make_manager_with_online_universe();
        let err = mgr.merge_universes(id, id).unwrap_err();
        assert_eq!(err, UniverseError::MergeSameUniverse);
    }

    #[test]
    fn test_merge_closed_source_fails() {
        let mut mgr = UniverseManager::new();
        let u1 = mgr.create_universe(default_settings());
        let u2 = mgr.create_universe(default_settings());
        mgr.start_universe(u1.id).unwrap();
        mgr.start_universe(u2.id).unwrap();
        mgr.close_universe(u1.id).unwrap();

        let err = mgr.merge_universes(u1.id, u2.id).unwrap_err();
        assert_eq!(err, UniverseError::AlreadyClosed);
    }

    #[test]
    fn test_merge_nonexistent_fails() {
        let (mut mgr, id) = make_manager_with_online_universe();
        let err = mgr.merge_universes(id, 999).unwrap_err();
        assert_eq!(err, UniverseError::NotFound);
    }

    // -- Serialization ------------------------------------------------------

    #[test]
    fn test_universe_serializes_to_json() {
        let mut mgr = UniverseManager::new();
        let u = mgr.create_universe(default_settings());
        let json = serde_json::to_string(&u).unwrap();
        let deserialized: Universe = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, u);
    }

    // -- Presets ------------------------------------------------------------

    #[test]
    fn test_preset_speed_universe() {
        let s = speed_universe_settings();
        assert_eq!(s.speed_factor, 4);
        assert_eq!(s.fleet_speed_factor, 4);
        assert_eq!(s.resource_multiplier, 4.0);
    }

    #[test]
    fn test_preset_war_universe() {
        let s = war_universe_settings();
        assert_eq!(s.debris_factor, 0.7);
        assert_eq!(s.fleet_speed_factor, 4);
        assert!(!s.is_marketplace_enabled);
    }

    // -- Not-found on transitions -------------------------------------------

    #[test]
    fn test_start_nonexistent_returns_not_found() {
        let mut mgr = UniverseManager::new();
        assert_eq!(mgr.start_universe(42).unwrap_err(), UniverseError::NotFound);
    }

    #[test]
    fn test_set_maintenance_nonexistent_returns_not_found() {
        let mut mgr = UniverseManager::new();
        assert_eq!(
            mgr.set_maintenance(42).unwrap_err(),
            UniverseError::NotFound
        );
    }

    // -- Default trait ------------------------------------------------------

    #[test]
    fn test_manager_default_is_empty() {
        let mgr = UniverseManager::default();
        assert!(mgr.list_universes().is_empty());
    }
}
