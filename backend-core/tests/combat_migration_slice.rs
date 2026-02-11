use backend_core::core::SimulateRequest;
use backend_core::sim::simulate_combat;
use std::collections::HashMap;

fn base_request() -> SimulateRequest {
    SimulateRequest {
        battle_id: "migration-slice".to_string(),
        attacker_ships: HashMap::new(),
        defender_ships: HashMap::new(),
        defender_defenses: HashMap::new(),
        attacker_tech: HashMap::new(),
        defender_tech: HashMap::new(),
        planet_metal: 0,
        planet_crystal: 0,
        planet_deuterium: 0,
        seed: "slice-seed".to_string(),
        universe: "default".to_string(),
    }
}

fn total_losses(map: &HashMap<String, i32>) -> i32 {
    map.values().sum()
}

#[test]
fn defender_defenses_are_included_in_combat_and_losses() {
    let mut req = base_request();
    req.seed = "defenses-seed".to_string();
    req.attacker_ships.insert("bomber".to_string(), 300);
    req.defender_defenses.insert("turret".to_string(), 10);

    let result = simulate_combat(&req);
    assert!(
        !result.rounds.is_empty(),
        "defender defenses should create defender units and at least one round"
    );
    assert!(
        result.defender_losses.contains_key("turret"),
        "defender losses should account for defenses"
    );
}

#[test]
fn universe_field_controls_ship_definition_loading() {
    let mut req_default = base_request();
    req_default.seed = "universe-seed".to_string();
    req_default.attacker_ships.insert("fighter".to_string(), 10);
    req_default.planet_metal = 1000;

    let mut req_migration = req_default.clone();
    req_migration.universe = "migration-test".to_string();

    let default_result = simulate_combat(&req_default);
    let migration_result = simulate_combat(&req_migration);

    let default_metal_loot = default_result.loot.unwrap_or_default().metal;
    let migration_metal_loot = migration_result.loot.unwrap_or_default().metal;

    assert!(default_metal_loot < 100);
    assert!(migration_metal_loot >= 400);
}

#[test]
fn tech_multipliers_increase_unit_effectiveness() {
    let mut base = base_request();
    base.attacker_ships.insert("fighter".to_string(), 60);
    base.attacker_ships.insert("bomber".to_string(), 20);
    base.defender_ships.insert("defender".to_string(), 80);
    base.defender_defenses.insert("turret".to_string(), 20);

    let mut weapons_boost = base.clone();
    weapons_boost
        .attacker_tech
        .insert("weapons".to_string(), 12);

    let mut shielding_boost = base.clone();
    shielding_boost
        .defender_tech
        .insert("shielding".to_string(), 12);

    let mut armor_boost = base.clone();
    armor_boost.defender_tech.insert("armor".to_string(), 12);

    let mut weapons_changed = false;
    let mut shielding_changed = false;
    let mut armor_changed = false;

    for n in 0..120 {
        let seed = format!("tech-seed-{n}");
        base.seed = seed.clone();
        weapons_boost.seed = seed.clone();
        shielding_boost.seed = seed.clone();
        armor_boost.seed = seed;

        let baseline = simulate_combat(&base);
        let w = simulate_combat(&weapons_boost);
        let s = simulate_combat(&shielding_boost);
        let a = simulate_combat(&armor_boost);

        if w.winner != baseline.winner
            || total_losses(&w.attacker_losses) != total_losses(&baseline.attacker_losses)
            || total_losses(&w.defender_losses) != total_losses(&baseline.defender_losses)
        {
            weapons_changed = true;
        }
        if s.winner != baseline.winner
            || total_losses(&s.attacker_losses) != total_losses(&baseline.attacker_losses)
            || total_losses(&s.defender_losses) != total_losses(&baseline.defender_losses)
        {
            shielding_changed = true;
        }
        if a.winner != baseline.winner
            || total_losses(&a.attacker_losses) != total_losses(&baseline.attacker_losses)
            || total_losses(&a.defender_losses) != total_losses(&baseline.defender_losses)
        {
            armor_changed = true;
        }

        if weapons_changed && shielding_changed && armor_changed {
            break;
        }
    }

    assert!(weapons_changed, "weapons tech should alter combat outcomes");
    assert!(
        shielding_changed,
        "shielding tech should alter combat outcomes"
    );
    assert!(armor_changed, "armor tech should alter combat outcomes");
}
