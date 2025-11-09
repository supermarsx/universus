import { Pool } from 'pg';

export class AchievementService {
  private db: Pool;

  constructor(db: Pool) {
    this.db = db;
  }

  // List all achievements
  async getAllAchievements() {
    const res = await this.db.query('SELECT * FROM achievements ORDER BY id');
    return res.rows;
  }

  // List all badges
  async getAllBadges() {
    const res = await this.db.query('SELECT * FROM badges ORDER BY id');
    return res.rows;
  }

  // List all rewards
  async getAllRewards() {
    const res = await this.db.query('SELECT * FROM rewards ORDER BY id');
    return res.rows;
  }

  // List all ladders
  async getAllLadders() {
    const res = await this.db.query('SELECT * FROM ladders ORDER BY id');
    return res.rows;
  }

  // List hall of fame entries
  async getHallOfFame(limit = 100) {
    const res = await this.db.query('SELECT * FROM hall_of_fame ORDER BY inducted_at DESC LIMIT $1', [limit]);
    return res.rows;
  }

  // Get user achievement progress
  async getUserAchievements(userId: number) {
    const res = await this.db.query(
      `SELECT a.*, ua.progress, ua.unlocked_at
       FROM achievements a
       LEFT JOIN user_achievements ua ON a.id = ua.achievement_id AND ua.user_id = $1
       ORDER BY a.id`,
      [userId]
    );
    return res.rows;
  }

  // Get user badges
  async getUserBadges(userId: number) {
    const res = await this.db.query(
      `SELECT b.*, ub.earned_at
       FROM badges b
       LEFT JOIN user_badges ub ON b.id = ub.badge_id AND ub.user_id = $1
       ORDER BY b.id`,
      [userId]
    );
    return res.rows;
  }

  // Get user rewards
  async getUserRewards(userId: number) {
    const res = await this.db.query(
      `SELECT r.*, ur.granted_at
       FROM rewards r
       LEFT JOIN user_rewards ur ON r.id = ur.reward_id AND ur.user_id = $1
       ORDER BY r.id`,
      [userId]
    );
    return res.rows;
  }

  // Award an achievement to a user
  async awardAchievement(userId: number, achievementId: number) {
    await this.db.query(
      `INSERT INTO user_achievements (user_id, achievement_id, progress, unlocked_at)
       VALUES ($1, $2, 1, NOW())
       ON CONFLICT (user_id, achievement_id) DO UPDATE SET unlocked_at = NOW(), progress = 1`,
      [userId, achievementId]
    );
  }

  // Award a badge to a user
  async awardBadge(userId: number, badgeId: number) {
    await this.db.query(
      `INSERT INTO user_badges (user_id, badge_id, earned_at)
       VALUES ($1, $2, NOW())
       ON CONFLICT (user_id, badge_id) DO NOTHING`,
      [userId, badgeId]
    );
  }

  // Award a reward to a user
  async awardReward(userId: number, rewardId: number) {
    await this.db.query(
      `INSERT INTO user_rewards (user_id, reward_id, granted_at)
       VALUES ($1, $2, NOW())
       ON CONFLICT (user_id, reward_id) DO NOTHING`,
      [userId, rewardId]
    );
  }
}
