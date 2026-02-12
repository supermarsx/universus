use crate::compute_espionage_outcome;
use serde_json::Value;

#[test]
fn compute_espionage_outcome_supports_snake_case_fields() {
    let payload = r#"{
        "probes": 3,
        "attacker_espionage": 2,
        "defender_espionage": 2,
        "seed": "seed-a"
    }"#;

    let raw = compute_espionage_outcome(payload.to_string()).expect("expected valid response");
    let out: Value = serde_json::from_str(&raw).expect("response should be valid json");

    assert_eq!(out["intel_level"], "standard");
    assert!((out["detail_score"].as_f64().unwrap() - 4.0).abs() < 1e-12);
    assert!((out["defense_score"].as_f64().unwrap() - 2.0).abs() < 1e-12);
    assert!((out["detection_chance"].as_f64().unwrap() - 0.4).abs() < 1e-12);
    assert!(out["detected"].is_boolean());
}

#[test]
fn compute_espionage_outcome_supports_camel_case_fields() {
    let payload = r#"{
        "probes": 3,
        "attackerEspionage": 1,
        "defenderEspionage": 5,
        "seed": "seed-b"
    }"#;

    let raw = compute_espionage_outcome(payload.to_string()).expect("expected valid response");
    let out: Value = serde_json::from_str(&raw).expect("response should be valid json");

    assert_eq!(out["intel_level"], "minimal");
    assert!((out["detail_score"].as_f64().unwrap() - 3.0).abs() < 1e-12);
    assert!((out["defense_score"].as_f64().unwrap() - 5.0).abs() < 1e-12);
    assert!((out["detection_chance"].as_f64().unwrap() - 0.6).abs() < 1e-12);
}

#[test]
fn compute_espionage_outcome_uses_default_seed_deterministically() {
    let payload = r#"{
        "probes": 15,
        "attacker_espionage": 2,
        "defender_espionage": 3
    }"#;

    let raw_a = compute_espionage_outcome(payload.to_string()).expect("expected valid response");
    let raw_b = compute_espionage_outcome(payload.to_string()).expect("expected valid response");
    let out_a: Value = serde_json::from_str(&raw_a).expect("response should be valid json");
    let out_b: Value = serde_json::from_str(&raw_b).expect("response should be valid json");

    assert_eq!(out_a["intel_level"], "full");
    assert_eq!(out_a, out_b);
}
