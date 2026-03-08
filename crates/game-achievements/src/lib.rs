#![forbid(unsafe_code)]

//! Achievement, badge, reward, and ladder system for Universus.
//!
//! Provides a [`Catalog`] of definitions (achievements, badges, rewards, ladders,
//! hall-of-fame entries) and an [`AchievementStore`] that tracks per-user progress,
//! unlocks, and awards.  Trigger conditions allow automatic evaluation of threshold-
//! based achievements (e.g. "accumulate 1 000 000 metal").

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Achievement Category & Tier
// ---------------------------------------------------------------------------

/// Broad classification for achievements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AchievementCategory {
    Combat,
    Economy,
    Exploration,
    Social,
    Special,
}

/// Rarity / difficulty tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AchievementTier {
    Bronze,
    Silver,
    Gold,
    Platinum,
}

// ---------------------------------------------------------------------------
// Trigger — automatic achievement evaluation
// ---------------------------------------------------------------------------

/// A statistic key that the game engine can report.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TriggerStat {
    /// Total fleets dispatched by the player.
    FleetsDispatched,
    /// Total metal accumulated (lifetime).
    MetalAccumulated,
    /// Total crystal accumulated (lifetime).
    CrystalAccumulated,
    /// Total deuterium accumulated (lifetime).
    DeuteriumAccumulated,
    /// Total combat victories.
    CombatVictories,
    /// Total espionage missions.
    EspionageMissions,
    /// Total planets colonized.
    PlanetsColonized,
    /// Total moons created.
    MoonsCreated,
    /// Highest building level reached.
    MaxBuildingLevel,
    /// Total alliance wars won.
    AllianceWarsWon,
    /// Custom stat identified by name.
    Custom(String),
}

/// A condition that, when met, automatically awards an achievement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TriggerCondition {
    /// The stat to evaluate.
    pub stat: TriggerStat,
    /// The threshold value that must be reached (inclusive).
    pub threshold: i64,
}

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Achievement {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub description: String,
    pub points: i32,
    pub badge_id: Option<i64>,
    pub reward_id: Option<i64>,
    pub is_secret: bool,
    pub category: AchievementCategory,
    pub tier: AchievementTier,
    /// Optional trigger condition for automatic evaluation.
    pub trigger: Option<TriggerCondition>,
    /// Target progress value (e.g. 1_000_000 for "accumulate 1M metal").
    /// When `None`, the achievement is binary (0 or complete).
    pub target_progress: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Badge {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub description: String,
    pub icon_url: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reward {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub description: String,
    pub reward_type: String,
    pub value: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ladder {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub description: String,
    pub start_time: String,
    pub end_time: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HallOfFameEntry {
    pub id: i64,
    pub ladder_id: Option<i64>,
    pub user_id: i64,
    pub achievement_id: Option<i64>,
    pub badge_id: Option<i64>,
    pub reward_id: Option<i64>,
    pub score: Option<i64>,
    pub rank: Option<i64>,
    pub season: Option<String>,
    pub inducted_at: String,
}

// ---------------------------------------------------------------------------
// User-facing views
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserAchievementView {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub description: String,
    pub points: i32,
    pub badge_id: Option<i64>,
    pub reward_id: Option<i64>,
    pub is_secret: bool,
    pub created_at: String,
    /// Current progress value (`None` if not started, `Some(1)` if complete for binary).
    pub progress: Option<i32>,
    /// ISO 8601 timestamp when the achievement was unlocked.
    pub unlocked_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserBadgeView {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub description: String,
    pub icon_url: Option<String>,
    pub created_at: String,
    pub earned_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserRewardView {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub description: String,
    pub reward_type: String,
    pub value: Option<i64>,
    pub created_at: String,
    pub granted_at: Option<String>,
}

// ---------------------------------------------------------------------------
// User summary
// ---------------------------------------------------------------------------

/// Aggregate statistics for a player's achievement progress.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserAchievementSummary {
    pub user_id: i64,
    pub total_achievements: usize,
    pub unlocked_achievements: usize,
    pub total_points_possible: i32,
    pub total_points_earned: i32,
    pub total_badges: usize,
    pub earned_badges: usize,
    pub total_rewards: usize,
    pub granted_rewards: usize,
}

// ---------------------------------------------------------------------------
// Progress record
// ---------------------------------------------------------------------------

/// Internal record of a user's progress toward an achievement.
#[derive(Debug, Clone, PartialEq)]
struct ProgressRecord {
    /// Current progress value.
    current: i64,
    /// Target to reach (if tracking), otherwise 1 for binary.
    target: i64,
    /// Timestamp when unlocked (progress >= target).
    unlocked_at: Option<String>,
}

impl ProgressRecord {
    fn binary_complete(now: &str) -> Self {
        Self {
            current: 1,
            target: 1,
            unlocked_at: Some(now.to_string()),
        }
    }

    fn is_complete(&self) -> bool {
        self.current >= self.target
    }
}

// ---------------------------------------------------------------------------
// Catalog
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Catalog {
    pub achievements: Vec<Achievement>,
    pub badges: Vec<Badge>,
    pub rewards: Vec<Reward>,
    pub ladders: Vec<Ladder>,
    pub hall_of_fame: Vec<HallOfFameEntry>,
}

impl Catalog {
    pub fn empty() -> Self {
        Self {
            achievements: Vec::new(),
            badges: Vec::new(),
            rewards: Vec::new(),
            ladders: Vec::new(),
            hall_of_fame: Vec::new(),
        }
    }

    /// Lookup an achievement by ID.
    pub fn achievement(&self, id: i64) -> Option<&Achievement> {
        self.achievements.iter().find(|a| a.id == id)
    }

    /// Lookup a badge by ID.
    pub fn badge(&self, id: i64) -> Option<&Badge> {
        self.badges.iter().find(|b| b.id == id)
    }

    /// Lookup a reward by ID.
    pub fn reward(&self, id: i64) -> Option<&Reward> {
        self.rewards.iter().find(|r| r.id == id)
    }

    /// Lookup a ladder by ID.
    pub fn ladder(&self, id: i64) -> Option<&Ladder> {
        self.ladders.iter().find(|l| l.id == id)
    }

    /// List achievements filtered by category.
    pub fn achievements_by_category(&self, category: AchievementCategory) -> Vec<&Achievement> {
        self.achievements
            .iter()
            .filter(|a| a.category == category)
            .collect()
    }

    /// List achievements filtered by tier.
    pub fn achievements_by_tier(&self, tier: AchievementTier) -> Vec<&Achievement> {
        self.achievements
            .iter()
            .filter(|a| a.tier == tier)
            .collect()
    }

    /// Get all achievements that have a trigger for the given stat.
    pub fn achievements_with_trigger(&self, stat: &TriggerStat) -> Vec<&Achievement> {
        self.achievements
            .iter()
            .filter(|a| a.trigger.as_ref().map(|t| &t.stat == stat).unwrap_or(false))
            .collect()
    }

    /// Total achievement points in the catalog.
    pub fn total_points(&self) -> i32 {
        self.achievements.iter().map(|a| a.points).sum()
    }

    /// Add a hall-of-fame entry.  Returns the assigned ID.
    pub fn add_hall_of_fame_entry(&mut self, mut entry: HallOfFameEntry) -> i64 {
        let next_id = self.hall_of_fame.iter().map(|e| e.id).max().unwrap_or(0) + 1;
        entry.id = next_id;
        self.hall_of_fame.push(entry);
        next_id
    }

    /// Remove a hall-of-fame entry by ID.  Returns `true` if found.
    pub fn remove_hall_of_fame_entry(&mut self, entry_id: i64) -> bool {
        let before = self.hall_of_fame.len();
        self.hall_of_fame.retain(|e| e.id != entry_id);
        self.hall_of_fame.len() < before
    }

    /// Filter hall-of-fame by ladder (and optionally season).
    pub fn hall_of_fame_by_ladder(
        &self,
        ladder_id: i64,
        season: Option<&str>,
    ) -> Vec<&HallOfFameEntry> {
        self.hall_of_fame
            .iter()
            .filter(|e| {
                e.ladder_id == Some(ladder_id)
                    && season
                        .map(|s| e.season.as_deref() == Some(s))
                        .unwrap_or(true)
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// CatalogBuilder
// ---------------------------------------------------------------------------

/// Fluent builder for constructing a [`Catalog`].
pub struct CatalogBuilder {
    catalog: Catalog,
}

impl CatalogBuilder {
    pub fn new() -> Self {
        Self {
            catalog: Catalog::empty(),
        }
    }

    pub fn achievement(mut self, achievement: Achievement) -> Self {
        self.catalog.achievements.push(achievement);
        self
    }

    pub fn badge(mut self, badge: Badge) -> Self {
        self.catalog.badges.push(badge);
        self
    }

    pub fn reward(mut self, reward: Reward) -> Self {
        self.catalog.rewards.push(reward);
        self
    }

    pub fn ladder(mut self, ladder: Ladder) -> Self {
        self.catalog.ladders.push(ladder);
        self
    }

    pub fn hall_of_fame(mut self, entry: HallOfFameEntry) -> Self {
        self.catalog.hall_of_fame.push(entry);
        self
    }

    pub fn build(self) -> Catalog {
        self.catalog
    }
}

impl Default for CatalogBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// AchievementStore
// ---------------------------------------------------------------------------

/// Tracks per-user achievement progress, badge awards, and reward grants.
#[derive(Debug, Default)]
pub struct AchievementStore {
    /// user_id → achievement_id → progress record.
    user_achievements: HashMap<i64, HashMap<i64, ProgressRecord>>,
    /// user_id → badge_id → earned_at timestamp.
    user_badges: HashMap<i64, HashMap<i64, String>>,
    /// user_id → reward_id → granted_at timestamp.
    user_rewards: HashMap<i64, HashMap<i64, String>>,
}

impl AchievementStore {
    pub fn new() -> Self {
        Self::default()
    }

    // -- list operations ----------------------------------------------------

    pub fn list_user_achievements(
        &self,
        catalog: &Catalog,
        user_id: i64,
    ) -> Vec<UserAchievementView> {
        let records = self.user_achievements.get(&user_id);
        catalog
            .achievements
            .iter()
            .map(|achievement| {
                let record = records.and_then(|m| m.get(&achievement.id));
                UserAchievementView {
                    id: achievement.id,
                    code: achievement.code.clone(),
                    name: achievement.name.clone(),
                    description: achievement.description.clone(),
                    points: achievement.points,
                    badge_id: achievement.badge_id,
                    reward_id: achievement.reward_id,
                    is_secret: achievement.is_secret,
                    created_at: achievement.created_at.clone(),
                    progress: record.map(|r| r.current as i32),
                    unlocked_at: record.and_then(|r| r.unlocked_at.clone()),
                }
            })
            .collect()
    }

    pub fn list_user_badges(&self, catalog: &Catalog, user_id: i64) -> Vec<UserBadgeView> {
        let earned = self.user_badges.get(&user_id);
        catalog
            .badges
            .iter()
            .map(|badge| UserBadgeView {
                id: badge.id,
                code: badge.code.clone(),
                name: badge.name.clone(),
                description: badge.description.clone(),
                icon_url: badge.icon_url.clone(),
                created_at: badge.created_at.clone(),
                earned_at: earned.and_then(|items| items.get(&badge.id).cloned()),
            })
            .collect()
    }

    pub fn list_user_rewards(&self, catalog: &Catalog, user_id: i64) -> Vec<UserRewardView> {
        let granted = self.user_rewards.get(&user_id);
        catalog
            .rewards
            .iter()
            .map(|reward| UserRewardView {
                id: reward.id,
                code: reward.code.clone(),
                name: reward.name.clone(),
                description: reward.description.clone(),
                reward_type: reward.reward_type.clone(),
                value: reward.value,
                created_at: reward.created_at.clone(),
                granted_at: granted.and_then(|items| items.get(&reward.id).cloned()),
            })
            .collect()
    }

    /// List only unlocked achievements for a user.
    pub fn list_unlocked_achievements(
        &self,
        catalog: &Catalog,
        user_id: i64,
    ) -> Vec<UserAchievementView> {
        self.list_user_achievements(catalog, user_id)
            .into_iter()
            .filter(|v| v.unlocked_at.is_some())
            .collect()
    }

    // -- award operations ---------------------------------------------------

    /// Award an achievement (binary — immediately marks complete).
    /// Returns `false` if the achievement ID does not exist in the catalog.
    pub fn award_achievement(
        &mut self,
        catalog: &Catalog,
        user_id: i64,
        achievement_id: i64,
    ) -> bool {
        if catalog.achievement(achievement_id).is_none() {
            return false;
        }
        let now = now_timestamp();
        self.user_achievements
            .entry(user_id)
            .or_default()
            .entry(achievement_id)
            .or_insert_with(|| ProgressRecord::binary_complete(&now));
        true
    }

    /// Award a badge.  Returns `false` if the badge ID does not exist.
    pub fn award_badge(&mut self, catalog: &Catalog, user_id: i64, badge_id: i64) -> bool {
        if catalog.badge(badge_id).is_none() {
            return false;
        }
        self.user_badges
            .entry(user_id)
            .or_default()
            .entry(badge_id)
            .or_insert_with(now_timestamp);
        true
    }

    /// Award a reward.  Returns `false` if the reward ID does not exist.
    pub fn award_reward(&mut self, catalog: &Catalog, user_id: i64, reward_id: i64) -> bool {
        if catalog.reward(reward_id).is_none() {
            return false;
        }
        self.user_rewards
            .entry(user_id)
            .or_default()
            .entry(reward_id)
            .or_insert_with(now_timestamp);
        true
    }

    // -- progress operations ------------------------------------------------

    /// Update progress toward an achievement.  Returns the new progress value.
    ///
    /// If the achievement has a `target_progress`, setting `current >= target`
    /// automatically marks it as unlocked.  For binary achievements (no target),
    /// any call with `value >= 1` completes it.
    pub fn set_progress(
        &mut self,
        catalog: &Catalog,
        user_id: i64,
        achievement_id: i64,
        value: i64,
    ) -> Option<i64> {
        let achievement = catalog.achievement(achievement_id)?;
        let target = achievement.target_progress.unwrap_or(1);
        let now = now_timestamp();
        let record = self
            .user_achievements
            .entry(user_id)
            .or_default()
            .entry(achievement_id)
            .or_insert_with(|| ProgressRecord {
                current: 0,
                target,
                unlocked_at: None,
            });
        record.current = value;
        if record.current >= record.target && record.unlocked_at.is_none() {
            record.unlocked_at = Some(now);
        }
        Some(record.current)
    }

    /// Increment progress by `delta`.  Returns the new progress value.
    pub fn increment_progress(
        &mut self,
        catalog: &Catalog,
        user_id: i64,
        achievement_id: i64,
        delta: i64,
    ) -> Option<i64> {
        let achievement = catalog.achievement(achievement_id)?;
        let target = achievement.target_progress.unwrap_or(1);
        let now = now_timestamp();
        let record = self
            .user_achievements
            .entry(user_id)
            .or_default()
            .entry(achievement_id)
            .or_insert_with(|| ProgressRecord {
                current: 0,
                target,
                unlocked_at: None,
            });
        record.current = record.current.saturating_add(delta);
        if record.current >= record.target && record.unlocked_at.is_none() {
            record.unlocked_at = Some(now);
        }
        Some(record.current)
    }

    /// Get current progress for a specific achievement.
    pub fn get_progress(&self, user_id: i64, achievement_id: i64) -> Option<(i64, i64)> {
        self.user_achievements
            .get(&user_id)?
            .get(&achievement_id)
            .map(|r| (r.current, r.target))
    }

    // -- trigger evaluation -------------------------------------------------

    /// Evaluate all trigger-based achievements for a given stat update.
    ///
    /// Returns a list of achievement IDs that were newly unlocked.
    pub fn evaluate_triggers(
        &mut self,
        catalog: &Catalog,
        user_id: i64,
        stat: &TriggerStat,
        value: i64,
    ) -> Vec<i64> {
        let candidates = catalog.achievements_with_trigger(stat);
        let mut newly_unlocked = Vec::new();

        for achievement in candidates {
            let trigger = achievement.trigger.as_ref().unwrap();
            let target = achievement.target_progress.unwrap_or(trigger.threshold);
            let now = now_timestamp();

            let record = self
                .user_achievements
                .entry(user_id)
                .or_default()
                .entry(achievement.id)
                .or_insert_with(|| ProgressRecord {
                    current: 0,
                    target,
                    unlocked_at: None,
                });

            let was_complete = record.is_complete();
            record.current = value;
            if value >= trigger.threshold && !was_complete {
                record.unlocked_at = Some(now);
                newly_unlocked.push(achievement.id);
            }
        }

        newly_unlocked
    }

    // -- revoke operations --------------------------------------------------

    /// Revoke an achievement.  Returns `true` if the user had it.
    pub fn revoke_achievement(&mut self, user_id: i64, achievement_id: i64) -> bool {
        self.user_achievements
            .get_mut(&user_id)
            .map(|m| m.remove(&achievement_id).is_some())
            .unwrap_or(false)
    }

    /// Revoke a badge.  Returns `true` if the user had it.
    pub fn revoke_badge(&mut self, user_id: i64, badge_id: i64) -> bool {
        self.user_badges
            .get_mut(&user_id)
            .map(|m| m.remove(&badge_id).is_some())
            .unwrap_or(false)
    }

    /// Revoke a reward.  Returns `true` if the user had it.
    pub fn revoke_reward(&mut self, user_id: i64, reward_id: i64) -> bool {
        self.user_rewards
            .get_mut(&user_id)
            .map(|m| m.remove(&reward_id).is_some())
            .unwrap_or(false)
    }

    // -- summary / stats ----------------------------------------------------

    /// Compute aggregate achievement statistics for a user.
    pub fn user_summary(&self, catalog: &Catalog, user_id: i64) -> UserAchievementSummary {
        let achievements = self.user_achievements.get(&user_id);
        let unlocked_count = achievements
            .map(|m| m.values().filter(|r| r.is_complete()).count())
            .unwrap_or(0);
        let points_earned: i32 = achievements
            .map(|m| {
                m.iter()
                    .filter(|(_, r)| r.is_complete())
                    .filter_map(|(aid, _)| catalog.achievement(*aid))
                    .map(|a| a.points)
                    .sum()
            })
            .unwrap_or(0);

        let earned_badges = self.user_badges.get(&user_id).map(|m| m.len()).unwrap_or(0);
        let granted_rewards = self
            .user_rewards
            .get(&user_id)
            .map(|m| m.len())
            .unwrap_or(0);

        UserAchievementSummary {
            user_id,
            total_achievements: catalog.achievements.len(),
            unlocked_achievements: unlocked_count,
            total_points_possible: catalog.total_points(),
            total_points_earned: points_earned,
            total_badges: catalog.badges.len(),
            earned_badges,
            total_rewards: catalog.rewards.len(),
            granted_rewards,
        }
    }

    /// Check whether a user has unlocked a specific achievement.
    pub fn has_achievement(&self, user_id: i64, achievement_id: i64) -> bool {
        self.user_achievements
            .get(&user_id)
            .and_then(|m| m.get(&achievement_id))
            .map(|r| r.is_complete())
            .unwrap_or(false)
    }

    /// Check whether a user has a specific badge.
    pub fn has_badge(&self, user_id: i64, badge_id: i64) -> bool {
        self.user_badges
            .get(&user_id)
            .map(|m| m.contains_key(&badge_id))
            .unwrap_or(false)
    }

    /// Check whether a user has a specific reward.
    pub fn has_reward(&self, user_id: i64, reward_id: i64) -> bool {
        self.user_rewards
            .get(&user_id)
            .map(|m| m.contains_key(&reward_id))
            .unwrap_or(false)
    }

    /// Total number of distinct users tracked in the store.
    pub fn tracked_users(&self) -> usize {
        let mut users: HashSet<&i64> = HashSet::new();
        users.extend(self.user_achievements.keys());
        users.extend(self.user_badges.keys());
        users.extend(self.user_rewards.keys());
        users.len()
    }
}

// ---------------------------------------------------------------------------
// Default catalog
// ---------------------------------------------------------------------------

/// Returns the built-in OGame-inspired achievement catalog.
pub fn default_catalog() -> Catalog {
    Catalog {
        achievements: vec![
            Achievement {
                id: 1,
                code: "first_fleet".to_string(),
                name: "First Fleet".to_string(),
                description: "Dispatch your first fleet.".to_string(),
                points: 10,
                badge_id: Some(1),
                reward_id: Some(1),
                is_secret: false,
                category: AchievementCategory::Exploration,
                tier: AchievementTier::Bronze,
                trigger: Some(TriggerCondition {
                    stat: TriggerStat::FleetsDispatched,
                    threshold: 1,
                }),
                target_progress: Some(1),
                created_at: "2026-02-13T00:00:00Z".to_string(),
            },
            Achievement {
                id: 2,
                code: "million_metal".to_string(),
                name: "Industrialist".to_string(),
                description: "Accumulate one million metal.".to_string(),
                points: 25,
                badge_id: Some(2),
                reward_id: None,
                is_secret: false,
                category: AchievementCategory::Economy,
                tier: AchievementTier::Silver,
                trigger: Some(TriggerCondition {
                    stat: TriggerStat::MetalAccumulated,
                    threshold: 1_000_000,
                }),
                target_progress: Some(1_000_000),
                created_at: "2026-02-13T00:00:00Z".to_string(),
            },
            Achievement {
                id: 3,
                code: "battle_hardened".to_string(),
                name: "Battle Hardened".to_string(),
                description: "Win 100 combat engagements.".to_string(),
                points: 50,
                badge_id: None,
                reward_id: Some(2),
                is_secret: false,
                category: AchievementCategory::Combat,
                tier: AchievementTier::Gold,
                trigger: Some(TriggerCondition {
                    stat: TriggerStat::CombatVictories,
                    threshold: 100,
                }),
                target_progress: Some(100),
                created_at: "2026-02-13T00:00:00Z".to_string(),
            },
            Achievement {
                id: 4,
                code: "secret_expedition".to_string(),
                name: "Into the Unknown".to_string(),
                description: "Discover the hidden expedition event.".to_string(),
                points: 100,
                badge_id: None,
                reward_id: None,
                is_secret: true,
                category: AchievementCategory::Special,
                tier: AchievementTier::Platinum,
                trigger: None,
                target_progress: None,
                created_at: "2026-02-13T00:00:00Z".to_string(),
            },
            Achievement {
                id: 5,
                code: "alliance_diplomat".to_string(),
                name: "Diplomat".to_string(),
                description: "Win 10 alliance wars through diplomacy.".to_string(),
                points: 30,
                badge_id: None,
                reward_id: None,
                is_secret: false,
                category: AchievementCategory::Social,
                tier: AchievementTier::Silver,
                trigger: Some(TriggerCondition {
                    stat: TriggerStat::AllianceWarsWon,
                    threshold: 10,
                }),
                target_progress: Some(10),
                created_at: "2026-02-13T00:00:00Z".to_string(),
            },
        ],
        badges: vec![
            Badge {
                id: 1,
                code: "cadet".to_string(),
                name: "Cadet".to_string(),
                description: "Issued for first command activity.".to_string(),
                icon_url: Some("/assets/badges/cadet.png".to_string()),
                created_at: "2026-02-13T00:00:00Z".to_string(),
            },
            Badge {
                id: 2,
                code: "industrialist".to_string(),
                name: "Industrialist".to_string(),
                description: "Issued for exceptional production output.".to_string(),
                icon_url: Some("/assets/badges/industrialist.png".to_string()),
                created_at: "2026-02-13T00:00:00Z".to_string(),
            },
            Badge {
                id: 3,
                code: "warlord".to_string(),
                name: "Warlord".to_string(),
                description: "Awarded for combat excellence.".to_string(),
                icon_url: Some("/assets/badges/warlord.png".to_string()),
                created_at: "2026-02-13T00:00:00Z".to_string(),
            },
        ],
        rewards: vec![
            Reward {
                id: 1,
                code: "starter_dm".to_string(),
                name: "Starter Pack".to_string(),
                description: "One-time dark matter grant.".to_string(),
                reward_type: "dark_matter".to_string(),
                value: Some(500),
                created_at: "2026-02-13T00:00:00Z".to_string(),
            },
            Reward {
                id: 2,
                code: "booster".to_string(),
                name: "Production Booster".to_string(),
                description: "Temporary production boost.".to_string(),
                reward_type: "booster".to_string(),
                value: None,
                created_at: "2026-02-13T00:00:00Z".to_string(),
            },
        ],
        ladders: vec![Ladder {
            id: 1,
            code: "season_alpha".to_string(),
            name: "Season Alpha".to_string(),
            description: "Founding season ladder.".to_string(),
            start_time: "2026-01-01T00:00:00Z".to_string(),
            end_time: "2026-03-31T23:59:59Z".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }],
        hall_of_fame: vec![HallOfFameEntry {
            id: 1,
            ladder_id: Some(1),
            user_id: 1,
            achievement_id: Some(1),
            badge_id: Some(1),
            reward_id: Some(1),
            score: Some(4200),
            rank: Some(1),
            season: Some("Alpha".to_string()),
            inducted_at: "2026-02-13T00:00:00Z".to_string(),
        }],
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn now_timestamp() -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("unix:{ts}")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_catalog() -> Catalog {
        default_catalog()
    }

    // -- default catalog --

    #[test]
    fn default_catalog_has_expected_counts() {
        let c = test_catalog();
        assert_eq!(c.achievements.len(), 5);
        assert_eq!(c.badges.len(), 3);
        assert_eq!(c.rewards.len(), 2);
        assert_eq!(c.ladders.len(), 1);
        assert_eq!(c.hall_of_fame.len(), 1);
    }

    #[test]
    fn default_catalog_total_points() {
        let c = test_catalog();
        // 10 + 25 + 50 + 100 + 30 = 215
        assert_eq!(c.total_points(), 215);
    }

    // -- catalog lookups --

    #[test]
    fn catalog_lookup_by_id() {
        let c = test_catalog();
        assert_eq!(c.achievement(1).unwrap().code, "first_fleet");
        assert_eq!(c.badge(2).unwrap().code, "industrialist");
        assert_eq!(c.reward(1).unwrap().code, "starter_dm");
        assert_eq!(c.ladder(1).unwrap().code, "season_alpha");
        assert!(c.achievement(999).is_none());
    }

    #[test]
    fn catalog_filter_by_category() {
        let c = test_catalog();
        let combat = c.achievements_by_category(AchievementCategory::Combat);
        assert_eq!(combat.len(), 1);
        assert_eq!(combat[0].code, "battle_hardened");
    }

    #[test]
    fn catalog_filter_by_tier() {
        let c = test_catalog();
        let silver = c.achievements_by_tier(AchievementTier::Silver);
        assert_eq!(silver.len(), 2);
    }

    #[test]
    fn catalog_trigger_lookup() {
        let c = test_catalog();
        let fleet_triggers = c.achievements_with_trigger(&TriggerStat::FleetsDispatched);
        assert_eq!(fleet_triggers.len(), 1);
        assert_eq!(fleet_triggers[0].id, 1);
    }

    // -- catalog builder --

    #[test]
    fn catalog_builder_produces_valid_catalog() {
        let catalog = CatalogBuilder::new()
            .achievement(Achievement {
                id: 100,
                code: "test".into(),
                name: "Test".into(),
                description: "A test achievement.".into(),
                points: 5,
                badge_id: None,
                reward_id: None,
                is_secret: false,
                category: AchievementCategory::Special,
                tier: AchievementTier::Bronze,
                trigger: None,
                target_progress: None,
                created_at: "2026-01-01T00:00:00Z".into(),
            })
            .badge(Badge {
                id: 100,
                code: "test_badge".into(),
                name: "TB".into(),
                description: "".into(),
                icon_url: None,
                created_at: "2026-01-01T00:00:00Z".into(),
            })
            .build();
        assert_eq!(catalog.achievements.len(), 1);
        assert_eq!(catalog.badges.len(), 1);
        assert!(catalog.rewards.is_empty());
    }

    // -- hall of fame management --

    #[test]
    fn add_and_remove_hall_of_fame_entry() {
        let mut c = test_catalog();
        assert_eq!(c.hall_of_fame.len(), 1);
        let new_id = c.add_hall_of_fame_entry(HallOfFameEntry {
            id: 0, // will be overridden
            ladder_id: Some(1),
            user_id: 42,
            achievement_id: None,
            badge_id: None,
            reward_id: None,
            score: Some(9999),
            rank: Some(2),
            season: Some("Alpha".into()),
            inducted_at: "2026-03-01T00:00:00Z".into(),
        });
        assert_eq!(new_id, 2);
        assert_eq!(c.hall_of_fame.len(), 2);

        assert!(c.remove_hall_of_fame_entry(new_id));
        assert_eq!(c.hall_of_fame.len(), 1);
        assert!(!c.remove_hall_of_fame_entry(999));
    }

    #[test]
    fn hall_of_fame_filter_by_ladder() {
        let c = test_catalog();
        let entries = c.hall_of_fame_by_ladder(1, Some("Alpha"));
        assert_eq!(entries.len(), 1);
        let entries_no_season = c.hall_of_fame_by_ladder(1, None);
        assert_eq!(entries_no_season.len(), 1);
        let entries_wrong = c.hall_of_fame_by_ladder(99, None);
        assert!(entries_wrong.is_empty());
    }

    // -- award achievement / badge / reward --

    #[test]
    fn award_achievement_returns_false_for_invalid_id() {
        let c = test_catalog();
        let mut store = AchievementStore::new();
        assert!(!store.award_achievement(&c, 1, 999));
    }

    #[test]
    fn award_achievement_marks_complete() {
        let c = test_catalog();
        let mut store = AchievementStore::new();
        assert!(store.award_achievement(&c, 1, 1));
        assert!(store.has_achievement(1, 1));

        let views = store.list_user_achievements(&c, 1);
        let first = views.iter().find(|v| v.id == 1).unwrap();
        assert_eq!(first.progress, Some(1));
        assert!(first.unlocked_at.is_some());
    }

    #[test]
    fn award_achievement_is_idempotent() {
        let c = test_catalog();
        let mut store = AchievementStore::new();
        assert!(store.award_achievement(&c, 1, 1));
        let views1 = store.list_user_achievements(&c, 1);
        let ts1 = views1
            .iter()
            .find(|v| v.id == 1)
            .unwrap()
            .unlocked_at
            .clone();

        // Award again — should not change timestamp
        assert!(store.award_achievement(&c, 1, 1));
        let views2 = store.list_user_achievements(&c, 1);
        let ts2 = views2
            .iter()
            .find(|v| v.id == 1)
            .unwrap()
            .unlocked_at
            .clone();
        assert_eq!(ts1, ts2);
    }

    #[test]
    fn award_badge_and_reward() {
        let c = test_catalog();
        let mut store = AchievementStore::new();
        assert!(store.award_badge(&c, 1, 1));
        assert!(store.has_badge(1, 1));
        assert!(!store.has_badge(1, 999));
        assert!(!store.award_badge(&c, 1, 999));

        assert!(store.award_reward(&c, 1, 1));
        assert!(store.has_reward(1, 1));
        assert!(!store.has_reward(1, 999));
        assert!(!store.award_reward(&c, 1, 999));
    }

    #[test]
    fn list_user_badges_shows_earned_timestamp() {
        let c = test_catalog();
        let mut store = AchievementStore::new();
        store.award_badge(&c, 1, 1);
        let views = store.list_user_badges(&c, 1);
        let cadet = views.iter().find(|v| v.id == 1).unwrap();
        assert!(cadet.earned_at.is_some());
        let other = views.iter().find(|v| v.id == 2).unwrap();
        assert!(other.earned_at.is_none());
    }

    #[test]
    fn list_user_rewards_shows_granted_timestamp() {
        let c = test_catalog();
        let mut store = AchievementStore::new();
        store.award_reward(&c, 1, 1);
        let views = store.list_user_rewards(&c, 1);
        let starter = views.iter().find(|v| v.id == 1).unwrap();
        assert!(starter.granted_at.is_some());
        let booster = views.iter().find(|v| v.id == 2).unwrap();
        assert!(booster.granted_at.is_none());
    }

    // -- progress tracking --

    #[test]
    fn set_progress_tracks_partial_completion() {
        let c = test_catalog();
        let mut store = AchievementStore::new();
        // "million_metal" has target_progress = 1_000_000
        let val = store.set_progress(&c, 1, 2, 500_000).unwrap();
        assert_eq!(val, 500_000);
        assert!(!store.has_achievement(1, 2));

        let (current, target) = store.get_progress(1, 2).unwrap();
        assert_eq!(current, 500_000);
        assert_eq!(target, 1_000_000);
    }

    #[test]
    fn set_progress_auto_unlocks_on_target_reached() {
        let c = test_catalog();
        let mut store = AchievementStore::new();
        store.set_progress(&c, 1, 2, 1_000_000);
        assert!(store.has_achievement(1, 2));
    }

    #[test]
    fn increment_progress_accumulates() {
        let c = test_catalog();
        let mut store = AchievementStore::new();
        store.increment_progress(&c, 1, 2, 300_000);
        store.increment_progress(&c, 1, 2, 400_000);
        let (current, _) = store.get_progress(1, 2).unwrap();
        assert_eq!(current, 700_000);
        assert!(!store.has_achievement(1, 2));

        store.increment_progress(&c, 1, 2, 300_001);
        assert!(store.has_achievement(1, 2));
    }

    #[test]
    fn set_progress_returns_none_for_invalid_id() {
        let c = test_catalog();
        let mut store = AchievementStore::new();
        assert!(store.set_progress(&c, 1, 999, 100).is_none());
    }

    #[test]
    fn get_progress_returns_none_when_not_started() {
        let store = AchievementStore::new();
        assert!(store.get_progress(1, 1).is_none());
    }

    // -- trigger evaluation --

    #[test]
    fn evaluate_triggers_unlocks_matching_achievements() {
        let c = test_catalog();
        let mut store = AchievementStore::new();
        let newly = store.evaluate_triggers(&c, 1, &TriggerStat::FleetsDispatched, 1);
        assert_eq!(newly, vec![1]);
        assert!(store.has_achievement(1, 1));
    }

    #[test]
    fn evaluate_triggers_does_not_double_unlock() {
        let c = test_catalog();
        let mut store = AchievementStore::new();
        let first = store.evaluate_triggers(&c, 1, &TriggerStat::FleetsDispatched, 1);
        assert_eq!(first.len(), 1);
        let second = store.evaluate_triggers(&c, 1, &TriggerStat::FleetsDispatched, 5);
        assert!(second.is_empty());
    }

    #[test]
    fn evaluate_triggers_below_threshold_does_not_unlock() {
        let c = test_catalog();
        let mut store = AchievementStore::new();
        let result = store.evaluate_triggers(&c, 1, &TriggerStat::CombatVictories, 50);
        assert!(result.is_empty());
        assert!(!store.has_achievement(1, 3));
    }

    #[test]
    fn evaluate_triggers_at_exact_threshold_unlocks() {
        let c = test_catalog();
        let mut store = AchievementStore::new();
        let result = store.evaluate_triggers(&c, 1, &TriggerStat::CombatVictories, 100);
        assert_eq!(result, vec![3]);
    }

    #[test]
    fn evaluate_triggers_ignores_no_trigger_achievements() {
        let c = test_catalog();
        let mut store = AchievementStore::new();
        // "secret_expedition" (id=4) has no trigger
        let result = store.evaluate_triggers(&c, 1, &TriggerStat::Custom("anything".into()), 999);
        assert!(result.is_empty());
    }

    // -- revoke --

    #[test]
    fn revoke_achievement_removes_it() {
        let c = test_catalog();
        let mut store = AchievementStore::new();
        store.award_achievement(&c, 1, 1);
        assert!(store.has_achievement(1, 1));
        assert!(store.revoke_achievement(1, 1));
        assert!(!store.has_achievement(1, 1));
    }

    #[test]
    fn revoke_returns_false_when_not_present() {
        let mut store = AchievementStore::new();
        assert!(!store.revoke_achievement(1, 1));
        assert!(!store.revoke_badge(1, 1));
        assert!(!store.revoke_reward(1, 1));
    }

    #[test]
    fn revoke_badge_and_reward() {
        let c = test_catalog();
        let mut store = AchievementStore::new();
        store.award_badge(&c, 1, 1);
        store.award_reward(&c, 1, 1);
        assert!(store.revoke_badge(1, 1));
        assert!(!store.has_badge(1, 1));
        assert!(store.revoke_reward(1, 1));
        assert!(!store.has_reward(1, 1));
    }

    // -- list unlocked --

    #[test]
    fn list_unlocked_achievements_filters_correctly() {
        let c = test_catalog();
        let mut store = AchievementStore::new();
        store.award_achievement(&c, 1, 1);
        store.set_progress(&c, 1, 2, 500); // partial, not unlocked
        let unlocked = store.list_unlocked_achievements(&c, 1);
        assert_eq!(unlocked.len(), 1);
        assert_eq!(unlocked[0].id, 1);
    }

    // -- user summary --

    #[test]
    fn user_summary_empty_store() {
        let c = test_catalog();
        let store = AchievementStore::new();
        let summary = store.user_summary(&c, 1);
        assert_eq!(summary.total_achievements, 5);
        assert_eq!(summary.unlocked_achievements, 0);
        assert_eq!(summary.total_points_possible, 215);
        assert_eq!(summary.total_points_earned, 0);
        assert_eq!(summary.total_badges, 3);
        assert_eq!(summary.earned_badges, 0);
        assert_eq!(summary.total_rewards, 2);
        assert_eq!(summary.granted_rewards, 0);
    }

    #[test]
    fn user_summary_with_awards() {
        let c = test_catalog();
        let mut store = AchievementStore::new();
        store.award_achievement(&c, 1, 1); // 10 points
        store.award_achievement(&c, 1, 4); // 100 points (secret)
        store.award_badge(&c, 1, 1);
        store.award_reward(&c, 1, 2);

        let summary = store.user_summary(&c, 1);
        assert_eq!(summary.unlocked_achievements, 2);
        assert_eq!(summary.total_points_earned, 110);
        assert_eq!(summary.earned_badges, 1);
        assert_eq!(summary.granted_rewards, 1);
    }

    // -- tracked users --

    #[test]
    fn tracked_users_counts_distinct_users() {
        let c = test_catalog();
        let mut store = AchievementStore::new();
        assert_eq!(store.tracked_users(), 0);
        store.award_achievement(&c, 1, 1);
        store.award_badge(&c, 2, 1);
        store.award_reward(&c, 3, 1);
        assert_eq!(store.tracked_users(), 3);
        // Same user across different maps still counts once
        store.award_badge(&c, 1, 1);
        assert_eq!(store.tracked_users(), 3);
    }

    // -- list for non-existent user returns defaults --

    #[test]
    fn list_achievements_for_unknown_user_returns_catalog_with_no_progress() {
        let c = test_catalog();
        let store = AchievementStore::new();
        let views = store.list_user_achievements(&c, 999);
        assert_eq!(views.len(), 5);
        for v in &views {
            assert!(v.progress.is_none());
            assert!(v.unlocked_at.is_none());
        }
    }

    #[test]
    fn list_badges_for_unknown_user_returns_catalog_with_no_earned() {
        let c = test_catalog();
        let store = AchievementStore::new();
        let views = store.list_user_badges(&c, 999);
        assert_eq!(views.len(), 3);
        for v in &views {
            assert!(v.earned_at.is_none());
        }
    }

    #[test]
    fn list_rewards_for_unknown_user_returns_catalog_with_no_granted() {
        let c = test_catalog();
        let store = AchievementStore::new();
        let views = store.list_user_rewards(&c, 999);
        assert_eq!(views.len(), 2);
        for v in &views {
            assert!(v.granted_at.is_none());
        }
    }

    // -- secret achievements --

    #[test]
    fn secret_achievement_visible_in_list() {
        let c = test_catalog();
        let store = AchievementStore::new();
        let views = store.list_user_achievements(&c, 1);
        let secret = views.iter().find(|v| v.id == 4).unwrap();
        assert!(secret.is_secret);
    }

    // -- multiple users isolation --

    #[test]
    fn different_users_have_isolated_progress() {
        let c = test_catalog();
        let mut store = AchievementStore::new();
        store.award_achievement(&c, 1, 1);
        store.award_badge(&c, 1, 1);
        assert!(store.has_achievement(1, 1));
        assert!(!store.has_achievement(2, 1));
        assert!(store.has_badge(1, 1));
        assert!(!store.has_badge(2, 1));
    }

    // -- edge cases --

    #[test]
    fn increment_progress_with_zero_delta_is_noop() {
        let c = test_catalog();
        let mut store = AchievementStore::new();
        store.set_progress(&c, 1, 2, 100);
        store.increment_progress(&c, 1, 2, 0);
        let (current, _) = store.get_progress(1, 2).unwrap();
        assert_eq!(current, 100);
    }

    #[test]
    fn set_progress_for_binary_achievement_unlocks_at_one() {
        let c = test_catalog();
        let mut store = AchievementStore::new();
        // "secret_expedition" (id=4) has no target_progress → binary
        store.set_progress(&c, 1, 4, 1);
        assert!(store.has_achievement(1, 4));
    }

    #[test]
    fn overshoot_progress_still_marks_complete() {
        let c = test_catalog();
        let mut store = AchievementStore::new();
        store.set_progress(&c, 1, 2, 2_000_000);
        assert!(store.has_achievement(1, 2));
        let (current, target) = store.get_progress(1, 2).unwrap();
        assert_eq!(current, 2_000_000);
        assert_eq!(target, 1_000_000);
    }

    #[test]
    fn catalog_empty_has_zero_points() {
        let c = Catalog::empty();
        assert_eq!(c.total_points(), 0);
        assert!(c.achievements.is_empty());
    }

    #[test]
    fn user_summary_user_id_matches() {
        let c = test_catalog();
        let store = AchievementStore::new();
        let summary = store.user_summary(&c, 42);
        assert_eq!(summary.user_id, 42);
    }
}
