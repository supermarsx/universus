use crate::simulate_battle;
use serde_json::{json, Value};

fn base_payload() -> Value {
    json!({
        "battle_id": "napi-max-rounds",
        "attacker_ships": { "fighter": 40 },
        "defender_ships": { "defender": 30 },
        "defender_defenses": {},
        "attacker_tech": {},
        "defender_tech": {},
        "planet_metal": 1000,
        "planet_crystal": 500,
        "planet_deuterium": 100,
        "seed": "napi-seed",
        "universe": "default"
    })
}

#[test]
fn simulate_battle_accepts_and_honors_max_rounds() {
    let mut payload = base_payload();
    payload["max_rounds"] = json!(1);

    let raw = simulate_battle(payload.to_string()).expect("expected valid response");
    let out: Value = serde_json::from_str(&raw).expect("response should be valid json");

    assert!(out["rounds"].is_array());
    assert!(out["rounds"].as_array().unwrap().len() <= 1);
}

#[test]
fn simulate_battle_ignores_non_positive_max_rounds() {
    let mut payload = base_payload();
    payload["max_rounds"] = json!(0);

    let raw = simulate_battle(payload.to_string()).expect("expected valid response");
    let out: Value = serde_json::from_str(&raw).expect("response should be valid json");

    assert!(out["winner"].is_string());
    assert!(out["rounds"].is_array());
    assert!(!out["rounds"].as_array().unwrap().is_empty());
}
