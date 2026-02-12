use crate::calculate_fleet_movement_by_type;
use serde_json::{json, Value};

#[test]
fn calculate_fleet_movement_by_type_supports_snake_case_aliases_and_shape() {
    let payload = json!({
        "origin_coords": {
            "galaxy": 1,
            "system": 1,
            "position": 1
        },
        "target_coords": {
            "galaxy": 1,
            "system": 2,
            "position": 5
        },
        "ship_counts": {
            "fighter": 2,
            "bomber": 1
        },
        "universe_name": "default"
    });

    let raw = calculate_fleet_movement_by_type(payload.to_string()).expect("expected valid response");
    let out: Value = serde_json::from_str(&raw).expect("response should be valid json");

    assert!(out["distance"].is_number());
    assert!(out["fleetSpeed"].is_number());
    assert!(out["travelTimeSeconds"].is_number());
    assert!(out["fuelNeeded"].is_number());
    assert!(out["cargoCapacity"].is_number());
}

#[test]
fn calculate_fleet_movement_by_type_is_deterministic_and_supports_camel_case_aliases() {
    let payload = json!({
        "originCoords": {
            "galaxyId": 1,
            "systemId": 1,
            "position": 1
        },
        "targetCoords": {
            "galaxyId": 1,
            "systemId": 2,
            "position": 5
        },
        "shipCounts": {
            "fighter": 2,
            "bomber": 1
        },
        "universeName": "default"
    });

    let raw_a = calculate_fleet_movement_by_type(payload.to_string()).expect("expected valid response");
    let raw_b = calculate_fleet_movement_by_type(payload.to_string()).expect("expected valid response");
    let out_a: Value = serde_json::from_str(&raw_a).expect("response should be valid json");
    let out_b: Value = serde_json::from_str(&raw_b).expect("response should be valid json");

    assert_eq!(out_a, out_b);
    assert_eq!(out_a["distance"].as_i64().unwrap(), 2795);
    assert!((out_a["fleetSpeed"].as_f64().unwrap() - 100.0).abs() < 1e-12);
    assert_eq!(out_a["travelTimeSeconds"].as_i64().unwrap(), 100620);
    assert!((out_a["fuelNeeded"].as_f64().unwrap() - 0.0).abs() < 1e-12);
    assert!((out_a["cargoCapacity"].as_f64().unwrap() - 30.0).abs() < 1e-12);
}
