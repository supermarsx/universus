/**
 * @module backend/services/acsService
 *
 * ACS (Alliance Combat System) service — creates and manages ACS groups,
 * membership and basic validation. Uses database transactions to ensure
 * consistent group creation and membership updates.
 */

import { PoolClient } from 'pg';
import { pool } from '../config/database';

interface CreateAcsPayload {
  missionType: string;
  targetGalaxy: number;
  targetSystem: number;
  targetPosition: number;
  departureWindowStart?: string;
  departureWindowEnd?: string;
  notes?: string;
}

export class AcsService {
  /**
   * Create a new ACS group for the user's alliance.
   *
   * Validates membership, creates the group and ensures the creator is a
   * member of the newly-created group.
   *
   * @param userId - Creator user id
   * @param payload - Group creation payload
   * @returns Newly created ACS group row
   */
  static async createGroup(userId: number, payload: CreateAcsPayload): Promise<any> {
    const client = await pool.connect();
    try {
      await client.query('BEGIN');

      const alliance = await this.getUserAlliance(client, userId);
      if (!alliance) {
        throw new Error('You must be in an alliance to create an ACS group.');
      }

      const result = await client.query(
        `INSERT INTO acs_groups (
            alliance_id,
            creator_id,
            mission_type,
            target_galaxy,
            target_system,
            target_position,
            departure_window_start,
            departure_window_end,
            notes
         ) VALUES ($1, $2, $3, $4, $5, $6, 
            COALESCE($7, CURRENT_TIMESTAMP),
            COALESCE($8, CURRENT_TIMESTAMP + INTERVAL '1 hour'),
            $9
         )
         RETURNING *`,
        [
          alliance,
          userId,
          payload.missionType || 'attack',
          payload.targetGalaxy,
          payload.targetSystem,
          payload.targetPosition,
          payload.departureWindowStart,
          payload.departureWindowEnd,
          payload.notes || null,
        ]
      );

      await client.query(
        `INSERT INTO acs_group_members (group_id, user_id)
         VALUES ($1, $2)
         ON CONFLICT DO NOTHING`,
        [result.rows[0].id, userId]
      );

      await client.query('COMMIT');
      return result.rows[0];
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }
  }

  /**
   * Join an existing ACS group (must belong to same alliance).
   *
   * @param userId - User id joining the group
   * @param groupId - ACS group id
   * @param planetId - Optional planet id associated with the join
   */
  static async joinGroup(userId: number, groupId: number, planetId?: number): Promise<void> {
    const client = await pool.connect();
    try {
      await client.query('BEGIN');

      const alliance = await this.getUserAlliance(client, userId);
      if (!alliance) {
        throw new Error('You must be in an alliance to join an ACS group.');
      }

      const group = await client.query('SELECT * FROM acs_groups WHERE id = $1', [groupId]);
      if (group.rows.length === 0) {
        throw new Error('ACS group not found.');
      }

      if (group.rows[0].alliance_id !== alliance) {
        throw new Error('You can only join ACS groups created by your alliance.');
      }

      await client.query(
        `INSERT INTO acs_group_members (group_id, user_id, planet_id)
         VALUES ($1, $2, $3)
         ON CONFLICT (group_id, user_id)
         DO UPDATE SET planet_id = COALESCE($3, acs_group_members.planet_id)`,
        [groupId, userId, planetId || null]
      );

      await client.query('COMMIT');
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }
  }

  /**
   * Leave an ACS group.
   *
   * @param userId - User id leaving
   * @param groupId - ACS group id
   */
  static async leaveGroup(userId: number, groupId: number): Promise<void> {
    await pool.query(
      'DELETE FROM acs_group_members WHERE group_id = $1 AND user_id = $2',
      [groupId, userId]
    );
  }

  /**
   * List active ACS groups for the user's alliance.
   *
   * @param userId - User id
   * @returns Array of ACS groups with member counts
   */
  static async listAllianceGroups(userId: number): Promise<any[]> {
    const alliance = await this.getUserAlliance(pool, userId);
    if (!alliance) return [];

    const result = await pool.query(
      `SELECT ag.*, COUNT(m.id) AS member_count
       FROM acs_groups ag
       LEFT JOIN acs_group_members m ON m.group_id = ag.id
       WHERE ag.alliance_id = $1
         AND ag.departure_window_end >= CURRENT_TIMESTAMP - INTERVAL '1 hour'
       GROUP BY ag.id
       ORDER BY ag.created_at DESC`,
      [alliance]
    );
    return result.rows;
  }

  /**
   * Helper - resolve the alliance id for a user (accepts a client for
   * transactional usage).
   *
   * @private
   */
  private static async getUserAlliance(client: PoolClient | typeof pool, userId: number): Promise<number | null> {
    const result = await client.query(
      'SELECT alliance_id FROM alliance_members WHERE user_id = $1',
      [userId]
    );
    return result.rows[0]?.alliance_id || null;
  }
}

export default AcsService;
