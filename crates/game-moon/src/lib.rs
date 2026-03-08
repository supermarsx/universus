#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Deterministic PRNG — Mulberry32 (same algorithm as game-combat)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct Mulberry32 {
    state: u32,
}

impl Mulberry32 {
    fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    fn next_u32(&mut self) -> u32 {
        let mut t = self.state.wrapping_add(0x6D2B79F5);
        self.state = t;
        t = t.wrapping_mul(t ^ (t >> 15));
        t = t.wrapping_add(t.wrapping_mul(t ^ (t >> 7)));
        t ^ (t >> 14)
    }

    fn next_f64(&mut self) -> f64 {
        (self.next_u32() as f64) / (u32::MAX as f64 + 1.0)
    }
}

// ---------------------------------------------------------------------------
// Core structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MoonBuildings {
    pub lunar_base_level: i32,
    pub sensor_phalanx_level: i32,
    pub jump_gate_level: i32,
    pub robotics_factory_level: i32,
    pub shipyard_level: i32,
}

impl Default for MoonBuildings {
    fn default() -> Self {
        Self {
            lunar_base_level: 0,
            sensor_phalanx_level: 0,
            jump_gate_level: 0,
            robotics_factory_level: 0,
            shipyard_level: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Moon {
    pub id: i64,
    pub planet_id: i64,
    pub name: String,
    pub diameter: i32,
    pub temperature_min: i32,
    pub temperature_max: i32,
    pub fields_used: i32,
    pub fields_max: i32,
    pub buildings: MoonBuildings,
    pub created_at: String,
    pub destroyed_at: Option<String>,
}

// ---------------------------------------------------------------------------
// Moon creation from combat debris (OGame formula)
// ---------------------------------------------------------------------------

/// Calculates the percentage chance of moon creation from combat debris.
///
/// Formula: chance% = min(20, floor((metal + crystal) / 100_000))
pub fn calculate_moon_chance(debris_metal: i64, debris_crystal: i64) -> f64 {
    let total = debris_metal.saturating_add(debris_crystal);
    let raw = total / 100_000;
    (raw as f64).min(20.0).max(0.0)
}

/// Deterministic check whether a moon should be created given a chance
/// percentage and a seed. Returns true if the roll is below the chance.
pub fn should_create_moon(chance: f64, seed: u64) -> bool {
    if chance <= 0.0 {
        return false;
    }
    let mut rng = Mulberry32::new(seed as u32);
    let roll = rng.next_f64() * 100.0; // 0..100
    roll < chance
}

/// Generates a moon diameter from the creation chance and a seed.
///
/// diameter = (chance * 100 + random(10..20)) * 100, clamped to 3474..8944
pub fn generate_moon_diameter(chance: f64, seed: u64) -> i32 {
    let mut rng = Mulberry32::new(seed as u32);
    // random value in [10, 20)
    let random_part = 10.0 + rng.next_f64() * 10.0;
    let raw = (chance * 100.0 + random_part) * 100.0;
    (raw as i32).clamp(3474, 8944)
}

/// Calculates the maximum number of fields on a moon from its diameter.
///
/// fields = floor(diameter / 1000)^2
pub fn calculate_moon_fields(diameter: i32) -> i32 {
    let base = diameter / 1000;
    base * base
}

/// Orchestrates moon creation from combat debris.
///
/// Returns `Some(Moon)` if the PRNG roll succeeds, otherwise `None`.
pub fn create_moon_from_combat(
    planet_id: i64,
    planet_name: &str,
    debris_metal: i64,
    debris_crystal: i64,
    temperature_min: i32,
    temperature_max: i32,
    seed: u64,
) -> Option<Moon> {
    let chance = calculate_moon_chance(debris_metal, debris_crystal);
    if !should_create_moon(chance, seed) {
        return None;
    }

    // Use a different seed derivation for diameter so the two rolls are
    // independent (creation check vs diameter).
    let diameter_seed = seed.wrapping_add(1);
    let diameter = generate_moon_diameter(chance, diameter_seed);
    let fields_max = calculate_moon_fields(diameter);

    Some(Moon {
        id: 0,
        planet_id,
        name: format!("{} Moon", planet_name),
        diameter,
        temperature_min,
        temperature_max,
        fields_used: 0,
        fields_max,
        buildings: MoonBuildings::default(),
        created_at: String::new(),
        destroyed_at: None,
    })
}

// ---------------------------------------------------------------------------
// Moon destruction (RIP / Deathstar attack)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MoonDestructionInput {
    pub attacker_id: i64,
    pub defender_id: i64,
    pub moon_id: i64,
    pub rip_count: i32,
    pub moon_diameter: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MoonDestructionResult {
    pub moon_destroyed: bool,
    pub fleet_destroyed: bool,
    pub moon_destruction_chance: f64,
    pub fleet_destruction_chance: f64,
}

/// Calculates the outcome of a RIP (Deathstar) moon-destruction mission.
///
/// Moon destruction chance:  (100 - sqrt(diameter)) * sqrt(rip_count), clamped 0..100
/// Fleet destruction chance: 0.5 * sqrt(diameter) / sqrt(rip_count),   clamped 0..100
///
/// Both checks are resolved with a deterministic PRNG.
pub fn calculate_moon_destruction(
    input: &MoonDestructionInput,
    seed: u64,
) -> MoonDestructionResult {
    let diameter_sqrt = (input.moon_diameter as f64).sqrt();
    let rip_sqrt = (input.rip_count as f64).max(1.0).sqrt();

    let moon_chance = ((100.0 - diameter_sqrt) * rip_sqrt).clamp(0.0, 100.0);
    let fleet_chance = (0.5 * diameter_sqrt / rip_sqrt).clamp(0.0, 100.0);

    let mut rng = Mulberry32::new(seed as u32);

    let moon_roll = rng.next_f64() * 100.0;
    let fleet_roll = rng.next_f64() * 100.0;

    MoonDestructionResult {
        moon_destroyed: moon_roll < moon_chance,
        fleet_destroyed: fleet_roll < fleet_chance,
        moon_destruction_chance: moon_chance,
        fleet_destruction_chance: fleet_chance,
    }
}

// ---------------------------------------------------------------------------
// Sensor Phalanx
// ---------------------------------------------------------------------------

/// Returns the scanning range in systems for a given sensor phalanx level.
///
/// range = level^2 - 1
pub fn phalanx_range(level: i32) -> i32 {
    if level <= 0 {
        return 0;
    }
    level * level - 1
}

/// Checks whether a phalanx at the given level and system coordinate can
/// scan a target system.
pub fn can_phalanx(phalanx_level: i32, moon_system: i32, target_system: i32) -> bool {
    let range = phalanx_range(phalanx_level);
    let distance = (moon_system - target_system).abs();
    distance <= range
}

// ---------------------------------------------------------------------------
// Jump Gate
// ---------------------------------------------------------------------------

/// The jump-gate cooldown duration in seconds (1 hour).
pub fn jump_gate_cooldown() -> i64 {
    3600
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JumpGateState {
    /// Unix timestamp (seconds) of last gate usage. 0 means never used.
    pub last_used: i64,
}

/// Returns whether the jump gate can be used right now.
pub fn can_use_jump_gate(state: &JumpGateState, now: i64) -> bool {
    remaining_cooldown(state, now) == 0
}

/// Returns the remaining cooldown in seconds (0 when ready).
pub fn remaining_cooldown(state: &JumpGateState, now: i64) -> i64 {
    if state.last_used == 0 {
        return 0;
    }
    let elapsed = now - state.last_used;
    let remaining = jump_gate_cooldown() - elapsed;
    remaining.max(0)
}

// ---------------------------------------------------------------------------
// Moon Building Costs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MoonBuildingCost {
    pub metal: i64,
    pub crystal: i64,
    pub deuterium: i64,
}

/// Returns the resource cost for constructing the next level of a moon
/// building.
///
/// Supported buildings:
/// - `"lunar_base"`: 20_000 × 2^(level-1) metal, 40_000 × 2^(level-1)
///   crystal, 20_000 × 2^(level-1) deuterium
/// - `"sensor_phalanx"`: 20_000 × 2^(level-1) metal, 40_000 × 2^(level-1)
///   crystal, 20_000 × 2^(level-1) deuterium
/// - `"jump_gate"`: 2_000_000 × 2^(level-1) metal, 4_000_000 × 2^(level-1)
///   crystal, 2_000_000 × 2^(level-1) deuterium
/// - `"robotics_factory"`: 400 × 2^(level-1) metal, 120 × 2^(level-1)
///   crystal, 200 × 2^(level-1) deuterium
/// - `"shipyard"`: 400 × 2^(level-1) metal, 200 × 2^(level-1) crystal,
///   100 × 2^(level-1) deuterium
///
/// Unknown buildings return zero cost.
pub fn moon_building_cost(building: &str, level: i32) -> MoonBuildingCost {
    if level <= 0 {
        return MoonBuildingCost {
            metal: 0,
            crystal: 0,
            deuterium: 0,
        };
    }

    let multiplier = 2_i64.pow((level - 1) as u32);

    match building {
        "lunar_base" => MoonBuildingCost {
            metal: 20_000 * multiplier,
            crystal: 40_000 * multiplier,
            deuterium: 20_000 * multiplier,
        },
        "sensor_phalanx" => MoonBuildingCost {
            metal: 20_000 * multiplier,
            crystal: 40_000 * multiplier,
            deuterium: 20_000 * multiplier,
        },
        "jump_gate" => MoonBuildingCost {
            metal: 2_000_000 * multiplier,
            crystal: 4_000_000 * multiplier,
            deuterium: 2_000_000 * multiplier,
        },
        "robotics_factory" => MoonBuildingCost {
            metal: 400 * multiplier,
            crystal: 120 * multiplier,
            deuterium: 200 * multiplier,
        },
        "shipyard" => MoonBuildingCost {
            metal: 400 * multiplier,
            crystal: 200 * multiplier,
            deuterium: 100 * multiplier,
        },
        _ => MoonBuildingCost {
            metal: 0,
            crystal: 0,
            deuterium: 0,
        },
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- PRNG sanity -------------------------------------------------------

    #[test]
    fn mulberry32_deterministic() {
        let mut a = Mulberry32::new(42);
        let mut b = Mulberry32::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn mulberry32_range() {
        let mut rng = Mulberry32::new(12345);
        for _ in 0..1000 {
            let v = rng.next_f64();
            assert!((0.0..1.0).contains(&v));
        }
    }

    // -- Moon chance -------------------------------------------------------

    #[test]
    fn moon_chance_zero_debris() {
        assert_eq!(calculate_moon_chance(0, 0), 0.0);
    }

    #[test]
    fn moon_chance_below_threshold() {
        // 50_000 + 30_000 = 80_000 → floor(80_000 / 100_000) = 0
        assert_eq!(calculate_moon_chance(50_000, 30_000), 0.0);
    }

    #[test]
    fn moon_chance_normal() {
        // 500_000 + 500_000 = 1_000_000 → floor(1_000_000 / 100_000) = 10
        assert_eq!(calculate_moon_chance(500_000, 500_000), 10.0);
    }

    #[test]
    fn moon_chance_capped_at_20() {
        // 5_000_000 total → 50, but capped at 20
        assert_eq!(calculate_moon_chance(3_000_000, 2_000_000), 20.0);
    }

    // -- Moon creation determinism -----------------------------------------

    #[test]
    fn should_create_moon_deterministic() {
        let r1 = should_create_moon(10.0, 999);
        let r2 = should_create_moon(10.0, 999);
        assert_eq!(r1, r2);
    }

    #[test]
    fn should_create_moon_zero_chance() {
        assert!(!should_create_moon(0.0, 42));
    }

    // -- Diameter generation -----------------------------------------------

    #[test]
    fn diameter_within_bounds() {
        for seed in 0..200 {
            let d = generate_moon_diameter(20.0, seed);
            assert!(d >= 3474, "diameter {} below min for seed {}", d, seed);
            assert!(d <= 8944, "diameter {} above max for seed {}", d, seed);
        }
    }

    #[test]
    fn diameter_deterministic() {
        let d1 = generate_moon_diameter(15.0, 777);
        let d2 = generate_moon_diameter(15.0, 777);
        assert_eq!(d1, d2);
    }

    #[test]
    fn diameter_increases_with_chance() {
        // Higher chance should generally produce larger diameters.
        // With same seed, the random component is identical, so only the
        // chance*100 term differs.
        let d_low = generate_moon_diameter(1.0, 100);
        let d_high = generate_moon_diameter(20.0, 100);
        assert!(d_high >= d_low);
    }

    // -- Moon fields -------------------------------------------------------

    #[test]
    fn moon_fields_calculation() {
        // diameter 3474 → floor(3474/1000) = 3 → 9
        assert_eq!(calculate_moon_fields(3474), 9);
        // diameter 8944 → floor(8944/1000) = 8 → 64
        assert_eq!(calculate_moon_fields(8944), 64);
        // diameter 5500 → floor(5500/1000) = 5 → 25
        assert_eq!(calculate_moon_fields(5500), 25);
    }

    // -- Full moon creation flow -------------------------------------------

    #[test]
    fn create_moon_from_combat_no_debris() {
        let result = create_moon_from_combat(1, "Earth", 0, 0, -40, 10, 42);
        assert!(result.is_none());
    }

    #[test]
    fn create_moon_from_combat_structure() {
        // Use a high debris amount (20% chance) and try many seeds to find
        // one that succeeds.
        let mut moon = None;
        for seed in 0..1000 {
            if let Some(m) =
                create_moon_from_combat(42, "Homeworld", 1_500_000, 500_000, -30, 20, seed)
            {
                moon = Some(m);
                break;
            }
        }
        let m = moon.expect("should create a moon with 20% chance within 1000 seeds");
        assert_eq!(m.planet_id, 42);
        assert_eq!(m.name, "Homeworld Moon");
        assert!(m.diameter >= 3474 && m.diameter <= 8944);
        assert!(m.fields_max > 0);
        assert_eq!(m.fields_used, 0);
        assert!(m.destroyed_at.is_none());
        assert_eq!(m.buildings, MoonBuildings::default());
    }

    // -- Moon destruction --------------------------------------------------

    #[test]
    fn destruction_chance_formulas() {
        let input = MoonDestructionInput {
            attacker_id: 1,
            defender_id: 2,
            moon_id: 10,
            rip_count: 100,
            moon_diameter: 8000,
        };
        let result = calculate_moon_destruction(&input, 42);

        // Moon chance: (100 - sqrt(8000)) * sqrt(100)
        //            = (100 - 89.44...) * 10 = ~105.57 → clamped to 100
        assert_eq!(result.moon_destruction_chance, 100.0);

        // Fleet chance: 0.5 * sqrt(8000) / sqrt(100)
        //             = 0.5 * 89.44... / 10 = ~4.47
        let expected_fleet = 0.5 * (8000_f64).sqrt() / (100_f64).sqrt();
        assert!((result.fleet_destruction_chance - expected_fleet).abs() < 0.01);
    }

    #[test]
    fn destruction_deterministic() {
        let input = MoonDestructionInput {
            attacker_id: 1,
            defender_id: 2,
            moon_id: 5,
            rip_count: 10,
            moon_diameter: 5000,
        };
        let r1 = calculate_moon_destruction(&input, 123);
        let r2 = calculate_moon_destruction(&input, 123);
        assert_eq!(r1, r2);
    }

    #[test]
    fn destruction_low_rip_count() {
        let input = MoonDestructionInput {
            attacker_id: 1,
            defender_id: 2,
            moon_id: 5,
            rip_count: 1,
            moon_diameter: 8944,
        };
        let result = calculate_moon_destruction(&input, 0);
        // With only 1 RIP against max diameter:
        // moon chance = (100 - 94.57) * 1 ≈ 5.43
        // fleet chance = 0.5 * 94.57 / 1 ≈ 47.28
        assert!(result.moon_destruction_chance < 10.0);
        assert!(result.fleet_destruction_chance > 40.0);
    }

    // -- Phalanx ----------------------------------------------------------

    #[test]
    fn phalanx_range_levels() {
        assert_eq!(phalanx_range(0), 0);
        assert_eq!(phalanx_range(1), 0); // 1^2 - 1 = 0
        assert_eq!(phalanx_range(2), 3); // 4 - 1
        assert_eq!(phalanx_range(3), 8); // 9 - 1
        assert_eq!(phalanx_range(7), 48); // 49 - 1
    }

    #[test]
    fn can_phalanx_checks() {
        // Level 3 → range 8
        assert!(can_phalanx(3, 100, 108));
        assert!(can_phalanx(3, 100, 92));
        assert!(!can_phalanx(3, 100, 109));
        // Level 0 → range 0
        assert!(!can_phalanx(0, 50, 51));
        // Same system always true for level >= 1 (distance 0)
        assert!(can_phalanx(1, 50, 50));
    }

    // -- Jump gate --------------------------------------------------------

    #[test]
    fn jump_gate_cooldown_value() {
        assert_eq!(jump_gate_cooldown(), 3600);
    }

    #[test]
    fn jump_gate_ready_when_never_used() {
        let state = JumpGateState { last_used: 0 };
        assert!(can_use_jump_gate(&state, 99999));
        assert_eq!(remaining_cooldown(&state, 99999), 0);
    }

    #[test]
    fn jump_gate_on_cooldown() {
        let state = JumpGateState { last_used: 1000 };
        // At t=2000, 1000s have passed, 2600 remain
        assert!(!can_use_jump_gate(&state, 2000));
        assert_eq!(remaining_cooldown(&state, 2000), 2600);
    }

    #[test]
    fn jump_gate_ready_after_cooldown() {
        let state = JumpGateState { last_used: 1000 };
        // At t=4600, exactly 3600s have passed
        assert!(can_use_jump_gate(&state, 4600));
        assert_eq!(remaining_cooldown(&state, 4600), 0);
        // Also after cooldown
        assert!(can_use_jump_gate(&state, 5000));
        assert_eq!(remaining_cooldown(&state, 5000), 0);
    }

    // -- Building costs ---------------------------------------------------

    #[test]
    fn lunar_base_cost_level_1() {
        let cost = moon_building_cost("lunar_base", 1);
        assert_eq!(cost.metal, 20_000);
        assert_eq!(cost.crystal, 40_000);
        assert_eq!(cost.deuterium, 20_000);
    }

    #[test]
    fn lunar_base_cost_level_3() {
        // 2^(3-1) = 4
        let cost = moon_building_cost("lunar_base", 3);
        assert_eq!(cost.metal, 80_000);
        assert_eq!(cost.crystal, 160_000);
        assert_eq!(cost.deuterium, 80_000);
    }

    #[test]
    fn jump_gate_cost_level_1() {
        let cost = moon_building_cost("jump_gate", 1);
        assert_eq!(cost.metal, 2_000_000);
        assert_eq!(cost.crystal, 4_000_000);
        assert_eq!(cost.deuterium, 2_000_000);
    }

    #[test]
    fn robotics_factory_cost_level_2() {
        // 2^(2-1) = 2
        let cost = moon_building_cost("robotics_factory", 2);
        assert_eq!(cost.metal, 800);
        assert_eq!(cost.crystal, 240);
        assert_eq!(cost.deuterium, 400);
    }

    #[test]
    fn shipyard_cost_level_1() {
        let cost = moon_building_cost("shipyard", 1);
        assert_eq!(cost.metal, 400);
        assert_eq!(cost.crystal, 200);
        assert_eq!(cost.deuterium, 100);
    }

    #[test]
    fn unknown_building_zero_cost() {
        let cost = moon_building_cost("unknown_building", 5);
        assert_eq!(cost.metal, 0);
        assert_eq!(cost.crystal, 0);
        assert_eq!(cost.deuterium, 0);
    }

    #[test]
    fn zero_level_zero_cost() {
        let cost = moon_building_cost("lunar_base", 0);
        assert_eq!(cost.metal, 0);
        assert_eq!(cost.crystal, 0);
        assert_eq!(cost.deuterium, 0);
    }

    // -- Serialization round-trip ------------------------------------------

    #[test]
    fn moon_serde_roundtrip() {
        let moon = Moon {
            id: 1,
            planet_id: 42,
            name: "Test Moon".into(),
            diameter: 5000,
            temperature_min: -40,
            temperature_max: 10,
            fields_used: 2,
            fields_max: 25,
            buildings: MoonBuildings {
                lunar_base_level: 3,
                sensor_phalanx_level: 2,
                jump_gate_level: 1,
                robotics_factory_level: 4,
                shipyard_level: 2,
            },
            created_at: "2026-01-01T00:00:00Z".into(),
            destroyed_at: None,
        };
        let json = serde_json::to_string(&moon).unwrap();
        let deserialized: Moon = serde_json::from_str(&json).unwrap();
        assert_eq!(moon, deserialized);
    }

    #[test]
    fn destruction_result_serde_roundtrip() {
        let result = MoonDestructionResult {
            moon_destroyed: true,
            fleet_destroyed: false,
            moon_destruction_chance: 75.5,
            fleet_destruction_chance: 12.3,
        };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: MoonDestructionResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
