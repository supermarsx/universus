use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AppState {
    inner: Arc<Mutex<GameState>>,
    shard_inner: Arc<Mutex<ShardState>>,
    analytics_inner: Arc<Mutex<AnalyticsState>>,
    universe_inner: Arc<Mutex<UniverseState>>,
    acs_inner: Arc<Mutex<AcsState>>,
}

struct GameState {
    players: HashMap<String, PlayerState>,
    config_parameters: HashMap<String, ConfigParameter>,
    config_history: Vec<ConfigHistoryEntry>,
}

struct ShardState {
    servers: HashMap<String, ShardServerRecord>,
    routing_migrations: i64,
}

struct AnalyticsState {
    total_events: i64,
    by_type: HashMap<String, i64>,
}

struct UniverseState {
    universes: HashMap<i64, UniverseRecord>,
    next_id: i64,
}

struct AcsState {
    groups: HashMap<i64, AcsGroupRecord>,
    next_id: i64,
}

#[derive(Clone)]
struct UniverseRecord {
    id: i64,
    name: String,
    speed: i32,
    registration_open: bool,
}

#[derive(Clone)]
struct AcsGroupRecord {
    id: i64,
    mission_type: String,
    target_galaxy: i32,
    target_system: i32,
    target_position: i32,
    member_planet_ids: Vec<i64>,
    departure_window_start: String,
    departure_window_end: String,
    notes: Option<String>,
}

#[derive(Clone)]
struct ShardServerRecord {
    server_id: String,
    server_type: String,
    region: String,
    endpoint: String,
    status: String,
    current_load: i64,
    max_capacity: i64,
    health_score: f64,
    last_heartbeat_unix: i64,
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
    player_blocks: Vec<PlayerBlockRecord>,
    theme_preferences: ThemePreferencesRecord,
    custom_css: String,
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
pub struct PlayerBlock {
    pub blocked_user_id: i64,
    pub username: String,
    pub scope: String,
    pub reason: Option<String>,
}

#[allow(dead_code)]
struct PlayerBlockRecord {
    blocked_user_id: i64,
    username: String,
    scope: String,
    reason: Option<String>,
}

#[derive(Clone)]
pub struct ThemePreferences {
    pub theme_key: String,
    pub reduce_motion: bool,
    pub high_contrast: bool,
}

struct ThemePreferencesRecord {
    theme_key: String,
    reduce_motion: bool,
    high_contrast: bool,
}

#[derive(Clone)]
pub struct ConfigParameterSnapshot {
    pub key: String,
    pub category: String,
    pub value: String,
    pub default_value: String,
    pub data_type: String,
    pub description: String,
}

#[derive(Clone)]
pub struct ConfigHistorySnapshot {
    pub change_id: i64,
    pub parameter_key: String,
    pub old_value: String,
    pub new_value: String,
    pub reason: String,
}

#[derive(Clone)]
struct ConfigParameter {
    key: String,
    category: String,
    value: String,
    default_value: String,
    data_type: String,
    description: String,
}

#[derive(Clone)]
struct ConfigHistoryEntry {
    change_id: i64,
    parameter_key: String,
    old_value: String,
    new_value: String,
    reason: String,
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

#[derive(Clone)]
pub struct ShardServerSnapshot {
    pub server_id: String,
    pub server_type: String,
    pub region: String,
    pub endpoint: String,
    pub status: String,
    pub current_load: i64,
    pub max_capacity: i64,
    pub health_score: f64,
    pub last_heartbeat_unix: i64,
}

#[derive(Clone)]
pub struct ShardHealthSnapshot {
    pub server_id: String,
    pub status: String,
    pub health_score: f64,
    pub current_load: i64,
    pub max_capacity: i64,
    pub load_percent: f64,
    pub last_heartbeat_unix: i64,
}

#[derive(Clone)]
pub struct RoutingStatsSnapshot {
    pub total_servers: usize,
    pub healthy_servers: usize,
    pub overloaded_servers: usize,
    pub total_capacity: i64,
    pub total_load: i64,
    pub average_load_percent: f64,
    pub migration_count: i64,
}

#[derive(Clone)]
pub struct RegisterShardServerInput {
    pub server_id: String,
    pub server_type: String,
    pub region: String,
    pub endpoint: String,
    pub status: String,
    pub current_load: i64,
    pub max_capacity: i64,
    pub health_score: f64,
}

#[derive(Clone)]
pub struct AnalyticsUsageSnapshot {
    pub total_events: i64,
    pub active_users: i64,
    pub by_type: Vec<(String, i64)>,
}

#[derive(Clone)]
pub struct UniverseSnapshot {
    pub id: i64,
    pub name: String,
    pub speed: i32,
    pub registration_open: bool,
}

#[derive(Clone)]
pub struct AcsGroupSnapshot {
    pub id: i64,
    pub mission_type: String,
    pub target_galaxy: i32,
    pub target_system: i32,
    pub target_position: i32,
    pub member_count: i32,
    pub departure_window_start: String,
    pub departure_window_end: String,
    pub notes: Option<String>,
}

#[derive(Clone)]
pub struct CreateAcsGroupInput {
    pub mission_type: String,
    pub target_galaxy: i32,
    pub target_system: i32,
    pub target_position: i32,
    pub departure_window_start: Option<String>,
    pub departure_window_end: Option<String>,
    pub notes: Option<String>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(GameState::default())),
            shard_inner: Arc::new(Mutex::new(ShardState::default())),
            analytics_inner: Arc::new(Mutex::new(AnalyticsState::default())),
            universe_inner: Arc::new(Mutex::new(UniverseState::default())),
            acs_inner: Arc::new(Mutex::new(AcsState::default())),
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

    pub fn list_player_blocks(&self, player_key: &str) -> Vec<PlayerBlock> {
        let mut game_state = self.inner.lock().expect("app state poisoned");
        let player = player_mut(&mut game_state, player_key);
        player
            .player_blocks
            .iter()
            .map(|entry| PlayerBlock {
                blocked_user_id: entry.blocked_user_id,
                username: entry.username.clone(),
                scope: entry.scope.clone(),
                reason: entry.reason.clone(),
            })
            .collect()
    }

    pub fn add_player_block(
        &self,
        player_key: &str,
        blocked_user_id: i64,
        username: &str,
        scope: &str,
        reason: Option<String>,
    ) -> Result<PlayerBlock, &'static str> {
        if blocked_user_id <= 0 {
            return Err("User not found");
        }
        if username.trim().is_empty() {
            return Err("User not found");
        }
        if username.eq_ignore_ascii_case("Commander") {
            return Err("You cannot block yourself");
        }

        let normalized_scope = match scope {
            "all" | "chat" | "messages" => scope,
            _ => "all",
        };

        let mut game_state = self.inner.lock().expect("app state poisoned");
        let player = player_mut(&mut game_state, player_key);
        if let Some(existing) = player
            .player_blocks
            .iter_mut()
            .find(|entry| entry.blocked_user_id == blocked_user_id)
        {
            existing.scope = normalized_scope.to_string();
            existing.reason = reason.clone();
            existing.username = username.to_string();
        } else {
            player.player_blocks.push(PlayerBlockRecord {
                blocked_user_id,
                username: username.to_string(),
                scope: normalized_scope.to_string(),
                reason: reason.clone(),
            });
        }

        Ok(PlayerBlock {
            blocked_user_id,
            username: username.to_string(),
            scope: normalized_scope.to_string(),
            reason,
        })
    }

    pub fn remove_player_block(
        &self,
        player_key: &str,
        target_identifier: &str,
    ) -> Result<(), &'static str> {
        let mut game_state = self.inner.lock().expect("app state poisoned");
        let player = player_mut(&mut game_state, player_key);

        let original_len = player.player_blocks.len();
        if let Ok(target_id) = target_identifier.parse::<i64>() {
            player
                .player_blocks
                .retain(|entry| entry.blocked_user_id != target_id);
        } else {
            player
                .player_blocks
                .retain(|entry| !entry.username.eq_ignore_ascii_case(target_identifier));
        }

        if player.player_blocks.len() == original_len {
            Err("Block not found")
        } else {
            Ok(())
        }
    }

    pub fn theme_preferences(&self, player_key: &str) -> ThemePreferences {
        let mut game_state = self.inner.lock().expect("app state poisoned");
        let player = player_mut(&mut game_state, player_key);
        ThemePreferences {
            theme_key: player.theme_preferences.theme_key.clone(),
            reduce_motion: player.theme_preferences.reduce_motion,
            high_contrast: player.theme_preferences.high_contrast,
        }
    }

    pub fn update_theme_preferences(
        &self,
        player_key: &str,
        theme_key: Option<String>,
        reduce_motion: Option<bool>,
        high_contrast: Option<bool>,
    ) -> ThemePreferences {
        let mut game_state = self.inner.lock().expect("app state poisoned");
        let player = player_mut(&mut game_state, player_key);

        if let Some(theme_key) = theme_key {
            if !theme_key.trim().is_empty() {
                player.theme_preferences.theme_key = theme_key;
            }
        }
        if let Some(reduce_motion) = reduce_motion {
            player.theme_preferences.reduce_motion = reduce_motion;
        }
        if let Some(high_contrast) = high_contrast {
            player.theme_preferences.high_contrast = high_contrast;
        }

        ThemePreferences {
            theme_key: player.theme_preferences.theme_key.clone(),
            reduce_motion: player.theme_preferences.reduce_motion,
            high_contrast: player.theme_preferences.high_contrast,
        }
    }

    pub fn user_custom_css(&self, player_key: &str) -> String {
        let mut game_state = self.inner.lock().expect("app state poisoned");
        let player = player_mut(&mut game_state, player_key);
        player.custom_css.clone()
    }

    pub fn update_user_custom_css(&self, player_key: &str, css: String) -> String {
        let mut game_state = self.inner.lock().expect("app state poisoned");
        let player = player_mut(&mut game_state, player_key);
        player.custom_css = css;
        player.custom_css.clone()
    }

    pub fn config_parameters(&self, category: Option<&str>) -> Vec<ConfigParameterSnapshot> {
        let game_state = self.inner.lock().expect("app state poisoned");
        let mut parameters = game_state
            .config_parameters
            .values()
            .filter(|parameter| {
                category
                    .map(|value| parameter.category.eq_ignore_ascii_case(value))
                    .unwrap_or(true)
            })
            .map(|parameter| ConfigParameterSnapshot {
                key: parameter.key.clone(),
                category: parameter.category.clone(),
                value: parameter.value.clone(),
                default_value: parameter.default_value.clone(),
                data_type: parameter.data_type.clone(),
                description: parameter.description.clone(),
            })
            .collect::<Vec<_>>();

        parameters.sort_by(|left, right| left.key.cmp(&right.key));
        parameters
    }

    pub fn config_parameter(&self, key: &str) -> Option<ConfigParameterSnapshot> {
        let game_state = self.inner.lock().expect("app state poisoned");
        game_state
            .config_parameters
            .get(key)
            .map(|parameter| ConfigParameterSnapshot {
                key: parameter.key.clone(),
                category: parameter.category.clone(),
                value: parameter.value.clone(),
                default_value: parameter.default_value.clone(),
                data_type: parameter.data_type.clone(),
                description: parameter.description.clone(),
            })
    }

    pub fn update_config_parameter(
        &self,
        key: &str,
        value: String,
        reason: String,
    ) -> Result<ConfigParameterSnapshot, &'static str> {
        let mut game_state = self.inner.lock().expect("app state poisoned");
        let history_id = game_state.config_history.len() as i64 + 1;
        let (category, default_value, data_type, description, old_value, new_value) = {
            let Some(parameter) = game_state.config_parameters.get_mut(key) else {
                return Err("Parameter not found");
            };

            let old_value = parameter.value.clone();
            parameter.value = value.clone();
            (
                parameter.category.clone(),
                parameter.default_value.clone(),
                parameter.data_type.clone(),
                parameter.description.clone(),
                old_value,
                parameter.value.clone(),
            )
        };

        game_state.config_history.push(ConfigHistoryEntry {
            change_id: history_id,
            parameter_key: key.to_string(),
            old_value,
            new_value: new_value.clone(),
            reason,
        });

        Ok(ConfigParameterSnapshot {
            key: key.to_string(),
            category,
            value: new_value,
            default_value,
            data_type,
            description,
        })
    }

    pub fn config_history(&self, limit: usize) -> Vec<ConfigHistorySnapshot> {
        let game_state = self.inner.lock().expect("app state poisoned");
        game_state
            .config_history
            .iter()
            .rev()
            .take(limit.max(1))
            .map(|entry| ConfigHistorySnapshot {
                change_id: entry.change_id,
                parameter_key: entry.parameter_key.clone(),
                old_value: entry.old_value.clone(),
                new_value: entry.new_value.clone(),
                reason: entry.reason.clone(),
            })
            .collect()
    }

    pub fn list_shard_servers(&self) -> Vec<ShardServerSnapshot> {
        let shard_state = self.shard_inner.lock().expect("app state poisoned");
        let mut servers = shard_state
            .servers
            .values()
            .map(shard_server_snapshot)
            .collect::<Vec<_>>();
        servers.sort_by(|left, right| left.server_id.cmp(&right.server_id));
        servers
    }

    pub fn register_shard_server(
        &self,
        input: RegisterShardServerInput,
    ) -> Result<ShardServerSnapshot, &'static str> {
        if input.server_id.trim().is_empty() {
            return Err("serverId is required");
        }
        if input.max_capacity <= 0 {
            return Err("maxCapacity must be greater than zero");
        }
        if input.current_load < 0 {
            return Err("currentLoad cannot be negative");
        }

        let mut shard_state = self.shard_inner.lock().expect("app state poisoned");
        let now = unix_timestamp();
        let server_id = input.server_id.clone();
        let existed = shard_state.servers.contains_key(&server_id);

        shard_state.servers.insert(
            server_id.clone(),
            ShardServerRecord {
                server_id: server_id.clone(),
                server_type: input.server_type,
                region: input.region,
                endpoint: input.endpoint,
                status: input.status,
                current_load: input.current_load,
                max_capacity: input.max_capacity,
                health_score: input.health_score,
                last_heartbeat_unix: now,
            },
        );

        if existed {
            shard_state.routing_migrations += 1;
        }

        let server = shard_state
            .servers
            .get(&server_id)
            .cloned()
            .expect("registered shard server must exist");
        Ok(shard_server_snapshot(&server))
    }

    pub fn shard_server_health(&self, server_id: &str) -> Option<ShardHealthSnapshot> {
        let shard_state = self.shard_inner.lock().expect("app state poisoned");
        shard_state.servers.get(server_id).map(|entry| {
            let load_percent = load_percent(entry.current_load, entry.max_capacity);
            ShardHealthSnapshot {
                server_id: entry.server_id.clone(),
                status: entry.status.clone(),
                health_score: entry.health_score,
                current_load: entry.current_load,
                max_capacity: entry.max_capacity,
                load_percent,
                last_heartbeat_unix: entry.last_heartbeat_unix,
            }
        })
    }

    pub fn shard_routing_stats(&self) -> RoutingStatsSnapshot {
        let shard_state = self.shard_inner.lock().expect("app state poisoned");
        let total_servers = shard_state.servers.len();
        let healthy_servers = shard_state
            .servers
            .values()
            .filter(|entry| entry.status == "online" && entry.health_score >= 0.7)
            .count();
        let overloaded_servers = shard_state
            .servers
            .values()
            .filter(|entry| load_percent(entry.current_load, entry.max_capacity) >= 80.0)
            .count();
        let total_capacity = shard_state
            .servers
            .values()
            .map(|entry| entry.max_capacity)
            .sum::<i64>();
        let total_load = shard_state
            .servers
            .values()
            .map(|entry| entry.current_load)
            .sum::<i64>();
        let average_load_percent = if total_capacity <= 0 {
            0.0
        } else {
            (total_load as f64 * 100.0) / total_capacity as f64
        };

        RoutingStatsSnapshot {
            total_servers,
            healthy_servers,
            overloaded_servers,
            total_capacity,
            total_load,
            average_load_percent: round_2(average_load_percent),
            migration_count: shard_state.routing_migrations,
        }
    }

    pub fn track_analytics_event(&self, event_type: &str) {
        let mut analytics_state = self.analytics_inner.lock().expect("app state poisoned");
        analytics_state.total_events += 1;
        *analytics_state
            .by_type
            .entry(event_type.to_string())
            .or_insert(0) += 1;
    }

    pub fn analytics_usage(&self, _days: i32) -> AnalyticsUsageSnapshot {
        let analytics_state = self.analytics_inner.lock().expect("app state poisoned");
        let mut by_type = analytics_state
            .by_type
            .iter()
            .map(|(key, value)| (key.clone(), *value))
            .collect::<Vec<_>>();
        by_type.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));

        AnalyticsUsageSnapshot {
            total_events: analytics_state.total_events,
            active_users: (analytics_state.total_events / 3).max(1),
            by_type,
        }
    }

    pub fn list_universes(&self) -> Vec<UniverseSnapshot> {
        let universe_state = self.universe_inner.lock().expect("app state poisoned");
        let mut universes = universe_state
            .universes
            .values()
            .map(universe_snapshot)
            .collect::<Vec<_>>();
        universes.sort_by(|left, right| left.id.cmp(&right.id));
        universes
    }

    pub fn get_universe(&self, id: i64) -> Option<UniverseSnapshot> {
        let universe_state = self.universe_inner.lock().expect("app state poisoned");
        universe_state.universes.get(&id).map(universe_snapshot)
    }

    pub fn create_universe(
        &self,
        name: &str,
        speed: i32,
        registration_open: bool,
    ) -> UniverseSnapshot {
        let mut universe_state = self.universe_inner.lock().expect("app state poisoned");
        let id = universe_state.next_id;
        universe_state.next_id += 1;
        let record = UniverseRecord {
            id,
            name: name.to_string(),
            speed,
            registration_open,
        };
        universe_state.universes.insert(id, record.clone());
        universe_snapshot(&record)
    }

    pub fn list_acs_groups(&self) -> Vec<AcsGroupSnapshot> {
        let acs_state = self.acs_inner.lock().expect("app state poisoned");
        let mut groups = acs_state
            .groups
            .values()
            .map(acs_group_snapshot)
            .collect::<Vec<_>>();
        groups.sort_by(|left, right| left.id.cmp(&right.id));
        groups
    }

    pub fn create_acs_group(&self, input: CreateAcsGroupInput) -> AcsGroupSnapshot {
        let mut acs_state = self.acs_inner.lock().expect("app state poisoned");
        let id = acs_state.next_id;
        acs_state.next_id += 1;
        let record = AcsGroupRecord {
            id,
            mission_type: input.mission_type,
            target_galaxy: input.target_galaxy,
            target_system: input.target_system,
            target_position: input.target_position,
            member_planet_ids: vec![1],
            departure_window_start: input
                .departure_window_start
                .unwrap_or_else(|| "2026-02-13T20:15:00Z".to_string()),
            departure_window_end: input
                .departure_window_end
                .unwrap_or_else(|| "2026-02-13T20:30:00Z".to_string()),
            notes: input.notes,
        };
        acs_state.groups.insert(id, record.clone());
        acs_group_snapshot(&record)
    }

    pub fn join_acs_group(&self, id: i64, planet_id: i64) -> Result<(), &'static str> {
        let mut acs_state = self.acs_inner.lock().expect("app state poisoned");
        let Some(group) = acs_state.groups.get_mut(&id) else {
            return Err("ACS group not found");
        };
        if !group.member_planet_ids.contains(&planet_id) {
            group.member_planet_ids.push(planet_id);
        }
        Ok(())
    }

    pub fn leave_acs_group(&self, id: i64) -> Result<(), &'static str> {
        let mut acs_state = self.acs_inner.lock().expect("app state poisoned");
        let Some(group) = acs_state.groups.get_mut(&id) else {
            return Err("ACS group not found");
        };
        if group.member_planet_ids.len() > 1 {
            group.member_planet_ids.pop();
        }
        Ok(())
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
            player_blocks: Vec::new(),
            theme_preferences: ThemePreferencesRecord {
                theme_key: "default".to_string(),
                reduce_motion: false,
                high_contrast: false,
            },
            custom_css: String::new(),
        }
    }
}

impl Default for GameState {
    fn default() -> Self {
        let mut config_parameters = HashMap::new();
        config_parameters.insert(
            "economy.resource_multiplier".to_string(),
            ConfigParameter {
                key: "economy.resource_multiplier".to_string(),
                category: "economy".to_string(),
                value: "1".to_string(),
                default_value: "1".to_string(),
                data_type: "integer".to_string(),
                description: "Global resource multiplier".to_string(),
            },
        );
        config_parameters.insert(
            "combat.debris_factor".to_string(),
            ConfigParameter {
                key: "combat.debris_factor".to_string(),
                category: "combat".to_string(),
                value: "0.3".to_string(),
                default_value: "0.3".to_string(),
                data_type: "float".to_string(),
                description: "Share of destroyed ships becoming debris".to_string(),
            },
        );

        Self {
            players: HashMap::new(),
            config_parameters,
            config_history: Vec::new(),
        }
    }
}

impl Default for ShardState {
    fn default() -> Self {
        Self {
            servers: HashMap::new(),
            routing_migrations: 0,
        }
    }
}

impl Default for AnalyticsState {
    fn default() -> Self {
        Self {
            total_events: 0,
            by_type: HashMap::new(),
        }
    }
}

impl Default for UniverseState {
    fn default() -> Self {
        let mut universes = HashMap::new();
        universes.insert(
            1,
            UniverseRecord {
                id: 1,
                name: "Andromeda".to_string(),
                speed: 4,
                registration_open: true,
            },
        );
        universes.insert(
            2,
            UniverseRecord {
                id: 2,
                name: "Pegasus".to_string(),
                speed: 6,
                registration_open: false,
            },
        );
        Self {
            universes,
            next_id: 101,
        }
    }
}

impl Default for AcsState {
    fn default() -> Self {
        let mut groups = HashMap::new();
        groups.insert(
            101,
            AcsGroupRecord {
                id: 101,
                mission_type: "attack".to_string(),
                target_galaxy: 1,
                target_system: 223,
                target_position: 9,
                member_planet_ids: vec![1, 2, 3],
                departure_window_start: "2026-02-13T20:00:00Z".to_string(),
                departure_window_end: "2026-02-13T20:10:00Z".to_string(),
                notes: Some("Synchronized strike".to_string()),
            },
        );
        Self {
            groups,
            next_id: 102,
        }
    }
}

fn universe_snapshot(entry: &UniverseRecord) -> UniverseSnapshot {
    UniverseSnapshot {
        id: entry.id,
        name: entry.name.clone(),
        speed: entry.speed,
        registration_open: entry.registration_open,
    }
}

fn acs_group_snapshot(entry: &AcsGroupRecord) -> AcsGroupSnapshot {
    AcsGroupSnapshot {
        id: entry.id,
        mission_type: entry.mission_type.clone(),
        target_galaxy: entry.target_galaxy,
        target_system: entry.target_system,
        target_position: entry.target_position,
        member_count: entry.member_planet_ids.len() as i32,
        departure_window_start: entry.departure_window_start.clone(),
        departure_window_end: entry.departure_window_end.clone(),
        notes: entry.notes.clone(),
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

fn shard_server_snapshot(entry: &ShardServerRecord) -> ShardServerSnapshot {
    ShardServerSnapshot {
        server_id: entry.server_id.clone(),
        server_type: entry.server_type.clone(),
        region: entry.region.clone(),
        endpoint: entry.endpoint.clone(),
        status: entry.status.clone(),
        current_load: entry.current_load,
        max_capacity: entry.max_capacity,
        health_score: entry.health_score,
        last_heartbeat_unix: entry.last_heartbeat_unix,
    }
}

fn unix_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn load_percent(current_load: i64, max_capacity: i64) -> f64 {
    if max_capacity <= 0 {
        0.0
    } else {
        round_2((current_load as f64 * 100.0) / max_capacity as f64)
    }
}

fn round_2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
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
