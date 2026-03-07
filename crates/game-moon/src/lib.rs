#![forbid(unsafe_code)]

use std::collections::HashMap;

use serde::Serialize;

pub fn crate_name() -> &'static str {
    "game-moon"
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Moon {
    pub id: i64,
    pub planet_id: i64,
    pub owner_id: i64,
    pub name: String,
    pub size: i32,
    pub temperature: i32,
    pub has_lunar_base: bool,
    pub has_sensor_phalanx: bool,
    pub has_jump_gate: bool,
    pub created_at: String,
}

pub fn calculate_moon_chance(debris_metal: i64, debris_crystal: i64) -> i32 {
    let total = debris_metal + debris_crystal;
    std::cmp::min(20, (total / 100_000) as i32)
}

pub struct MoonStore {
    next_id: i64,
    moons: HashMap<i64, Moon>,
}

impl MoonStore {
    pub fn new() -> Self {
        let seed = Moon {
            id: 1,
            planet_id: 1,
            owner_id: 1,
            name: "Luna".to_string(),
            size: 8000,
            temperature: -40,
            has_lunar_base: true,
            has_sensor_phalanx: false,
            has_jump_gate: false,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let mut moons = HashMap::new();
        moons.insert(seed.id, seed);
        Self { next_id: 2, moons }
    }

    pub fn create_moon(
        &mut self,
        planet_id: i64,
        owner_id: i64,
        name: String,
        debris_metal: i64,
        debris_crystal: i64,
    ) -> Option<Moon> {
        let chance = calculate_moon_chance(debris_metal, debris_crystal);
        if chance <= 0 {
            return None;
        }
        let size = std::cmp::min(20, std::cmp::max(3, chance)) * 500;
        let id = self.next_id;
        self.next_id += 1;
        let moon = Moon {
            id,
            planet_id,
            owner_id,
            name,
            size,
            temperature: -20,
            has_lunar_base: false,
            has_sensor_phalanx: false,
            has_jump_gate: false,
            created_at: now_timestamp(),
        };
        self.moons.insert(id, moon.clone());
        Some(moon)
    }

    pub fn get_moon(&self, moon_id: i64) -> Option<Moon> {
        self.moons.get(&moon_id).cloned()
    }

    pub fn list_moons(&self, owner_id: i64) -> Vec<Moon> {
        let mut result: Vec<Moon> = self
            .moons
            .values()
            .filter(|m| m.owner_id == owner_id)
            .cloned()
            .collect();
        result.sort_by_key(|m| m.id);
        result
    }

    pub fn get_moon_for_planet(&self, planet_id: i64) -> Option<Moon> {
        self.moons
            .values()
            .find(|m| m.planet_id == planet_id)
            .cloned()
    }

    pub fn destroy_moon(&mut self, moon_id: i64) -> bool {
        self.moons.remove(&moon_id).is_some()
    }

    pub fn upgrade_facility(&mut self, moon_id: i64, facility: &str) -> bool {
        if let Some(moon) = self.moons.get_mut(&moon_id) {
            match facility {
                "lunar_base" => {
                    moon.has_lunar_base = true;
                    true
                }
                "sensor_phalanx" => {
                    moon.has_sensor_phalanx = true;
                    true
                }
                "jump_gate" => {
                    moon.has_jump_gate = true;
                    true
                }
                _ => false,
            }
        } else {
            false
        }
    }
}

impl Default for MoonStore {
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
        assert_eq!(crate_name(), "game-moon");
    }

    #[test]
    fn seeded_moon_exists() {
        let store = MoonStore::new();
        let moon = store.get_moon(1).expect("seed moon should exist");
        assert_eq!(moon.name, "Luna");
        assert_eq!(moon.planet_id, 1);
        assert!(moon.has_lunar_base);
    }

    #[test]
    fn calculate_chance_zero_for_small_debris() {
        assert_eq!(calculate_moon_chance(50_000, 40_000), 0);
    }

    #[test]
    fn calculate_chance_capped_at_20() {
        assert_eq!(calculate_moon_chance(2_000_000, 1_000_000), 20);
    }

    #[test]
    fn create_moon_with_sufficient_debris() {
        let mut store = MoonStore::new();
        let moon = store
            .create_moon(5, 2, "Titan".to_string(), 500_000, 500_000)
            .expect("should create moon");
        assert_eq!(moon.planet_id, 5);
        assert_eq!(moon.owner_id, 2);
        assert_eq!(moon.size, 5000); // min(20, max(3, 10)) * 500
    }

    #[test]
    fn create_moon_fails_with_insufficient_debris() {
        let mut store = MoonStore::new();
        let result = store.create_moon(5, 2, "Tiny".to_string(), 10, 20);
        assert!(result.is_none());
    }

    #[test]
    fn list_moons_filters_by_owner() {
        let mut store = MoonStore::new();
        store.create_moon(10, 3, "A".to_string(), 200_000, 100_000);
        store.create_moon(11, 3, "B".to_string(), 200_000, 100_000);
        store.create_moon(12, 4, "C".to_string(), 200_000, 100_000);
        assert_eq!(store.list_moons(3).len(), 2);
        assert_eq!(store.list_moons(4).len(), 1);
        assert_eq!(store.list_moons(99).len(), 0);
    }

    #[test]
    fn get_moon_for_planet_returns_correct_moon() {
        let store = MoonStore::new();
        let moon = store.get_moon_for_planet(1).expect("seed planet 1 moon");
        assert_eq!(moon.id, 1);
        assert!(store.get_moon_for_planet(999).is_none());
    }

    #[test]
    fn destroy_moon_removes_it() {
        let mut store = MoonStore::new();
        assert!(store.destroy_moon(1));
        assert!(store.get_moon(1).is_none());
        assert!(!store.destroy_moon(1));
    }

    #[test]
    fn upgrade_facility_sets_flags() {
        let mut store = MoonStore::new();
        let id = store
            .create_moon(20, 5, "Base".to_string(), 300_000, 0)
            .unwrap()
            .id;
        assert!(store.upgrade_facility(id, "sensor_phalanx"));
        assert!(store.upgrade_facility(id, "jump_gate"));
        let moon = store.get_moon(id).unwrap();
        assert!(moon.has_sensor_phalanx);
        assert!(moon.has_jump_gate);
        assert!(!moon.has_lunar_base);
    }

    #[test]
    fn upgrade_unknown_facility_returns_false() {
        let mut store = MoonStore::new();
        assert!(!store.upgrade_facility(1, "warp_drive"));
    }

    #[test]
    fn upgrade_nonexistent_moon_returns_false() {
        let mut store = MoonStore::new();
        assert!(!store.upgrade_facility(999, "lunar_base"));
    }

    #[test]
    fn create_moon_minimum_size() {
        let mut store = MoonStore::new();
        // chance = 1 -> max(3,1)=3 -> 3*500=1500
        let moon = store
            .create_moon(30, 6, "Small".to_string(), 100_000, 0)
            .expect("should create");
        assert_eq!(moon.size, 1500);
    }
}
