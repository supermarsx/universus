use crate::core::{
    CombatResult as ProtoCombatResult, Debris as ProtoDebris, Loot as ProtoLoot,
    RoundResult as ProtoRoundResult, SimulateRequest,
};
use game_combat::{CombatInput, CombatResult};

pub fn simulate_combat(req: &SimulateRequest) -> ProtoCombatResult {
    let input = adapt_request(req);
    let result = game_combat::simulate_combat(&input);
    let mut proto = adapt_result(result);
    apply_backend_core_universe_loot(req, &mut proto);
    proto
}

fn adapt_request(req: &SimulateRequest) -> CombatInput {
    CombatInput {
        attacker_ships: req.attacker_ships.clone(),
        defender_ships: req.defender_ships.clone(),
        defender_defenses: req.defender_defenses.clone(),
        attacker_tech: req.attacker_tech.clone(),
        defender_tech: req.defender_tech.clone(),
        planet_metal: req.planet_metal,
        planet_crystal: req.planet_crystal,
        planet_deuterium: req.planet_deuterium,
        seed: req.seed.clone(),
        universe: req.universe.clone(),
        max_rounds: req.max_rounds,
    }
}

fn adapt_result(result: CombatResult) -> ProtoCombatResult {
    ProtoCombatResult {
        winner: result.winner,
        rounds: result
            .rounds
            .into_iter()
            .map(|round| ProtoRoundResult {
                attacker_shots: round.attacker_shots,
                defender_shots: round.defender_shots,
                attacker_destroyed: round.attacker_destroyed,
                defender_destroyed: round.defender_destroyed,
            })
            .collect(),
        attacker_losses: result.attacker_losses,
        defender_losses: result.defender_losses,
        loot: Some(ProtoLoot {
            metal: result.loot.metal,
            crystal: result.loot.crystal,
            deuterium: result.loot.deuterium,
        }),
        debris: Some(ProtoDebris {
            metal: result.debris.metal,
            crystal: result.debris.crystal,
        }),
    }
}

fn apply_backend_core_universe_loot(req: &SimulateRequest, out: &mut ProtoCombatResult) {
    if out.winner != "attacker" {
        return;
    }
    let ship_defs = load_backend_core_ship_defs(&req.universe);
    if ship_defs.is_empty() {
        return;
    }

    let mut surviving_cargo = 0i64;
    for (ship_type, initial_count) in &req.attacker_ships {
        let lost = out.attacker_losses.get(ship_type).copied().unwrap_or(0);
        let surviving = (initial_count - lost).max(0) as i64;
        let cargo = ship_defs
            .get(ship_type)
            .and_then(|def| def.cargo)
            .unwrap_or(0);
        surviving_cargo += surviving * cargo;
    }

    let loot = if surviving_cargo <= 0 {
        ProtoLoot {
            metal: 0,
            crystal: 0,
            deuterium: 0,
        }
    } else {
        let available_m = (req.planet_metal as f64 * 0.5).floor() as i64;
        let available_c = (req.planet_crystal as f64 * 0.5).floor() as i64;
        let available_d = (req.planet_deuterium as f64 * 0.5).floor() as i64;
        let total_available = available_m + available_c + available_d;
        if total_available <= 0 {
            ProtoLoot {
                metal: 0,
                crystal: 0,
                deuterium: 0,
            }
        } else {
            let capacity_used = surviving_cargo.min(total_available);
            ProtoLoot {
                metal: ((available_m as f64 / total_available as f64) * capacity_used as f64)
                    .floor() as i64,
                crystal: ((available_c as f64 / total_available as f64) * capacity_used as f64)
                    .floor() as i64,
                deuterium: ((available_d as f64 / total_available as f64) * capacity_used as f64)
                    .floor() as i64,
            }
        }
    };

    out.loot = Some(loot);
}

fn load_backend_core_ship_defs(
    universe: &str,
) -> std::collections::HashMap<String, game_fleet::ships::ShipDef> {
    if universe.trim().is_empty() {
        return std::collections::HashMap::new();
    }

    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join(universe)
        .join("ships.json");
    let Ok(contents) = std::fs::read_to_string(path) else {
        return std::collections::HashMap::new();
    };
    serde_json::from_str(&contents).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::SimulateRequest;
    use std::collections::HashMap;

    fn request_fixture() -> SimulateRequest {
        SimulateRequest {
            battle_id: "battle-1".to_string(),
            attacker_ships: HashMap::from([("fighter".to_string(), 10)]),
            defender_ships: HashMap::from([("defender".to_string(), 5)]),
            defender_defenses: HashMap::from([("turret".to_string(), 2)]),
            attacker_tech: HashMap::from([("weapons".to_string(), 5)]),
            defender_tech: HashMap::from([("shielding".to_string(), 4)]),
            planet_metal: 1000,
            planet_crystal: 500,
            planet_deuterium: 100,
            seed: "seed-x".to_string(),
            universe: "default".to_string(),
            max_rounds: Some(6),
        }
    }

    #[test]
    fn adapt_request_copies_all_supported_fields() {
        let req = request_fixture();
        let input = adapt_request(&req);

        assert_eq!(input.attacker_ships, req.attacker_ships);
        assert_eq!(input.defender_ships, req.defender_ships);
        assert_eq!(input.defender_defenses, req.defender_defenses);
        assert_eq!(input.attacker_tech, req.attacker_tech);
        assert_eq!(input.defender_tech, req.defender_tech);
        assert_eq!(input.planet_metal, req.planet_metal);
        assert_eq!(input.planet_crystal, req.planet_crystal);
        assert_eq!(input.planet_deuterium, req.planet_deuterium);
        assert_eq!(input.seed, req.seed);
        assert_eq!(input.universe, req.universe);
        assert_eq!(input.max_rounds, req.max_rounds);
    }

    #[test]
    fn adapt_result_maps_domain_result_to_protobuf_result() {
        let result = CombatResult {
            winner: "attacker".to_string(),
            rounds: vec![game_combat::RoundResult {
                attacker_shots: 3,
                defender_shots: 2,
                attacker_destroyed: 1,
                defender_destroyed: 4,
            }],
            attacker_losses: HashMap::from([("fighter".to_string(), 1)]),
            defender_losses: HashMap::from([("defender".to_string(), 4)]),
            loot: game_combat::Loot {
                metal: 120,
                crystal: 80,
                deuterium: 20,
            },
            debris: game_combat::Debris {
                metal: 300,
                crystal: 150,
            },
        };

        let proto = adapt_result(result);

        assert_eq!(proto.winner, "attacker");
        assert_eq!(proto.rounds.len(), 1);
        assert_eq!(proto.rounds[0].attacker_shots, 3);
        assert_eq!(proto.attacker_losses.get("fighter"), Some(&1));
        assert_eq!(proto.defender_losses.get("defender"), Some(&4));
        assert_eq!(proto.loot.as_ref().map(|l| l.metal), Some(120));
        assert_eq!(proto.debris.as_ref().map(|d| d.crystal), Some(150));
    }
}
