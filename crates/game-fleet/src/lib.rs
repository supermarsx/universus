#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

pub mod ships {
    use serde::Deserialize;
    use std::collections::HashMap;

    #[derive(Debug, Deserialize, Clone)]
    pub struct ShipDef {
        pub name: String,
        pub weapon: Option<f64>,
        pub shield: Option<f64>,
        pub hull: Option<f64>,
        pub cargo: Option<i64>,
        pub metal_cost: Option<i64>,
        pub crystal_cost: Option<i64>,
        pub deuterium_cost: Option<i64>,
        pub rapid_fire: Option<HashMap<String, i32>>,
    }

    pub fn load_ships_for_universe(universe: &str) -> HashMap<String, ShipDef> {
        let assets_path = format!(
            "{}/assets/{}/ships.json",
            env!("CARGO_MANIFEST_DIR"),
            universe
        );
        if let Ok(s) = std::fs::read_to_string(&assets_path) {
            if let Ok(m) = serde_json::from_str(&s) {
                return m;
            }
        }

        let json = r#"
        {
            "fighter": { "name": "fighter", "weapon": 100.0, "shield": 50.0, "hull": 200.0, "cargo": 5, "metal_cost": 300, "crystal_cost": 100, "deuterium_cost": 0 },
            "bomber": { "name": "bomber", "weapon": 400.0, "shield": 150.0, "hull": 600.0, "cargo": 20, "metal_cost": 1200, "crystal_cost": 800, "deuterium_cost": 0, "rapid_fire": {"defender": 2} },
            "defender": { "name": "defender", "weapon": 150.0, "shield": 80.0, "hull": 400.0, "cargo": 0, "metal_cost": 800, "crystal_cost": 300, "deuterium_cost": 0 },
            "turret": { "name": "turret", "weapon": 300.0, "shield": 200.0, "hull": 900.0, "cargo": 0, "metal_cost": 2000, "crystal_cost": 1200, "deuterium_cost": 0, "rapid_fire": {"fighter": 3} }
        }
        "#;
        serde_json::from_str(json).expect("ships json parse")
    }

    pub fn load_default_ships() -> HashMap<String, ShipDef> {
        load_ships_for_universe("default")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FleetShipInput {
    pub count: i32,
    pub base_speed: f64,
    pub fuel_consumption: f64,
    pub cargo: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FleetMovementInput {
    pub origin_galaxy: i32,
    pub origin_system: i32,
    pub origin_position: i32,
    pub target_galaxy: i32,
    pub target_system: i32,
    pub target_position: i32,
    pub ships: Vec<FleetShipInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FleetMovementResult {
    pub distance: i32,
    pub fleet_speed: f64,
    pub travel_time_seconds: i32,
    pub fuel_needed: f64,
    pub cargo_capacity: f64,
}

pub fn calculate_distance(
    origin_galaxy: i32,
    origin_system: i32,
    origin_position: i32,
    target_galaxy: i32,
    target_system: i32,
    target_position: i32,
) -> i32 {
    if origin_galaxy != target_galaxy {
        (origin_galaxy - target_galaxy).abs() * 20000
    } else if origin_system != target_system {
        (origin_system - target_system).abs() * 5 * 19 + 2700
    } else {
        (origin_position - target_position).abs() * 5 + 1000
    }
}

pub fn calculate_movement(input: &FleetMovementInput) -> FleetMovementResult {
    let distance = calculate_distance(
        input.origin_galaxy,
        input.origin_system,
        input.origin_position,
        input.target_galaxy,
        input.target_system,
        input.target_position,
    );

    let mut min_speed = f64::INFINITY;
    let mut fuel_needed = 0.0f64;
    let mut cargo_capacity = 0.0f64;

    for ship in &input.ships {
        if ship.count <= 0 {
            continue;
        }
        if ship.base_speed > 0.0 {
            min_speed = min_speed.min(ship.base_speed);
        }
        let count = ship.count as f64;
        fuel_needed += ship.fuel_consumption * count * (distance as f64 / 100.0);
        cargo_capacity += ship.cargo * count;
    }

    let fleet_speed = if min_speed.is_finite() {
        min_speed
    } else {
        0.0
    };
    let travel_time_seconds = if fleet_speed > 0.0 {
        ((distance as f64 / fleet_speed) * 3600.0).ceil() as i32
    } else {
        0
    };

    cargo_capacity -= fuel_needed;

    FleetMovementResult {
        distance,
        fleet_speed,
        travel_time_seconds,
        fuel_needed,
        cargo_capacity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_uses_expected_tiers() {
        assert_eq!(calculate_distance(1, 1, 1, 2, 1, 1), 20000);
        assert_eq!(calculate_distance(1, 1, 1, 1, 2, 1), 2795);
        assert_eq!(calculate_distance(1, 1, 1, 1, 1, 2), 1005);
    }

    #[test]
    fn movement_sanity_matches_backend_formula() {
        let input = FleetMovementInput {
            origin_galaxy: 1,
            origin_system: 1,
            origin_position: 1,
            target_galaxy: 1,
            target_system: 2,
            target_position: 1,
            ships: vec![
                FleetShipInput {
                    count: 10,
                    base_speed: 1000.0,
                    fuel_consumption: 2.0,
                    cargo: 50.0,
                },
                FleetShipInput {
                    count: 1,
                    base_speed: 500.0,
                    fuel_consumption: 5.0,
                    cargo: 100.0,
                },
            ],
        };

        let result = calculate_movement(&input);

        assert_eq!(result.distance, 2795);
        assert_eq!(result.fleet_speed, 500.0);
        assert_eq!(result.travel_time_seconds, 20124);
        assert!((result.fuel_needed - 698.75).abs() < 1e-9);
        assert!((result.cargo_capacity - (-98.75)).abs() < 1e-9);
    }
}
