/**
 * @module backend/services/playerBlockService
 *
 * Player block management: list, add, remove and query player block
 * relationships. Provides helpers to check mutual blocks and scope-specific
 * blocking behavior used by the chat and messaging subsystems.
 */

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
  /**
   * List all blocks created by a user.
   *
   * @param userId - The id of the user whose blocks will be returned.
   * @returns Array of PlayerBlockRow objects.
   */
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

  /**
   * Block a user with an optional scope and expiration.
   * If a block already exists for the same scope it will be updated.
   *
   * @param userId - The user creating the block.
   * @param blockedUserId - The user being blocked.
   * @param blockScope - Scope of the block (all/chat/messages).
   * @param reason - Optional reason for the block.
   * @param expiresAt - Optional expiration date for the block.
   * @returns The created or updated PlayerBlockRow.
   */
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

  /**
   * Remove a block between two users optionally restricting by scope.
   *
   * @param userId - The user who created the block.
   * @param blockedUserId - The user that was blocked.
   * @param blockScope - Optional scope filter; when provided only blocks matching the scope are removed.
   * @returns True when a block was removed, false otherwise.
   */
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

  /**
   * Check whether a user has blocked another user within a particular scope.
   *
   * @param userId - The user who may have created the block.
   * @param blockedUserId - The potentially blocked user.
   * @param scope - Block scope to check (defaults to 'all').
   * @returns True when an active block exists, false otherwise.
   */
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

  /**
   * Check whether either user has blocked the other within the given scope.
   * This is a convenience method used by messaging and chat systems to determine
   * whether communication should be permitted.
   *
   * @param userId - First user id.
   * @param otherUserId - Second user id.
   * @param scope - Block scope to check (defaults to 'all').
   * @returns True when either user has an active block against the other.
   */
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
