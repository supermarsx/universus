#![forbid(unsafe_code)]

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub fn crate_name() -> &'static str {
    "game-leaderboard"
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum LeaderboardCategory {
    Overall,
    Fleet,
    Research,
    Buildings,
    Defense,
    Economy,
}

#[derive(Clone, Debug, Serialize)]
pub struct LeaderboardEntry {
    pub rank: i32,
    pub user_id: i64,
    pub username: String,
    pub alliance_tag: Option<String>,
    pub score: i64,
}

pub struct ScoreUpdate {
    pub user_id: i64,
    pub username: String,
    pub alliance_tag: Option<String>,
    pub category: LeaderboardCategory,
    pub score: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct AllianceScore {
    pub alliance_tag: String,
    pub total_score: i64,
    pub member_count: i32,
    pub rank: i32,
}

#[derive(Clone)]
struct PlayerEntry {
    user_id: i64,
    username: String,
    alliance_tag: Option<String>,
    score: i64,
}

pub struct Leaderboard {
    scores: HashMap<LeaderboardCategory, HashMap<i64, PlayerEntry>>,
}

impl Leaderboard {
    pub fn new() -> Self {
        let mut lb = Self {
            scores: HashMap::new(),
        };

        let seed = vec![
            ScoreUpdate {
                user_id: 1,
                username: "CommanderNova".to_string(),
                alliance_tag: Some("NOVA".to_string()),
                category: LeaderboardCategory::Overall,
                score: 125_000,
            },
            ScoreUpdate {
                user_id: 2,
                username: "StarForge".to_string(),
                alliance_tag: Some("NOVA".to_string()),
                category: LeaderboardCategory::Overall,
                score: 98_000,
            },
            ScoreUpdate {
                user_id: 3,
                username: "DarkMatter".to_string(),
                alliance_tag: Some("VOID".to_string()),
                category: LeaderboardCategory::Overall,
                score: 87_500,
            },
            ScoreUpdate {
                user_id: 4,
                username: "IronClad".to_string(),
                alliance_tag: None,
                category: LeaderboardCategory::Overall,
                score: 65_200,
            },
            ScoreUpdate {
                user_id: 1,
                username: "CommanderNova".to_string(),
                alliance_tag: Some("NOVA".to_string()),
                category: LeaderboardCategory::Fleet,
                score: 42_000,
            },
            ScoreUpdate {
                user_id: 3,
                username: "DarkMatter".to_string(),
                alliance_tag: Some("VOID".to_string()),
                category: LeaderboardCategory::Fleet,
                score: 55_000,
            },
        ];

        for update in seed {
            lb.update_score(update);
        }

        lb
    }

    pub fn update_score(&mut self, update: ScoreUpdate) {
        let category_map = self.scores.entry(update.category).or_default();
        let entry = category_map.entry(update.user_id).or_insert_with(|| PlayerEntry {
            user_id: update.user_id,
            username: update.username.clone(),
            alliance_tag: update.alliance_tag.clone(),
            score: 0,
        });
        entry.score = update.score;
        entry.username = update.username;
        entry.alliance_tag = update.alliance_tag;
    }

    pub fn get_rankings(
        &self,
        category: &LeaderboardCategory,
        limit: usize,
    ) -> Vec<LeaderboardEntry> {
        let Some(category_map) = self.scores.get(category) else {
            return Vec::new();
        };

        let mut entries: Vec<&PlayerEntry> = category_map.values().collect();
        entries.sort_by(|a, b| b.score.cmp(&a.score));

        entries
            .into_iter()
            .take(limit)
            .enumerate()
            .map(|(i, pe)| LeaderboardEntry {
                rank: (i + 1) as i32,
                user_id: pe.user_id,
                username: pe.username.clone(),
                alliance_tag: pe.alliance_tag.clone(),
                score: pe.score,
            })
            .collect()
    }

    pub fn get_player_rank(
        &self,
        category: &LeaderboardCategory,
        user_id: i64,
    ) -> Option<LeaderboardEntry> {
        let category_map = self.scores.get(category)?;

        let mut entries: Vec<&PlayerEntry> = category_map.values().collect();
        entries.sort_by(|a, b| b.score.cmp(&a.score));

        entries.iter().enumerate().find_map(|(i, pe)| {
            if pe.user_id == user_id {
                Some(LeaderboardEntry {
                    rank: (i + 1) as i32,
                    user_id: pe.user_id,
                    username: pe.username.clone(),
                    alliance_tag: pe.alliance_tag.clone(),
                    score: pe.score,
                })
            } else {
                None
            }
        })
    }

    pub fn get_top_alliances(&self, limit: usize) -> Vec<AllianceScore> {
        let Some(overall_map) = self.scores.get(&LeaderboardCategory::Overall) else {
            return Vec::new();
        };

        let mut alliance_agg: HashMap<String, (i64, i32)> = HashMap::new();
        for pe in overall_map.values() {
            if let Some(ref tag) = pe.alliance_tag {
                let entry = alliance_agg.entry(tag.clone()).or_insert((0, 0));
                entry.0 += pe.score;
                entry.1 += 1;
            }
        }

        let mut alliances: Vec<(String, i64, i32)> = alliance_agg
            .into_iter()
            .map(|(tag, (total, count))| (tag, total, count))
            .collect();
        alliances.sort_by(|a, b| b.1.cmp(&a.1));

        alliances
            .into_iter()
            .take(limit)
            .enumerate()
            .map(|(i, (tag, total, count))| AllianceScore {
                alliance_tag: tag,
                total_score: total,
                member_count: count,
                rank: (i + 1) as i32,
            })
            .collect()
    }
}

impl Default for Leaderboard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crate_name() {
        assert_eq!(crate_name(), "game-leaderboard");
    }

    #[test]
    fn test_new_leaderboard_has_seeded_entries() {
        let lb = Leaderboard::new();
        let rankings = lb.get_rankings(&LeaderboardCategory::Overall, 10);
        assert_eq!(rankings.len(), 4);
        assert_eq!(rankings[0].username, "CommanderNova");
        assert_eq!(rankings[0].rank, 1);
    }

    #[test]
    fn test_rankings_sorted_descending() {
        let lb = Leaderboard::new();
        let rankings = lb.get_rankings(&LeaderboardCategory::Overall, 10);
        for w in rankings.windows(2) {
            assert!(w[0].score >= w[1].score);
        }
    }

    #[test]
    fn test_rankings_limit() {
        let lb = Leaderboard::new();
        let rankings = lb.get_rankings(&LeaderboardCategory::Overall, 2);
        assert_eq!(rankings.len(), 2);
        assert_eq!(rankings[0].rank, 1);
        assert_eq!(rankings[1].rank, 2);
    }

    #[test]
    fn test_update_score_upserts() {
        let mut lb = Leaderboard::new();
        lb.update_score(ScoreUpdate {
            user_id: 1,
            username: "CommanderNova".to_string(),
            alliance_tag: Some("NOVA".to_string()),
            category: LeaderboardCategory::Overall,
            score: 200_000,
        });
        let entry = lb
            .get_player_rank(&LeaderboardCategory::Overall, 1)
            .unwrap();
        assert_eq!(entry.score, 200_000);
        assert_eq!(entry.rank, 1);
    }

    #[test]
    fn test_update_score_new_player() {
        let mut lb = Leaderboard::new();
        lb.update_score(ScoreUpdate {
            user_id: 99,
            username: "Newcomer".to_string(),
            alliance_tag: None,
            category: LeaderboardCategory::Research,
            score: 5_000,
        });
        let entry = lb
            .get_player_rank(&LeaderboardCategory::Research, 99)
            .unwrap();
        assert_eq!(entry.score, 5_000);
        assert_eq!(entry.username, "Newcomer");
    }

    #[test]
    fn test_get_player_rank_not_found() {
        let lb = Leaderboard::new();
        let result = lb.get_player_rank(&LeaderboardCategory::Overall, 999);
        assert!(result.is_none());
    }

    #[test]
    fn test_get_player_rank_empty_category() {
        let lb = Leaderboard::new();
        let result = lb.get_player_rank(&LeaderboardCategory::Defense, 1);
        assert!(result.is_none());
    }

    #[test]
    fn test_get_top_alliances() {
        let lb = Leaderboard::new();
        let alliances = lb.get_top_alliances(10);
        assert_eq!(alliances.len(), 2);
        assert_eq!(alliances[0].alliance_tag, "NOVA");
        assert_eq!(alliances[0].total_score, 125_000 + 98_000);
        assert_eq!(alliances[0].member_count, 2);
        assert_eq!(alliances[0].rank, 1);
        assert_eq!(alliances[1].alliance_tag, "VOID");
        assert_eq!(alliances[1].rank, 2);
    }

    #[test]
    fn test_get_top_alliances_limit() {
        let lb = Leaderboard::new();
        let alliances = lb.get_top_alliances(1);
        assert_eq!(alliances.len(), 1);
        assert_eq!(alliances[0].alliance_tag, "NOVA");
    }

    #[test]
    fn test_fleet_category_rankings() {
        let lb = Leaderboard::new();
        let rankings = lb.get_rankings(&LeaderboardCategory::Fleet, 10);
        assert_eq!(rankings.len(), 2);
        assert_eq!(rankings[0].username, "DarkMatter");
        assert_eq!(rankings[0].score, 55_000);
    }

    #[test]
    fn test_empty_category_returns_empty() {
        let lb = Leaderboard::new();
        let rankings = lb.get_rankings(&LeaderboardCategory::Buildings, 10);
        assert!(rankings.is_empty());
    }
}
