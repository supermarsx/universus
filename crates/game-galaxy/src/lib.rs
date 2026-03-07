#![forbid(unsafe_code)]

use serde::Serialize;
use std::collections::HashMap;

pub fn crate_name() -> &'static str {
    "game-galaxy"
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GalaxyConfig {
    pub num_galaxies: i32,
    pub num_systems: i32,
    pub num_positions: i32,
}

pub fn default_config() -> GalaxyConfig {
    GalaxyConfig {
        num_galaxies: 9,
        num_systems: 499,
        num_positions: 15,
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemSlot {
    pub galaxy: i32,
    pub system: i32,
    pub position: i32,
    pub owner_id: Option<i64>,
    pub planet_name: Option<String>,
    pub has_moon: bool,
    pub debris_metal: i64,
    pub debris_crystal: i64,
}

/// In-memory store for galaxy/system/position data.
/// Keys are (galaxy, system, position) tuples.
pub struct GalaxyStore {
    config: GalaxyConfig,
    slots: HashMap<(i32, i32, i32), SystemSlot>,
}

impl GalaxyStore {
    pub fn new(config: GalaxyConfig) -> Self {
        let mut slots = HashMap::new();
        for g in 1..=config.num_galaxies {
            for s in 1..=config.num_systems {
                for p in 1..=config.num_positions {
                    slots.insert(
                        (g, s, p),
                        SystemSlot {
                            galaxy: g,
                            system: s,
                            position: p,
                            owner_id: None,
                            planet_name: None,
                            has_moon: false,
                            debris_metal: 0,
                            debris_crystal: 0,
                        },
                    );
                }
            }
        }
        Self { config, slots }
    }

    pub fn with_defaults() -> Self {
        Self::new(default_config())
    }

    pub fn is_valid_position(&self, galaxy: i32, system: i32, position: i32) -> bool {
        galaxy >= 1
            && galaxy <= self.config.num_galaxies
            && system >= 1
            && system <= self.config.num_systems
            && position >= 1
            && position <= self.config.num_positions
    }

    pub fn view_system(&self, galaxy: i32, system: i32) -> Vec<SystemSlot> {
        let mut result: Vec<SystemSlot> = (1..=self.config.num_positions)
            .filter_map(|p| self.slots.get(&(galaxy, system, p)).cloned())
            .collect();
        result.sort_by_key(|s| s.position);
        result
    }

    pub fn occupy_position(
        &mut self,
        galaxy: i32,
        system: i32,
        position: i32,
        owner_id: i64,
        planet_name: String,
    ) -> bool {
        if !self.is_valid_position(galaxy, system, position) {
            return false;
        }
        let key = (galaxy, system, position);
        if let Some(slot) = self.slots.get_mut(&key) {
            if slot.owner_id.is_some() {
                return false;
            }
            slot.owner_id = Some(owner_id);
            slot.planet_name = Some(planet_name);
            true
        } else {
            false
        }
    }

    pub fn vacate_position(&mut self, galaxy: i32, system: i32, position: i32) -> bool {
        if !self.is_valid_position(galaxy, system, position) {
            return false;
        }
        let key = (galaxy, system, position);
        if let Some(slot) = self.slots.get_mut(&key) {
            if slot.owner_id.is_none() {
                return false;
            }
            slot.owner_id = None;
            slot.planet_name = None;
            true
        } else {
            false
        }
    }

    pub fn set_moon(&mut self, galaxy: i32, system: i32, position: i32, has_moon: bool) -> bool {
        if !self.is_valid_position(galaxy, system, position) {
            return false;
        }
        let key = (galaxy, system, position);
        if let Some(slot) = self.slots.get_mut(&key) {
            slot.has_moon = has_moon;
            true
        } else {
            false
        }
    }

    pub fn add_debris(
        &mut self,
        galaxy: i32,
        system: i32,
        position: i32,
        metal: i64,
        crystal: i64,
    ) -> bool {
        if !self.is_valid_position(galaxy, system, position) {
            return false;
        }
        let key = (galaxy, system, position);
        if let Some(slot) = self.slots.get_mut(&key) {
            slot.debris_metal += metal;
            slot.debris_crystal += crystal;
            true
        } else {
            false
        }
    }

    pub fn find_empty_position(&self, galaxy: i32) -> Option<(i32, i32, i32)> {
        for s in 1..=self.config.num_systems {
            for p in 1..=self.config.num_positions {
                if let Some(slot) = self.slots.get(&(galaxy, s, p)) {
                    if slot.owner_id.is_none() {
                        return Some((galaxy, s, p));
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small_config() -> GalaxyConfig {
        GalaxyConfig {
            num_galaxies: 2,
            num_systems: 3,
            num_positions: 4,
        }
    }

    #[test]
    fn test_crate_name() {
        assert_eq!(crate_name(), "game-galaxy");
    }

    #[test]
    fn test_default_config() {
        let cfg = default_config();
        assert_eq!(cfg.num_galaxies, 9);
        assert_eq!(cfg.num_systems, 499);
        assert_eq!(cfg.num_positions, 15);
    }

    #[test]
    fn test_view_system_returns_all_positions() {
        let store = GalaxyStore::new(small_config());
        let slots = store.view_system(1, 1);
        assert_eq!(slots.len(), 4);
        for (i, slot) in slots.iter().enumerate() {
            assert_eq!(slot.position, (i as i32) + 1);
            assert!(slot.owner_id.is_none());
        }
    }

    #[test]
    fn test_occupy_and_view() {
        let mut store = GalaxyStore::new(small_config());
        assert!(store.occupy_position(1, 2, 3, 42, "Homeworld".to_string()));
        let slots = store.view_system(1, 2);
        let slot = &slots[2];
        assert_eq!(slot.owner_id, Some(42));
        assert_eq!(slot.planet_name.as_deref(), Some("Homeworld"));
    }

    #[test]
    fn test_occupy_already_occupied() {
        let mut store = GalaxyStore::new(small_config());
        assert!(store.occupy_position(1, 1, 1, 10, "Alpha".to_string()));
        assert!(!store.occupy_position(1, 1, 1, 20, "Beta".to_string()));
    }

    #[test]
    fn test_vacate_position() {
        let mut store = GalaxyStore::new(small_config());
        assert!(store.occupy_position(1, 1, 1, 10, "Alpha".to_string()));
        assert!(store.vacate_position(1, 1, 1));
        assert!(!store.vacate_position(1, 1, 1)); // already vacant
        let slots = store.view_system(1, 1);
        assert!(slots[0].owner_id.is_none());
    }

    #[test]
    fn test_set_moon_and_debris() {
        let mut store = GalaxyStore::new(small_config());
        assert!(store.set_moon(1, 1, 1, true));
        assert!(store.add_debris(1, 1, 1, 500, 300));
        assert!(store.add_debris(1, 1, 1, 100, 50));
        let slots = store.view_system(1, 1);
        assert!(slots[0].has_moon);
        assert_eq!(slots[0].debris_metal, 600);
        assert_eq!(slots[0].debris_crystal, 350);
    }

    #[test]
    fn test_find_empty_position() {
        let mut store = GalaxyStore::new(small_config());
        let pos = store.find_empty_position(1);
        assert_eq!(pos, Some((1, 1, 1)));

        store.occupy_position(1, 1, 1, 1, "P1".to_string());
        let pos = store.find_empty_position(1);
        assert_eq!(pos, Some((1, 1, 2)));
    }

    #[test]
    fn test_is_valid_position() {
        let store = GalaxyStore::new(small_config());
        assert!(store.is_valid_position(1, 1, 1));
        assert!(store.is_valid_position(2, 3, 4));
        assert!(!store.is_valid_position(0, 1, 1));
        assert!(!store.is_valid_position(3, 1, 1));
        assert!(!store.is_valid_position(1, 4, 1));
        assert!(!store.is_valid_position(1, 1, 5));
    }

    #[test]
    fn test_invalid_position_operations() {
        let mut store = GalaxyStore::new(small_config());
        assert!(!store.occupy_position(0, 0, 0, 1, "X".to_string()));
        assert!(!store.vacate_position(99, 1, 1));
        assert!(!store.set_moon(1, 1, 99, true));
        assert!(!store.add_debris(1, 99, 1, 10, 10));
    }

    #[test]
    fn test_with_defaults() {
        let store = GalaxyStore::with_defaults();
        assert!(store.is_valid_position(9, 499, 15));
        assert!(!store.is_valid_position(10, 1, 1));
    }
}
