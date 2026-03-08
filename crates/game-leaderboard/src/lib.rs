#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

// ---------------------------------------------------------------------------
// ScoreCategory
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScoreCategory {
    Total,
    Economy,
    Research,
    Military,
    MilitaryBuilt,
    MilitaryDestroyed,
    MilitaryLost,
    Honor,
}

impl fmt::Display for ScoreCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ScoreCategory::Total => "Total",
            ScoreCategory::Economy => "Economy",
            ScoreCategory::Research => "Research",
            ScoreCategory::Military => "Military",
            ScoreCategory::MilitaryBuilt => "MilitaryBuilt",
            ScoreCategory::MilitaryDestroyed => "MilitaryDestroyed",
            ScoreCategory::MilitaryLost => "MilitaryLost",
            ScoreCategory::Honor => "Honor",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for ScoreCategory {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Total" => Ok(ScoreCategory::Total),
            "Economy" => Ok(ScoreCategory::Economy),
            "Research" => Ok(ScoreCategory::Research),
            "Military" => Ok(ScoreCategory::Military),
            "MilitaryBuilt" => Ok(ScoreCategory::MilitaryBuilt),
            "MilitaryDestroyed" => Ok(ScoreCategory::MilitaryDestroyed),
            "MilitaryLost" => Ok(ScoreCategory::MilitaryLost),
            "Honor" => Ok(ScoreCategory::Honor),
            _ => Err(format!("unknown score category: {}", s)),
        }
    }
}

// ---------------------------------------------------------------------------
// Score Calculation Functions
// ---------------------------------------------------------------------------

/// Sum of all building costs (metal + crystal + deuterium) across all levels, divided by 1000.
pub fn calculate_economy_score(
    buildings: &HashMap<String, i32>,
    building_costs: impl Fn(&str, i32) -> (i64, i64, i64),
) -> i64 {
    let mut total: i64 = 0;
    for (name, &level) in buildings {
        for lvl in 1..=level {
            let (metal, crystal, deuterium) = building_costs(name, lvl);
            total += metal + crystal + deuterium;
        }
    }
    total / 1000
}

/// Sum of all research costs (metal + crystal + deuterium) across all levels, divided by 1000.
pub fn calculate_research_score(
    technologies: &HashMap<String, i32>,
    research_costs: impl Fn(&str, i32) -> (i64, i64, i64),
) -> i64 {
    let mut total: i64 = 0;
    for (name, &level) in technologies {
        for lvl in 1..=level {
            let (metal, crystal, deuterium) = research_costs(name, lvl);
            total += metal + crystal + deuterium;
        }
    }
    total / 1000
}

/// Military score for units built: sum of per-unit costs for ships and defenses, divided by 1000.
pub fn calculate_military_score_built(
    ships: &HashMap<String, i32>,
    defenses: &HashMap<String, i32>,
    unit_costs: impl Fn(&str) -> (i64, i64, i64),
) -> i64 {
    let mut total: i64 = 0;
    for (name, &count) in ships.iter().chain(defenses.iter()) {
        let (metal, crystal, deuterium) = unit_costs(name);
        total += (metal + crystal + deuterium) * count as i64;
    }
    total / 1000
}

/// Military score for units destroyed: sum of per-unit costs for destroyed units, divided by 1000.
pub fn calculate_military_score_destroyed(
    destroyed_units: &HashMap<String, i32>,
    unit_costs: impl Fn(&str) -> (i64, i64, i64),
) -> i64 {
    let mut total: i64 = 0;
    for (name, &count) in destroyed_units {
        let (metal, crystal, deuterium) = unit_costs(name);
        total += (metal + crystal + deuterium) * count as i64;
    }
    total / 1000
}

/// Military score for units lost: sum of per-unit costs for lost units, divided by 1000.
pub fn calculate_military_score_lost(
    lost_units: &HashMap<String, i32>,
    unit_costs: impl Fn(&str) -> (i64, i64, i64),
) -> i64 {
    let mut total: i64 = 0;
    for (name, &count) in lost_units {
        let (metal, crystal, deuterium) = unit_costs(name);
        total += (metal + crystal + deuterium) * count as i64;
    }
    total / 1000
}

/// Honor score = net wins = (attacks_won + defenses_won) - attacks_lost.
pub fn calculate_honor_score(attacks_won: i32, defenses_won: i32, attacks_lost: i32) -> i64 {
    (attacks_won + defenses_won - attacks_lost) as i64
}

/// Total score = economy + research + military_built.
pub fn calculate_total_score(economy: i64, research: i64, military_built: i64) -> i64 {
    economy + research + military_built
}

// ---------------------------------------------------------------------------
// Ranking Structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerRanking {
    pub player_id: i64,
    pub player_name: String,
    pub alliance_tag: Option<String>,
    pub score: i64,
    pub rank: i32,
    pub previous_rank: Option<i32>,
    pub rank_change: i32,
    pub last_updated: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AllianceRanking {
    pub alliance_id: i64,
    pub alliance_tag: String,
    pub alliance_name: String,
    pub total_score: i64,
    pub average_score: i64,
    pub member_count: i32,
    pub rank: i32,
    pub previous_rank: Option<i32>,
    pub rank_change: i32,
}

// ---------------------------------------------------------------------------
// Internal Player Entry (per-category data stored in LeaderboardStore)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct PlayerEntry {
    player_id: i64,
    player_name: String,
    alliance_tag: Option<String>,
    scores: HashMap<ScoreCategory, i64>,
    rank: HashMap<ScoreCategory, i32>,
    previous_rank: HashMap<ScoreCategory, Option<i32>>,
    last_updated: String,
}

// ---------------------------------------------------------------------------
// LeaderboardStore
// ---------------------------------------------------------------------------

/// Recalculation interval in seconds (1 hour).
pub const RECALCULATION_INTERVAL_SECS: i64 = 3600;

/// Returns true if a recalculation should occur based on the interval.
pub fn should_recalculate(last_recalc: i64, now: i64) -> bool {
    now - last_recalc >= RECALCULATION_INTERVAL_SECS
}

#[derive(Debug, Clone)]
pub struct LeaderboardStore {
    players: HashMap<i64, PlayerEntry>,
    alliances: Vec<AllianceRanking>,
    /// Per-player score history: player_id -> Vec<(timestamp, score)>
    score_history: HashMap<i64, Vec<(String, i64)>>,
}

impl LeaderboardStore {
    pub fn new() -> Self {
        Self {
            players: HashMap::new(),
            alliances: Vec::new(),
            score_history: HashMap::new(),
        }
    }

    /// Insert or update a player's score for a given category.
    pub fn update_player_score(
        &mut self,
        player_id: i64,
        player_name: &str,
        alliance_tag: Option<&str>,
        category: ScoreCategory,
        score: i64,
    ) {
        let entry = self
            .players
            .entry(player_id)
            .or_insert_with(|| PlayerEntry {
                player_id,
                player_name: player_name.to_string(),
                alliance_tag: alliance_tag.map(|s| s.to_string()),
                scores: HashMap::new(),
                rank: HashMap::new(),
                previous_rank: HashMap::new(),
                last_updated: String::new(),
            });
        entry.player_name = player_name.to_string();
        entry.alliance_tag = alliance_tag.map(|s| s.to_string());
        entry.scores.insert(category, score);

        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        entry.last_updated = format!("unix:{}", ts);
    }

    /// Get a player's score for a given category.
    pub fn get_player_score(&self, player_id: i64, category: ScoreCategory) -> Option<i64> {
        self.players
            .get(&player_id)
            .and_then(|e| e.scores.get(&category).copied())
    }

    /// Get player rankings for a category, sorted by score descending, with pagination.
    pub fn get_player_rankings(
        &self,
        category: ScoreCategory,
        offset: usize,
        limit: usize,
    ) -> Vec<PlayerRanking> {
        let mut entries: Vec<&PlayerEntry> = self
            .players
            .values()
            .filter(|e| e.scores.contains_key(&category))
            .collect();

        entries.sort_by(|a, b| {
            let sa = a.scores.get(&category).copied().unwrap_or(0);
            let sb = b.scores.get(&category).copied().unwrap_or(0);
            sb.cmp(&sa).then_with(|| a.player_id.cmp(&b.player_id))
        });

        entries
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|e| self.to_player_ranking(e, category))
            .collect()
    }

    /// Get a specific player's rank in a category.
    pub fn get_player_rank(&self, player_id: i64, category: ScoreCategory) -> Option<i32> {
        self.players
            .get(&player_id)
            .and_then(|e| e.rank.get(&category).copied())
    }

    /// Recalculate ranks for a single category. Ranks are 1-based, ties share the same rank.
    pub fn recalculate_ranks(&mut self, category: ScoreCategory) {
        let mut entries: Vec<(i64, i64)> = self
            .players
            .values()
            .filter_map(|e| e.scores.get(&category).map(|&score| (e.player_id, score)))
            .collect();

        // Sort by score descending, then by player_id ascending for determinism
        entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        let mut current_rank: i32 = 0;
        let mut last_score: Option<i64> = None;

        for (i, &(player_id, score)) in entries.iter().enumerate() {
            // If score differs from the previous player, assign rank = position + 1
            if last_score != Some(score) {
                current_rank = (i as i32) + 1;
            }
            last_score = Some(score);

            if let Some(entry) = self.players.get_mut(&player_id) {
                let old_rank = entry.rank.get(&category).copied();
                entry.previous_rank.insert(category, old_rank);
                entry.rank.insert(category, current_rank);
            }
        }
    }

    /// Recalculate ranks for all categories.
    pub fn recalculate_all_ranks(&mut self) {
        let categories = [
            ScoreCategory::Total,
            ScoreCategory::Economy,
            ScoreCategory::Research,
            ScoreCategory::Military,
            ScoreCategory::MilitaryBuilt,
            ScoreCategory::MilitaryDestroyed,
            ScoreCategory::MilitaryLost,
            ScoreCategory::Honor,
        ];
        for cat in &categories {
            self.recalculate_ranks(*cat);
        }
    }

    /// Return the top N players for a given category.
    pub fn top_players(&self, category: ScoreCategory, count: usize) -> Vec<PlayerRanking> {
        self.get_player_rankings(category, 0, count)
    }

    /// Get alliance rankings with pagination, sorted by total_score descending.
    pub fn get_alliance_rankings(&self, offset: usize, limit: usize) -> Vec<AllianceRanking> {
        let mut sorted = self.alliances.clone();
        sorted.sort_by(|a, b| {
            b.total_score
                .cmp(&a.total_score)
                .then_with(|| a.alliance_id.cmp(&b.alliance_id))
        });
        sorted.into_iter().skip(offset).take(limit).collect()
    }

    /// Update or insert an alliance ranking from member score data.
    /// `member_scores` is a slice of (player_id, total_score) pairs.
    pub fn update_alliance_ranking(
        &mut self,
        alliance_id: i64,
        alliance_tag: &str,
        alliance_name: &str,
        member_scores: &[(i64, i64)],
    ) {
        let member_count = member_scores.len() as i32;
        let total_score: i64 = member_scores.iter().map(|(_, s)| s).sum();
        let average_score = if member_count > 0 {
            total_score / member_count as i64
        } else {
            0
        };

        if let Some(existing) = self
            .alliances
            .iter_mut()
            .find(|a| a.alliance_id == alliance_id)
        {
            existing.alliance_tag = alliance_tag.to_string();
            existing.alliance_name = alliance_name.to_string();
            existing.previous_rank = Some(existing.rank);
            existing.total_score = total_score;
            existing.average_score = average_score;
            existing.member_count = member_count;
        } else {
            self.alliances.push(AllianceRanking {
                alliance_id,
                alliance_tag: alliance_tag.to_string(),
                alliance_name: alliance_name.to_string(),
                total_score,
                average_score,
                member_count,
                rank: 0,
                previous_rank: None,
                rank_change: 0,
            });
        }

        // Recalculate alliance ranks
        self.recalculate_alliance_ranks();
    }

    /// Search players by name with case-insensitive prefix matching.
    pub fn search_player(&self, query: &str, limit: usize) -> Vec<PlayerRanking> {
        let query_lower = query.to_lowercase();
        let mut matches: Vec<&PlayerEntry> = self
            .players
            .values()
            .filter(|e| e.player_name.to_lowercase().starts_with(&query_lower))
            .collect();

        // Sort by Total score descending as default, then by player_id
        matches.sort_by(|a, b| {
            let sa = a.scores.get(&ScoreCategory::Total).copied().unwrap_or(0);
            let sb = b.scores.get(&ScoreCategory::Total).copied().unwrap_or(0);
            sb.cmp(&sa).then_with(|| a.player_id.cmp(&b.player_id))
        });

        matches
            .into_iter()
            .take(limit)
            .map(|e| self.to_player_ranking(e, ScoreCategory::Total))
            .collect()
    }

    /// Return the score history for a player: Vec<(timestamp, score)>.
    pub fn player_history(&self, player_id: i64) -> Vec<(String, i64)> {
        self.score_history
            .get(&player_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Record a snapshot of all players' Total scores at the given timestamp.
    pub fn record_score_snapshot(&mut self, timestamp: &str) {
        for entry in self.players.values() {
            let score = entry
                .scores
                .get(&ScoreCategory::Total)
                .copied()
                .unwrap_or(0);
            self.score_history
                .entry(entry.player_id)
                .or_default()
                .push((timestamp.to_string(), score));
        }
    }

    /// Return the total number of players that have at least one score entry.
    pub fn total_ranked_players(&self) -> usize {
        self.players.len()
    }

    // -- private helpers --

    fn to_player_ranking(&self, entry: &PlayerEntry, category: ScoreCategory) -> PlayerRanking {
        let rank = entry.rank.get(&category).copied().unwrap_or(0);
        let previous_rank = entry.previous_rank.get(&category).copied().unwrap_or(None);
        let rank_change = previous_rank.map(|pr| pr - rank).unwrap_or(0);

        PlayerRanking {
            player_id: entry.player_id,
            player_name: entry.player_name.clone(),
            alliance_tag: entry.alliance_tag.clone(),
            score: entry.scores.get(&category).copied().unwrap_or(0),
            rank,
            previous_rank,
            rank_change,
            last_updated: entry.last_updated.clone(),
        }
    }

    fn recalculate_alliance_ranks(&mut self) {
        self.alliances.sort_by(|a, b| {
            b.total_score
                .cmp(&a.total_score)
                .then_with(|| a.alliance_id.cmp(&b.alliance_id))
        });

        let mut current_rank: i32 = 0;
        let mut last_score: Option<i64> = None;

        for (i, alliance) in self.alliances.iter_mut().enumerate() {
            if last_score != Some(alliance.total_score) {
                current_rank = (i as i32) + 1;
            }
            last_score = Some(alliance.total_score);

            let old_rank = alliance.previous_rank.unwrap_or(alliance.rank);
            alliance.previous_rank = Some(old_rank);
            alliance.rank = current_rank;
            alliance.rank_change = old_rank - current_rank;
        }
    }
}

impl Default for LeaderboardStore {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_building_costs(name: &str, level: i32) -> (i64, i64, i64) {
        match name {
            "metal_mine" => (60 * level as i64, 15 * level as i64, 0),
            "crystal_mine" => (48 * level as i64, 24 * level as i64, 0),
            "solar_plant" => (75 * level as i64, 30 * level as i64, 0),
            _ => (100 * level as i64, 50 * level as i64, 25 * level as i64),
        }
    }

    fn dummy_research_costs(name: &str, level: i32) -> (i64, i64, i64) {
        match name {
            "energy" => (0, 800 * level as i64, 400 * level as i64),
            "laser" => (200 * level as i64, 100 * level as i64, 0),
            _ => (500 * level as i64, 250 * level as i64, 125 * level as i64),
        }
    }

    fn dummy_unit_costs(name: &str) -> (i64, i64, i64) {
        match name {
            "light_fighter" => (3000, 1000, 0),
            "heavy_fighter" => (6000, 4000, 0),
            "cruiser" => (20000, 7000, 2000),
            "rocket_launcher" => (2000, 0, 0),
            "light_laser" => (1500, 500, 0),
            _ => (1000, 500, 250),
        }
    }

    // -- Score Calculation Tests --

    #[test]
    fn test_economy_score_basic() {
        let mut buildings = HashMap::new();
        buildings.insert("metal_mine".to_string(), 5);
        // metal_mine costs: sum over lvl 1..=5 of (60*lvl + 15*lvl) = 75*(1+2+3+4+5) = 75*15 = 1125
        let score = calculate_economy_score(&buildings, dummy_building_costs);
        assert_eq!(score, 1125 / 1000); // 1
    }

    #[test]
    fn test_economy_score_multiple_buildings() {
        let mut buildings = HashMap::new();
        buildings.insert("metal_mine".to_string(), 10);
        buildings.insert("crystal_mine".to_string(), 10);
        let score = calculate_economy_score(&buildings, dummy_building_costs);
        // metal_mine: sum 1..=10 of 75*lvl = 75 * 55 = 4125
        // crystal_mine: sum 1..=10 of 72*lvl = 72 * 55 = 3960
        // total = 8085, /1000 = 8
        assert_eq!(score, 8);
    }

    #[test]
    fn test_research_score() {
        let mut techs = HashMap::new();
        techs.insert("energy".to_string(), 3);
        // energy: sum 1..=3 of (800*lvl + 400*lvl) = 1200*(1+2+3) = 7200
        let score = calculate_research_score(&techs, dummy_research_costs);
        assert_eq!(score, 7200 / 1000); // 7
    }

    #[test]
    fn test_military_score_built() {
        let mut ships = HashMap::new();
        ships.insert("light_fighter".to_string(), 10);

        let mut defenses = HashMap::new();
        defenses.insert("rocket_launcher".to_string(), 5);

        // light_fighter: (3000+1000+0)*10 = 40000
        // rocket_launcher: (2000+0+0)*5 = 10000
        // total = 50000, /1000 = 50
        let score = calculate_military_score_built(&ships, &defenses, dummy_unit_costs);
        assert_eq!(score, 50);
    }

    #[test]
    fn test_military_score_destroyed() {
        let mut destroyed = HashMap::new();
        destroyed.insert("cruiser".to_string(), 3);
        // cruiser: (20000+7000+2000)*3 = 87000, /1000 = 87
        let score = calculate_military_score_destroyed(&destroyed, dummy_unit_costs);
        assert_eq!(score, 87);
    }

    #[test]
    fn test_military_score_lost() {
        let mut lost = HashMap::new();
        lost.insert("heavy_fighter".to_string(), 2);
        // heavy_fighter: (6000+4000+0)*2 = 20000, /1000 = 20
        let score = calculate_military_score_lost(&lost, dummy_unit_costs);
        assert_eq!(score, 20);
    }

    #[test]
    fn test_honor_score() {
        let score = calculate_honor_score(10, 5, 3);
        assert_eq!(score, 12); // 10 + 5 - 3
    }

    #[test]
    fn test_honor_score_negative() {
        let score = calculate_honor_score(1, 0, 5);
        assert_eq!(score, -4);
    }

    #[test]
    fn test_total_score() {
        let total = calculate_total_score(100, 200, 300);
        assert_eq!(total, 600);
    }

    // -- ScoreCategory Display / FromStr --

    #[test]
    fn test_score_category_display_and_from_str() {
        let categories = [
            ScoreCategory::Total,
            ScoreCategory::Economy,
            ScoreCategory::Research,
            ScoreCategory::Military,
            ScoreCategory::MilitaryBuilt,
            ScoreCategory::MilitaryDestroyed,
            ScoreCategory::MilitaryLost,
            ScoreCategory::Honor,
        ];
        for cat in &categories {
            let s = cat.to_string();
            let parsed: ScoreCategory = s.parse().unwrap();
            assert_eq!(*cat, parsed);
        }

        let err = "Nonsense".parse::<ScoreCategory>();
        assert!(err.is_err());
    }

    // -- LeaderboardStore: Ranking & Ties --

    #[test]
    fn test_rankings_sorted_descending_with_ties() {
        let mut store = LeaderboardStore::new();
        store.update_player_score(1, "Alice", None, ScoreCategory::Total, 5000);
        store.update_player_score(2, "Bob", None, ScoreCategory::Total, 7000);
        store.update_player_score(3, "Charlie", None, ScoreCategory::Total, 7000);
        store.update_player_score(4, "Diana", None, ScoreCategory::Total, 3000);

        store.recalculate_ranks(ScoreCategory::Total);

        // Bob and Charlie tie at rank 1, Alice rank 3, Diana rank 4
        assert_eq!(store.get_player_rank(2, ScoreCategory::Total), Some(1));
        assert_eq!(store.get_player_rank(3, ScoreCategory::Total), Some(1));
        assert_eq!(store.get_player_rank(1, ScoreCategory::Total), Some(3));
        assert_eq!(store.get_player_rank(4, ScoreCategory::Total), Some(4));

        let top = store.top_players(ScoreCategory::Total, 2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].score, 7000);
        assert_eq!(top[1].score, 7000);
    }

    #[test]
    fn test_get_player_rankings_pagination() {
        let mut store = LeaderboardStore::new();
        for i in 1..=10 {
            store.update_player_score(
                i,
                &format!("Player{}", i),
                None,
                ScoreCategory::Economy,
                i * 100,
            );
        }
        store.recalculate_ranks(ScoreCategory::Economy);

        let page = store.get_player_rankings(ScoreCategory::Economy, 2, 3);
        assert_eq!(page.len(), 3);
        // Scores should be descending: 1000, 900, 800, 700, 600, ...
        // offset=2 -> 800, 700, 600
        assert_eq!(page[0].score, 800);
        assert_eq!(page[1].score, 700);
        assert_eq!(page[2].score, 600);
    }

    #[test]
    fn test_rank_change_after_recalculation() {
        let mut store = LeaderboardStore::new();
        store.update_player_score(1, "Alice", None, ScoreCategory::Total, 5000);
        store.update_player_score(2, "Bob", None, ScoreCategory::Total, 3000);
        store.recalculate_ranks(ScoreCategory::Total);

        assert_eq!(store.get_player_rank(1, ScoreCategory::Total), Some(1));
        assert_eq!(store.get_player_rank(2, ScoreCategory::Total), Some(2));

        // Bob overtakes Alice
        store.update_player_score(2, "Bob", None, ScoreCategory::Total, 9000);
        store.recalculate_ranks(ScoreCategory::Total);

        assert_eq!(store.get_player_rank(2, ScoreCategory::Total), Some(1));
        assert_eq!(store.get_player_rank(1, ScoreCategory::Total), Some(2));

        // Bob improved from rank 2 to rank 1 -> rank_change = 2 - 1 = 1
        let rankings = store.get_player_rankings(ScoreCategory::Total, 0, 10);
        let bob = rankings.iter().find(|r| r.player_id == 2).unwrap();
        assert_eq!(bob.previous_rank, Some(2));
        assert_eq!(bob.rank_change, 1);
    }

    // -- Alliance Rankings --

    #[test]
    fn test_alliance_rankings() {
        let mut store = LeaderboardStore::new();

        store.update_alliance_ranking(100, "AAA", "Alliance Alpha", &[(1, 5000), (2, 3000)]);
        store.update_alliance_ranking(200, "BBB", "Alliance Beta", &[(3, 10000)]);

        let rankings = store.get_alliance_rankings(0, 10);
        assert_eq!(rankings.len(), 2);
        // Beta has higher total_score
        assert_eq!(rankings[0].alliance_id, 200);
        assert_eq!(rankings[0].total_score, 10000);
        assert_eq!(rankings[0].average_score, 10000);
        assert_eq!(rankings[0].member_count, 1);
        assert_eq!(rankings[0].rank, 1);

        assert_eq!(rankings[1].alliance_id, 100);
        assert_eq!(rankings[1].total_score, 8000);
        assert_eq!(rankings[1].average_score, 4000);
        assert_eq!(rankings[1].member_count, 2);
        assert_eq!(rankings[1].rank, 2);
    }

    // -- Search --

    #[test]
    fn test_search_player_case_insensitive_prefix() {
        let mut store = LeaderboardStore::new();
        store.update_player_score(1, "AlphaWolf", None, ScoreCategory::Total, 1000);
        store.update_player_score(2, "alphaBear", None, ScoreCategory::Total, 2000);
        store.update_player_score(3, "BetaFox", None, ScoreCategory::Total, 5000);

        let results = store.search_player("alpha", 10);
        assert_eq!(results.len(), 2);
        // alphaBear has higher Total score, so comes first
        assert_eq!(results[0].player_id, 2);
        assert_eq!(results[1].player_id, 1);

        let results_empty = store.search_player("gamma", 10);
        assert!(results_empty.is_empty());
    }

    // -- Snapshots / History --

    #[test]
    fn test_record_score_snapshot_and_player_history() {
        let mut store = LeaderboardStore::new();
        store.update_player_score(1, "Alice", None, ScoreCategory::Total, 1000);
        store.update_player_score(2, "Bob", None, ScoreCategory::Total, 2000);

        store.record_score_snapshot("2026-03-08T00:00:00Z");

        store.update_player_score(1, "Alice", None, ScoreCategory::Total, 1500);
        store.record_score_snapshot("2026-03-08T01:00:00Z");

        let history = store.player_history(1);
        assert_eq!(history.len(), 2);
        assert_eq!(history[0], ("2026-03-08T00:00:00Z".to_string(), 1000));
        assert_eq!(history[1], ("2026-03-08T01:00:00Z".to_string(), 1500));

        let bob_history = store.player_history(2);
        assert_eq!(bob_history.len(), 2);
        assert_eq!(bob_history[0].1, 2000);
        assert_eq!(bob_history[1].1, 2000);
    }

    // -- Recalculation Schedule --

    #[test]
    fn test_should_recalculate() {
        assert!(!should_recalculate(1000, 1000));
        assert!(!should_recalculate(1000, 2000));
        assert!(!should_recalculate(1000, 4599));
        assert!(should_recalculate(1000, 4600)); // 1000 + 3600
        assert!(should_recalculate(0, 3600));
        assert!(should_recalculate(0, 7200));
    }

    // -- Total Ranked Players --

    #[test]
    fn test_total_ranked_players() {
        let mut store = LeaderboardStore::new();
        assert_eq!(store.total_ranked_players(), 0);

        store.update_player_score(1, "Alice", None, ScoreCategory::Total, 100);
        assert_eq!(store.total_ranked_players(), 1);

        store.update_player_score(2, "Bob", None, ScoreCategory::Economy, 200);
        assert_eq!(store.total_ranked_players(), 2);

        // Updating existing player doesn't increase count
        store.update_player_score(1, "Alice", Some("TAG"), ScoreCategory::Economy, 300);
        assert_eq!(store.total_ranked_players(), 2);
    }

    // -- Recalculate All Ranks --

    #[test]
    fn test_recalculate_all_ranks() {
        let mut store = LeaderboardStore::new();
        store.update_player_score(1, "Alice", None, ScoreCategory::Total, 5000);
        store.update_player_score(1, "Alice", None, ScoreCategory::Economy, 2000);
        store.update_player_score(2, "Bob", None, ScoreCategory::Total, 8000);
        store.update_player_score(2, "Bob", None, ScoreCategory::Economy, 1000);

        store.recalculate_all_ranks();

        assert_eq!(store.get_player_rank(2, ScoreCategory::Total), Some(1));
        assert_eq!(store.get_player_rank(1, ScoreCategory::Total), Some(2));
        assert_eq!(store.get_player_rank(1, ScoreCategory::Economy), Some(1));
        assert_eq!(store.get_player_rank(2, ScoreCategory::Economy), Some(2));
    }

    // -- Empty store edge cases --

    #[test]
    fn test_empty_store_operations() {
        let store = LeaderboardStore::new();
        assert_eq!(store.get_player_score(999, ScoreCategory::Total), None);
        assert_eq!(store.get_player_rank(999, ScoreCategory::Total), None);
        assert!(store.top_players(ScoreCategory::Total, 10).is_empty());
        assert!(store
            .get_player_rankings(ScoreCategory::Total, 0, 10)
            .is_empty());
        assert!(store.get_alliance_rankings(0, 10).is_empty());
        assert!(store.search_player("nobody", 10).is_empty());
        assert!(store.player_history(1).is_empty());
        assert_eq!(store.total_ranked_players(), 0);
    }
}
