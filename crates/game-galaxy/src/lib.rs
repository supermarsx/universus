#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

// ---------------------------------------------------------------------------
// Galaxy Configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GalaxyConfig {
    pub max_galaxies: i32,
    pub max_systems: i32,
    pub max_positions: i32,
}

impl Default for GalaxyConfig {
    fn default() -> Self {
        Self {
            max_galaxies: 9,
            max_systems: 499,
            max_positions: 15,
        }
    }
}

// ---------------------------------------------------------------------------
// Galaxy Error
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GalaxyError {
    PositionOccupied,
    OutOfBounds { field: String, value: i32, max: i32 },
    NoFreePositions,
}

impl fmt::Display for GalaxyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GalaxyError::PositionOccupied => write!(f, "Position is already occupied"),
            GalaxyError::OutOfBounds { field, value, max } => {
                write!(f, "{field} value {value} is out of bounds (max {max})")
            }
            GalaxyError::NoFreePositions => write!(f, "No free positions available"),
        }
    }
}

// ---------------------------------------------------------------------------
// Coordinate Validation
// ---------------------------------------------------------------------------

pub fn validate_coordinates(
    galaxy: i32,
    system: i32,
    position: i32,
    config: &GalaxyConfig,
) -> Result<(), GalaxyError> {
    if galaxy < 1 || galaxy > config.max_galaxies {
        return Err(GalaxyError::OutOfBounds {
            field: "galaxy".to_string(),
            value: galaxy,
            max: config.max_galaxies,
        });
    }
    if system < 1 || system > config.max_systems {
        return Err(GalaxyError::OutOfBounds {
            field: "system".to_string(),
            value: system,
            max: config.max_systems,
        });
    }
    if position < 1 || position > config.max_positions {
        return Err(GalaxyError::OutOfBounds {
            field: "position".to_string(),
            value: position,
            max: config.max_positions,
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Galaxy Map Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GalaxyPosition {
    pub galaxy: i32,
    pub system: i32,
    pub position: i32,
    pub planet_id: Option<i64>,
    pub player_id: Option<i64>,
    pub player_name: Option<String>,
    pub planet_name: Option<String>,
    pub moon_id: Option<i64>,
    pub debris_metal: i64,
    pub debris_crystal: i64,
    pub is_inactive: bool,
    pub is_vacation: bool,
    pub is_banned: bool,
    pub alliance_tag: Option<String>,
}

impl GalaxyPosition {
    fn empty(galaxy: i32, system: i32, position: i32) -> Self {
        Self {
            galaxy,
            system,
            position,
            planet_id: None,
            player_id: None,
            player_name: None,
            planet_name: None,
            moon_id: None,
            debris_metal: 0,
            debris_crystal: 0,
            is_inactive: false,
            is_vacation: false,
            is_banned: false,
            alliance_tag: None,
        }
    }

    fn is_occupied(&self) -> bool {
        self.planet_id.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemView {
    pub galaxy: i32,
    pub system: i32,
    pub positions: Vec<GalaxyPosition>,
}

// ---------------------------------------------------------------------------
// Galaxy Store (in-memory)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct GalaxyStore {
    pub config: GalaxyConfig,
    positions: HashMap<(i32, i32, i32), GalaxyPosition>,
}

impl GalaxyStore {
    pub fn new(config: GalaxyConfig) -> Self {
        Self {
            config,
            positions: HashMap::new(),
        }
    }

    pub fn place_planet(
        &mut self,
        galaxy: i32,
        system: i32,
        position: i32,
        planet_id: i64,
        player_id: i64,
        player_name: &str,
        planet_name: &str,
    ) -> Result<(), GalaxyError> {
        validate_coordinates(galaxy, system, position, &self.config)?;

        let key = (galaxy, system, position);
        if let Some(existing) = self.positions.get(&key) {
            if existing.is_occupied() {
                return Err(GalaxyError::PositionOccupied);
            }
        }

        let entry = self
            .positions
            .entry(key)
            .or_insert_with(|| GalaxyPosition::empty(galaxy, system, position));

        entry.planet_id = Some(planet_id);
        entry.player_id = Some(player_id);
        entry.player_name = Some(player_name.to_string());
        entry.planet_name = Some(planet_name.to_string());

        Ok(())
    }

    pub fn remove_planet(&mut self, galaxy: i32, system: i32, position: i32) -> bool {
        let key = (galaxy, system, position);
        if let Some(pos) = self.positions.get_mut(&key) {
            if pos.is_occupied() {
                pos.planet_id = None;
                pos.player_id = None;
                pos.player_name = None;
                pos.planet_name = None;
                pos.moon_id = None;
                pos.is_inactive = false;
                pos.is_vacation = false;
                pos.is_banned = false;
                pos.alliance_tag = None;
                return true;
            }
        }
        false
    }

    pub fn get_position(&self, galaxy: i32, system: i32, position: i32) -> Option<&GalaxyPosition> {
        self.positions.get(&(galaxy, system, position))
    }

    pub fn get_system_view(&self, galaxy: i32, system: i32) -> SystemView {
        let mut positions = Vec::with_capacity(self.config.max_positions as usize);
        for pos in 1..=self.config.max_positions {
            let gp = self
                .positions
                .get(&(galaxy, system, pos))
                .cloned()
                .unwrap_or_else(|| GalaxyPosition::empty(galaxy, system, pos));
            positions.push(gp);
        }
        SystemView {
            galaxy,
            system,
            positions,
        }
    }

    pub fn update_debris(
        &mut self,
        galaxy: i32,
        system: i32,
        position: i32,
        metal: i64,
        crystal: i64,
    ) {
        let entry = self
            .positions
            .entry((galaxy, system, position))
            .or_insert_with(|| GalaxyPosition::empty(galaxy, system, position));
        entry.debris_metal += metal;
        entry.debris_crystal += crystal;
    }

    pub fn collect_debris(&mut self, galaxy: i32, system: i32, position: i32) -> (i64, i64) {
        if let Some(pos) = self.positions.get_mut(&(galaxy, system, position)) {
            let metal = pos.debris_metal;
            let crystal = pos.debris_crystal;
            pos.debris_metal = 0;
            pos.debris_crystal = 0;
            (metal, crystal)
        } else {
            (0, 0)
        }
    }

    pub fn find_free_position(&self, galaxy: i32, system: i32) -> Option<i32> {
        for pos in 1..=self.config.max_positions {
            match self.positions.get(&(galaxy, system, pos)) {
                None => return Some(pos),
                Some(gp) if !gp.is_occupied() => return Some(pos),
                _ => {}
            }
        }
        None
    }

    pub fn find_free_system_position(&self, galaxy_hint: i32) -> Option<(i32, i32, i32)> {
        // Search in the hinted galaxy first, then expand outward.
        let max_g = self.config.max_galaxies;
        let mut galaxies_to_try = Vec::with_capacity(max_g as usize);

        let hint = galaxy_hint.clamp(1, max_g);
        galaxies_to_try.push(hint);

        for offset in 1..max_g {
            let lower = hint - offset;
            let upper = hint + offset;
            if lower >= 1 {
                galaxies_to_try.push(lower);
            }
            if upper <= max_g {
                galaxies_to_try.push(upper);
            }
        }

        for g in galaxies_to_try {
            for s in 1..=self.config.max_systems {
                if let Some(p) = self.find_free_position(g, s) {
                    return Some((g, s, p));
                }
            }
        }

        None
    }

    pub fn count_planets_for_player(&self, player_id: i64) -> usize {
        self.positions
            .values()
            .filter(|gp| gp.player_id == Some(player_id))
            .count()
    }

    pub fn list_player_planets(&self, player_id: i64) -> Vec<(i32, i32, i32)> {
        let mut coords: Vec<(i32, i32, i32)> = self
            .positions
            .values()
            .filter(|gp| gp.player_id == Some(player_id))
            .map(|gp| (gp.galaxy, gp.system, gp.position))
            .collect();
        coords.sort();
        coords
    }
}

// ---------------------------------------------------------------------------
// Deterministic PRNG (Mulberry32)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Mulberry32 {
    state: u32,
}

impl Mulberry32 {
    fn new(seed: u64) -> Self {
        // Fold a u64 seed into u32, ensuring non-zero state.
        let s = (seed as u32) ^ ((seed >> 32) as u32);
        Self {
            state: if s == 0 { 0x9e3779b9 } else { s },
        }
    }

    fn next_u32(&mut self) -> u32 {
        let mut t = self.state.wrapping_add(0x6D2B79F5);
        self.state = t;
        t = t.wrapping_mul(t ^ (t >> 15));
        t = t.wrapping_add(t.wrapping_mul(t ^ (t >> 7)));
        t ^ (t >> 14)
    }

    /// Returns a value in `[0, bound)`.
    fn next_bounded(&mut self, bound: u32) -> u32 {
        if bound == 0 {
            return 0;
        }
        self.next_u32() % bound
    }
}

// ---------------------------------------------------------------------------
// Galaxy Generation (NPC planets)
// ---------------------------------------------------------------------------

pub fn generate_npc_planets(store: &mut GalaxyStore, count: usize, seed: u64) {
    let mut rng = Mulberry32::new(seed);

    let max_g = store.config.max_galaxies as u32;
    let max_s = store.config.max_systems as u32;
    let max_p = store.config.max_positions as u32;

    let mut placed = 0usize;
    let max_attempts = count * 20; // avoid infinite loops in dense galaxies
    let mut attempts = 0usize;

    while placed < count && attempts < max_attempts {
        attempts += 1;

        let g = (rng.next_bounded(max_g) + 1) as i32;
        let s = (rng.next_bounded(max_s) + 1) as i32;
        let p = (rng.next_bounded(max_p) + 1) as i32;

        let npc_planet_id = -(placed as i64 + 1); // negative IDs for NPCs
        let npc_player_id = 0_i64; // player 0 = NPC

        let result = store.place_planet(
            g,
            s,
            p,
            npc_planet_id,
            npc_player_id,
            "NPC",
            &format!("Colony-{g}:{s}:{p}"),
        );

        if result.is_ok() {
            placed += 1;
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn default_store() -> GalaxyStore {
        GalaxyStore::new(GalaxyConfig::default())
    }

    // -- Config defaults --

    #[test]
    fn config_defaults() {
        let cfg = GalaxyConfig::default();
        assert_eq!(cfg.max_galaxies, 9);
        assert_eq!(cfg.max_systems, 499);
        assert_eq!(cfg.max_positions, 15);
    }

    // -- Coordinate validation --

    #[test]
    fn validate_coordinates_accepts_valid() {
        let cfg = GalaxyConfig::default();
        assert!(validate_coordinates(1, 1, 1, &cfg).is_ok());
        assert!(validate_coordinates(9, 499, 15, &cfg).is_ok());
        assert!(validate_coordinates(5, 250, 8, &cfg).is_ok());
    }

    #[test]
    fn validate_coordinates_rejects_out_of_bounds() {
        let cfg = GalaxyConfig::default();
        assert_eq!(
            validate_coordinates(0, 1, 1, &cfg),
            Err(GalaxyError::OutOfBounds {
                field: "galaxy".to_string(),
                value: 0,
                max: 9
            })
        );
        assert_eq!(
            validate_coordinates(10, 1, 1, &cfg),
            Err(GalaxyError::OutOfBounds {
                field: "galaxy".to_string(),
                value: 10,
                max: 9
            })
        );
        assert_eq!(
            validate_coordinates(1, 500, 1, &cfg),
            Err(GalaxyError::OutOfBounds {
                field: "system".to_string(),
                value: 500,
                max: 499
            })
        );
        assert_eq!(
            validate_coordinates(1, 1, 16, &cfg),
            Err(GalaxyError::OutOfBounds {
                field: "position".to_string(),
                value: 16,
                max: 15
            })
        );
        assert_eq!(
            validate_coordinates(1, 0, 1, &cfg),
            Err(GalaxyError::OutOfBounds {
                field: "system".to_string(),
                value: 0,
                max: 499
            })
        );
    }

    // -- Placement --

    #[test]
    fn place_planet_success() {
        let mut store = default_store();
        let result = store.place_planet(1, 1, 1, 100, 10, "Alice", "Homeworld");
        assert!(result.is_ok());

        let pos = store.get_position(1, 1, 1).unwrap();
        assert_eq!(pos.planet_id, Some(100));
        assert_eq!(pos.player_id, Some(10));
        assert_eq!(pos.player_name.as_deref(), Some("Alice"));
        assert_eq!(pos.planet_name.as_deref(), Some("Homeworld"));
    }

    #[test]
    fn place_planet_rejects_occupied() {
        let mut store = default_store();
        store
            .place_planet(1, 1, 1, 100, 10, "Alice", "Homeworld")
            .unwrap();

        let result = store.place_planet(1, 1, 1, 200, 20, "Bob", "Colony");
        assert_eq!(result, Err(GalaxyError::PositionOccupied));
    }

    #[test]
    fn place_planet_rejects_out_of_bounds() {
        let mut store = default_store();
        let result = store.place_planet(0, 1, 1, 100, 10, "Alice", "Homeworld");
        assert!(matches!(result, Err(GalaxyError::OutOfBounds { .. })));
    }

    // -- Remove --

    #[test]
    fn remove_planet_clears_position() {
        let mut store = default_store();
        store
            .place_planet(3, 100, 8, 500, 42, "Charlie", "Mars")
            .unwrap();

        assert!(store.remove_planet(3, 100, 8));
        let pos = store.get_position(3, 100, 8).unwrap();
        assert!(pos.planet_id.is_none());
        assert!(pos.player_id.is_none());

        // Removing again returns false (no planet present).
        assert!(!store.remove_planet(3, 100, 8));
    }

    #[test]
    fn remove_planet_allows_reuse() {
        let mut store = default_store();
        store
            .place_planet(1, 1, 1, 100, 10, "Alice", "Homeworld")
            .unwrap();
        store.remove_planet(1, 1, 1);

        // The position is now free and can be re-occupied.
        let result = store.place_planet(1, 1, 1, 200, 20, "Bob", "Colony");
        assert!(result.is_ok());
        assert_eq!(store.get_position(1, 1, 1).unwrap().planet_id, Some(200));
    }

    // -- System View --

    #[test]
    fn system_view_returns_all_positions() {
        let mut store = default_store();
        store
            .place_planet(1, 50, 3, 100, 10, "Alice", "Planet-A")
            .unwrap();
        store
            .place_planet(1, 50, 7, 101, 11, "Bob", "Planet-B")
            .unwrap();

        let view = store.get_system_view(1, 50);
        assert_eq!(view.galaxy, 1);
        assert_eq!(view.system, 50);
        assert_eq!(view.positions.len(), 15);

        // Position 3 should be occupied.
        assert_eq!(view.positions[2].planet_id, Some(100));
        // Position 7 should be occupied.
        assert_eq!(view.positions[6].planet_id, Some(101));
        // Position 1 should be empty.
        assert!(view.positions[0].planet_id.is_none());
    }

    // -- Debris --

    #[test]
    fn debris_update_and_collect() {
        let mut store = default_store();

        // Add debris to an empty position.
        store.update_debris(2, 10, 5, 1000, 500);
        store.update_debris(2, 10, 5, 300, 200);

        let pos = store.get_position(2, 10, 5).unwrap();
        assert_eq!(pos.debris_metal, 1300);
        assert_eq!(pos.debris_crystal, 700);

        // Collect clears debris and returns amounts.
        let (metal, crystal) = store.collect_debris(2, 10, 5);
        assert_eq!(metal, 1300);
        assert_eq!(crystal, 700);

        let pos = store.get_position(2, 10, 5).unwrap();
        assert_eq!(pos.debris_metal, 0);
        assert_eq!(pos.debris_crystal, 0);

        // Collecting from a position without debris returns zeros.
        let (m2, c2) = store.collect_debris(9, 1, 1);
        assert_eq!(m2, 0);
        assert_eq!(c2, 0);
    }

    // -- Free position finding --

    #[test]
    fn find_free_position_in_system() {
        let mut store = default_store();

        // Empty system: first free position is 1.
        assert_eq!(store.find_free_position(1, 1), Some(1));

        // Occupy positions 1..=14.
        for p in 1..=14 {
            store
                .place_planet(1, 1, p, p as i64, 1, "Test", &format!("P{p}"))
                .unwrap();
        }
        assert_eq!(store.find_free_position(1, 1), Some(15));

        // Occupy position 15 -> system full.
        store.place_planet(1, 1, 15, 15, 1, "Test", "P15").unwrap();
        assert_eq!(store.find_free_position(1, 1), None);
    }

    #[test]
    fn find_free_system_position_basic() {
        let store = default_store();
        // Completely empty galaxy — should find (hint, 1, 1).
        let result = store.find_free_system_position(5);
        assert_eq!(result, Some((5, 1, 1)));
    }

    #[test]
    fn find_free_system_position_expands_to_other_galaxies() {
        // Use a tiny config so we can fill an entire galaxy.
        let config = GalaxyConfig {
            max_galaxies: 3,
            max_systems: 2,
            max_positions: 2,
        };
        let mut store = GalaxyStore::new(config);

        // Fill galaxy 2 completely (2 systems * 2 positions = 4 planets).
        let mut id = 1i64;
        for s in 1..=2 {
            for p in 1..=2 {
                store.place_planet(2, s, p, id, 1, "Filler", "X").unwrap();
                id += 1;
            }
        }

        // Hint galaxy 2 — it's full, should find in galaxy 1 or 3.
        let result = store.find_free_system_position(2);
        assert!(result.is_some());
        let (g, _s, _p) = result.unwrap();
        assert!(g == 1 || g == 3);
    }

    // -- Player planet tracking --

    #[test]
    fn count_and_list_player_planets() {
        let mut store = default_store();
        store.place_planet(1, 1, 1, 100, 10, "Alice", "P1").unwrap();
        store
            .place_planet(2, 50, 8, 101, 10, "Alice", "P2")
            .unwrap();
        store
            .place_planet(3, 200, 15, 102, 10, "Alice", "P3")
            .unwrap();
        store.place_planet(1, 1, 2, 200, 20, "Bob", "P1").unwrap();

        assert_eq!(store.count_planets_for_player(10), 3);
        assert_eq!(store.count_planets_for_player(20), 1);
        assert_eq!(store.count_planets_for_player(99), 0);

        let alice_planets = store.list_player_planets(10);
        assert_eq!(alice_planets, vec![(1, 1, 1), (2, 50, 8), (3, 200, 15)]);
    }

    // -- NPC generation --

    #[test]
    fn npc_generation_deterministic() {
        let mut store1 = default_store();
        generate_npc_planets(&mut store1, 50, 42);

        let mut store2 = default_store();
        generate_npc_planets(&mut store2, 50, 42);

        // Same seed produces the same planets.
        let npc1 = store1.list_player_planets(0);
        let npc2 = store2.list_player_planets(0);
        assert_eq!(npc1, npc2);
        assert_eq!(npc1.len(), 50);
    }

    #[test]
    fn npc_generation_different_seeds_differ() {
        let mut store1 = default_store();
        generate_npc_planets(&mut store1, 20, 1);

        let mut store2 = default_store();
        generate_npc_planets(&mut store2, 20, 2);

        let npc1 = store1.list_player_planets(0);
        let npc2 = store2.list_player_planets(0);
        // Very unlikely (essentially impossible) to be identical with different seeds.
        assert_ne!(npc1, npc2);
    }

    #[test]
    fn npc_generation_respects_bounds() {
        let config = GalaxyConfig {
            max_galaxies: 2,
            max_systems: 3,
            max_positions: 4,
        };
        let mut store = GalaxyStore::new(config);
        generate_npc_planets(&mut store, 10, 99);

        let planets = store.list_player_planets(0);
        for (g, s, p) in &planets {
            assert!(*g >= 1 && *g <= 2, "galaxy {g} out of bounds");
            assert!(*s >= 1 && *s <= 3, "system {s} out of bounds");
            assert!(*p >= 1 && *p <= 4, "position {p} out of bounds");
        }
        assert_eq!(planets.len(), 10);
    }

    // -- Serialization round-trip --

    #[test]
    fn galaxy_position_serialization_roundtrip() {
        let gp = GalaxyPosition {
            galaxy: 1,
            system: 42,
            position: 7,
            planet_id: Some(100),
            player_id: Some(10),
            player_name: Some("Alice".to_string()),
            planet_name: Some("Homeworld".to_string()),
            moon_id: None,
            debris_metal: 500,
            debris_crystal: 250,
            is_inactive: false,
            is_vacation: true,
            is_banned: false,
            alliance_tag: Some("TEST".to_string()),
        };

        let json = serde_json::to_string(&gp).unwrap();
        let deserialized: GalaxyPosition = serde_json::from_str(&json).unwrap();
        assert_eq!(gp, deserialized);
    }
}
