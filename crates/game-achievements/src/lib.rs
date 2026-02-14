#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};

use serde::Serialize;

#[derive(Clone, Serialize)]
pub struct Achievement {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub description: String,
    pub points: i32,
    pub badge_id: Option<i64>,
    pub reward_id: Option<i64>,
    pub is_secret: bool,
    pub created_at: String,
}

#[derive(Clone, Serialize)]
pub struct Badge {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub description: String,
    pub icon_url: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Serialize)]
pub struct Reward {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub description: String,
    pub reward_type: String,
    pub value: Option<i64>,
    pub created_at: String,
}

#[derive(Clone, Serialize)]
pub struct Ladder {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub description: String,
    pub start_time: String,
    pub end_time: String,
    pub created_at: String,
}

#[derive(Clone, Serialize)]
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

#[derive(Clone, Serialize)]
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
    pub progress: Option<i32>,
    pub unlocked_at: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct UserBadgeView {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub description: String,
    pub icon_url: Option<String>,
    pub created_at: String,
    pub earned_at: Option<String>,
}

#[derive(Clone, Serialize)]
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

#[derive(Clone)]
pub struct Catalog {
    pub achievements: Vec<Achievement>,
    pub badges: Vec<Badge>,
    pub rewards: Vec<Reward>,
    pub ladders: Vec<Ladder>,
    pub hall_of_fame: Vec<HallOfFameEntry>,
}

#[derive(Default)]
pub struct AchievementStore {
    user_achievements: HashMap<i64, HashMap<i64, String>>,
    user_badges: HashMap<i64, HashMap<i64, String>>,
    user_rewards: HashMap<i64, HashMap<i64, String>>,
}

impl AchievementStore {
    pub fn list_user_achievements(&self, catalog: &Catalog, user_id: i64) -> Vec<UserAchievementView> {
        let unlocked = self.user_achievements.get(&user_id);
        catalog
            .achievements
            .iter()
            .map(|achievement| UserAchievementView {
                id: achievement.id,
                code: achievement.code.clone(),
                name: achievement.name.clone(),
                description: achievement.description.clone(),
                points: achievement.points,
                badge_id: achievement.badge_id,
                reward_id: achievement.reward_id,
                is_secret: achievement.is_secret,
                created_at: achievement.created_at.clone(),
                progress: unlocked
                    .and_then(|items| items.get(&achievement.id))
                    .map(|_| 1),
                unlocked_at: unlocked.and_then(|items| items.get(&achievement.id).cloned()),
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

    pub fn award_achievement(&mut self, catalog: &Catalog, user_id: i64, achievement_id: i64) -> bool {
        if !contains_id(catalog.achievements.iter().map(|item| item.id), achievement_id) {
            return false;
        }
        self.user_achievements
            .entry(user_id)
            .or_default()
            .insert(achievement_id, now_timestamp());
        true
    }

    pub fn award_badge(&mut self, catalog: &Catalog, user_id: i64, badge_id: i64) -> bool {
        if !contains_id(catalog.badges.iter().map(|item| item.id), badge_id) {
            return false;
        }
        self.user_badges
            .entry(user_id)
            .or_default()
            .insert(badge_id, now_timestamp());
        true
    }

    pub fn award_reward(&mut self, catalog: &Catalog, user_id: i64, reward_id: i64) -> bool {
        if !contains_id(catalog.rewards.iter().map(|item| item.id), reward_id) {
            return false;
        }
        self.user_rewards
            .entry(user_id)
            .or_default()
            .insert(reward_id, now_timestamp());
        true
    }
}

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

fn contains_id<I>(items: I, id: i64) -> bool
where
    I: Iterator<Item = i64>,
{
    let set: HashSet<i64> = items.collect();
    set.contains(&id)
}

fn now_timestamp() -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("unix:{ts}")
}
