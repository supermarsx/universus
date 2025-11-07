import express, { Request, Response } from 'express';
import { authenticateToken } from '../middleware/auth';
import { pool } from '../config/database';
import { AuthRequest } from '../types';

const router = express.Router();

router.use(authenticateToken);

router.get('/', async (req: Request, res: Response) => {
  try {
    const galaxy = parseInt(req.query.galaxy as string) || 1;
    const system = parseInt(req.query.system as string) || 1;

    const result = await pool.query(
      `SELECT p.id, p.name, p.galaxy, p.system, p.position, p.planet_type, 
              u.id as user_id, u.username, u.alliance_id, a.tag as alliance_tag
       FROM planets p
       LEFT JOIN users u ON p.user_id = u.id
       LEFT JOIN alliances a ON u.alliance_id = a.id
       WHERE p.galaxy = $1 AND p.system = $2
       ORDER BY p.position`,
      [galaxy, system]
    );

    res.json(result.rows);
  } catch (error: any) {
    console.error('Error fetching galaxy:', error);
    res.status(500).json({ error: error.message });
  }
});

router.get('/debris', async (req: Request, res: Response) => {
  try {
    const galaxy = parseInt(req.query.galaxy as string) || 1;
    const system = parseInt(req.query.system as string) || 1;

    const result = await pool.query(
      `SELECT * FROM debris_fields 
       WHERE galaxy = $1 AND system = $2 AND (metal > 0 OR crystal > 0)`,
      [galaxy, system]
    );

    res.json(result.rows);
  } catch (error: any) {
    console.error('Error fetching debris:', error);
    res.status(500).json({ error: error.message });
  }
});

export default router;
