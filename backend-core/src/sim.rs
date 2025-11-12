use crate::core::{SimulateRequest, CombatResult, RoundResult, Loot, Debris};

// Simple deterministic PRNG: xorshift32
#[derive(Clone, Copy)]
struct XorShift32 {
    state: u32,
}

impl XorShift32 {
    fn new(seed: u32) -> Self { Self { state: if seed == 0 { 0xdead_beef } else { seed } } }
    fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }
    fn next_f64(&mut self) -> f64 {
        (self.next_u32() as f64) / (u32::MAX as f64)
    }
}

pub fn simulate_combat(req: &SimulateRequest) -> CombatResult {
    // Build a simple deterministic simulation: attackers and defenders fire, random chance to destroy
    let mut rng = XorShift32::new(calc_seed(&req.seed));

    let mut attacker_losses: std::collections::HashMap<String, i32> = std::collections::HashMap::new();
    let mut defender_losses: std::collections::HashMap<String, i32> = std::collections::HashMap::new();
    let mut rounds: Vec<RoundResult> = Vec::new();

    // Sum total ships
    let mut atk_total: i32 = req.attacker_ships.values().sum();
    let mut def_total: i32 = req.defender_ships.values().sum();

    let mut round = 0;
    while round < 50 && atk_total > 0 && def_total > 0 {
        round += 1;
        // simple firing: shots proportional to count
        let atk_shots = atk_total;
        let def_shots = def_total;

        // each shot has a small chance to destroy an enemy ship
        let mut atk_destroyed = 0;
        let mut def_destroyed = 0;

        for _ in 0..atk_shots {
            if rng.next_f64() < 0.03 { // 3% chance
                def_destroyed += 1;
            }
        }
        for _ in 0..def_shots {
            if rng.next_f64() < 0.025 { // 2.5% chance
                atk_destroyed += 1;
            }
        }

        // clamp
        if def_destroyed > def_total { def_destroyed = def_total; }
        if atk_destroyed > atk_total { atk_destroyed = atk_total; }

        // subtract from totals
        def_total -= def_destroyed;
        atk_total -= atk_destroyed;

        // record distributed losses simply by removing from ship maps proportionally
        distribute_losses(&req.attacker_ships, &mut attacker_losses, atk_destroyed);
        distribute_losses(&req.defender_ships, &mut defender_losses, def_destroyed);

        rounds.push(RoundResult {
            attacker_shots: atk_shots,
            defender_shots: def_shots,
            attacker_destroyed: atk_destroyed,
            defender_destroyed: def_destroyed,
        });

        if atk_total <= 0 || def_total <= 0 { break; }
    }

    let winner = if atk_total > def_total { "attacker" } else if def_total > atk_total { "defender" } else { "draw" };

    // simplistic loot: attacker captures small percent of defender resources
    let loot = Loot { metal: (req.planet_metal as f64 * 0.1) as i64, crystal: (req.planet_crystal as f64 * 0.1) as i64, deuterium: (req.planet_deuterium as f64 * 0.05) as i64 };
    let debris = Debris { metal: (req.planet_metal as f64 * 0.01) as i64, crystal: (req.planet_crystal as f64 * 0.01) as i64 };

    CombatResult {
        winner: winner.to_string(),
        rounds,
        attacker_losses: attacker_losses.into_iter().collect(),
        defender_losses: defender_losses.into_iter().collect(),
        loot: Some(loot),
        debris: Some(debris),
    }
}

fn calc_seed(seed: &str) -> u32 {
    // simple FNV-1a 32-bit
    let mut hash: u32 = 0x811c9dc5;
    for b in seed.as_bytes() {
        hash ^= *b as u32;
        hash = hash.wrapping_mul(0x01000193);
    }
    hash
}

fn distribute_losses(original: &std::collections::HashMap<String, i32>, losses: &mut std::collections::HashMap<String, i32>, mut n: i32) {
    if n <= 0 { return; }
    // iterate over entries and remove proportionally
    let mut items: Vec<(&String, &i32)> = original.iter().collect();
    items.sort_by_key(|(k, _)| k.clone());
    let total: i32 = original.values().sum();
    if total <= 0 { return; }

    for (k, v) in items {
        if n <= 0 { break; }
        let take = (( *v as f64 / total as f64) * (n as f64)).round() as i32;
        let t = take.min(n).min(*v);
        if t > 0 {
            *losses.entry(k.clone()).or_insert(0) += t;
            n -= t;
        }
    }
    // if remaining, assign to first key
    if n > 0 {
        if let Some((k, _)) = items.first() {
            *losses.entry((*k).clone()).or_insert(0) += n;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::SimulateRequest;
    use std::collections::HashMap;

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
        // most likely differ; assert at least winner differs or round count differs
        if r1.winner == r2.winner {
            assert_ne!(r1.rounds.len(), r2.rounds.len());
        }
    }
}
