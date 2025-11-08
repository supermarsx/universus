import { Request, Response, NextFunction } from 'express';
import jwt from 'jsonwebtoken';
import { pool } from '../config/database';
import { AuthRequest, User } from '../types';

export const authenticateToken = async (
  req: Request,
  res: Response,
  next: NextFunction
) => {
  try {
    const authHeader = req.headers['authorization'];
    const token = authHeader && authHeader.split(' ')[1]; // Bearer TOKEN

    if (!token) {
      return res.status(401).json({ error: 'Access token required' });
    }

    const secret = process.env.JWT_SECRET || 'your_super_secret_jwt_key';
    const decoded = jwt.verify(token, secret) as { userId: number };

    // Fetch user from database
    const result = await pool.query(
      `SELECT 
        u.id,
        u.username,
        u.email,
        u.dark_matter,
        u.created_at,
        u.last_login,
        u.is_admin,
        u.is_banned,
        u.alliance_id,
        au.admin_level,
        CASE 
          WHEN u.is_admin THEN TRUE
          WHEN au.admin_level IN ('super_admin', 'game_admin', 'moderator') THEN TRUE
          ELSE FALSE
        END AS is_moderator
       FROM users u
       LEFT JOIN admin_users au ON au.user_id = u.id AND au.is_active = TRUE
       WHERE u.id = $1`,
      [decoded.userId]
    );

    if (result.rows.length === 0) {
      return res.status(403).json({ error: 'User not found' });
    }

    const user: User = result.rows[0];

    if (user.is_banned) {
      return res.status(403).json({ error: 'Account banned' });
    }

    (req as AuthRequest).user = {
      ...user,
      admin_level: result.rows[0].admin_level || null,
      is_moderator: result.rows[0].is_moderator || user.is_admin || false,
    };
    next();
  } catch (error) {
    console.error('Authentication error:', error);
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
    return res.status(403).json({ error: 'Admin access required' });
  }
  next();
};
