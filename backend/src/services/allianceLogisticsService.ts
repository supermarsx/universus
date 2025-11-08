import { pool } from '../config/database';
import { AlliancePermission } from '../types/alliance';
import { AllianceService } from './allianceService';

interface DepotRequestPayload {
  hostPlanetId: number;
  guestPlanetId: number;
  fleetId?: number;
  requestedDeuterium: number;
  notes?: string;
}

interface DepotApprovalPayload {
  sessionId: number;
  approvedAmount: number;
}

interface SharedTransportPayload {
  fromPlanetId: number;
  targetType: 'treasury' | 'member';
  targetPlanetId?: number;
  resourceType: 'metal' | 'crystal' | 'deuterium';
  amount: number;
  notes?: string;
}

const allianceService = new AllianceService();

export class AllianceLogisticsService {
  async requestDepotDock(
    allianceId: number,
    userId: number,
    payload: DepotRequestPayload
  ) {
    if (payload.requestedDeuterium <= 0) {
      throw new Error('Requested deuterium must be greater than zero');
    }

    const client = await pool.connect();
    try {
      await client.query('BEGIN');

      const [hostPlanet, guestPlanet] = await Promise.all([
        client.query(
          `SELECT p.*, u.alliance_id, u.username
             FROM planets p
             JOIN users u ON u.id = p.user_id
            WHERE p.id = $1`,
          [payload.hostPlanetId]
        ),
        client.query(
          `SELECT p.*, u.alliance_id, u.username
             FROM planets p
             JOIN users u ON u.id = p.user_id
            WHERE p.id = $1`,
          [payload.guestPlanetId]
        ),
      ]);

      if (!hostPlanet.rows[0] || !guestPlanet.rows[0]) {
        throw new Error('Invalid host or guest planet');
      }

      if (hostPlanet.rows[0].alliance_id !== allianceId) {
        throw new Error('Host planet does not belong to this alliance');
      }

      if (guestPlanet.rows[0].alliance_id !== allianceId) {
        throw new Error('Guest planet does not belong to this alliance');
      }

      if (guestPlanet.rows[0].user_id !== userId) {
        throw new Error('You can only request depot services for your own planet');
      }

      if ((hostPlanet.rows[0].alliance_depot || 0) <= 0) {
        throw new Error('Host planet does not have an Alliance Depot');
      }

      const duration = Math.max(hostPlanet.rows[0].alliance_depot * 60, 600);
      const expiresAt = new Date(Date.now() + duration * 1000);

      const insert = await client.query(
        `INSERT INTO alliance_depot_sessions (
            alliance_id,
            host_planet_id,
            fleet_id,
            guest_user_id,
            status,
            remaining_duration,
            created_at,
            expires_at,
            metadata
        ) VALUES ($1, $2, $3, $4, 'pending', $5, NOW(), $6, $7)
        RETURNING *`,
        [
          allianceId,
          payload.hostPlanetId,
          payload.fleetId || null,
          userId,
          duration,
          expiresAt,
          JSON.stringify({
            guestPlanetId: payload.guestPlanetId,
            requestedDeuterium: payload.requestedDeuterium,
            notes: payload.notes || null,
          }),
        ]
      );

      await client.query(
        `INSERT INTO alliance_history (alliance_id, event_type, description, related_user_id)
         VALUES ($1, 'depot_request', $2, $3)`,
        [
          allianceId,
          `${guestPlanet.rows[0].username} requested ${payload.requestedDeuterium} deut from ${hostPlanet.rows[0].username}`,
          userId,
        ]
      );

      await client.query('COMMIT');
      return insert.rows[0];
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }
  }

  async approveDepotDock(
    allianceId: number,
    approverId: number,
    payload: DepotApprovalPayload
  ) {
    const client = await pool.connect();
    try {
      await client.query('BEGIN');

      const sessionResult = await client.query(
        `SELECT * FROM alliance_depot_sessions
          WHERE id = $1 AND alliance_id = $2 FOR UPDATE`,
        [payload.sessionId, allianceId]
      );

      const session = sessionResult.rows[0];
      if (!session) {
        throw new Error('Depot session not found');
      }
      if (session.status !== 'pending') {
        throw new Error('Session already processed');
      }

      const metadata = session.metadata || {};
      const guestPlanetId = metadata.guestPlanetId;
      const requestedAmount = metadata.requestedDeuterium || 0;
      const hostPlanetId = session.host_planet_id;

      const hostPlanet = await client.query(
        `SELECT p.*, u.username
         FROM planets p
         JOIN users u ON u.id = p.user_id
         WHERE p.id = $1`,
        [hostPlanetId]
      );
      const guestPlanet = await client.query(
        `SELECT p.*, u.username
         FROM planets p
         JOIN users u ON u.id = p.user_id
         WHERE p.id = $1`,
        [guestPlanetId]
      );

      if (!hostPlanet.rows[0] || !guestPlanet.rows[0]) {
        throw new Error('Invalid planet references');
      }

      const hasPermission = await allianceService.checkPermission(
        allianceId,
        approverId,
        AlliancePermission.MANAGE_RESOURCES
      );

      if (
        !hasPermission &&
        hostPlanet.rows[0].user_id !== approverId
      ) {
        throw new Error('You cannot approve this depot request');
      }

      const amount = Math.min(
        payload.approvedAmount,
        requestedAmount,
        hostPlanet.rows[0].deuterium || 0
      );

      if (amount <= 0) {
        throw new Error('Cannot approve zero deuterium');
      }

      await client.query(
        `UPDATE planets SET deuterium = deuterium - $1 WHERE id = $2`,
        [amount, hostPlanetId]
      );
      await client.query(
        `UPDATE planets SET deuterium = deuterium + $1 WHERE id = $2`,
        [amount, guestPlanetId]
      );

      await client.query(
        `UPDATE alliance_depot_sessions
         SET status = 'fulfilled',
             deuterium_consumed = $1,
             metadata = metadata || jsonb_build_object('approved_amount', $1),
             expires_at = NOW()
         WHERE id = $2`,
        [amount, payload.sessionId]
      );

      await client.query(
        `INSERT INTO alliance_history (alliance_id, event_type, description, related_user_id)
         VALUES ($1, 'depot_refuel', $2, $3)`,
        [
          allianceId,
          `${guestPlanet.rows[0].username} received ${amount} deut from ${hostPlanet.rows[0].username}`,
          approverId,
        ]
      );

      await client.query('COMMIT');
      return { amount, sessionId: payload.sessionId };
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }
  }

  async cancelDepotSession(allianceId: number, userId: number, sessionId: number) {
    const result = await pool.query(
      `UPDATE alliance_depot_sessions
         SET status = 'cancelled'
       WHERE id = $1 AND alliance_id = $2 AND status = 'pending'
       RETURNING *`,
      [sessionId, allianceId]
    );
    if (!result.rows[0]) {
      throw new Error('Session not found or already processed');
    }
    await pool.query(
      `INSERT INTO alliance_history (alliance_id, event_type, description, related_user_id)
       VALUES ($1, 'depot_cancelled', $2, $3)`,
      [
        allianceId,
        `Depot request #${sessionId} cancelled`,
        userId,
      ]
    );
  }

  async cancelDepotSessionByFleet(fleetId: number): Promise<void> {
    await pool.query(
      `UPDATE alliance_depot_sessions
          SET status = 'cancelled'
        WHERE fleet_id = $1 AND status IN ('pending', 'active')`,
      [fleetId]
    );
  }

  async createSharedTransport(
    allianceId: number,
    userId: number,
    payload: SharedTransportPayload
  ) {
    if (payload.amount <= 0) {
      throw new Error('Amount must be positive');
    }

    const resourceColumn = payload.resourceType;

    const contributorPlanetResult = await pool.query(
      'SELECT * FROM planets WHERE id = $1',
      [payload.fromPlanetId]
    );
    const contributorPlanet = contributorPlanetResult.rows[0];
    if (!contributorPlanet) {
      throw new Error('Origin planet not found');
    }
    if (contributorPlanet.user_id !== userId) {
      throw new Error('You do not control the origin planet');
    }

    if (contributorPlanet[resourceColumn] < payload.amount) {
      throw new Error('Insufficient resources for transport');
    }

    await pool.query(
      `UPDATE planets SET ${resourceColumn} = ${resourceColumn} - $1 WHERE id = $2`,
      [payload.amount, payload.fromPlanetId]
    );

    if (payload.targetType === 'treasury') {
      await pool.query(
        `UPDATE alliances
           SET ${resourceColumn}_treasury = ${resourceColumn}_treasury + $1
         WHERE id = $2`,
        [payload.amount, allianceId]
      );
      await pool.query(
        `INSERT INTO alliance_contributions (alliance_id, user_id, contribution_type, amount, metadata)
         VALUES ($1, $2, $3, $4, $5)`,
        [
          allianceId,
          userId,
          payload.resourceType,
          payload.amount,
          JSON.stringify({ transport: true, notes: payload.notes || null }),
        ]
      );
    } else if (payload.targetType === 'member') {
      if (!payload.targetPlanetId) {
        throw new Error('Target planet required for member transport');
      }
      await pool.query(
        `UPDATE planets SET ${resourceColumn} = ${resourceColumn} + $1 WHERE id = $2`,
        [payload.amount, payload.targetPlanetId]
      );
    }

    await pool.query(
      `INSERT INTO alliance_history (alliance_id, event_type, description, related_user_id, metadata)
       VALUES ($1, 'shared_transport', $2, $3, $4)`,
      [
        allianceId,
        `Transported ${payload.amount} ${payload.resourceType} via alliance logistics`,
        userId,
        JSON.stringify({
          targetType: payload.targetType,
          notes: payload.notes || null,
        }),
      ]
    );
  }

  async cancelDepotSessionByFleet(fleetId: number): Promise<void> {
    await pool.query(
      `UPDATE alliance_depot_sessions
          SET status = 'cancelled'
        WHERE fleet_id = $1 AND status IN ('pending', 'active')`,
      [fleetId]
    );
  }

  async getDepotSessions(
    allianceId: number,
    userId: number,
    status?: string
  ) {
    const member = await pool.query(
      'SELECT rank FROM alliance_members WHERE alliance_id = $1 AND user_id = $2',
      [allianceId, userId]
    );
    if (!member.rows.length) {
      throw new Error('You are not part of this alliance');
    }

    const params: any[] = [allianceId];
    let whereClause = 'WHERE s.alliance_id = $1';
    if (status) {
      params.push(status);
      whereClause += ` AND s.status = $${params.length}`;
    }

    const result = await pool.query(
      `SELECT
         s.*,
         hp.name as host_planet_name,
         hp.galaxy as host_galaxy,
         hp.system as host_system,
         hp.position as host_position,
         host_user.username as host_username,
         guest_user.username as guest_username
       FROM alliance_depot_sessions s
       JOIN planets hp ON hp.id = s.host_planet_id
       JOIN users host_user ON host_user.id = hp.user_id
       JOIN users guest_user ON guest_user.id = s.guest_user_id
       ${whereClause}
       ORDER BY s.created_at DESC
       LIMIT 50`,
      params
    );

    return result.rows.map((row) => {
      const metadata =
        row.metadata && typeof row.metadata === 'object'
          ? row.metadata
          : row.metadata
          ? JSON.parse(row.metadata)
          : {};

      return {
        id: row.id,
        alliance_id: row.alliance_id,
        host_planet_id: row.host_planet_id,
        host_planet_name: row.host_planet_name,
        host_galaxy: row.host_galaxy,
        host_system: row.host_system,
        host_position: row.host_position,
        host_username: row.host_username,
        guest_user_id: row.guest_user_id,
        guest_username: row.guest_username,
        status: row.status,
        remaining_duration: row.remaining_duration,
        deuterium_consumed: Number(row.deuterium_consumed || 0),
        created_at: row.created_at,
        expires_at: row.expires_at,
        metadata,
      };
    });
  }
}

export default new AllianceLogisticsService();
