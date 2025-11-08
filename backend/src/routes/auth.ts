import express, { Request, Response } from 'express';
import { AuthService } from '../services/authService';
import { botProtectionService } from '../services/botProtectionService';

const router = express.Router();

router.get('/bot-challenge', async (req: Request, res: Response) => {
  try {
    const challenge = await botProtectionService.createChallenge({
      ip: req.ip,
      userAgent: req.get('user-agent') || undefined,
    });
    res.json(challenge);
  } catch (error) {
    console.error('Bot challenge generation failed:', error);
    res.status(500).json({ error: 'Unable to create bot challenge' });
  }
});

router.post('/register', async (req: Request, res: Response) => {
  try {
    const { username, email, password, bot_challenge } = req.body;

    if (!username || !email || !password) {
      return res.status(400).json({ error: 'Missing required fields' });
    }

    const challengeValid = await botProtectionService.validateChallenge(bot_challenge);
    if (!challengeValid) {
      return res.status(400).json({ error: 'Bot verification failed' });
    }

    const result = await AuthService.register(username, email, password);
    res.status(201).json(result);
  } catch (error: any) {
    console.error('Registration error:', error);
    res.status(400).json({ error: error.message });
  }
});

router.post('/login', async (req: Request, res: Response) => {
  try {
    const { username, password, bot_challenge } = req.body;

    if (!username || !password) {
      return res.status(400).json({ error: 'Missing required fields' });
    }

    const challengeValid = await botProtectionService.validateChallenge(bot_challenge);
    if (!challengeValid) {
      return res.status(400).json({ error: 'Bot verification failed' });
    }

    const result = await AuthService.login(username, password);
    res.status(200).json(result);
  } catch (error: any) {
    console.error('Login error:', error);
    res.status(401).json({ error: error.message });
  }
});

export default router;
