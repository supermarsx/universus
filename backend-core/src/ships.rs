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
    pub rapid_fire: Option<HashMap<String,i32>>,
}

/// Load built-in ship definitions for a named universe. This function returns a map keyed by ship type.
pub fn load_ships_for_universe(universe: &str) -> HashMap<String, ShipDef> {
    // For now we only support the "default" universe and keep the JSON embedded.
    // In future this can read `assets/<universe>/ships.json` or similar.
    let json = match universe {
        "default" | "" => r#"
        {
            "fighter": { "name": "fighter", "weapon": 100.0, "shield": 50.0, "hull": 200.0, "cargo": 5, "metal_cost": 300, "crystal_cost": 100, "deuterium_cost": 0 },
            "bomber": { "name": "bomber", "weapon": 400.0, "shield": 150.0, "hull": 600.0, "cargo": 20, "metal_cost": 1200, "crystal_cost": 800, "deuterium_cost": 0, "rapid_fire": {"defender": 2} },
            "defender": { "name": "defender", "weapon": 150.0, "shield": 80.0, "hull": 400.0, "cargo": 0, "metal_cost": 800, "crystal_cost": 300, "deuterium_cost": 0 },
            "turret": { "name": "turret", "weapon": 300.0, "shield": 200.0, "hull": 900.0, "cargo": 0, "metal_cost": 2000, "crystal_cost": 1200, "deuterium_cost": 0, "rapid_fire": {"fighter": 3} }
        }
        "#,
        _ => {
            // unknown universe: fall back to default
            r#"
            {
                "fighter": { "name": "fighter", "weapon": 100.0, "shield": 50.0, "hull": 200.0, "cargo": 5, "metal_cost": 300, "crystal_cost": 100, "deuterium_cost": 0 }
            }
            "#
        }
    };
    serde_json::from_str(json).expect("ships json parse")
}

/// Convenience loader for the default universe
pub fn load_default_ships() -> HashMap<String, ShipDef> { load_ships_for_universe("default") }


#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn load_ships() {
        let m = load_default_ships();
        assert!(m.contains_key("fighter"));
        assert_eq!(m.get("fighter").unwrap().cargo.unwrap(), 5);
    }
}
