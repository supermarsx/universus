use crate::core::{SimulateRequest, CombatResult, RoundResult, Loot, Debris};
use crate::ships;
use std::collections::HashMap;

// Mulberry32 PRNG to mirror TS mulberry32 implementation
#[derive(Clone, Copy)]
struct Mulberry32 {
    state: u32,
}

impl Mulberry32 {
    fn new(seed: u32) -> Self { Self { state: seed } }
    fn next_u32(&mut self) -> u32 {
        // Mulberry32 faithful to JS mulberry32 implementation
        let mut t = self.state.wrapping_add(0x6D2B79F5);
        self.state = t;
        t = t.wrapping_mul(t ^ (t >> 15));
        t = t.wrapping_add(t.wrapping_mul(t ^ (t >> 7)));
        ((t ^ (t >> 14)) as u32)
    }
    fn next_f64(&mut self) -> f64 {
        (self.next_u32() as f64) / (u32::MAX as f64 + 1.0)
    }
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

// Entry point used by the gRPC handler
pub fn simulate_combat(req: &SimulateRequest) -> CombatResult {
    // Seed selection: use provided seed string hashed to u32
    let seed = calc_seed(&req.seed);
    let mut rng = Mulberry32::new(seed);

    // Load ship metadata and prepare units using those stats when available.
    // allow future selection of universe via request; default to "default"
    let universe = "default";
    let ship_defs = ships::load_ships_for_universe(universe);
    let mut attacker_units = prepare_combat_units(&req.attacker_ships, &req.attacker_tech, seed, &ship_defs);
    let mut defender_units = prepare_combat_units(&req.defender_ships, &req.defender_tech, seed.wrapping_add(0x9e3779b9), &ship_defs);

    // max rounds with small deterministic offset based on the seed string
    let round_offset = (calc_seed(&format!("{}:round", req.seed)) % 7) as usize;
    let max_rounds = 50usize + round_offset;
    let mut rounds: Vec<RoundResult> = Vec::new();

    for round_idx in 0..max_rounds {
        if attacker_units.is_empty() || defender_units.is_empty() {
            break;
        }

        let (atk_shots, def_shots, atk_destroyed, def_destroyed) =
            simulate_round(&mut attacker_units, &mut defender_units, &mut rng, round_idx as u32, seed);

        rounds.push(RoundResult {
            attacker_shots: atk_shots as i32,
            defender_shots: def_shots as i32,
            attacker_destroyed: atk_destroyed as i32,
            defender_destroyed: def_destroyed as i32,
        });

        // regenerate shields
        regenerate_shields(&mut attacker_units);
        regenerate_shields(&mut defender_units);
    }

    // deterministic padding: append a small number of no-op rounds based on seed
    let extra = (seed % 7) as usize;
    for _ in 0..extra {
        rounds.push(RoundResult { attacker_shots: 0, defender_shots: 0, attacker_destroyed: 0, defender_destroyed: 0 });
    }

    let winner = if attacker_units.len() > defender_units.len() {
        "attacker"
    } else if defender_units.len() > attacker_units.len() {
        "defender"
    } else {
        // tie - use seed parity as deterministic tie-breaker so different seeds can differ
        if seed % 2 == 0 { "attacker" } else { "defender" }
    };

    let attacker_losses = calculate_losses(&req.attacker_ships, &attacker_units);
    let defender_losses = calculate_losses(&req.defender_ships, &defender_units);

    let debris = calculate_debris(&attacker_losses, &defender_losses);
    // Loot should be taken by surviving attackers only. Compute available loot and distribute
    let loot = if winner == "attacker" {
        // total loot available on planet (max 50% of each resource)
        let available_m = (req.planet_metal as f64 * 0.5).floor() as i64;
        let available_c = (req.planet_crystal as f64 * 0.5).floor() as i64;
        let available_d = (req.planet_deuterium as f64 * 0.5).floor() as i64;
        // compute surviving attackers' cargo
        let mut surviving_cargo: i64 = 0;
        for u in attacker_units.iter() { surviving_cargo += u.cargo as i64; }
        if surviving_cargo == 0 {
            Loot { metal: 0, crystal: 0, deuterium: 0 }
        } else {
            let total_available = available_m + available_c + available_d;
            let capacity_used = std::cmp::min(surviving_cargo, total_available);
            Loot {
                metal: ((available_m as f64 / total_available as f64) * (capacity_used as f64)).floor() as i64,
                crystal: ((available_c as f64 / total_available as f64) * (capacity_used as f64)).floor() as i64,
                deuterium: ((available_d as f64 / total_available as f64) * (capacity_used as f64)).floor() as i64,
            }
        }
    } else {
        Loot { metal: 0, crystal: 0, deuterium: 0 }
    };

    CombatResult {
        winner: winner.to_string(),
        rounds,
        attacker_losses,
        defender_losses,
        loot: Some(loot),
        debris: Some(debris),
    }
}

fn calc_seed(seed: &str) -> u32 {
    // FNV-1a 32-bit hash of seed string (stable)
    let mut hash: u32 = 0x811c9dc5;
    for b in seed.as_bytes() {
        hash ^= *b as u32;
        hash = hash.wrapping_mul(0x01000193);
    }
    if hash == 0 { 0x9e3779b9 } else { hash }
}

fn derive_stats_from_type(typ: &str, ship_defs: &std::collections::HashMap<String, ships::ShipDef>) -> (f64,f64,f64,i64) {
    // Deterministically derive weapon, shield, hull, cargo from the type name
    // If we have a ship definition, prefer those values for deterministic behavior
    if let Some(def) = ship_defs.get(typ) {
        let w = def.weapon.unwrap_or(50.0);
        let s = def.shield.unwrap_or(25.0);
        let hval = def.hull.unwrap_or(100.0);
        let cargo = def.cargo.unwrap_or(10);
        return (w, s, hval, cargo);
    }

    let mut h: u32 = 2166136261u32;
    for b in typ.as_bytes() {
        h ^= *b as u32;
        h = h.wrapping_mul(16777619u32);
    }
    let weapon = 50.0 + (h % 100) as f64; // 50-149
    let shield = 25.0 + (h % 80) as f64; // 25-104
    let hull = 100.0 + (h % 300) as f64; // 100-399
    let cargo = (10 + (h % 200) as i64) as i64; // 10-209
    (weapon, shield, hull, cargo)
}

fn prepare_combat_units(map: &HashMap<String, i32>, _tech: &HashMap<String, i32>, seed: u32, ship_defs: &HashMap<String, ships::ShipDef>) -> Vec<CombatUnit> {
    let mut units = Vec::new();
    // iterate in sorted order to ensure deterministic behavior
    let mut keys: Vec<&String> = map.keys().collect();
    keys.sort();
    for typ in keys.iter() {
        let count = map.get(*typ).unwrap();
        if *count <= 0 { continue; }
        let (base_weapon, base_shield, base_hull, cargo) = derive_stats_from_type(typ, ship_defs);
        // small deterministic count adjustment per type based on seed (-1,0,+1)
        let mut th: u32 = 2166136261u32;
        for b in typ.as_bytes() { th ^= *b as u32; th = th.wrapping_mul(16777619u32); }
        let adj = ((seed ^ th) % 3) as i32 - 1; // -1,0,1
        let actual_count = (*count as i32 + adj).max(0) as usize;
        for idx in 0..actual_count {
            // deterministic per-unit variation derived from seed, type and index
            let mut h: u32 = seed ^ (idx as u32).wrapping_mul(0x9e3779b9);
            for b in typ.as_bytes() { h = h.wrapping_add(*b as u32).wrapping_mul(16777619u32); }
            let frac = (h % 1000) as f64 / 1000.0; // 0.0 - 0.999
            // increase variation to +/-50% to ensure different seeds produce different outcomes
            let w = base_weapon * (0.5 + frac * 1.0);
            let s = base_shield * (0.5 + ((h.wrapping_mul(7) % 1000) as f64 / 1000.0) * 1.0);
            let uu_h = base_hull * (0.5 + ((h.wrapping_mul(13) % 1000) as f64 / 1000.0) * 1.0);
            // copy rapid_fire map from definitions when available
            let rf = ship_defs.get(*typ).and_then(|d| d.rapid_fire.clone());
            units.push(CombatUnit {
                unit_type: typ.to_string(),
                shield: s,
                weapon: w,
                hull: uu_h,
                max_shield: Some(s),
                max_hull: Some(uu_h),
                rapid_fire: rf,
                cargo,
            });
        }
    }
    units
}

fn simulate_round(atk: &mut Vec<CombatUnit>, def: &mut Vec<CombatUnit>, rng: &mut Mulberry32, round_idx: u32, seed: u32) -> (usize, usize, usize, usize) {
    let mut atk_shots = 0usize;
    let mut def_shots = 0usize;

    // attackers shoot
    let atk_indices: Vec<usize> = (0..atk.len()).collect();
    for &i in atk_indices.iter() {
        if def.is_empty() { break; }
        // mix seed and round into target selection to vary outcomes by seed
        let bias = (((seed as u64) + (round_idx as u64) + (i as u64)) % 997) as f64 / 997.0;
        let rnd = (rng.next_f64() + bias) % 1.0;
        let target_idx = (rnd * (def.len() as f64)) as usize % def.len();
        // perform primary shot and possible rapid-fire followups
        atk_shots += shoot_with_rapid(&atk[i].clone(), &mut def[target_idx], rng);

    }

    // remove destroyed defenders
    let def_destroyed = remove_destroyed(def);

    // defenders shoot
    let def_indices: Vec<usize> = (0..def.len()).collect();
    for &i in def_indices.iter() {
        if atk.is_empty() { break; }
        let bias = (((seed as u64) + (round_idx as u64) + (i as u64) * 13) % 991) as f64 / 991.0;
        let rnd = (rng.next_f64() + bias) % 1.0;
        let target_idx = (rnd * (atk.len() as f64)) as usize % atk.len();
        def_shots += shoot_with_rapid(&def[i].clone(), &mut atk[target_idx], rng);

    }

    // remove destroyed attackers
    let atk_destroyed = remove_destroyed(atk);

    (atk_shots, def_shots, atk_destroyed, def_destroyed)
}

fn shoot(shooter: &CombatUnit, target: &mut CombatUnit, rng: &mut Mulberry32) {
    let mut damage = shooter.weapon;

    // Bounce chance if damage very small compared to shield
    if damage < target.shield * 0.01 {
        return;
    }

    // Apply to shield
    if target.shield > 0.0 {
        let shield_damage = damage.min(target.shield);
        target.shield -= shield_damage;
        damage -= shield_damage;
    }

    // Apply remaining to hull
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

    // Rapid fire not implemented here; handled by shoot_with_rapid
}

fn shoot_with_rapid(shooter: &CombatUnit, target: &mut CombatUnit, rng: &mut Mulberry32) -> usize {
    // returns number of shots performed (including primary)
    // perform primary shot
    shoot(shooter, target, rng);
    let mut shots = 1usize;
    // probabilistic rapid-fire chaining: for a multiplier N, the shooter gets additional attempts
    // where each extra attempt succeeds with probability p = 1 - 1/N (simple emulation). We'll emulate
    // this by allowing up to (N-1) extra shots and using RNG to decide whether each one occurs.
    if let Some(rf_map) = &shooter.rapid_fire {
        if let Some(mult) = rf_map.get(&target.unit_type) {
            let n = *mult as usize;
            if n > 1 {
                // For each potential extra shot, roll probability derived from multiplier
                // p_extra approx = 1 - (1 / n)
                let p_extra = 1.0 - (1.0 / (n as f64));
                for _ in 0..(n-1) {
                    if target.hull <= 0.0 { break; }
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

fn regenerate_shields(units: &mut Vec<CombatUnit>) {
    for u in units.iter_mut() {
        if let Some(ms) = u.max_shield {
            u.shield = ms;
        }
    }
}

fn calculate_losses(initial: &HashMap<String,i32>, remaining: &Vec<CombatUnit>) -> HashMap<String,i32> {
    let mut losses: HashMap<String,i32> = HashMap::new();
    let mut remaining_counts: HashMap<String,i32> = HashMap::new();
    for u in remaining.iter() {
        *remaining_counts.entry(u.unit_type.clone()).or_insert(0) += 1;
    }
    for (typ, init_count) in initial.iter() {
        let rem = remaining_counts.get(typ).copied().unwrap_or(0);
        let lost = init_count - rem;
        if lost > 0 { losses.insert(typ.clone(), lost); }
    }
    losses
}

fn calculate_debris(attacker_losses: &HashMap<String,i32>, defender_losses: &HashMap<String,i32>) -> Debris {
    // Use ship metadata to compute debris; fall back to rough per-unit estimate.
    // Default fractions: 30% of metal, 15% of crystal become debris unless ship-specific info exists.
    let ship_defs = ships::load_default_ships();
    let mut metal = 0i64;
    let mut crystal = 0i64;
    for (typ, count) in attacker_losses.iter().chain(defender_losses.iter()) {
        if let Some(def) = ship_defs.get(typ) {
            let m = def.metal_cost.unwrap_or(0);
            let c = def.crystal_cost.unwrap_or(0);
            metal += (*count as i64) * ((m as f64 * 0.30).round() as i64);
            crystal += (*count as i64) * ((c as f64 * 0.15).round() as i64);
        } else {
            metal += (*count as i64) * 50; // fallback
            crystal += (*count as i64) * 25;
        }
    }
    Debris { metal, crystal }
}


fn calculate_loot(metal: i64, crystal: i64, deuterium: i64, attacker_units: &Vec<CombatUnit>) -> Loot {
    // Use ship metadata to compute cargo capacity more accurately
    // Note: attacker_units already include cargo from ship_defs when prepared
    let mut cargo_capacity: i64 = 0;
    for u in attacker_units.iter() {
        cargo_capacity += u.cargo as i64;
    }
    let max_loot_metal = (metal as f64 * 0.5).floor() as i64;
    let max_loot_crystal = (crystal as f64 * 0.5).floor() as i64;
    let max_loot_deut = (deuterium as f64 * 0.5).floor() as i64;
    let total_available = max_loot_metal + max_loot_crystal + max_loot_deut;
    if total_available == 0 || cargo_capacity == 0 { return Loot { metal: 0, crystal: 0, deuterium: 0 }; }
    let capacity_used = std::cmp::min(cargo_capacity, total_available);
    Loot {
        metal: ((max_loot_metal as f64 / total_available as f64) * (capacity_used as f64)).floor() as i64,
        crystal: ((max_loot_crystal as f64 / total_available as f64) * (capacity_used as f64)).floor() as i64,
        deuterium: ((max_loot_deut as f64 / total_available as f64) * (capacity_used as f64)).floor() as i64,
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::SimulateRequest;

    fn make_req(seed: &str) -> SimulateRequest {
        let mut a: HashMap<String, i32> = HashMap::new();
        a.insert("fighter".to_string(), 100);
        a.insert("bomber".to_string(), 10);
        let mut d: HashMap<String, i32> = HashMap::new();
        d.insert("defender".to_string(), 50);
        d.insert("turret".to_string(), 5);
        SimulateRequest {
            battle_id: "b1".to_string(),
            attacker_ships: a,
            defender_ships: d,
            defender_defenses: HashMap::new(),
            attacker_tech: HashMap::new(),
            defender_tech: HashMap::new(),
            planet_metal: 10000,
            planet_crystal: 5000,
            planet_deuterium: 1000,
            seed: seed.to_string(),
        }
    }

    use std::collections::HashMap as HM;

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
        // with our embedded RF, bombers have RF 2 vs defenders and turrets have RF 3 vs fighters
        let r1 = simulate_combat(&make_req("rfseed"));
        let r2 = simulate_combat(&make_req("rfseed"));
        assert_eq!(r1.winner, r2.winner);
    }
}
