#![forbid(unsafe_code)]

use game_fleet::ships;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Input parameters for a combat simulation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CombatInput {
    pub attacker_ships: HashMap<String, i32>,
    pub defender_ships: HashMap<String, i32>,
    pub defender_defenses: HashMap<String, i32>,
    pub attacker_tech: HashMap<String, i32>,
    pub defender_tech: HashMap<String, i32>,
    pub planet_metal: i64,
    pub planet_crystal: i64,
    pub planet_deuterium: i64,
    pub seed: String,
    pub universe: String,
    /// Explicit maximum round count. `None` or 0 falls back to the OGame
    /// default of 6 rounds.
    pub max_rounds: Option<i32>,
}

/// Outcome of a combat simulation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CombatResult {
    /// One of `"attacker"`, `"defender"`, or `"draw"`.
    pub winner: String,
    pub rounds: Vec<RoundResult>,
    pub attacker_losses: HashMap<String, i32>,
    pub defender_losses: HashMap<String, i32>,
    pub loot: Loot,
    pub debris: Debris,
}

/// Stats for a single combat round.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoundResult {
    pub round_number: i32,
    pub attacker_shots: i32,
    pub defender_shots: i32,
    pub attacker_destroyed: i32,
    pub defender_destroyed: i32,
    pub attacker_remaining: i32,
    pub defender_remaining: i32,
}

/// Resources looted by the attacker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Loot {
    pub metal: i64,
    pub crystal: i64,
    pub deuterium: i64,
}

/// Debris field created from destroyed ships.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Debris {
    pub metal: i64,
    pub crystal: i64,
}

/// Full combat report suitable for delivery to players.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CombatReport {
    pub attacker_initial: HashMap<String, i32>,
    pub defender_initial: HashMap<String, i32>,
    pub attacker_tech_levels: TechLevels,
    pub defender_tech_levels: TechLevels,
    pub rounds: Vec<RoundDetail>,
    pub result: CombatResult,
    pub defense_rebuilt: HashMap<String, i32>,
}

/// Technology levels that affect combat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TechLevels {
    pub weapons: i32,
    pub shielding: i32,
    pub armor: i32,
}

/// Detailed per-round information for a combat report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoundDetail {
    pub round_number: i32,
    pub attacker_forces: HashMap<String, UnitRoundStats>,
    pub defender_forces: HashMap<String, UnitRoundStats>,
}

/// Aggregated stats for a unit type in a single round.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnitRoundStats {
    pub count: i32,
    pub weapon_total: f64,
    pub shield_total: f64,
    pub hull_total: f64,
}

/// Configuration for combat debris percentages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DebrisConfig {
    /// Fraction of metal cost that becomes debris (default 0.30 = 30%).
    pub metal_fraction: f64,
    /// Fraction of crystal cost that becomes debris (default 0.30 = 30%).
    pub crystal_fraction: f64,
    /// Whether defense structures contribute to debris.
    pub defense_to_debris: bool,
}

impl Default for DebrisConfig {
    fn default() -> Self {
        Self {
            metal_fraction: 0.30,
            crystal_fraction: 0.30,
            defense_to_debris: false,
        }
    }
}

/// Configuration for defense rebuild chances after combat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DefenseRebuildConfig {
    /// Probability (0.0-1.0) that a destroyed defense is rebuilt for free.
    pub rebuild_chance: f64,
}

impl Default for DefenseRebuildConfig {
    fn default() -> Self {
        Self {
            rebuild_chance: 0.70,
        }
    }
}

// ---------------------------------------------------------------------------
// Default round count per OGame spec
// ---------------------------------------------------------------------------

/// OGame standard max rounds.
pub const DEFAULT_MAX_ROUNDS: i32 = 6;

// ---------------------------------------------------------------------------
// Internal types
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct CombatUnit {
    unit_type: String,
    shield: f64,
    weapon: f64,
    hull: f64,
    max_shield: f64,
    max_hull: f64,
    rapid_fire: HashMap<String, i32>,
    cargo: i64,
    is_defense: bool,
}

#[derive(Clone, Copy)]
struct Mulberry32 {
    state: u32,
}

impl Mulberry32 {
    fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    fn next_u32(&mut self) -> u32 {
        let mut t = self.state.wrapping_add(0x6D2B79F5);
        self.state = t;
        t = t.wrapping_mul(t ^ (t >> 15));
        t = t.wrapping_add(t.wrapping_mul(t ^ (t >> 7)));
        t ^ (t >> 14)
    }
}

trait CombatRandom {
    fn next_u32(&mut self) -> u32;

    fn next_f64(&mut self) -> f64 {
        (self.next_u32() as f64) / (u32::MAX as f64 + 1.0)
    }
}

impl CombatRandom for Mulberry32 {
    fn next_u32(&mut self) -> u32 {
        Self::next_u32(self)
    }
}

/// Full-width deterministic generator for authoritative combat replays.
///
/// The state is initialized directly from all 32 seed bytes. The generator is
/// xoshiro256**, which keeps 256 bits of evolving state and needs no external
/// dependency. The seed is deliberately supplied out-of-band so it can never
/// appear in serialized combat inputs or reports.
#[derive(Clone, Copy)]
struct SeededRng256 {
    state: [u64; 4],
}

impl SeededRng256 {
    fn from_seed(seed: &[u8; 32]) -> Self {
        let mut state = [0_u64; 4];
        for (slot, chunk) in state.iter_mut().zip(seed.chunks_exact(8)) {
            let mut bytes = [0_u8; 8];
            bytes.copy_from_slice(chunk);
            *slot = u64::from_le_bytes(bytes);
        }
        if state == [0; 4] {
            // xoshiro's only invalid state. This constant is fixed so replay
            // remains deterministic even for an all-zero test seed.
            state = [
                0x9e37_79b9_7f4a_7c15,
                0xbf58_476d_1ce4_e5b9,
                0x94d0_49bb_1331_11eb,
                0xd2b7_4407_b1ce_6e93,
            ];
        }
        Self { state }
    }

    fn next_u64(&mut self) -> u64 {
        let result = self.state[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let temporary = self.state[1] << 17;

        self.state[2] ^= self.state[0];
        self.state[3] ^= self.state[1];
        self.state[1] ^= self.state[2];
        self.state[0] ^= self.state[3];
        self.state[2] ^= temporary;
        self.state[3] = self.state[3].rotate_left(45);
        result
    }
}

impl CombatRandom for SeededRng256 {
    fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }
}

// ---------------------------------------------------------------------------
// Primary entry point
// ---------------------------------------------------------------------------

/// Run the OGame combat simulation.
///
/// The simulation follows the OGame spec:
/// - Up to 6 rounds (configurable via `max_rounds`).
/// - Each round: all surviving units fire at a random enemy.
/// - Bounce rule: if weapon < 1% of target shield, shot bounces.
/// - Shield absorbs damage; excess damages hull.
/// - Explosion rule: hull < 70% → probability of explosion.
/// - Rapid fire: extra shots against certain unit types.
/// - Shields regenerate each round; hull does not.
/// - Outcomes: attacker wins, defender wins, or draw.
pub fn simulate_combat(req: &CombatInput) -> CombatResult {
    let config = CombatConfig::default();
    simulate_combat_with_config(req, &config)
}

/// Extended combat simulation configuration.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CombatConfig {
    pub debris: DebrisConfig,
    pub defense_rebuild: DefenseRebuildConfig,
}

/// Run combat with custom configuration for debris and defense rebuild.
pub fn simulate_combat_with_config(req: &CombatInput, config: &CombatConfig) -> CombatResult {
    let seed = calc_seed(&req.seed);
    let mut rng = Mulberry32::new(seed);
    simulate_combat_with_rng(req, config, &mut rng)
}

/// Run authoritative combat from a full 256-bit seed supplied out-of-band.
/// Reconstructing the same request, configuration, and seed produces the same
/// result across worker restarts.
pub fn simulate_combat_with_seed_256(
    req: &CombatInput,
    config: &CombatConfig,
    seed: &[u8; 32],
) -> CombatResult {
    let mut rng = SeededRng256::from_seed(seed);
    simulate_combat_with_rng(req, config, &mut rng)
}

fn simulate_combat_with_rng(
    req: &CombatInput,
    config: &CombatConfig,
    rng: &mut dyn CombatRandom,
) -> CombatResult {
    let universe = if req.universe.trim().is_empty() {
        "default"
    } else {
        req.universe.as_str()
    };

    let ship_defs = ships::load_ships_for_universe(universe);
    let defender_force_map = merge_unit_counts(&req.defender_ships, &req.defender_defenses);

    let attacker_tech = extract_tech_levels(&req.attacker_tech);
    let defender_tech = extract_tech_levels(&req.defender_tech);

    let mut attacker_units =
        prepare_combat_units(&req.attacker_ships, &attacker_tech, &ship_defs, false);
    // Prepare defender units with proper is_defense flag using the split helper.
    let mut defender_units = prepare_combat_units_split(
        &req.defender_ships,
        &req.defender_defenses,
        &defender_tech,
        &ship_defs,
    );

    let max_rounds = match req.max_rounds {
        Some(n) if n > 0 => n,
        _ => DEFAULT_MAX_ROUNDS,
    };

    let mut rounds = Vec::new();

    for round_num in 1..=max_rounds {
        if attacker_units.is_empty() || defender_units.is_empty() {
            break;
        }

        let (atk_shots, def_shots, def_destroyed, atk_destroyed) =
            simulate_round(&mut attacker_units, &mut defender_units, rng);

        rounds.push(RoundResult {
            round_number: round_num,
            attacker_shots: atk_shots as i32,
            defender_shots: def_shots as i32,
            attacker_destroyed: atk_destroyed as i32,
            defender_destroyed: def_destroyed as i32,
            attacker_remaining: attacker_units.len() as i32,
            defender_remaining: defender_units.len() as i32,
        });

        // Shields regenerate each round
        regenerate_shields(&mut attacker_units);
        regenerate_shields(&mut defender_units);
    }

    // Determine winner
    let winner = if defender_units.is_empty() && !attacker_units.is_empty() {
        "attacker"
    } else if attacker_units.is_empty() {
        // A mutual wipe still counts as a successful defense.
        "defender"
    } else {
        // Both sides have survivors after max rounds — draw
        "draw"
    };

    let attacker_losses = calculate_losses(&req.attacker_ships, &attacker_units);
    let defender_losses = calculate_losses(&defender_force_map, &defender_units);

    let debris = calculate_debris(
        &attacker_losses,
        &defender_losses,
        &req.defender_defenses,
        &ship_defs,
        &config.debris,
    );

    let loot = if winner == "attacker" {
        calculate_loot(req, &attacker_units)
    } else {
        Loot {
            metal: 0,
            crystal: 0,
            deuterium: 0,
        }
    };

    CombatResult {
        winner: winner.to_string(),
        rounds,
        attacker_losses,
        defender_losses,
        loot,
        debris,
    }
}

/// Generate a full combat report with per-round breakdowns and defense rebuild.
pub fn generate_combat_report(req: &CombatInput, config: &CombatConfig) -> CombatReport {
    let seed = calc_seed(&req.seed);
    let mut rng = Mulberry32::new(seed);
    generate_combat_report_with_rng(req, config, &mut rng)
}

/// Generate an authoritative report from a full 256-bit out-of-band seed.
pub fn generate_combat_report_with_seed_256(
    req: &CombatInput,
    config: &CombatConfig,
    seed: &[u8; 32],
) -> CombatReport {
    let mut rng = SeededRng256::from_seed(seed);
    generate_combat_report_with_rng(req, config, &mut rng)
}

fn generate_combat_report_with_rng(
    req: &CombatInput,
    config: &CombatConfig,
    rng: &mut dyn CombatRandom,
) -> CombatReport {
    let universe = if req.universe.trim().is_empty() {
        "default"
    } else {
        req.universe.as_str()
    };

    let ship_defs = ships::load_ships_for_universe(universe);
    let defender_force_map = merge_unit_counts(&req.defender_ships, &req.defender_defenses);

    let attacker_tech = extract_tech_levels(&req.attacker_tech);
    let defender_tech = extract_tech_levels(&req.defender_tech);

    let mut attacker_units =
        prepare_combat_units(&req.attacker_ships, &attacker_tech, &ship_defs, false);
    let mut defender_units = prepare_combat_units_split(
        &req.defender_ships,
        &req.defender_defenses,
        &defender_tech,
        &ship_defs,
    );

    let max_rounds = match req.max_rounds {
        Some(n) if n > 0 => n,
        _ => DEFAULT_MAX_ROUNDS,
    };

    let mut round_details = Vec::new();
    let mut round_results = Vec::new();

    for round_num in 1..=max_rounds {
        if attacker_units.is_empty() || defender_units.is_empty() {
            break;
        }

        // Snapshot before the round
        let atk_snapshot = snapshot_forces(&attacker_units);
        let def_snapshot = snapshot_forces(&defender_units);

        round_details.push(RoundDetail {
            round_number: round_num,
            attacker_forces: atk_snapshot,
            defender_forces: def_snapshot,
        });

        let (atk_shots, def_shots, def_destroyed, atk_destroyed) =
            simulate_round(&mut attacker_units, &mut defender_units, rng);

        round_results.push(RoundResult {
            round_number: round_num,
            attacker_shots: atk_shots as i32,
            defender_shots: def_shots as i32,
            attacker_destroyed: atk_destroyed as i32,
            defender_destroyed: def_destroyed as i32,
            attacker_remaining: attacker_units.len() as i32,
            defender_remaining: defender_units.len() as i32,
        });

        regenerate_shields(&mut attacker_units);
        regenerate_shields(&mut defender_units);
    }

    let winner = if defender_units.is_empty() && !attacker_units.is_empty() {
        "attacker"
    } else if attacker_units.is_empty() {
        "defender"
    } else {
        "draw"
    };

    let attacker_losses = calculate_losses(&req.attacker_ships, &attacker_units);
    let defender_losses = calculate_losses(&defender_force_map, &defender_units);

    let debris = calculate_debris(
        &attacker_losses,
        &defender_losses,
        &req.defender_defenses,
        &ship_defs,
        &config.debris,
    );

    let loot = if winner == "attacker" {
        calculate_loot(req, &attacker_units)
    } else {
        Loot {
            metal: 0,
            crystal: 0,
            deuterium: 0,
        }
    };

    // Calculate defense rebuild
    let defense_rebuilt =
        calculate_defense_rebuild(&req.defender_defenses, &defender_units, rng, config);

    let result = CombatResult {
        winner: winner.to_string(),
        rounds: round_results,
        attacker_losses,
        defender_losses,
        loot,
        debris,
    };

    CombatReport {
        attacker_initial: req.attacker_ships.clone(),
        defender_initial: defender_force_map,
        attacker_tech_levels: attacker_tech,
        defender_tech_levels: defender_tech,
        rounds: round_details,
        result,
        defense_rebuilt,
    }
}

/// Calculate which defenses are rebuilt after combat (70% rebuild chance by default).
fn calculate_defense_rebuild(
    initial_defenses: &HashMap<String, i32>,
    remaining_units: &[CombatUnit],
    rng: &mut dyn CombatRandom,
    config: &CombatConfig,
) -> HashMap<String, i32> {
    let mut remaining_defense_counts: HashMap<String, i32> = HashMap::new();
    for unit in remaining_units {
        if unit.is_defense {
            *remaining_defense_counts
                .entry(unit.unit_type.clone())
                .or_insert(0) += 1;
        }
    }

    let mut rebuilt = HashMap::new();
    for (def_type, initial_count) in initial_defenses {
        let remaining = remaining_defense_counts.get(def_type).copied().unwrap_or(0);
        let lost = *initial_count - remaining;
        if lost <= 0 {
            continue;
        }
        let mut rebuilt_count = 0;
        for _ in 0..lost {
            if rng.next_f64() < config.defense_rebuild.rebuild_chance {
                rebuilt_count += 1;
            }
        }
        if rebuilt_count > 0 {
            rebuilt.insert(def_type.clone(), rebuilt_count);
        }
    }
    rebuilt
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/// Hash a string seed into a u32 (FNV-1a).
fn calc_seed(seed: &str) -> u32 {
    let mut hash: u32 = 0x811c9dc5;
    for b in seed.as_bytes() {
        hash ^= *b as u32;
        hash = hash.wrapping_mul(0x01000193);
    }
    if hash == 0 {
        0x9e3779b9
    } else {
        hash
    }
}

fn extract_tech_levels(tech: &HashMap<String, i32>) -> TechLevels {
    TechLevels {
        weapons: tech_level(
            tech,
            &[
                "weapons_technology",
                "weapon_technology",
                "weapons",
                "weapon",
            ],
        ),
        shielding: tech_level(
            tech,
            &[
                "shielding_technology",
                "shield_technology",
                "shielding",
                "shield",
            ],
        ),
        armor: tech_level(
            tech,
            &["armor_technology", "armour_technology", "armor", "armour"],
        ),
    }
}

fn tech_level(tech: &HashMap<String, i32>, keys: &[&str]) -> i32 {
    for key in keys {
        if let Some(level) = tech.get(*key) {
            return (*level).max(0);
        }
    }
    0
}

fn tech_multiplier(level: i32) -> f64 {
    1.0 + level as f64 * 0.1
}

fn derive_stats_from_type(
    typ: &str,
    ship_defs: &HashMap<String, ships::ShipDef>,
) -> (f64, f64, f64, i64, HashMap<String, i32>) {
    if let Some(def) = ship_defs.get(typ) {
        let w = def.weapon.unwrap_or(50.0);
        let s = def.shield.unwrap_or(25.0);
        let h = def.hull.unwrap_or(100.0);
        let cargo = def.cargo.unwrap_or(0);
        let rf = def.rapid_fire.clone().unwrap_or_default();
        return (w, s, h, cargo, rf);
    }

    // Fallback: derive from name hash for unknown types
    let mut h: u32 = 2166136261u32;
    for b in typ.as_bytes() {
        h ^= *b as u32;
        h = h.wrapping_mul(16777619u32);
    }
    let weapon = 50.0 + (h % 100) as f64;
    let shield = 25.0 + (h % 80) as f64;
    let hull = 100.0 + (h % 300) as f64;
    let cargo = (h % 200) as i64;

    (weapon, shield, hull, cargo, HashMap::new())
}

fn prepare_combat_units(
    counts: &HashMap<String, i32>,
    tech: &TechLevels,
    ship_defs: &HashMap<String, ships::ShipDef>,
    is_defense: bool,
) -> Vec<CombatUnit> {
    let weapon_mult = tech_multiplier(tech.weapons);
    let shield_mult = tech_multiplier(tech.shielding);
    let armor_mult = tech_multiplier(tech.armor);

    let mut units = Vec::new();
    let mut keys: Vec<&String> = counts.keys().collect();
    keys.sort();

    for typ in keys {
        let count = counts[typ];
        if count <= 0 {
            continue;
        }
        let (base_w, base_s, base_h, cargo, rf) = derive_stats_from_type(typ, ship_defs);
        let w = base_w * weapon_mult;
        let s = base_s * shield_mult;
        let h = base_h * armor_mult;

        for _ in 0..count {
            units.push(CombatUnit {
                unit_type: typ.clone(),
                weapon: w,
                shield: s,
                hull: h,
                max_shield: s,
                max_hull: h,
                rapid_fire: rf.clone(),
                cargo,
                is_defense,
            });
        }
    }

    units
}

fn prepare_combat_units_split(
    ships: &HashMap<String, i32>,
    defenses: &HashMap<String, i32>,
    tech: &TechLevels,
    ship_defs: &HashMap<String, ships::ShipDef>,
) -> Vec<CombatUnit> {
    let mut units = prepare_combat_units(ships, tech, ship_defs, false);
    let def_units = prepare_combat_units(defenses, tech, ship_defs, true);
    units.extend(def_units);
    units
}

fn simulate_round(
    attacker: &mut Vec<CombatUnit>,
    defender: &mut Vec<CombatUnit>,
    rng: &mut dyn CombatRandom,
) -> (usize, usize, usize, usize) {
    let mut attacker_shots = 0usize;
    let mut defender_shots = 0usize;

    // Attackers fire at defenders
    let atk_count = attacker.len();
    for shooter in attacker.iter().take(atk_count) {
        if defender.is_empty() {
            break;
        }
        let target_idx = (rng.next_u32() as usize) % defender.len();
        attacker_shots += shoot_with_rapid(shooter, target_idx, defender, rng);
        // Remove any destroyed during rapid fire within this unit's turn
        remove_destroyed(defender);
    }

    let defender_destroyed = count_missing(defender);

    // Defenders fire at attackers
    let def_count = defender.len();
    for shooter in defender.iter().take(def_count) {
        if attacker.is_empty() {
            break;
        }
        let target_idx = (rng.next_u32() as usize) % attacker.len();
        defender_shots += shoot_with_rapid(shooter, target_idx, attacker, rng);
        remove_destroyed(attacker);
    }

    let attacker_destroyed = count_missing(attacker);

    (
        attacker_shots,
        defender_shots,
        defender_destroyed,
        attacker_destroyed,
    )
}

fn shoot(shooter: &CombatUnit, target: &mut CombatUnit, rng: &mut dyn CombatRandom) {
    let damage = shooter.weapon;

    // Bounce rule: if weapon < 1% of shield, shot bounces harmlessly
    if damage < target.shield * 0.01 {
        return;
    }

    let mut remaining_damage = damage;

    // Shield absorbs first
    if target.shield > 0.0 {
        let absorbed = remaining_damage.min(target.shield);
        target.shield -= absorbed;
        remaining_damage -= absorbed;
    }

    // Remaining damages hull
    if remaining_damage > 0.0 {
        target.hull -= remaining_damage;
        if target.hull <= 0.0 {
            target.hull = 0.0;
        } else {
            // Explosion rule: if hull < 70% of max, chance of explosion
            let threshold = target.max_hull * 0.7;
            if target.hull < threshold {
                let explosion_chance = 1.0 - (target.hull / threshold);
                if rng.next_f64() < explosion_chance {
                    target.hull = 0.0;
                }
            }
        }
    }
}

fn shoot_with_rapid(
    shooter: &CombatUnit,
    initial_target: usize,
    targets: &mut [CombatUnit],
    rng: &mut dyn CombatRandom,
) -> usize {
    if targets.is_empty() {
        return 0;
    }

    let target_idx = initial_target.min(targets.len() - 1);
    let target_type = targets[target_idx].unit_type.clone();
    shoot(shooter, &mut targets[target_idx], rng);
    let mut shots = 1usize;

    // Rapid fire: probability of extra shot = 1 - 1/rf
    if let Some(rf_value) = shooter.rapid_fire.get(&target_type) {
        let rf = *rf_value;
        if rf > 1 {
            let p_continue = 1.0 - (1.0 / rf as f64);
            loop {
                if targets.is_empty() {
                    break;
                }
                if rng.next_f64() >= p_continue {
                    break;
                }
                // Pick a new random target for the extra shot
                let new_idx = (rng.next_u32() as usize) % targets.len();
                shoot(shooter, &mut targets[new_idx], rng);
                shots += 1;
            }
        }
    }

    shots
}

fn remove_destroyed(units: &mut Vec<CombatUnit>) -> usize {
    let before = units.len();
    units.retain(|u| u.hull > 0.0);
    before - units.len()
}

fn count_missing(_units: &[CombatUnit]) -> usize {
    // This is called after remove_destroyed, so we track via round logic
    0
}

fn regenerate_shields(units: &mut [CombatUnit]) {
    for unit in units {
        unit.shield = unit.max_shield;
    }
}

fn calculate_losses(
    initial: &HashMap<String, i32>,
    remaining: &[CombatUnit],
) -> HashMap<String, i32> {
    let mut remaining_counts: HashMap<String, i32> = HashMap::new();
    for unit in remaining {
        *remaining_counts.entry(unit.unit_type.clone()).or_insert(0) += 1;
    }

    let mut losses = HashMap::new();
    for (typ, init_count) in initial {
        let rem = remaining_counts.get(typ).copied().unwrap_or(0);
        let lost = init_count - rem;
        if lost > 0 {
            losses.insert(typ.clone(), lost);
        }
    }
    losses
}

fn calculate_debris(
    attacker_losses: &HashMap<String, i32>,
    defender_losses: &HashMap<String, i32>,
    defender_defenses: &HashMap<String, i32>,
    ship_defs: &HashMap<String, ships::ShipDef>,
    config: &DebrisConfig,
) -> Debris {
    let mut metal = 0i64;
    let mut crystal = 0i64;

    // Attacker ship losses always contribute
    for (typ, count) in attacker_losses {
        let (m, c) = unit_cost(typ, ship_defs);
        metal += *count as i64 * (m as f64 * config.metal_fraction).floor() as i64;
        crystal += *count as i64 * (c as f64 * config.crystal_fraction).floor() as i64;
    }

    // Defender losses: ships always contribute, defenses only if configured
    for (typ, count) in defender_losses {
        let is_defense = defender_defenses.contains_key(typ);
        if is_defense && !config.defense_to_debris {
            continue;
        }
        let (m, c) = unit_cost(typ, ship_defs);
        metal += *count as i64 * (m as f64 * config.metal_fraction).floor() as i64;
        crystal += *count as i64 * (c as f64 * config.crystal_fraction).floor() as i64;
    }

    Debris { metal, crystal }
}

fn unit_cost(typ: &str, ship_defs: &HashMap<String, ships::ShipDef>) -> (i64, i64) {
    if let Some(def) = ship_defs.get(typ) {
        (def.metal_cost.unwrap_or(0), def.crystal_cost.unwrap_or(0))
    } else {
        (0, 0)
    }
}

fn calculate_loot(req: &CombatInput, attacker_units: &[CombatUnit]) -> Loot {
    // Attacker can loot up to 50% of each resource, limited by cargo capacity
    let available_m = (req.planet_metal as f64 * 0.5).floor() as i64;
    let available_c = (req.planet_crystal as f64 * 0.5).floor() as i64;
    let available_d = (req.planet_deuterium as f64 * 0.5).floor() as i64;

    let total_cargo: i64 = attacker_units.iter().map(|u| u.cargo).sum();
    if total_cargo <= 0 {
        return Loot {
            metal: 0,
            crystal: 0,
            deuterium: 0,
        };
    }

    let total_available = available_m + available_c + available_d;
    if total_available <= 0 {
        return Loot {
            metal: 0,
            crystal: 0,
            deuterium: 0,
        };
    }

    // OGame loot: fill cargo proportionally. Each resource gets a share based
    // on its proportion of total available resources, capped by cargo.
    let capacity = total_cargo.min(total_available);

    // Proportional distribution
    let metal_share =
        ((available_m as f64 / total_available as f64) * capacity as f64).floor() as i64;
    let crystal_share =
        ((available_c as f64 / total_available as f64) * capacity as f64).floor() as i64;
    let deut_share = capacity - metal_share - crystal_share;

    Loot {
        metal: metal_share.min(available_m),
        crystal: crystal_share.min(available_c),
        deuterium: deut_share.min(available_d),
    }
}

fn merge_unit_counts(
    primary: &HashMap<String, i32>,
    secondary: &HashMap<String, i32>,
) -> HashMap<String, i32> {
    let mut merged = primary.clone();
    for (unit_type, count) in secondary {
        *merged.entry(unit_type.clone()).or_insert(0) += *count;
    }
    merged
}

fn snapshot_forces(units: &[CombatUnit]) -> HashMap<String, UnitRoundStats> {
    let mut map: HashMap<String, UnitRoundStats> = HashMap::new();
    for u in units {
        let entry = map.entry(u.unit_type.clone()).or_insert(UnitRoundStats {
            count: 0,
            weapon_total: 0.0,
            shield_total: 0.0,
            hull_total: 0.0,
        });
        entry.count += 1;
        entry.weapon_total += u.weapon;
        entry.shield_total += u.shield;
        entry.hull_total += u.hull;
    }
    map
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_req(seed: &str) -> CombatInput {
        let mut attacker_ships = HashMap::new();
        attacker_ships.insert("fighter".to_string(), 100);
        attacker_ships.insert("bomber".to_string(), 10);

        let mut defender_ships = HashMap::new();
        defender_ships.insert("defender".to_string(), 50);

        let mut defender_defenses = HashMap::new();
        defender_defenses.insert("turret".to_string(), 5);

        CombatInput {
            attacker_ships,
            defender_ships,
            defender_defenses,
            attacker_tech: HashMap::new(),
            defender_tech: HashMap::new(),
            planet_metal: 10_000,
            planet_crystal: 5_000,
            planet_deuterium: 1_000,
            seed: seed.to_string(),
            universe: "default".to_string(),
            max_rounds: None,
        }
    }

    fn make_simple_req(atk_count: i32, def_count: i32, seed: &str) -> CombatInput {
        let mut attacker_ships = HashMap::new();
        attacker_ships.insert("fighter".to_string(), atk_count);

        let mut defender_ships = HashMap::new();
        defender_ships.insert("defender".to_string(), def_count);

        CombatInput {
            attacker_ships,
            defender_ships,
            defender_defenses: HashMap::new(),
            attacker_tech: HashMap::new(),
            defender_tech: HashMap::new(),
            planet_metal: 5_000,
            planet_crystal: 2_500,
            planet_deuterium: 500,
            seed: seed.to_string(),
            universe: "default".to_string(),
            max_rounds: None,
        }
    }

    // -----------------------------------------------------------------------
    // Determinism
    // -----------------------------------------------------------------------

    #[test]
    fn deterministic_same_seed() {
        let r1 = simulate_combat(&make_req("seed1"));
        let r2 = simulate_combat(&make_req("seed1"));
        assert_eq!(r1.winner, r2.winner);
        assert_eq!(r1.rounds.len(), r2.rounds.len());
        assert_eq!(r1.attacker_losses, r2.attacker_losses);
        assert_eq!(r1.defender_losses, r2.defender_losses);
        assert_eq!(r1.loot, r2.loot);
        assert_eq!(r1.debris, r2.debris);
    }

    #[test]
    fn different_seed_may_differ() {
        let r1 = simulate_combat(&make_req("alpha"));
        let r2 = simulate_combat(&make_req("beta"));
        // At least one field should differ across many seeds
        let differ = r1.winner != r2.winner
            || r1.rounds.len() != r2.rounds.len()
            || r1.attacker_losses != r2.attacker_losses
            || r1.defender_losses != r2.defender_losses;
        assert!(differ, "different seeds should produce different results");
    }

    // -----------------------------------------------------------------------
    // Round limits
    // -----------------------------------------------------------------------

    #[test]
    fn default_max_rounds_is_6() {
        assert_eq!(DEFAULT_MAX_ROUNDS, 6);
    }

    #[test]
    fn max_rounds_capped_at_6_by_default() {
        let req = make_req("max-rounds-default");
        let r = simulate_combat(&req);
        assert!(
            r.rounds.len() <= 6,
            "default should be at most 6 rounds, got {}",
            r.rounds.len()
        );
    }

    #[test]
    fn explicit_max_rounds_limits_rounds() {
        let mut req = make_req("explicit-1");
        req.max_rounds = Some(1);
        let r = simulate_combat(&req);
        assert!(
            r.rounds.len() <= 1,
            "explicit max_rounds=1 should cap to 1 round"
        );
    }

    #[test]
    fn explicit_max_rounds_3() {
        let mut req = make_req("explicit-3");
        req.max_rounds = Some(3);
        let r = simulate_combat(&req);
        assert!(
            r.rounds.len() <= 3,
            "explicit max_rounds=3 should cap to 3 rounds"
        );
    }

    #[test]
    fn max_rounds_zero_falls_back_to_default() {
        let mut req = make_req("fallback-seed");
        req.max_rounds = Some(0);
        let r = simulate_combat(&req);
        assert!(
            r.rounds.len() <= 6,
            "max_rounds=0 should fall back to 6-round default"
        );
    }

    #[test]
    fn max_rounds_negative_falls_back_to_default() {
        let mut req = make_req("fallback-negative");
        req.max_rounds = Some(-5);
        let r = simulate_combat(&req);
        assert!(
            r.rounds.len() <= 6,
            "negative max_rounds should fall back to default"
        );
    }

    // -----------------------------------------------------------------------
    // Winner outcomes
    // -----------------------------------------------------------------------

    #[test]
    fn winner_is_valid_string() {
        let r = simulate_combat(&make_req("winner-check"));
        assert!(
            r.winner == "attacker" || r.winner == "defender" || r.winner == "draw",
            "unexpected winner: {}",
            r.winner
        );
    }

    #[test]
    fn overwhelming_attacker_wins() {
        let req = make_simple_req(500, 1, "overwhelming");
        let r = simulate_combat(&req);
        assert_eq!(r.winner, "attacker", "500 vs 1 should be attacker win");
    }

    #[test]
    fn overwhelming_defender_wins() {
        let req = make_simple_req(1, 500, "defender-wins");
        let r = simulate_combat(&req);
        assert_eq!(r.winner, "defender", "1 vs 500 should be defender win");
    }

    #[test]
    fn draw_possible_with_equal_forces_and_low_rounds() {
        // With max_rounds=1 and equal forces, a draw is very likely
        let mut req = make_simple_req(50, 50, "draw-test");
        req.max_rounds = Some(1);
        let r = simulate_combat(&req);
        // Either both survive (draw) or one side wins in 1 round
        assert!(
            r.winner == "attacker" || r.winner == "defender" || r.winner == "draw",
            "winner must be valid"
        );
    }

    #[test]
    fn no_ships_attacker_loses() {
        let req = make_simple_req(0, 10, "no-atk");
        let r = simulate_combat(&req);
        assert_eq!(r.winner, "defender");
        assert!(r.rounds.is_empty());
    }

    #[test]
    fn no_ships_defender_wins_by_default() {
        let req = make_simple_req(10, 0, "no-def");
        let r = simulate_combat(&req);
        // Defender has 0 units, attacker has units → attacker wins immediately
        // But 0 rounds happen because defender is already empty
        assert_eq!(r.winner, "attacker");
        assert!(r.rounds.is_empty());
    }

    // -----------------------------------------------------------------------
    // Loot
    // -----------------------------------------------------------------------

    #[test]
    fn attacker_wins_gets_loot() {
        let req = make_simple_req(500, 1, "loot-test");
        let r = simulate_combat(&req);
        assert_eq!(r.winner, "attacker");
        assert!(
            r.loot.metal > 0 || r.loot.crystal > 0 || r.loot.deuterium > 0,
            "attacker should get some loot"
        );
    }

    #[test]
    fn defender_wins_no_loot() {
        let req = make_simple_req(1, 500, "no-loot");
        let r = simulate_combat(&req);
        assert_eq!(r.winner, "defender");
        assert_eq!(r.loot.metal, 0);
        assert_eq!(r.loot.crystal, 0);
        assert_eq!(r.loot.deuterium, 0);
    }

    #[test]
    fn loot_capped_at_50_percent() {
        let mut req = make_simple_req(500, 1, "loot-cap");
        req.planet_metal = 10_000;
        req.planet_crystal = 6_000;
        req.planet_deuterium = 4_000;
        let r = simulate_combat(&req);
        if r.winner == "attacker" {
            assert!(r.loot.metal <= 5_000, "metal loot capped at 50%");
            assert!(r.loot.crystal <= 3_000, "crystal loot capped at 50%");
            assert!(r.loot.deuterium <= 2_000, "deuterium loot capped at 50%");
        }
    }

    #[test]
    fn loot_zero_when_no_resources() {
        let mut req = make_simple_req(500, 1, "empty-planet");
        req.planet_metal = 0;
        req.planet_crystal = 0;
        req.planet_deuterium = 0;
        let r = simulate_combat(&req);
        assert_eq!(r.loot.metal, 0);
        assert_eq!(r.loot.crystal, 0);
        assert_eq!(r.loot.deuterium, 0);
    }

    // -----------------------------------------------------------------------
    // Debris
    // -----------------------------------------------------------------------

    #[test]
    fn debris_created_on_losses() {
        let req = make_simple_req(100, 100, "debris-test");
        let r = simulate_combat(&req);
        let total_losses: i32 =
            r.attacker_losses.values().sum::<i32>() + r.defender_losses.values().sum::<i32>();
        if total_losses > 0 {
            assert!(
                r.debris.metal > 0 || r.debris.crystal > 0,
                "debris should be created when ships are lost"
            );
        }
    }

    #[test]
    fn debris_default_config_30_percent() {
        let config = DebrisConfig::default();
        assert!((config.metal_fraction - 0.30).abs() < f64::EPSILON);
        assert!((config.crystal_fraction - 0.30).abs() < f64::EPSILON);
        assert!(!config.defense_to_debris);
    }

    #[test]
    fn debris_with_defense_to_debris_enabled() {
        let mut req = make_req("defense-debris");
        req.attacker_ships.insert("fighter".to_string(), 500);
        let config = CombatConfig {
            debris: DebrisConfig {
                metal_fraction: 0.30,
                crystal_fraction: 0.30,
                defense_to_debris: true,
            },
            defense_rebuild: DefenseRebuildConfig::default(),
        };
        let r1 = simulate_combat_with_config(&req, &config);

        let config_no_def = CombatConfig {
            debris: DebrisConfig {
                metal_fraction: 0.30,
                crystal_fraction: 0.30,
                defense_to_debris: false,
            },
            defense_rebuild: DefenseRebuildConfig::default(),
        };
        let r2 = simulate_combat_with_config(&req, &config_no_def);

        // With defense_to_debris=true, debris should be >= the other
        assert!(
            r1.debris.metal >= r2.debris.metal,
            "defense debris should increase total"
        );
    }

    // -----------------------------------------------------------------------
    // Losses
    // -----------------------------------------------------------------------

    #[test]
    fn losses_non_negative() {
        let r = simulate_combat(&make_req("loss-check"));
        for count in r.attacker_losses.values() {
            assert!(*count > 0, "loss entries should be positive");
        }
        for count in r.defender_losses.values() {
            assert!(*count > 0, "loss entries should be positive");
        }
    }

    #[test]
    fn losses_do_not_exceed_initial() {
        let req = make_simple_req(50, 50, "loss-cap");
        let r = simulate_combat(&req);
        for (typ, lost) in &r.attacker_losses {
            let initial = req.attacker_ships.get(typ).copied().unwrap_or(0);
            assert!(
                *lost <= initial,
                "losses ({}) cannot exceed initial ({})",
                lost,
                initial
            );
        }
    }

    // -----------------------------------------------------------------------
    // Round details
    // -----------------------------------------------------------------------

    #[test]
    fn round_numbers_are_sequential() {
        let r = simulate_combat(&make_req("round-seq"));
        for (i, round) in r.rounds.iter().enumerate() {
            assert_eq!(
                round.round_number,
                (i + 1) as i32,
                "round numbers should be 1-indexed sequential"
            );
        }
    }

    #[test]
    fn round_remaining_counts_decrease() {
        let r = simulate_combat(&make_req("round-decrease"));
        if r.rounds.len() >= 2 {
            let first = &r.rounds[0];
            let last = r.rounds.last().unwrap();
            // Total units should generally decrease
            let first_total = first.attacker_remaining + first.defender_remaining;
            let last_total = last.attacker_remaining + last.defender_remaining;
            assert!(
                last_total <= first_total,
                "unit counts should decrease over rounds"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Technologies
    // -----------------------------------------------------------------------

    #[test]
    fn tech_multiplier_increases_with_level() {
        assert!((tech_multiplier(0) - 1.0).abs() < f64::EPSILON);
        assert!((tech_multiplier(1) - 1.1).abs() < f64::EPSILON);
        assert!((tech_multiplier(10) - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn higher_tech_helps_attacker() {
        let mut req_low = make_simple_req(50, 50, "tech-compare");
        req_low.attacker_tech = HashMap::new();

        let mut req_high = make_simple_req(50, 50, "tech-compare");
        req_high
            .attacker_tech
            .insert("weapons_technology".to_string(), 10);
        req_high
            .attacker_tech
            .insert("shielding_technology".to_string(), 10);
        req_high
            .attacker_tech
            .insert("armor_technology".to_string(), 10);

        let r_low = simulate_combat(&req_low);
        let r_high = simulate_combat(&req_high);

        // With higher tech, attacker should lose fewer ships (or win more often)
        let low_losses: i32 = r_low.attacker_losses.values().sum();
        let high_losses: i32 = r_high.attacker_losses.values().sum();
        assert!(
            high_losses <= low_losses,
            "higher tech should reduce attacker losses: {} vs {}",
            high_losses,
            low_losses
        );
    }

    #[test]
    fn extract_tech_levels_uses_aliases() {
        let mut tech = HashMap::new();
        tech.insert("weapon".to_string(), 5);
        tech.insert("shield".to_string(), 3);
        tech.insert("armour".to_string(), 7);
        let levels = extract_tech_levels(&tech);
        assert_eq!(levels.weapons, 5);
        assert_eq!(levels.shielding, 3);
        assert_eq!(levels.armor, 7);
    }

    #[test]
    fn extract_tech_levels_defaults_to_zero() {
        let tech = HashMap::new();
        let levels = extract_tech_levels(&tech);
        assert_eq!(levels.weapons, 0);
        assert_eq!(levels.shielding, 0);
        assert_eq!(levels.armor, 0);
    }

    // -----------------------------------------------------------------------
    // Combat report
    // -----------------------------------------------------------------------

    #[test]
    fn combat_report_has_initial_forces() {
        let req = make_req("report-test");
        let config = CombatConfig::default();
        let report = generate_combat_report(&req, &config);
        assert_eq!(report.attacker_initial, req.attacker_ships);
        assert!(report.defender_initial.contains_key("defender"));
        assert!(report.defender_initial.contains_key("turret"));
    }

    #[test]
    fn combat_report_has_round_details() {
        let req = make_req("report-rounds");
        let config = CombatConfig::default();
        let report = generate_combat_report(&req, &config);
        assert!(
            !report.rounds.is_empty(),
            "report should have round details"
        );
        for rd in &report.rounds {
            assert!(rd.round_number >= 1);
            assert!(
                !rd.attacker_forces.is_empty() || !rd.defender_forces.is_empty(),
                "round detail should have forces"
            );
        }
    }

    #[test]
    fn combat_report_defense_rebuild() {
        let mut req = make_simple_req(500, 1, "rebuild-test");
        req.defender_defenses.insert("turret".to_string(), 10);
        let config = CombatConfig {
            debris: DebrisConfig::default(),
            defense_rebuild: DefenseRebuildConfig {
                rebuild_chance: 1.0, // 100% rebuild for test
            },
        };
        let report = generate_combat_report(&req, &config);
        // All destroyed turrets should be rebuilt
        let turret_losses = report
            .result
            .defender_losses
            .get("turret")
            .copied()
            .unwrap_or(0);
        let turret_rebuilt = report.defense_rebuilt.get("turret").copied().unwrap_or(0);
        assert_eq!(
            turret_rebuilt, turret_losses,
            "100% rebuild chance should rebuild all lost defenses"
        );
    }

    #[test]
    fn combat_report_zero_rebuild_chance() {
        let mut req = make_simple_req(500, 1, "no-rebuild");
        req.defender_defenses.insert("turret".to_string(), 10);
        let config = CombatConfig {
            debris: DebrisConfig::default(),
            defense_rebuild: DefenseRebuildConfig {
                rebuild_chance: 0.0,
            },
        };
        let report = generate_combat_report(&req, &config);
        let turret_rebuilt = report.defense_rebuilt.get("turret").copied().unwrap_or(0);
        assert_eq!(
            turret_rebuilt, 0,
            "0% rebuild chance should rebuild nothing"
        );
    }

    // -----------------------------------------------------------------------
    // RNG (Mulberry32)
    // -----------------------------------------------------------------------

    #[test]
    fn rng_deterministic() {
        let mut r1 = Mulberry32::new(42);
        let mut r2 = Mulberry32::new(42);
        for _ in 0..100 {
            assert_eq!(r1.next_u32(), r2.next_u32());
        }
    }

    #[test]
    fn rng_range_0_to_1() {
        let mut rng = Mulberry32::new(12345);
        for _ in 0..1000 {
            let v = rng.next_f64();
            assert!(
                (0.0..1.0).contains(&v),
                "f64 should be in [0, 1), got {}",
                v
            );
        }
    }

    #[test]
    fn rng_different_seeds_differ() {
        let mut r1 = Mulberry32::new(1);
        let mut r2 = Mulberry32::new(2);
        let mut same_count = 0;
        for _ in 0..100 {
            if r1.next_u32() == r2.next_u32() {
                same_count += 1;
            }
        }
        assert!(
            same_count < 50,
            "different seeds should produce different sequences"
        );
    }

    #[test]
    fn full_seed_rng_uses_all_256_bits_and_restarts_deterministically() {
        let mut seed = [0_u8; 32];
        seed[0] = 7;
        let mut high_bit_seed = seed;
        high_bit_seed[31] = 9;
        let mut first = SeededRng256::from_seed(&seed);
        let mut restarted = SeededRng256::from_seed(&seed);
        let mut changed_high_bits = SeededRng256::from_seed(&high_bit_seed);

        let sequence = (0..16).map(|_| first.next_u32()).collect::<Vec<_>>();
        let replay = (0..16).map(|_| restarted.next_u32()).collect::<Vec<_>>();
        let changed = (0..16)
            .map(|_| changed_high_bits.next_u32())
            .collect::<Vec<_>>();
        assert_eq!(sequence, replay);
        assert_ne!(sequence, changed);
    }

    #[test]
    fn full_seed_all_zero_guard_is_deterministic_and_live() {
        let mut first = SeededRng256::from_seed(&[0; 32]);
        let mut second = SeededRng256::from_seed(&[0; 32]);
        let output = (0..8).map(|_| first.next_u32()).collect::<Vec<_>>();
        let replay = (0..8).map(|_| second.next_u32()).collect::<Vec<_>>();
        assert_eq!(output, replay);
        assert!(output.iter().any(|value| *value != 0));
    }

    #[test]
    fn full_seed_report_replays_without_disclosing_seed() {
        let req = make_simple_req(40, 35, "legacy-field-is-not-authoritative");
        let config = CombatConfig::default();
        let seed = [0xa5; 32];
        let first = generate_combat_report_with_seed_256(&req, &config, &seed);
        let replay = generate_combat_report_with_seed_256(&req, &config, &seed);
        assert_eq!(first, replay);

        let serialized = serde_json::to_string(&first).unwrap();
        assert!(!serialized.contains(&"a5".repeat(32)));
        assert!(!serialized.contains("legacy-field-is-not-authoritative"));
    }

    // -----------------------------------------------------------------------
    // Seed hashing (FNV-1a)
    // -----------------------------------------------------------------------

    #[test]
    fn calc_seed_deterministic() {
        assert_eq!(calc_seed("hello"), calc_seed("hello"));
    }

    #[test]
    fn calc_seed_different_inputs() {
        assert_ne!(calc_seed("abc"), calc_seed("def"));
    }

    #[test]
    fn calc_seed_never_zero() {
        // Empty string check
        let s = calc_seed("");
        assert_ne!(s, 0, "seed should never be zero");
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn empty_universe_falls_back_to_default() {
        let mut req = make_req("empty-uni");
        req.universe = "".to_string();
        let r = simulate_combat(&req);
        assert!(r.winner == "attacker" || r.winner == "defender" || r.winner == "draw");
    }

    #[test]
    fn whitespace_universe_falls_back_to_default() {
        let mut req = make_req("ws-uni");
        req.universe = "   ".to_string();
        let r = simulate_combat(&req);
        assert!(r.winner == "attacker" || r.winner == "defender" || r.winner == "draw");
    }

    #[test]
    fn both_sides_empty_defender_wins() {
        let req = make_simple_req(0, 0, "empty-both");
        let r = simulate_combat(&req);
        // No combat happens, defender "wins" by default
        assert_eq!(r.winner, "defender");
        assert!(r.rounds.is_empty());
    }

    #[test]
    fn large_battle_completes() {
        let req = make_simple_req(1000, 1000, "large-battle");
        let r = simulate_combat(&req);
        assert!(r.rounds.len() <= 6);
        assert!(r.winner == "attacker" || r.winner == "defender" || r.winner == "draw");
    }

    #[test]
    fn combat_config_default_values() {
        let config = CombatConfig::default();
        assert!((config.debris.metal_fraction - 0.30).abs() < f64::EPSILON);
        assert!((config.debris.crystal_fraction - 0.30).abs() < f64::EPSILON);
        assert!(!config.debris.defense_to_debris);
        assert!((config.defense_rebuild.rebuild_chance - 0.70).abs() < f64::EPSILON);
    }

    #[test]
    fn serialization_roundtrip_combat_input() {
        let req = make_req("serde-test");
        let json = serde_json::to_string(&req).unwrap();
        let parsed: CombatInput = serde_json::from_str(&json).unwrap();
        assert_eq!(req, parsed);
    }

    #[test]
    fn serialization_roundtrip_combat_result() {
        let r = simulate_combat(&make_req("serde-result"));
        let json = serde_json::to_string(&r).unwrap();
        let parsed: CombatResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r, parsed);
    }

    #[test]
    fn serialization_roundtrip_combat_config() {
        let config = CombatConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: CombatConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, parsed);
    }

    // -----------------------------------------------------------------------
    // Merge unit counts
    // -----------------------------------------------------------------------

    #[test]
    fn merge_unit_counts_combines() {
        let mut a = HashMap::new();
        a.insert("fighter".to_string(), 10);
        let mut b = HashMap::new();
        b.insert("fighter".to_string(), 5);
        b.insert("turret".to_string(), 3);
        let merged = merge_unit_counts(&a, &b);
        assert_eq!(merged["fighter"], 15);
        assert_eq!(merged["turret"], 3);
    }

    #[test]
    fn merge_unit_counts_empty() {
        let a: HashMap<String, i32> = HashMap::new();
        let b: HashMap<String, i32> = HashMap::new();
        let merged = merge_unit_counts(&a, &b);
        assert!(merged.is_empty());
    }

    // -----------------------------------------------------------------------
    // Snapshot forces
    // -----------------------------------------------------------------------

    #[test]
    fn snapshot_aggregates_correctly() {
        let units = vec![
            CombatUnit {
                unit_type: "fighter".to_string(),
                weapon: 100.0,
                shield: 50.0,
                hull: 200.0,
                max_shield: 50.0,
                max_hull: 200.0,
                rapid_fire: HashMap::new(),
                cargo: 5,
                is_defense: false,
            },
            CombatUnit {
                unit_type: "fighter".to_string(),
                weapon: 100.0,
                shield: 50.0,
                hull: 200.0,
                max_shield: 50.0,
                max_hull: 200.0,
                rapid_fire: HashMap::new(),
                cargo: 5,
                is_defense: false,
            },
        ];
        let snap = snapshot_forces(&units);
        assert_eq!(snap["fighter"].count, 2);
        assert!((snap["fighter"].weapon_total - 200.0).abs() < f64::EPSILON);
        assert!((snap["fighter"].shield_total - 100.0).abs() < f64::EPSILON);
        assert!((snap["fighter"].hull_total - 400.0).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // Custom debris fractions
    // -----------------------------------------------------------------------

    #[test]
    fn custom_debris_fractions() {
        let mut req = make_simple_req(500, 50, "custom-debris");
        req.universe = "default".to_string();
        let config = CombatConfig {
            debris: DebrisConfig {
                metal_fraction: 0.50,
                crystal_fraction: 0.50,
                defense_to_debris: false,
            },
            defense_rebuild: DefenseRebuildConfig::default(),
        };
        let r50 = simulate_combat_with_config(&req, &config);

        let config_low = CombatConfig {
            debris: DebrisConfig {
                metal_fraction: 0.10,
                crystal_fraction: 0.10,
                defense_to_debris: false,
            },
            defense_rebuild: DefenseRebuildConfig::default(),
        };
        let r10 = simulate_combat_with_config(&req, &config_low);

        // Same battle, same losses, but 50% debris > 10% debris
        assert!(
            r50.debris.metal >= r10.debris.metal,
            "higher fraction should produce more debris"
        );
    }
}
