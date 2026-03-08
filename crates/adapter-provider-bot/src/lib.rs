#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// BotPersonality
// ---------------------------------------------------------------------------

/// Personality archetypes that govern bot decision-making.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BotPersonality {
    Aggressive,
    Defensive,
    Trader,
    Explorer,
    Miner,
    Balanced,
}

impl Display for BotPersonality {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::Aggressive => "Aggressive",
            Self::Defensive => "Defensive",
            Self::Trader => "Trader",
            Self::Explorer => "Explorer",
            Self::Miner => "Miner",
            Self::Balanced => "Balanced",
        };
        f.write_str(label)
    }
}

impl FromStr for BotPersonality {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "aggressive" => Ok(Self::Aggressive),
            "defensive" => Ok(Self::Defensive),
            "trader" => Ok(Self::Trader),
            "explorer" => Ok(Self::Explorer),
            "miner" => Ok(Self::Miner),
            "balanced" => Ok(Self::Balanced),
            other => Err(format!("unknown bot personality: {other}")),
        }
    }
}

// ---------------------------------------------------------------------------
// ResourcePriority
// ---------------------------------------------------------------------------

/// Weights describing how a bot prioritizes the three core resources.
/// The weights are normalised so they sum to 1.0.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourcePriority {
    pub metal_weight: f64,
    pub crystal_weight: f64,
    pub deuterium_weight: f64,
}

impl ResourcePriority {
    /// Create a new `ResourcePriority`, normalising the weights so they sum to 1.0.
    /// If all inputs are zero the weights are distributed equally.
    pub fn new(metal: f64, crystal: f64, deuterium: f64) -> Self {
        let sum = metal + crystal + deuterium;
        if sum <= 0.0 {
            return Self {
                metal_weight: 1.0 / 3.0,
                crystal_weight: 1.0 / 3.0,
                deuterium_weight: 1.0 / 3.0,
            };
        }
        Self {
            metal_weight: metal / sum,
            crystal_weight: crystal / sum,
            deuterium_weight: deuterium / sum,
        }
    }
}

// ---------------------------------------------------------------------------
// FleetComposition
// ---------------------------------------------------------------------------

/// Ratio-based fleet build preferences. Values need not sum to 1.0 —
/// they are treated as relative weights.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetComposition {
    pub fighters_ratio: f64,
    pub bombers_ratio: f64,
    pub cargo_ratio: f64,
    pub recycler_ratio: f64,
}

impl FleetComposition {
    pub fn new(fighters: f64, bombers: f64, cargo: f64, recycler: f64) -> Self {
        Self {
            fighters_ratio: fighters,
            bombers_ratio: bombers,
            cargo_ratio: cargo,
            recycler_ratio: recycler,
        }
    }

    /// Return the dominant ship class name.
    pub fn dominant_class(&self) -> &'static str {
        let pairs: [(&str, f64); 4] = [
            ("fighter", self.fighters_ratio),
            ("bomber", self.bombers_ratio),
            ("cargo", self.cargo_ratio),
            ("recycler", self.recycler_ratio),
        ];
        pairs
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(name, _)| *name)
            .unwrap_or("fighter")
    }
}

// ---------------------------------------------------------------------------
// BotConfig
// ---------------------------------------------------------------------------

/// Full configuration for a single bot instance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BotConfig {
    pub personality: BotPersonality,
    /// 0.0 (passive) to 1.0 (very aggressive).
    pub aggression_level: f64,
    /// Expansion eagerness multiplier (higher = colonise faster).
    pub expansion_rate: f64,
    pub resource_priority: ResourcePriority,
    pub fleet_composition: FleetComposition,
    /// Pairs of `(start_hour, end_hour)` in 24h format when the bot is active.
    pub activity_hours: Vec<(i32, i32)>,
    /// Artificial delay (milliseconds) before acting — makes the bot seem human.
    pub response_delay_ms: u64,
}

impl BotConfig {
    /// Convenience constructor with sensible defaults for the given personality.
    pub fn for_personality(personality: BotPersonality) -> Self {
        match personality {
            BotPersonality::Aggressive => Self {
                personality,
                aggression_level: 0.9,
                expansion_rate: 0.6,
                resource_priority: ResourcePriority::new(0.4, 0.3, 0.3),
                fleet_composition: FleetComposition::new(0.6, 0.2, 0.1, 0.1),
                activity_hours: vec![(6, 23)],
                response_delay_ms: 500,
            },
            BotPersonality::Defensive => Self {
                personality,
                aggression_level: 0.2,
                expansion_rate: 0.3,
                resource_priority: ResourcePriority::new(0.4, 0.4, 0.2),
                fleet_composition: FleetComposition::new(0.3, 0.1, 0.2, 0.4),
                activity_hours: vec![(8, 22)],
                response_delay_ms: 1500,
            },
            BotPersonality::Trader => Self {
                personality,
                aggression_level: 0.1,
                expansion_rate: 0.4,
                resource_priority: ResourcePriority::new(0.3, 0.3, 0.4),
                fleet_composition: FleetComposition::new(0.1, 0.0, 0.7, 0.2),
                activity_hours: vec![(7, 21)],
                response_delay_ms: 2000,
            },
            BotPersonality::Explorer => Self {
                personality,
                aggression_level: 0.3,
                expansion_rate: 0.9,
                resource_priority: ResourcePriority::new(0.3, 0.3, 0.4),
                fleet_composition: FleetComposition::new(0.2, 0.0, 0.3, 0.5),
                activity_hours: vec![(0, 23)],
                response_delay_ms: 1000,
            },
            BotPersonality::Miner => Self {
                personality,
                aggression_level: 0.1,
                expansion_rate: 0.5,
                resource_priority: ResourcePriority::new(0.5, 0.3, 0.2),
                fleet_composition: FleetComposition::new(0.1, 0.0, 0.5, 0.4),
                activity_hours: vec![(6, 22)],
                response_delay_ms: 3000,
            },
            BotPersonality::Balanced => Self {
                personality,
                aggression_level: 0.5,
                expansion_rate: 0.5,
                resource_priority: ResourcePriority::new(0.34, 0.33, 0.33),
                fleet_composition: FleetComposition::new(0.3, 0.2, 0.3, 0.2),
                activity_hours: vec![(8, 22)],
                response_delay_ms: 1500,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// BotResources / NearbyPlayer / BotGameState
// ---------------------------------------------------------------------------

/// Snapshot of a bot's current resources.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BotResources {
    pub metal: i64,
    pub crystal: i64,
    pub deuterium: i64,
    pub energy: i64,
}

/// A player visible in the bot's neighbourhood.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NearbyPlayer {
    pub player_id: i64,
    pub score: i64,
    pub is_inactive: bool,
    pub distance: i32,
    pub alliance_tag: Option<String>,
}

/// Everything the bot can observe for decision-making.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BotGameState {
    pub player_id: i64,
    pub score: i64,
    pub resources: BotResources,
    pub buildings: HashMap<String, i32>,
    pub technologies: HashMap<String, i32>,
    pub ships: HashMap<String, i32>,
    pub defenses: HashMap<String, i32>,
    pub planet_count: i32,
    pub max_planets: i32,
    pub nearby_players: Vec<NearbyPlayer>,
    pub incoming_fleets: i32,
}

// ---------------------------------------------------------------------------
// Decision types
// ---------------------------------------------------------------------------

/// Details for a fleet dispatch decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetDecisionDetails {
    pub mission: String,
    pub target_galaxy: i32,
    pub target_system: i32,
    pub target_position: i32,
    pub ships: HashMap<String, i32>,
    pub cargo_metal: i64,
    pub cargo_crystal: i64,
    pub cargo_deuterium: i64,
}

/// Details for a trade decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TradeDecision {
    pub sell_resource: String,
    pub sell_amount: i64,
    pub buy_resource: String,
    pub buy_amount: i64,
}

/// The set of possible actions a bot can decide to take.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BotDecision {
    BuildMine(String),
    BuildDefense(String, i32),
    ResearchTech(String),
    BuildShips(String, i32),
    SendFleet(FleetDecisionDetails),
    ExpandColony,
    Wait(i64),
    Trade(TradeDecision),
}

// ---------------------------------------------------------------------------
// BotDecisionEngine
// ---------------------------------------------------------------------------

/// Core AI logic: given a config and a game-state snapshot, decide the next action.
#[derive(Debug, Clone)]
pub struct BotDecisionEngine {
    pub config: BotConfig,
}

impl BotDecisionEngine {
    pub fn new(config: BotConfig) -> Self {
        Self { config }
    }

    /// Main entry point — returns the best next action for the bot.
    pub fn decide_next_action(&self, state: &BotGameState) -> BotDecision {
        // If there are incoming fleets, react defensively regardless of personality.
        if state.incoming_fleets > 0 {
            return self.decide_under_threat(state);
        }

        match self.config.personality {
            BotPersonality::Aggressive => self.decide_aggressive(state),
            BotPersonality::Defensive => self.decide_defensive(state),
            BotPersonality::Trader => self.decide_trader(state),
            BotPersonality::Explorer => self.decide_explorer(state),
            BotPersonality::Miner => self.decide_miner(state),
            BotPersonality::Balanced => self.decide_balanced(state),
        }
    }

    // -- private helpers ------------------------------------------------

    fn decide_under_threat(&self, state: &BotGameState) -> BotDecision {
        let defense_level = total_value(&state.defenses);
        if defense_level < 10 {
            return BotDecision::BuildDefense("rocket_launcher".into(), 5);
        }
        BotDecision::BuildDefense("plasma_turret".into(), 2)
    }

    fn decide_aggressive(&self, state: &BotGameState) -> BotDecision {
        let fighter_count = state.ships.get("fighter").copied().unwrap_or(0);

        // Attack weak inactive neighbours first.
        if fighter_count >= 20 {
            if let Some(target) = self.find_attack_target(state) {
                let mut ships = HashMap::new();
                ships.insert("fighter".into(), fighter_count);
                return BotDecision::SendFleet(FleetDecisionDetails {
                    mission: "attack".into(),
                    target_galaxy: 1,
                    target_system: target.distance,
                    target_position: 4,
                    ships,
                    cargo_metal: 0,
                    cargo_crystal: 0,
                    cargo_deuterium: 0,
                });
            }
        }

        // Otherwise build fighters.
        if state.resources.metal >= 3000 && state.resources.crystal >= 1000 {
            return BotDecision::BuildShips("fighter".into(), 10);
        }

        // Fall back to upgrading weapons tech.
        let weapons_level = state.technologies.get("weapons").copied().unwrap_or(0);
        if weapons_level < 10 && state.resources.metal >= 2000 {
            return BotDecision::ResearchTech("weapons".into());
        }

        // Need resources — build a mine.
        self.decide_mine_upgrade(state)
    }

    fn decide_defensive(&self, state: &BotGameState) -> BotDecision {
        let defense_level = total_value(&state.defenses);
        let shield_level = state.technologies.get("shielding").copied().unwrap_or(0);

        // Build defenses first.
        if defense_level < 50 && state.resources.metal >= 2000 {
            return BotDecision::BuildDefense("rocket_launcher".into(), 10);
        }

        // Research shielding.
        if shield_level < 8 && state.resources.crystal >= 3000 {
            return BotDecision::ResearchTech("shielding".into());
        }

        // Build plasma turrets.
        if state.resources.metal >= 5000 && state.resources.crystal >= 5000 {
            return BotDecision::BuildDefense("plasma_turret".into(), 2);
        }

        self.decide_mine_upgrade(state)
    }

    fn decide_trader(&self, state: &BotGameState) -> BotDecision {
        let cargo_count = state.ships.get("cargo").copied().unwrap_or(0);

        // Build cargo ships.
        if cargo_count < 30 && state.resources.metal >= 2000 {
            return BotDecision::BuildShips("cargo".into(), 5);
        }

        // Trade excess resources.
        if state.resources.metal > 10_000 && state.resources.deuterium < 3000 {
            return BotDecision::Trade(TradeDecision {
                sell_resource: "metal".into(),
                sell_amount: 5000,
                buy_resource: "deuterium".into(),
                buy_amount: 2500,
            });
        }
        if state.resources.crystal > 10_000 && state.resources.metal < 3000 {
            return BotDecision::Trade(TradeDecision {
                sell_resource: "crystal".into(),
                sell_amount: 5000,
                buy_resource: "metal".into(),
                buy_amount: 5000,
            });
        }

        self.decide_mine_upgrade(state)
    }

    fn decide_explorer(&self, state: &BotGameState) -> BotDecision {
        // Expand colony if possible.
        if state.planet_count < state.max_planets {
            let colony_ship = state.ships.get("colony_ship").copied().unwrap_or(0);
            if colony_ship > 0 {
                return BotDecision::ExpandColony;
            }
            // Build a colony ship.
            if state.resources.metal >= 10_000
                && state.resources.crystal >= 5000
                && state.resources.deuterium >= 5000
            {
                return BotDecision::BuildShips("colony_ship".into(), 1);
            }
        }

        // Build probes for espionage.
        let probe_count = state.ships.get("probe").copied().unwrap_or(0);
        if probe_count < 10 && state.resources.crystal >= 1000 {
            return BotDecision::BuildShips("probe".into(), 5);
        }

        // Research astrophysics.
        let astro_level = state.technologies.get("astrophysics").copied().unwrap_or(0);
        if astro_level < 6 && state.resources.metal >= 4000 {
            return BotDecision::ResearchTech("astrophysics".into());
        }

        self.decide_mine_upgrade(state)
    }

    fn decide_miner(&self, state: &BotGameState) -> BotDecision {
        let metal_mine = state.buildings.get("metal_mine").copied().unwrap_or(0);
        let crystal_mine = state.buildings.get("crystal_mine").copied().unwrap_or(0);
        let deuterium_synth = state
            .buildings
            .get("deuterium_synthesizer")
            .copied()
            .unwrap_or(0);

        // Always prioritise mines.
        if metal_mine <= crystal_mine && metal_mine <= deuterium_synth {
            return BotDecision::BuildMine("metal_mine".into());
        }
        if crystal_mine <= deuterium_synth {
            return BotDecision::BuildMine("crystal_mine".into());
        }
        if state.resources.metal >= 1000 {
            return BotDecision::BuildMine("deuterium_synthesizer".into());
        }

        // Build storage when mines are high.
        if metal_mine >= 15 {
            let storage_level = state.buildings.get("metal_storage").copied().unwrap_or(0);
            if storage_level < metal_mine / 2 {
                return BotDecision::BuildMine("metal_storage".into());
            }
        }

        // Build cargo ships to transport resources.
        let cargo_count = state.ships.get("cargo").copied().unwrap_or(0);
        if cargo_count < 10 && state.resources.metal >= 2000 {
            return BotDecision::BuildShips("cargo".into(), 5);
        }

        BotDecision::Wait(60)
    }

    fn decide_balanced(&self, state: &BotGameState) -> BotDecision {
        let metal_mine = state.buildings.get("metal_mine").copied().unwrap_or(0);
        let fighter_count = state.ships.get("fighter").copied().unwrap_or(0);
        let defense_level = total_value(&state.defenses);
        let weapons_level = state.technologies.get("weapons").copied().unwrap_or(0);

        // Economy first.
        if metal_mine < 10 {
            return self.decide_mine_upgrade(state);
        }

        // Some military.
        if fighter_count < 15 && state.resources.metal >= 3000 {
            return BotDecision::BuildShips("fighter".into(), 5);
        }

        // Some defense.
        if defense_level < 20 && state.resources.metal >= 2000 {
            return BotDecision::BuildDefense("rocket_launcher".into(), 5);
        }

        // Research.
        if weapons_level < 5 && state.resources.metal >= 2000 {
            return BotDecision::ResearchTech("weapons".into());
        }

        // Expand if possible.
        if state.planet_count < state.max_planets {
            let colony_ship = state.ships.get("colony_ship").copied().unwrap_or(0);
            if colony_ship > 0 {
                return BotDecision::ExpandColony;
            }
        }

        // Fallback.
        self.decide_mine_upgrade(state)
    }

    /// Generic mine upgrade helper — builds the lowest-level mine.
    fn decide_mine_upgrade(&self, state: &BotGameState) -> BotDecision {
        let metal_mine = state.buildings.get("metal_mine").copied().unwrap_or(0);
        let crystal_mine = state.buildings.get("crystal_mine").copied().unwrap_or(0);
        let deuterium_synth = state
            .buildings
            .get("deuterium_synthesizer")
            .copied()
            .unwrap_or(0);

        if metal_mine <= crystal_mine && metal_mine <= deuterium_synth {
            BotDecision::BuildMine("metal_mine".into())
        } else if crystal_mine <= deuterium_synth {
            BotDecision::BuildMine("crystal_mine".into())
        } else {
            BotDecision::BuildMine("deuterium_synthesizer".into())
        }
    }

    /// Find the best nearby target for an attack.
    fn find_attack_target<'a>(&self, state: &'a BotGameState) -> Option<&'a NearbyPlayer> {
        state
            .nearby_players
            .iter()
            .filter(|p| p.is_inactive || (p.score as f64) < (state.score as f64) * 0.7)
            .min_by_key(|p| p.distance)
    }
}

/// Sum all values in a `HashMap<String, i32>`.
fn total_value(map: &HashMap<String, i32>) -> i32 {
    map.values().sum()
}

// ---------------------------------------------------------------------------
// Bot Activity Simulation
// ---------------------------------------------------------------------------

/// Returns `true` if the bot should be active at the given hour of day (0-23).
pub fn should_be_active(config: &BotConfig, hour_of_day: i32) -> bool {
    config.activity_hours.iter().any(|&(start, end)| {
        if start <= end {
            hour_of_day >= start && hour_of_day <= end
        } else {
            // Wraps past midnight, e.g. (22, 6).
            hour_of_day >= start || hour_of_day <= end
        }
    })
}

/// Calculate the base interval (in seconds) between bot actions based on
/// the personality. More aggressive / active personalities act more frequently.
pub fn calculate_action_interval(config: &BotConfig) -> i64 {
    let base_seconds: i64 = match config.personality {
        BotPersonality::Aggressive => 120,
        BotPersonality::Defensive => 300,
        BotPersonality::Trader => 240,
        BotPersonality::Explorer => 180,
        BotPersonality::Miner => 360,
        BotPersonality::Balanced => 240,
    };

    // Scale inversely by aggression — more aggressive → shorter interval.
    let factor = 1.0 - (config.aggression_level * 0.5);
    (base_seconds as f64 * factor).round() as i64
}

/// Simulate a human-like "think time" in milliseconds.  Uses a simple
/// deterministic hash of `seed` so that results are reproducible in tests.
pub fn simulate_think_time(config: &BotConfig, seed: u64) -> u64 {
    let base = config.response_delay_ms;
    // Pseudo-random jitter derived from seed (no external RNG crate needed).
    let jitter = ((seed.wrapping_mul(6364136223846793005).wrapping_add(1)) >> 33) % (base + 1);
    base + jitter
}

// ---------------------------------------------------------------------------
// BotScheduler
// ---------------------------------------------------------------------------

/// Entry tracking a single bot's schedule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotScheduleEntry {
    pub bot_id: i64,
    pub config: BotConfig,
    pub last_action_time: i64,
}

/// Manages scheduling for many bots — determines which bots are due for
/// their next action.
#[derive(Debug, Clone, Default)]
pub struct BotScheduler {
    pub entries: HashMap<i64, BotScheduleEntry>,
}

impl BotScheduler {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Register a bot so the scheduler tracks it.
    pub fn register_bot(&mut self, bot_id: i64, config: BotConfig) {
        self.entries.insert(
            bot_id,
            BotScheduleEntry {
                bot_id,
                config,
                last_action_time: 0,
            },
        );
    }

    /// Remove a bot from the scheduler.
    pub fn unregister_bot(&mut self, bot_id: i64) {
        self.entries.remove(&bot_id);
    }

    /// Return the IDs of all bots whose interval has elapsed as of `now`
    /// (epoch seconds).
    pub fn due_bots(&self, now: i64) -> Vec<i64> {
        self.entries
            .values()
            .filter(|entry| {
                let interval = calculate_action_interval(&entry.config);
                now - entry.last_action_time >= interval
            })
            .map(|entry| entry.bot_id)
            .collect()
    }

    /// Record that a bot has just taken an action at time `now`.
    pub fn record_action(&mut self, bot_id: i64, now: i64) {
        if let Some(entry) = self.entries.get_mut(&bot_id) {
            entry.last_action_time = now;
        }
    }

    /// How many bots are currently registered.
    pub fn active_bot_count(&self) -> usize {
        self.entries.len()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- helpers ---------------------------------------------------------

    fn empty_state() -> BotGameState {
        BotGameState {
            player_id: 1,
            score: 1000,
            resources: BotResources {
                metal: 50_000,
                crystal: 50_000,
                deuterium: 50_000,
                energy: 500,
            },
            buildings: HashMap::new(),
            technologies: HashMap::new(),
            ships: HashMap::new(),
            defenses: HashMap::new(),
            planet_count: 1,
            max_planets: 9,
            nearby_players: vec![],
            incoming_fleets: 0,
        }
    }

    fn state_with_mines(metal: i32, crystal: i32, deut: i32) -> BotGameState {
        let mut s = empty_state();
        s.buildings.insert("metal_mine".into(), metal);
        s.buildings.insert("crystal_mine".into(), crystal);
        s.buildings.insert("deuterium_synthesizer".into(), deut);
        s
    }

    // -- BotPersonality --------------------------------------------------

    #[test]
    fn personality_display_and_from_str() {
        for personality in [
            BotPersonality::Aggressive,
            BotPersonality::Defensive,
            BotPersonality::Trader,
            BotPersonality::Explorer,
            BotPersonality::Miner,
            BotPersonality::Balanced,
        ] {
            let text = personality.to_string();
            let parsed: BotPersonality = text.parse().expect("parse failed");
            assert_eq!(parsed, personality);
        }
    }

    #[test]
    fn personality_from_str_case_insensitive() {
        assert_eq!(
            "AGGRESSIVE".parse::<BotPersonality>().unwrap(),
            BotPersonality::Aggressive
        );
        assert_eq!(
            "miner".parse::<BotPersonality>().unwrap(),
            BotPersonality::Miner
        );
    }

    #[test]
    fn personality_from_str_unknown() {
        assert!("pirate".parse::<BotPersonality>().is_err());
    }

    // -- ResourcePriority ------------------------------------------------

    #[test]
    fn resource_priority_normalises() {
        let rp = ResourcePriority::new(2.0, 3.0, 5.0);
        let sum = rp.metal_weight + rp.crystal_weight + rp.deuterium_weight;
        assert!((sum - 1.0).abs() < 1e-9, "sum was {sum}");
        assert!((rp.metal_weight - 0.2).abs() < 1e-9);
        assert!((rp.crystal_weight - 0.3).abs() < 1e-9);
        assert!((rp.deuterium_weight - 0.5).abs() < 1e-9);
    }

    #[test]
    fn resource_priority_all_zero_gives_equal() {
        let rp = ResourcePriority::new(0.0, 0.0, 0.0);
        let expected = 1.0 / 3.0;
        assert!((rp.metal_weight - expected).abs() < 1e-9);
        assert!((rp.crystal_weight - expected).abs() < 1e-9);
        assert!((rp.deuterium_weight - expected).abs() < 1e-9);
    }

    // -- FleetComposition ------------------------------------------------

    #[test]
    fn fleet_composition_dominant_class() {
        let fc = FleetComposition::new(0.1, 0.1, 0.7, 0.1);
        assert_eq!(fc.dominant_class(), "cargo");

        let fc2 = FleetComposition::new(0.6, 0.2, 0.1, 0.1);
        assert_eq!(fc2.dominant_class(), "fighter");
    }

    // -- Decision engine: one test per personality -----------------------

    #[test]
    fn aggressive_attacks_weak_neighbour() {
        let config = BotConfig::for_personality(BotPersonality::Aggressive);
        let engine = BotDecisionEngine::new(config);

        let mut state = empty_state();
        state.ships.insert("fighter".into(), 30);
        state.nearby_players.push(NearbyPlayer {
            player_id: 99,
            score: 200, // much weaker
            is_inactive: true,
            distance: 5,
            alliance_tag: None,
        });

        match engine.decide_next_action(&state) {
            BotDecision::SendFleet(details) => {
                assert_eq!(details.mission, "attack");
            }
            other => panic!("expected SendFleet, got {other:?}"),
        }
    }

    #[test]
    fn aggressive_builds_fighters_without_target() {
        let config = BotConfig::for_personality(BotPersonality::Aggressive);
        let engine = BotDecisionEngine::new(config);
        let state = empty_state(); // no neighbours, no ships

        match engine.decide_next_action(&state) {
            BotDecision::BuildShips(name, count) => {
                assert_eq!(name, "fighter");
                assert!(count > 0);
            }
            other => panic!("expected BuildShips(fighter), got {other:?}"),
        }
    }

    #[test]
    fn defensive_builds_defenses() {
        let config = BotConfig::for_personality(BotPersonality::Defensive);
        let engine = BotDecisionEngine::new(config);
        let state = empty_state();

        match engine.decide_next_action(&state) {
            BotDecision::BuildDefense(name, count) => {
                assert_eq!(name, "rocket_launcher");
                assert!(count > 0);
            }
            other => panic!("expected BuildDefense, got {other:?}"),
        }
    }

    #[test]
    fn trader_builds_cargo_or_trades() {
        let config = BotConfig::for_personality(BotPersonality::Trader);
        let engine = BotDecisionEngine::new(config);
        let state = empty_state();

        match engine.decide_next_action(&state) {
            BotDecision::BuildShips(name, _) => assert_eq!(name, "cargo"),
            BotDecision::Trade(_) => {} // also fine
            other => panic!("expected BuildShips(cargo) or Trade, got {other:?}"),
        }
    }

    #[test]
    fn explorer_expands_colony() {
        let config = BotConfig::for_personality(BotPersonality::Explorer);
        let engine = BotDecisionEngine::new(config);

        let mut state = empty_state();
        state.ships.insert("colony_ship".into(), 1);
        state.planet_count = 2;
        state.max_planets = 9;

        assert_eq!(engine.decide_next_action(&state), BotDecision::ExpandColony);
    }

    #[test]
    fn miner_upgrades_lowest_mine() {
        let config = BotConfig::for_personality(BotPersonality::Miner);
        let engine = BotDecisionEngine::new(config);

        let state = state_with_mines(5, 8, 7);

        match engine.decide_next_action(&state) {
            BotDecision::BuildMine(name) => assert_eq!(name, "metal_mine"),
            other => panic!("expected BuildMine(metal_mine), got {other:?}"),
        }
    }

    #[test]
    fn balanced_upgrades_economy_first() {
        let config = BotConfig::for_personality(BotPersonality::Balanced);
        let engine = BotDecisionEngine::new(config);
        let state = state_with_mines(3, 3, 3);

        // Metal mine level < 10 so balanced should mine first.
        match engine.decide_next_action(&state) {
            BotDecision::BuildMine(_) => {}
            other => panic!("expected BuildMine, got {other:?}"),
        }
    }

    #[test]
    fn incoming_fleet_triggers_defense() {
        let config = BotConfig::for_personality(BotPersonality::Trader); // even a trader defends
        let engine = BotDecisionEngine::new(config);

        let mut state = empty_state();
        state.incoming_fleets = 3;

        match engine.decide_next_action(&state) {
            BotDecision::BuildDefense(_, _) => {}
            other => panic!("expected BuildDefense under threat, got {other:?}"),
        }
    }

    // -- Activity hours --------------------------------------------------

    #[test]
    fn activity_hours_simple_range() {
        let config = BotConfig::for_personality(BotPersonality::Balanced);
        // Default balanced: (8, 22)
        assert!(should_be_active(&config, 10));
        assert!(should_be_active(&config, 8));
        assert!(should_be_active(&config, 22));
        assert!(!should_be_active(&config, 3));
    }

    #[test]
    fn activity_hours_wrap_midnight() {
        let mut config = BotConfig::for_personality(BotPersonality::Aggressive);
        config.activity_hours = vec![(22, 6)];

        assert!(should_be_active(&config, 23));
        assert!(should_be_active(&config, 0));
        assert!(should_be_active(&config, 5));
        assert!(!should_be_active(&config, 12));
    }

    // -- Action intervals ------------------------------------------------

    #[test]
    fn action_interval_varies_by_personality() {
        let aggressive =
            calculate_action_interval(&BotConfig::for_personality(BotPersonality::Aggressive));
        let miner = calculate_action_interval(&BotConfig::for_personality(BotPersonality::Miner));
        // Aggressive should act more frequently.
        assert!(aggressive < miner, "aggressive={aggressive}, miner={miner}");
    }

    #[test]
    fn action_interval_respects_aggression() {
        let mut config = BotConfig::for_personality(BotPersonality::Balanced);
        config.aggression_level = 0.0;
        let slow = calculate_action_interval(&config);

        config.aggression_level = 1.0;
        let fast = calculate_action_interval(&config);

        assert!(fast < slow, "fast={fast}, slow={slow}");
    }

    // -- Simulate think time ---------------------------------------------

    #[test]
    fn simulate_think_time_deterministic() {
        let config = BotConfig::for_personality(BotPersonality::Balanced);
        let t1 = simulate_think_time(&config, 42);
        let t2 = simulate_think_time(&config, 42);
        assert_eq!(t1, t2);
    }

    #[test]
    fn simulate_think_time_at_least_base() {
        let config = BotConfig::for_personality(BotPersonality::Balanced);
        for seed in 0..100 {
            let t = simulate_think_time(&config, seed);
            assert!(
                t >= config.response_delay_ms,
                "seed={seed}: {t} < base {}",
                config.response_delay_ms
            );
        }
    }

    // -- Scheduler -------------------------------------------------------

    #[test]
    fn scheduler_register_and_due() {
        let mut scheduler = BotScheduler::new();
        let config = BotConfig::for_personality(BotPersonality::Aggressive);
        let interval = calculate_action_interval(&config);

        scheduler.register_bot(1, config);
        assert_eq!(scheduler.active_bot_count(), 1);

        // At time 0 the bot is due (last_action_time == 0, interval elapsed).
        let due = scheduler.due_bots(interval);
        assert!(due.contains(&1));

        // Record action at time `interval`.
        scheduler.record_action(1, interval);

        // Not due again immediately.
        let due = scheduler.due_bots(interval + 1);
        assert!(!due.contains(&1));

        // Due again after another interval.
        let due = scheduler.due_bots(interval * 2);
        assert!(due.contains(&1));
    }

    #[test]
    fn scheduler_unregister() {
        let mut scheduler = BotScheduler::new();
        scheduler.register_bot(10, BotConfig::for_personality(BotPersonality::Miner));
        assert_eq!(scheduler.active_bot_count(), 1);

        scheduler.unregister_bot(10);
        assert_eq!(scheduler.active_bot_count(), 0);
        assert!(scheduler.due_bots(999_999).is_empty());
    }

    #[test]
    fn scheduler_multiple_bots() {
        let mut scheduler = BotScheduler::new();
        scheduler.register_bot(1, BotConfig::for_personality(BotPersonality::Aggressive));
        scheduler.register_bot(2, BotConfig::for_personality(BotPersonality::Miner));
        scheduler.register_bot(3, BotConfig::for_personality(BotPersonality::Trader));

        assert_eq!(scheduler.active_bot_count(), 3);

        // All should be due at a large enough time.
        let due = scheduler.due_bots(100_000);
        assert_eq!(due.len(), 3);
    }

    // -- Serde round-trip ------------------------------------------------

    #[test]
    fn bot_config_serde_round_trip() {
        let config = BotConfig::for_personality(BotPersonality::Explorer);
        let json = serde_json::to_string(&config).expect("serialize");
        let restored: BotConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(config, restored);
    }

    #[test]
    fn bot_decision_serde_round_trip() {
        let decisions = vec![
            BotDecision::BuildMine("metal_mine".into()),
            BotDecision::BuildDefense("plasma_turret".into(), 3),
            BotDecision::ResearchTech("weapons".into()),
            BotDecision::BuildShips("fighter".into(), 10),
            BotDecision::ExpandColony,
            BotDecision::Wait(120),
            BotDecision::Trade(TradeDecision {
                sell_resource: "metal".into(),
                sell_amount: 5000,
                buy_resource: "deuterium".into(),
                buy_amount: 2500,
            }),
            BotDecision::SendFleet(FleetDecisionDetails {
                mission: "attack".into(),
                target_galaxy: 1,
                target_system: 50,
                target_position: 4,
                ships: {
                    let mut m = HashMap::new();
                    m.insert("fighter".into(), 20);
                    m
                },
                cargo_metal: 0,
                cargo_crystal: 0,
                cargo_deuterium: 0,
            }),
        ];

        for decision in &decisions {
            let json = serde_json::to_string(decision).expect("serialize");
            let restored: BotDecision = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(*decision, restored);
        }
    }
}
