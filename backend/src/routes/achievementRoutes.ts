import express from 'express';
import { pool } from '../config/database';
import { AchievementService } from '../services/achievementService';

const router = express.Router();
const achievementService = new AchievementService(pool);

// List all achievements
router.get('/achievements', async (req, res) => {
  const achievements = await achievementService.getAllAchievements();
  res.json(achievements);
});

// List all badges
router.get('/badges', async (req, res) => {
  const badges = await achievementService.getAllBadges();
  res.json(badges);
});

// List all rewards
router.get('/rewards', async (req, res) => {
  const rewards = await achievementService.getAllRewards();
  res.json(rewards);
});

// List all ladders
router.get('/ladders', async (req, res) => {
  const ladders = await achievementService.getAllLadders();
  res.json(ladders);
});

// List hall of fame
router.get('/hall-of-fame', async (req, res) => {
  const limit = parseInt(req.query.limit as string) || 100;
  const hof = await achievementService.getHallOfFame(limit);
  res.json(hof);
});

// Get user achievement progress
router.get('/user/:userId/achievements', async (req, res) => {
  const userId = parseInt(req.params.userId);
  const achievements = await achievementService.getUserAchievements(userId);
  res.json(achievements);
});

// Get user badges
router.get('/user/:userId/badges', async (req, res) => {
  const userId = parseInt(req.params.userId);
  const badges = await achievementService.getUserBadges(userId);
  res.json(badges);
});

// Get user rewards
router.get('/user/:userId/rewards', async (req, res) => {
  const userId = parseInt(req.params.userId);
  const rewards = await achievementService.getUserRewards(userId);
  res.json(rewards);
});

// Award achievement to user
router.post('/user/:userId/achievements/:achievementId', async (req, res) => {
  const userId = parseInt(req.params.userId);
  const achievementId = parseInt(req.params.achievementId);
  await achievementService.awardAchievement(userId, achievementId);
  res.json({ success: true });
});

// Award badge to user
router.post('/user/:userId/badges/:badgeId', async (req, res) => {
  const userId = parseInt(req.params.userId);
  const badgeId = parseInt(req.params.badgeId);
  await achievementService.awardBadge(userId, badgeId);
  res.json({ success: true });
});

// Award reward to user
router.post('/user/:userId/rewards/:rewardId', async (req, res) => {
  const userId = parseInt(req.params.userId);
  const rewardId = parseInt(req.params.rewardId);
  await achievementService.awardReward(userId, rewardId);
  res.json({ success: true });
});

export default router;
