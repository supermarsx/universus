/// Ship definitions and helpers used by the simulation core.
///
/// This module exposes a small `ShipDef` model and convenience loaders that
/// read ship definitions for a named "universe". The loader first attempts
/// to read `assets/<universe>/ships.json` relative to the crate manifest
/// directory. If the file is missing or invalid the function falls back to a
/// built-in embedded JSON payload suitable for development and tests.
///
/// Notes on behaviour and performance:
/// - Currently the loader returns an owned `HashMap<String, ShipDef>` on
///   each call. Callers are expected to cache the result if repeated access
///   is required for performance-critical code (the gRPC server uses
///   prewarming to avoid repeated loads).
/// - Fields in `ShipDef` are optional so a missing value can be interpreted
///   by higher-level logic (e.g. defaulting weapon/shield/hull values).
///
/// Example
/// ```no_run
/// use backend_core::ships;
/// let ships = ships::load_ships_for_universe("default");
/// if let Some(fighter) = ships.get("fighter") {
///     println!("fighter cargo = {:?}", fighter.cargo);
/// }
/// ```
use serde::Deserialize;
use std::collections::HashMap;

/// Description of a single ship type used by the simulator.
///
/// All numeric fields are optional (wrapped in `Option`) to allow the JSON
/// source to omit values; callers should handle `None` appropriately.
#[derive(Debug, Deserialize, Clone)]
pub struct ShipDef {
    /// Canonical ship type key (also used as the map key returned by the
    /// loader).
    pub name: String,

    /// Offensive power. `None` indicates the value is unspecified.
    pub weapon: Option<f64>,

    /// Shield strength.
    pub shield: Option<f64>,

    /// Hull / hitpoints.
    pub hull: Option<f64>,

    /// Cargo capacity.
    pub cargo: Option<i64>,

    /// Metal cost to build this ship.
    pub metal_cost: Option<i64>,

    /// Crystal cost to build this ship.
    pub crystal_cost: Option<i64>,

    /// Deuterium cost to build this ship.
    pub deuterium_cost: Option<i64>,

    /// Rapid-fire relationships mapping target ship key -> shots per volley.
    pub rapid_fire: Option<HashMap<String, i32>>,
}

/// Load ship definitions for a named universe.
///
/// The function attempts to read `assets/<universe>/ships.json` relative to
/// the crate manifest (`CARGO_MANIFEST_DIR`). If the file is present and
/// valid JSON it will be parsed into `HashMap<String, ShipDef>` and
/// returned. If reading or parsing the file fails the function falls back to
/// an embedded JSON payload providing a minimal set of ship definitions for
/// development and tests.
///
/// # Parameters
/// - `universe`: the universe identifier used to locate an assets file. Use
///   `"default"` for the default embedded definitions.
///
/// # Returns
/// An owned `HashMap<String, ShipDef>` keyed by ship type name.
///
/// # Panics
/// The embedded fallback contains a hard-coded JSON string which is parsed
/// with `expect("ships json parse")`; if this parse fails the function
/// will panic. This should not happen unless the embedded JSON is
/// accidentally corrupted during development.
pub fn load_ships_for_universe(universe: &str) -> HashMap<String, ShipDef> {
    // For now we only support the "default" universe and keep the JSON embedded.
    // In future this can read `assets/<universe>/ships.json` or similar.
    // Try to load from assets/<universe>/ships.json relative to crate root. Fall back
    // to embedded JSON if the file can't be read or parsed.
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

    // Embedded fallback
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

/// Convenience loader for the default universe.
///
/// This small helper delegates to `load_ships_for_universe("default")` and
/// provides a concise name for tests and code that only needs the default
/// set.
pub fn load_default_ships() -> HashMap<String, ShipDef> {
    load_ships_for_universe("default")
}

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
