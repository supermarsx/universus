use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AppState {
    inner: Arc<Mutex<GameState>>,
}

#[derive(Default)]
struct GameState {
    players: HashMap<String, PlayerState>,
}

#[derive(Clone)]
pub struct PlayerResources {
    pub metal: i64,
    pub crystal: i64,
    pub deuterium: i64,
    pub dark_matter: i64,
}

struct PlayerState {
    resources: PlayerResources,
    fleet_log: Vec<FleetMissionRecord>,
    research_queues: HashMap<String, Vec<ResearchQueueRecord>>,
    shipyard_queues: HashMap<String, Vec<ShipyardQueueRecord>>,
    building_queues: HashMap<String, Vec<BuildingQueueRecord>>,
}

#[allow(dead_code)]
struct FleetMissionRecord {
    command_id: String,
    mission: String,
    target: String,
    total_ships: i64,
}

#[allow(dead_code)]
struct ResearchQueueRecord {
    queue_id: String,
    tech_id: String,
    level_target: i32,
    finishes_in_seconds: i64,
}

#[allow(dead_code)]
struct ShipyardQueueRecord {
    order_id: String,
    ship_type: String,
    count: i64,
    completes_in_seconds: i64,
}

#[allow(dead_code)]
struct BuildingQueueRecord {
    queue_id: String,
    building_type: String,
    level_target: i32,
    finishes_in_seconds: i64,
}

#[derive(Clone)]
pub struct FleetMission {
    pub command_id: String,
}

#[derive(Clone)]
pub struct QueuedResearch {
    pub queue_id: String,
    pub tech_id: String,
    pub level_target: i32,
    pub finishes_in_seconds: i64,
}

#[derive(Clone)]
pub struct QueuedShipBuild {
    pub order_id: String,
    pub planet_id: String,
    pub ship_type: String,
    pub count: i64,
    pub completes_in_seconds: i64,
}

#[derive(Clone)]
pub struct QueuedBuildingUpgrade {
    pub queue_id: String,
    pub planet_id: String,
    pub building_type: String,
    pub level_target: i32,
    pub finishes_in_seconds: i64,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(GameState::default())),
        }
    }

    pub fn account_resources(&self, player_key: &str) -> PlayerResources {
        let mut game_state = self.inner.lock().expect("app state poisoned");
        let player = player_mut(&mut game_state, player_key);
        player.resources.clone()
    }

    pub fn enqueue_fleet_mission(
        &self,
        player_key: &str,
        mission: String,
        target: String,
        total_ships: i64,
    ) -> FleetMission {
        let mut game_state = self.inner.lock().expect("app state poisoned");
        let player = player_mut(&mut game_state, player_key);
        let command_id = format!("cmd-fleet-{:03}", player.fleet_log.len() + 1);
        player.fleet_log.push(FleetMissionRecord {
            command_id: command_id.clone(),
            mission,
            target,
            total_ships,
        });
        FleetMission { command_id }
    }

    pub fn enqueue_research(
        &self,
        player_key: &str,
        planet_id: &str,
        technology_type: &str,
    ) -> Result<QueuedResearch, &'static str> {
        let Some((tech_id, metal, crystal, deuterium, finishes_in_seconds)) =
            research_config(technology_type)
        else {
            return Err("Research technology not found");
        };

        let mut game_state = self.inner.lock().expect("app state poisoned");
        let player = player_mut(&mut game_state, player_key);
        let queue = player
            .research_queues
            .entry(planet_id.to_string())
            .or_default();
        let level_target = queue
            .iter()
            .filter(|entry| entry.tech_id == tech_id)
            .count()
            .saturating_add(1) as i32;
        let queue_id = format!("rq-{}-{:03}", normalize_id(planet_id), queue.len() + 1);
        if !spend_resources(&mut player.resources, metal, crystal, deuterium) {
            return Err("Insufficient resources");
        }

        queue.push(ResearchQueueRecord {
            queue_id: queue_id.clone(),
            tech_id: tech_id.to_string(),
            level_target,
            finishes_in_seconds,
        });

        Ok(QueuedResearch {
            queue_id,
            tech_id: tech_id.to_string(),
            level_target,
            finishes_in_seconds,
        })
    }

    pub fn enqueue_ship_build(
        &self,
        player_key: &str,
        planet_id: &str,
        ship_type: &str,
        count: i64,
    ) -> Result<QueuedShipBuild, &'static str> {
        if count <= 0 {
            return Err("Quantity must be greater than zero");
        }
        let Some((normalized_ship_type, metal, crystal, deuterium, build_time_seconds)) =
            ship_config(ship_type)
        else {
            return Err("Ship type not found");
        };

        let total_metal = metal.saturating_mul(count);
        let total_crystal = crystal.saturating_mul(count);
        let total_deuterium = deuterium.saturating_mul(count);
        let total_build_time = build_time_seconds.saturating_mul(count);

        let mut game_state = self.inner.lock().expect("app state poisoned");
        let player = player_mut(&mut game_state, player_key);
        let queue = player
            .shipyard_queues
            .entry(planet_id.to_string())
            .or_default();
        let order_id = format!("o-{}-{:03}", normalize_id(planet_id), queue.len() + 1);
        if !spend_resources(
            &mut player.resources,
            total_metal,
            total_crystal,
            total_deuterium,
        ) {
            return Err("Insufficient resources");
        }

        queue.push(ShipyardQueueRecord {
            order_id: order_id.clone(),
            ship_type: normalized_ship_type.to_string(),
            count,
            completes_in_seconds: total_build_time,
        });

        Ok(QueuedShipBuild {
            order_id,
            planet_id: planet_id.to_string(),
            ship_type: normalized_ship_type.to_string(),
            count,
            completes_in_seconds: total_build_time,
        })
    }

    pub fn enqueue_building_upgrade(
        &self,
        player_key: &str,
        planet_id: &str,
        building_type: &str,
    ) -> Result<QueuedBuildingUpgrade, &'static str> {
        let Some((normalized_building, metal, crystal, deuterium, build_time_seconds)) =
            building_config(building_type)
        else {
            return Err("Building type not found");
        };

        let mut game_state = self.inner.lock().expect("app state poisoned");
        let player = player_mut(&mut game_state, player_key);
        let queue = player
            .building_queues
            .entry(planet_id.to_string())
            .or_default();
        let level_target = queue
            .iter()
            .filter(|entry| entry.building_type == normalized_building)
            .count()
            .saturating_add(1) as i32;
        let queue_id = format!("bq-{}-{:03}", normalize_id(planet_id), queue.len() + 1);
        if !spend_resources(&mut player.resources, metal, crystal, deuterium) {
            return Err("Insufficient resources");
        }

        queue.push(BuildingQueueRecord {
            queue_id: queue_id.clone(),
            building_type: normalized_building.to_string(),
            level_target,
            finishes_in_seconds: build_time_seconds,
        });

        Ok(QueuedBuildingUpgrade {
            queue_id,
            planet_id: planet_id.to_string(),
            building_type: normalized_building.to_string(),
            level_target,
            finishes_in_seconds: build_time_seconds,
        })
    }
}

fn player_mut<'a>(game_state: &'a mut GameState, player_key: &str) -> &'a mut PlayerState {
    game_state
        .players
        .entry(player_key.to_string())
        .or_insert_with(PlayerState::default)
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            resources: PlayerResources {
                metal: 125_000,
                crystal: 94_500,
                deuterium: 40_250,
                dark_matter: 1_500,
            },
            fleet_log: Vec::new(),
            research_queues: HashMap::new(),
            shipyard_queues: HashMap::new(),
            building_queues: HashMap::new(),
        }
    }
}

fn spend_resources(
    resources: &mut PlayerResources,
    metal: i64,
    crystal: i64,
    deuterium: i64,
) -> bool {
    if resources.metal < metal || resources.crystal < crystal || resources.deuterium < deuterium {
        return false;
    }
    resources.metal -= metal;
    resources.crystal -= crystal;
    resources.deuterium -= deuterium;
    true
}

fn normalize_id(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>()
        .to_lowercase()
}

fn research_config(technology_type: &str) -> Option<(&'static str, i64, i64, i64, i64)> {
    match technology_type.trim() {
        "energy_technology" | "energy_tech" => {
            Some(("energy_technology", 24_000, 12_000, 5_000, 5_400))
        }
        "weapons_technology" | "weapons_tech" => {
            Some(("weapons_technology", 31_000, 15_500, 6_000, 7_200))
        }
        "hyperspace_drive" => Some(("hyperspace_drive", 52_000, 39_000, 21_000, 14_400)),
        _ => None,
    }
}

fn ship_config(ship_type: &str) -> Option<(&'static str, i64, i64, i64, i64)> {
    match ship_type.trim() {
        "light_fighter" | "lightFighter" => Some(("light_fighter", 3_000, 1_000, 0, 45)),
        "small_cargo" | "smallCargo" => Some(("small_cargo", 2_000, 2_000, 0, 60)),
        _ => None,
    }
}

fn building_config(building_type: &str) -> Option<(&'static str, i64, i64, i64, i64)> {
    match building_type.trim() {
        "metal_mine" => Some(("metal_mine", 2_500, 500, 0, 300)),
        "crystal_mine" => Some(("crystal_mine", 1_800, 900, 0, 360)),
        "deuterium_synthesizer" => Some(("deuterium_synthesizer", 2_200, 2_200, 0, 420)),
        _ => None,
    }
}
