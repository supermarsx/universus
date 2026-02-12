use std::collections::HashMap;

pub use game_fleet::ships::ShipDef;

pub fn load_ships_for_universe(universe: &str) -> HashMap<String, ShipDef> {
    let assets_path = format!(
        "{}/assets/{}/ships.json",
        env!("CARGO_MANIFEST_DIR"),
        universe
    );
    if let Ok(content) = std::fs::read_to_string(assets_path) {
        if let Ok(defs) = serde_json::from_str(&content) {
            return defs;
        }
    }

    game_fleet::ships::load_ships_for_universe(universe)
}

pub fn load_default_ships() -> HashMap<String, ShipDef> {
    game_fleet::ships::load_default_ships()
}
