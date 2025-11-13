import { Request, Response, NextFunction } from 'express';
import jwt from 'jsonwebtoken';
import { pool } from '../config/database';
import { AuthRequest, User } from '../types';
import logger from '../config/logger';

export const authenticateToken = async (
  req: Request,
  res: Response,
  next: NextFunction
) => {
  try {
    const authHeader = req.headers['authorization'];
    const token = authHeader && authHeader.split(' ')[1]; // Bearer TOKEN


    if (!token) {
      logger.warn('Access token required', { ip: req.ip });
      return res.status(401).json({ error: 'Access token required' });
    }

    const secret = process.env.JWT_SECRET || 'your_super_secret_jwt_key';
    const decoded = jwt.verify(token, secret) as { userId: number };

    // Fetch user from database
    const result = await pool.query(
      'SELECT id, username, email, dark_matter, created_at, last_login, is_admin, is_banned, alliance_id FROM users WHERE id = $1',
      [decoded.userId]
    );


    if (result.rows.length === 0) {
      logger.warn('User not found for token', { userId: decoded.userId, ip: req.ip });
      return res.status(403).json({ error: 'User not found' });
    }

    const user: User = result.rows[0];


    if (user.is_banned) {
      logger.warn('Banned user attempted access', { userId: user.id, ip: req.ip });
      return res.status(403).json({ error: 'Account banned' });
    }

    (req as AuthRequest).user = user;
    logger.info('User authenticated', { userId: user.id, ip: req.ip });
    next();
  } catch (error) {
    logger.error('Authentication error', { error, ip: req.ip });
    return res.status(403).json({ error: 'Invalid or expired token' });
  }
};

export const requireAdmin = (
  req: Request,
  res: Response,
  next: NextFunction
) => {
  const authReq = req as AuthRequest;
  if (!authReq.user || !authReq.user.is_admin) {
    logger.warn('Admin access required', { userId: authReq.user?.id, ip: req.ip });
    return res.status(403).json({ error: 'Admin access required' });
  }
  logger.info('Admin access granted', { userId: authReq.user.id, ip: req.ip });
  next();
};
