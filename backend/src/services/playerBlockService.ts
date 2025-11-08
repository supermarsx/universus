import { pool } from '../config/database';

export type BlockScope = 'all' | 'chat' | 'messages';

interface PlayerBlockRow {
  id: number;
  user_id: number;
  blocked_user_id: number;
  block_scope: BlockScope;
  reason: string | null;
  created_at: Date;
  expires_at: Date | null;
  username?: string;
}

class PlayerBlockService {
  async listBlocks(userId: number): Promise<PlayerBlockRow[]> {
    const result = await pool.query(
      `SELECT pb.*, u.username 
         FROM player_blocks pb
         JOIN users u ON pb.blocked_user_id = u.id
        WHERE pb.user_id = $1
        ORDER BY pb.created_at DESC`,
      [userId]
    );

    return result.rows;
  }

  async blockUser(
    userId: number,
    blockedUserId: number,
    blockScope: BlockScope = 'all',
    reason?: string,
    expiresAt?: Date | null
  ): Promise<PlayerBlockRow> {
    const result = await pool.query(
      `INSERT INTO player_blocks (
         user_id, blocked_user_id, block_scope, reason, expires_at
       ) VALUES ($1, $2, $3, $4, $5)
       ON CONFLICT (user_id, blocked_user_id, block_scope)
       DO UPDATE SET
         reason = EXCLUDED.reason,
         expires_at = EXCLUDED.expires_at,
         created_at = CURRENT_TIMESTAMP
       RETURNING *`,
      [userId, blockedUserId, blockScope, reason || null, expiresAt || null]
    );

    return result.rows[0];
  }

  async unblockUser(
    userId: number,
    blockedUserId: number,
    blockScope?: BlockScope
  ): Promise<boolean> {
    const result = await pool.query(
      `DELETE FROM player_blocks 
        WHERE user_id = $1 
          AND blocked_user_id = $2
          AND ($3::text IS NULL OR block_scope = $3)`,
      [userId, blockedUserId, blockScope || null]
    );

    return (result.rowCount || 0) > 0;
  }

  async isBlocked(
    userId: number,
    blockedUserId: number,
    scope: BlockScope = 'all'
  ): Promise<boolean> {
    const result = await pool.query(
      `SELECT 1 FROM player_blocks
        WHERE user_id = $1
          AND blocked_user_id = $2
          AND (block_scope = 'all' OR block_scope = $3)
          AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)`,
      [userId, blockedUserId, scope]
    );

    return result.rows.length > 0;
  }

  async isBlockedEither(
    userId: number,
    otherUserId: number,
    scope: BlockScope = 'all'
  ): Promise<boolean> {
    const [a, b] = await Promise.all([
      this.isBlocked(userId, otherUserId, scope),
      this.isBlocked(otherUserId, userId, scope),
    ]);
    return a || b;
  }
}

export default new PlayerBlockService();
