#![forbid(unsafe_code)]

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub fn crate_name() -> &'static str {
    "game-universe"
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UniverseSettings {
    pub speed: i32,
    pub fleet_speed: i32,
    pub research_speed: i32,
    pub storage_multiplier: f64,
    pub debris_factor: f64,
    pub max_galaxies: i32,
    pub max_systems: i32,
    pub max_positions: i32,
    pub noob_protection_points: i64,
    pub alliance_max_members: i32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Universe {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub settings: UniverseSettings,
    pub player_count: i32,
    pub is_open: bool,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UniverseStats {
    pub id: i64,
    pub name: String,
    pub player_count: i32,
    pub is_open: bool,
    pub speed: i32,
}

pub fn default_settings() -> UniverseSettings {
    UniverseSettings {
        speed: 1,
        fleet_speed: 1,
        research_speed: 1,
        storage_multiplier: 1.0,
        debris_factor: 0.3,
        max_galaxies: 9,
        max_systems: 499,
        max_positions: 15,
        noob_protection_points: 5000,
        alliance_max_members: 50,
    }
}

pub struct UniverseStore {
    next_id: i64,
    universes: HashMap<i64, Universe>,
}

impl UniverseStore {
    pub fn new() -> Self {
        let mut store = Self {
            next_id: 1,
            universes: HashMap::new(),
        };
        store.create_universe(
            "Alpha".to_string(),
            "The first universe.".to_string(),
            default_settings(),
        );
        store
    }

    pub fn create_universe(
        &mut self,
        name: String,
        description: String,
        settings: UniverseSettings,
    ) -> Universe {
        let id = self.next_id;
        self.next_id += 1;
        let universe = Universe {
            id,
            name,
            description,
            settings,
            player_count: 0,
            is_open: true,
            created_at: now_timestamp(),
        };
        self.universes.insert(id, universe.clone());
        universe
    }

    pub fn get_universe(&self, id: i64) -> Option<Universe> {
        self.universes.get(&id).cloned()
    }

    pub fn list_universes(&self) -> Vec<Universe> {
        let mut list: Vec<Universe> = self.universes.values().cloned().collect();
        list.sort_by_key(|u| u.id);
        list
    }

    pub fn update_settings(&mut self, id: i64, settings: UniverseSettings) -> bool {
        if let Some(universe) = self.universes.get_mut(&id) {
            universe.settings = settings;
            return true;
        }
        false
    }

    pub fn increment_player_count(&mut self, id: i64) -> bool {
        if let Some(universe) = self.universes.get_mut(&id) {
            universe.player_count += 1;
            return true;
        }
        false
    }

    pub fn set_open(&mut self, id: i64, is_open: bool) -> bool {
        if let Some(universe) = self.universes.get_mut(&id) {
            universe.is_open = is_open;
            return true;
        }
        false
    }

    pub fn get_stats(&self, id: i64) -> Option<UniverseStats> {
        self.universes.get(&id).map(|u| UniverseStats {
            id: u.id,
            name: u.name.clone(),
            player_count: u.player_count,
            is_open: u.is_open,
            speed: u.settings.speed,
        })
    }
}

impl Default for UniverseStore {
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
        assert_eq!(crate_name(), "game-universe");
    }

    #[test]
    fn new_store_has_seeded_alpha() {
        let store = UniverseStore::new();
        let alpha = store.get_universe(1).expect("seeded Alpha universe");
        assert_eq!(alpha.name, "Alpha");
        assert!(alpha.is_open);
        assert_eq!(alpha.player_count, 0);
    }

    #[test]
    fn default_settings_values() {
        let s = default_settings();
        assert_eq!(s.speed, 1);
        assert_eq!(s.fleet_speed, 1);
        assert_eq!(s.research_speed, 1);
        assert!((s.storage_multiplier - 1.0).abs() < f64::EPSILON);
        assert!((s.debris_factor - 0.3).abs() < f64::EPSILON);
        assert_eq!(s.max_galaxies, 9);
        assert_eq!(s.max_systems, 499);
        assert_eq!(s.max_positions, 15);
        assert_eq!(s.noob_protection_points, 5000);
        assert_eq!(s.alliance_max_members, 50);
    }

    #[test]
    fn create_universe_assigns_incrementing_ids() {
        let mut store = UniverseStore::new();
        let u2 = store.create_universe("Beta".into(), "Second".into(), default_settings());
        let u3 = store.create_universe("Gamma".into(), "Third".into(), default_settings());
        assert_eq!(u2.id, 2);
        assert_eq!(u3.id, 3);
    }

    #[test]
    fn list_universes_returns_sorted() {
        let mut store = UniverseStore::new();
        store.create_universe("Beta".into(), "Second".into(), default_settings());
        let list = store.list_universes();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "Alpha");
        assert_eq!(list[1].name, "Beta");
    }

    #[test]
    fn update_settings_changes_values() {
        let mut store = UniverseStore::new();
        let mut new_settings = default_settings();
        new_settings.speed = 5;
        new_settings.fleet_speed = 3;
        assert!(store.update_settings(1, new_settings));
        let u = store.get_universe(1).unwrap();
        assert_eq!(u.settings.speed, 5);
        assert_eq!(u.settings.fleet_speed, 3);
    }

    #[test]
    fn update_settings_returns_false_for_missing() {
        let mut store = UniverseStore::new();
        assert!(!store.update_settings(999, default_settings()));
    }

    #[test]
    fn increment_player_count_works() {
        let mut store = UniverseStore::new();
        assert!(store.increment_player_count(1));
        assert!(store.increment_player_count(1));
        let u = store.get_universe(1).unwrap();
        assert_eq!(u.player_count, 2);
    }

    #[test]
    fn increment_player_count_returns_false_for_missing() {
        let mut store = UniverseStore::new();
        assert!(!store.increment_player_count(999));
    }

    #[test]
    fn set_open_toggles_state() {
        let mut store = UniverseStore::new();
        assert!(store.set_open(1, false));
        assert!(!store.get_universe(1).unwrap().is_open);
        assert!(store.set_open(1, true));
        assert!(store.get_universe(1).unwrap().is_open);
    }

    #[test]
    fn set_open_returns_false_for_missing() {
        let mut store = UniverseStore::new();
        assert!(!store.set_open(999, false));
    }

    #[test]
    fn get_stats_returns_correct_data() {
        let mut store = UniverseStore::new();
        store.increment_player_count(1);
        let stats = store.get_stats(1).expect("stats for Alpha");
        assert_eq!(stats.id, 1);
        assert_eq!(stats.name, "Alpha");
        assert_eq!(stats.player_count, 1);
        assert!(stats.is_open);
        assert_eq!(stats.speed, 1);
    }

    #[test]
    fn get_stats_returns_none_for_missing() {
        let store = UniverseStore::new();
        assert!(store.get_stats(999).is_none());
    }

    #[test]
    fn get_universe_returns_none_for_missing() {
        let store = UniverseStore::new();
        assert!(store.get_universe(999).is_none());
    }
}
