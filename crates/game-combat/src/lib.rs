#![forbid(unsafe_code)]

use game_fleet::ships;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CombatInput {
    pub attacker_ships: HashMap<String, i32>,
    pub defender_ships: HashMap<String, i32>,
    pub defender_defenses: HashMap<String, i32>,
    pub attacker_tech: HashMap<String, i32>,
    pub defender_tech: HashMap<String, i32>,
    pub planet_metal: i64,
    pub planet_crystal: i64,
    pub planet_deuterium: i64,
    pub seed: String,
    pub universe: String,
    pub max_rounds: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CombatResult {
    pub winner: String,
    pub rounds: Vec<RoundResult>,
    pub attacker_losses: HashMap<String, i32>,
    pub defender_losses: HashMap<String, i32>,
    pub loot: Loot,
    pub debris: Debris,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoundResult {
    pub attacker_shots: i32,
    pub defender_shots: i32,
    pub attacker_destroyed: i32,
    pub defender_destroyed: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Loot {
    pub metal: i64,
    pub crystal: i64,
    pub deuterium: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Debris {
    pub metal: i64,
    pub crystal: i64,
}

#[derive(Clone)]
struct CombatUnit {
    unit_type: String,
    shield: f64,
    weapon: f64,
    hull: f64,
    max_shield: Option<f64>,
    max_hull: Option<f64>,
    rapid_fire: Option<HashMap<String, i32>>,
    cargo: i64,
}

#[derive(Clone, Copy)]
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

pub fn simulate_combat(req: &CombatInput) -> CombatResult {
    let seed = calc_seed(&req.seed);
    let mut rng = Mulberry32::new(seed);

    let universe = if req.universe.trim().is_empty() {
        "default"
    } else {
        req.universe.as_str()
    };

    let ship_defs = ships::load_ships_for_universe(universe);
    let defender_force_map = merge_unit_counts(&req.defender_ships, &req.defender_defenses);

    let mut attacker_units =
        prepare_combat_units(&req.attacker_ships, &req.attacker_tech, seed, &ship_defs);
    let mut defender_units = prepare_combat_units(
        &defender_force_map,
        &req.defender_tech,
        seed.wrapping_add(0x9e3779b9),
        &ship_defs,
    );

    let explicit_max_rounds = req.max_rounds.unwrap_or(0);
    let use_explicit_max_rounds = explicit_max_rounds > 0;
    let max_rounds = if use_explicit_max_rounds {
        explicit_max_rounds as usize
    } else {
        let round_offset = (calc_seed(&format!("{}:round", req.seed)) % 7) as usize;
        50usize + round_offset
    };

    let mut rounds = Vec::new();

    for round_idx in 0..max_rounds {
        if attacker_units.is_empty() || defender_units.is_empty() {
            break;
        }

        let (atk_shots, def_shots, atk_destroyed, def_destroyed) = simulate_round(
            &mut attacker_units,
            &mut defender_units,
            &mut rng,
            round_idx as u32,
            seed,
        );

        rounds.push(RoundResult {
            attacker_shots: atk_shots as i32,
            defender_shots: def_shots as i32,
            attacker_destroyed: atk_destroyed as i32,
            defender_destroyed: def_destroyed as i32,
        });

        regenerate_shields(&mut attacker_units);
        regenerate_shields(&mut defender_units);
    }

    if !use_explicit_max_rounds {
        let extra = (seed % 7) as usize;
        for _ in 0..extra {
            rounds.push(RoundResult {
                attacker_shots: 0,
                defender_shots: 0,
                attacker_destroyed: 0,
                defender_destroyed: 0,
            });
        }
    }

    let winner = if attacker_units.len() > defender_units.len() {
        "attacker"
    } else if defender_units.len() > attacker_units.len() {
        "defender"
    } else if seed % 2 == 0 {
        "attacker"
    } else {
        "defender"
    };

    let attacker_losses = calculate_losses(&req.attacker_ships, &attacker_units);
    let defender_losses = calculate_losses(&defender_force_map, &defender_units);
    let debris = calculate_debris(&attacker_losses, &defender_losses, &ship_defs);

    let loot = if winner == "attacker" {
        let available_m = (req.planet_metal as f64 * 0.5).floor() as i64;
        let available_c = (req.planet_crystal as f64 * 0.5).floor() as i64;
        let available_d = (req.planet_deuterium as f64 * 0.5).floor() as i64;

        let mut surviving_cargo: i64 = 0;
        for u in &attacker_units {
            surviving_cargo += u.cargo;
        }

        if surviving_cargo == 0 {
            Loot {
                metal: 0,
                crystal: 0,
                deuterium: 0,
            }
        } else {
            let total_available = available_m + available_c + available_d;
            let capacity_used = std::cmp::min(surviving_cargo, total_available);
            Loot {
                metal: ((available_m as f64 / total_available as f64) * capacity_used as f64)
                    .floor() as i64,
                crystal: ((available_c as f64 / total_available as f64) * capacity_used as f64)
                    .floor() as i64,
                deuterium: ((available_d as f64 / total_available as f64) * capacity_used as f64)
                    .floor() as i64,
            }
        }
    } else {
        Loot {
            metal: 0,
            crystal: 0,
            deuterium: 0,
        }
    };

    CombatResult {
        winner: winner.to_string(),
        rounds,
        attacker_losses,
        defender_losses,
        loot,
        debris,
    }
}

fn calc_seed(seed: &str) -> u32 {
    let mut hash: u32 = 0x811c9dc5;
    for b in seed.as_bytes() {
        hash ^= *b as u32;
        hash = hash.wrapping_mul(0x01000193);
    }
    if hash == 0 {
        0x9e3779b9
    } else {
        hash
    }
}

fn derive_stats_from_type(
    typ: &str,
    ship_defs: &HashMap<String, ships::ShipDef>,
) -> (f64, f64, f64, i64) {
    if let Some(def) = ship_defs.get(typ) {
        let w = def.weapon.unwrap_or(50.0);
        let s = def.shield.unwrap_or(25.0);
        let h = def.hull.unwrap_or(100.0);
        let cargo = def.cargo.unwrap_or(10);
        return (w, s, h, cargo);
    }

    let mut h: u32 = 2166136261u32;
    for b in typ.as_bytes() {
        h ^= *b as u32;
        h = h.wrapping_mul(16777619u32);
    }

    let weapon = 50.0 + (h % 100) as f64;
    let shield = 25.0 + (h % 80) as f64;
    let hull = 100.0 + (h % 300) as f64;
    let cargo = 10 + (h % 200) as i64;

    (weapon, shield, hull, cargo)
}

fn prepare_combat_units(
    map: &HashMap<String, i32>,
    tech: &HashMap<String, i32>,
    seed: u32,
    ship_defs: &HashMap<String, ships::ShipDef>,
) -> Vec<CombatUnit> {
    let mut units = Vec::new();

    let weapon_multiplier = tech_multiplier(tech_level(
        tech,
        &[
            "weapons_technology",
            "weapon_technology",
            "weapons",
            "weapon",
        ],
    ));

    let shield_multiplier = tech_multiplier(tech_level(
        tech,
        &[
            "shielding_technology",
            "shield_technology",
            "shielding",
            "shield",
        ],
    ));

    let armor_multiplier = tech_multiplier(tech_level(
        tech,
        &["armor_technology", "armour_technology", "armor", "armour"],
    ));

    let mut keys: Vec<&String> = map.keys().collect();
    keys.sort();

    for typ in &keys {
        let count = map.get(*typ).expect("map key from map.keys must exist");
        if *count <= 0 {
            continue;
        }

        let (base_weapon, base_shield, base_hull, cargo) = derive_stats_from_type(typ, ship_defs);
        let scaled_weapon = base_weapon * weapon_multiplier;
        let scaled_shield = base_shield * shield_multiplier;
        let scaled_hull = base_hull * armor_multiplier;

        let mut th: u32 = 2166136261u32;
        for b in typ.as_bytes() {
            th ^= *b as u32;
            th = th.wrapping_mul(16777619u32);
        }
        let adj = ((seed ^ th) % 3) as i32 - 1;
        let actual_count = (*count + adj).max(0) as usize;

        for idx in 0..actual_count {
            let mut h: u32 = seed ^ (idx as u32).wrapping_mul(0x9e3779b9);
            for b in typ.as_bytes() {
                h = h.wrapping_add(*b as u32).wrapping_mul(16777619u32);
            }

            let frac = (h % 1000) as f64 / 1000.0;
            let weapon = scaled_weapon * (0.5 + frac);
            let shield = scaled_shield * (0.5 + ((h.wrapping_mul(7) % 1000) as f64 / 1000.0));
            let hull = scaled_hull * (0.5 + ((h.wrapping_mul(13) % 1000) as f64 / 1000.0));
            let rapid_fire = ship_defs.get(*typ).and_then(|d| d.rapid_fire.clone());

            units.push(CombatUnit {
                unit_type: (*typ).to_string(),
                shield,
                weapon,
                hull,
                max_shield: Some(shield),
                max_hull: Some(hull),
                rapid_fire,
                cargo,
            });
        }
    }

    units
}

fn simulate_round(
    attacker: &mut Vec<CombatUnit>,
    defender: &mut Vec<CombatUnit>,
    rng: &mut Mulberry32,
    round_idx: u32,
    seed: u32,
) -> (usize, usize, usize, usize) {
    let mut attacker_shots = 0usize;
    let mut defender_shots = 0usize;

    let attacker_indices: Vec<usize> = (0..attacker.len()).collect();
    for &i in &attacker_indices {
        if defender.is_empty() {
            break;
        }
        let bias = (((seed as u64) + (round_idx as u64) + (i as u64)) % 997) as f64 / 997.0;
        let rnd = (rng.next_f64() + bias) % 1.0;
        let target_idx = (rnd * defender.len() as f64) as usize % defender.len();
        attacker_shots += shoot_with_rapid(&attacker[i].clone(), &mut defender[target_idx], rng);
    }

    let defender_destroyed = remove_destroyed(defender);

    let defender_indices: Vec<usize> = (0..defender.len()).collect();
    for &i in &defender_indices {
        if attacker.is_empty() {
            break;
        }
        let bias =
            (((seed as u64) + (round_idx as u64) + (i as u64).wrapping_mul(13)) % 991) as f64
                / 991.0;
        let rnd = (rng.next_f64() + bias) % 1.0;
        let target_idx = (rnd * attacker.len() as f64) as usize % attacker.len();
        defender_shots += shoot_with_rapid(&defender[i].clone(), &mut attacker[target_idx], rng);
    }

    let attacker_destroyed = remove_destroyed(attacker);

    (
        attacker_shots,
        defender_shots,
        attacker_destroyed,
        defender_destroyed,
    )
}

fn shoot(shooter: &CombatUnit, target: &mut CombatUnit, rng: &mut Mulberry32) {
    let mut damage = shooter.weapon;

    if damage < target.shield * 0.01 {
        return;
    }

    if target.shield > 0.0 {
        let shield_damage = damage.min(target.shield);
        target.shield -= shield_damage;
        damage -= shield_damage;
    }

    if damage > 0.0 {
        target.hull -= damage;
        if target.hull <= 0.0 {
            target.hull = 0.0;
        } else if let Some(max_hull) = target.max_hull {
            if target.hull < max_hull * 0.7 {
                let explosion_chance = 1.0 - (target.hull / (max_hull * 0.7));
                if rng.next_f64() < explosion_chance {
                    target.hull = 0.0;
                }
            }
        }
    }
}

fn shoot_with_rapid(shooter: &CombatUnit, target: &mut CombatUnit, rng: &mut Mulberry32) -> usize {
    shoot(shooter, target, rng);
    let mut shots = 1usize;

    if let Some(rapid_fire) = &shooter.rapid_fire {
        if let Some(mult) = rapid_fire.get(&target.unit_type) {
            let n = *mult as usize;
            if n > 1 {
                let p_extra = 1.0 - (1.0 / n as f64);
                for _ in 0..(n - 1) {
                    if target.hull <= 0.0 {
                        break;
                    }
                    if rng.next_f64() < p_extra {
                        shoot(shooter, target, rng);
                        shots += 1;
                    } else {
                        break;
                    }
                }
            }
        }
    }

    shots
}

fn remove_destroyed(units: &mut Vec<CombatUnit>) -> usize {
    let mut destroyed = 0usize;
    let mut i = 0usize;
    while i < units.len() {
        if units[i].hull <= 0.0 {
            units.swap_remove(i);
            destroyed += 1;
        } else {
            i += 1;
        }
    }
    destroyed
}

fn regenerate_shields(units: &mut [CombatUnit]) {
    for unit in units {
        if let Some(max_shield) = unit.max_shield {
            unit.shield = max_shield;
        }
    }
}

fn calculate_losses(initial: &HashMap<String, i32>, remaining: &[CombatUnit]) -> HashMap<String, i32> {
    let mut losses = HashMap::new();
    let mut remaining_counts = HashMap::new();

    for unit in remaining {
        *remaining_counts.entry(unit.unit_type.clone()).or_insert(0) += 1;
    }

    for (typ, init_count) in initial {
        let rem = remaining_counts.get(typ).copied().unwrap_or(0);
        let lost = init_count - rem;
        if lost > 0 {
            losses.insert(typ.clone(), lost);
        }
    }

    losses
}

fn calculate_debris(
    attacker_losses: &HashMap<String, i32>,
    defender_losses: &HashMap<String, i32>,
    ship_defs: &HashMap<String, ships::ShipDef>,
) -> Debris {
    let mut metal = 0i64;
    let mut crystal = 0i64;

    for (typ, count) in attacker_losses.iter().chain(defender_losses.iter()) {
        if let Some(def) = ship_defs.get(typ) {
            let m = def.metal_cost.unwrap_or(0);
            let c = def.crystal_cost.unwrap_or(0);
            metal += *count as i64 * (m as f64 * 0.30).round() as i64;
            crystal += *count as i64 * (c as f64 * 0.15).round() as i64;
        } else {
            metal += *count as i64 * 50;
            crystal += *count as i64 * 25;
        }
    }

    Debris { metal, crystal }
}

fn merge_unit_counts(
    primary: &HashMap<String, i32>,
    secondary: &HashMap<String, i32>,
) -> HashMap<String, i32> {
    let mut merged = primary.clone();
    for (unit_type, count) in secondary {
        *merged.entry(unit_type.clone()).or_insert(0) += *count;
    }
    merged
}

fn tech_level(tech: &HashMap<String, i32>, keys: &[&str]) -> i32 {
    for key in keys {
        if let Some(level) = tech.get(*key) {
            return (*level).max(0);
        }
    }
    0
}

fn tech_multiplier(level: i32) -> f64 {
    1.0 + level as f64 * 0.1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_req(seed: &str) -> CombatInput {
        let mut attacker_ships = HashMap::new();
        attacker_ships.insert("fighter".to_string(), 100);
        attacker_ships.insert("bomber".to_string(), 10);

        let mut defender_ships = HashMap::new();
        defender_ships.insert("defender".to_string(), 50);
        defender_ships.insert("turret".to_string(), 5);

        CombatInput {
            attacker_ships,
            defender_ships,
            defender_defenses: HashMap::new(),
            attacker_tech: HashMap::new(),
            defender_tech: HashMap::new(),
            planet_metal: 10_000,
            planet_crystal: 5_000,
            planet_deuterium: 1_000,
            seed: seed.to_string(),
            universe: "default".to_string(),
            max_rounds: None,
        }
    }

    #[test]
    fn deterministic_same_seed() {
        let r1 = simulate_combat(&make_req("seed1"));
        let r2 = simulate_combat(&make_req("seed1"));
        assert_eq!(r1.winner, r2.winner);
        assert_eq!(r1.rounds.len(), r2.rounds.len());
        assert_eq!(r1.attacker_losses, r2.attacker_losses);
        assert_eq!(r1.defender_losses, r2.defender_losses);
    }

    #[test]
    fn different_seed_differs() {
        let r1 = simulate_combat(&make_req("seed1"));
        let r2 = simulate_combat(&make_req("seed2"));
        if r1.winner == r2.winner {
            assert_ne!(r1.rounds.len(), r2.rounds.len());
        }
    }

    #[test]
    fn rapid_fire_changes_outcome_deterministically() {
        let r1 = simulate_combat(&make_req("rfseed"));
        let r2 = simulate_combat(&make_req("rfseed"));
        assert_eq!(r1.winner, r2.winner);
    }

    #[test]
    fn explicit_max_rounds_limits_total_round_count() {
        let mut req = make_req("max-rounds-seed");
        req.max_rounds = Some(1);
        let r = simulate_combat(&req);
        assert!(
            r.rounds.len() <= 1,
            "explicit max_rounds should cap total rounds"
        );
    }

    #[test]
    fn explicit_non_positive_max_rounds_falls_back_to_default_behavior() {
        let mut default_req = make_req("fallback-seed");
        default_req.max_rounds = None;
        let default_result = simulate_combat(&default_req);

        let mut zero_req = make_req("fallback-seed");
        zero_req.max_rounds = Some(0);
        let zero_result = simulate_combat(&zero_req);

        assert_eq!(default_result.winner, zero_result.winner);
        assert_eq!(default_result.rounds, zero_result.rounds);
        assert_eq!(default_result.attacker_losses, zero_result.attacker_losses);
        assert_eq!(default_result.defender_losses, zero_result.defender_losses);
    }
}
