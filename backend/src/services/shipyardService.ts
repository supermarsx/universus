/**
 * @module backend/services/shipyardService
 *
 * Shipyard production service. Manages starting and cancelling production
 * queues for ships and defenses, deducting resources, and calculating
 * build times using game configuration.
 */

import { pool } from '../config/database';
import { SHIPS, DEFENSES } from '../config/gameConfig';
import { gameConfig } from './gameConfigAdapter';
import { resolveLocation, LocationType } from './locationService';

interface ShipyardQueueParams {
  planetId: number;
  locationType?: LocationType;
  moonId?: number;
}

export class ShipyardService {
  /**
   * Start production for a ship or defense unit.
   *
   * Deducts resources from the location, computes build time using game
   * configuration, inserts a queue entry and returns the queued entry.
   *
   * @param userId - Owner initiating production
   * @param unitType - Unit key from SHIPS or DEFENSES
   * @param quantity - Number of units to produce (must be > 0)
   * @param options - Optional location identifiers (planetId, moonId, locationType)
   * @returns The queued entry decorated with progress/secondsRemaining
   * @throws Error when validation fails (insufficient resources, invalid unit, etc.)
   */
  static async startProduction(
    userId: number,
    unitType: string,
    quantity: number,
    options: {
      planetId?: number;
      moonId?: number;
      locationType?: LocationType;
      expectedPlanetId?: number;
    }
  ): Promise<any> {
    if (quantity <= 0) {
      throw new Error('Quantity must be greater than zero');
    }

    const client = await pool.connect();

    try {
      await client.query('BEGIN');

      const unitConfig = SHIPS[unitType] || DEFENSES[unitType];
      if (!unitConfig) {
        throw new Error('Invalid unit type');
      }

      const location = await resolveLocation(client, userId, {
        planetId: options.planetId,
        moonId: options.moonId,
        locationType: options.locationType,
        expectedPlanetId: options.expectedPlanetId ?? options.planetId,
      });

      if (!location.shipyardLevel) {
        throw new Error('Shipyard required');
      }

      const totalCost = {
        metal: unitConfig.cost.metal * quantity,
        crystal: unitConfig.cost.crystal * quantity,
        deuterium: unitConfig.cost.deuterium * quantity,
      };

      if (
        location.record.metal < totalCost.metal ||
        location.record.crystal < totalCost.crystal ||
        location.record.deuterium < totalCost.deuterium
      ) {
        throw new Error('Insufficient resources');
      }

      await client.query(
        `UPDATE ${location.resourceTable}
         SET metal = metal - $1,
             crystal = crystal - $2,
             deuterium = deuterium - $3
         WHERE id = $4`,
        [totalCost.metal, totalCost.crystal, totalCost.deuterium, location.primaryId]
      );

      const singleBuildTime = await gameConfig.calculateShipBuildTime(
        unitType,
        location.shipyardLevel,
        location.naniteLevel
      );
      const gameSpeed = parseFloat(process.env.GAME_SPEED || '1');
      const totalBuildTime = (singleBuildTime * quantity) / gameSpeed;
      const endTime = new Date(Date.now() + totalBuildTime * 1000);

      const insertResult = await client.query(
        `INSERT INTO shipyard_queue
           (planet_id, moon_id, location_type, unit_type, quantity,
            start_time, end_time, metal_cost, crystal_cost, deuterium_cost)
         VALUES ($1, $2, $3, $4, $5, NOW(), $6, $7, $8, $9)
         RETURNING *`,
        [
          location.planetId,
          location.moonId,
          location.type,
          unitType,
          quantity,
          endTime,
          totalCost.metal,
          totalCost.crystal,
          totalCost.deuterium,
        ]
      );

      await client.query('COMMIT');
      return this.decorateQueueEntry(insertResult.rows[0]);
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }
  }

  /**
   * Retrieve the shipyard queue for a given location.
   *
   * @param params - Object describing the target location (planetId, optional moonId, optional locationType)
   * @returns Array of decorated queue entries
   */
  static async getQueue(params: ShipyardQueueParams): Promise<any[]> {
    const locationType: LocationType = params.locationType ?? 'planet';
    const values: Array<string | number> = [locationType, params.planetId];
    let whereClause = 'location_type = $1 AND planet_id = $2';

    if (locationType === 'moon') {
      if (!params.moonId) {
        throw new Error('moonId is required when requesting moon queues');
      }
      values.push(params.moonId);
      whereClause += ' AND moon_id = $3';
    }

    const result = await pool.query(
      `SELECT * FROM shipyard_queue WHERE ${whereClause} ORDER BY start_time`,
      values
    );

    return result.rows.map((queue) => this.decorateQueueEntry(queue));
  }

  /**
   * Cancel a queued production item and refund a portion of resources.
   *
   * Validates ownership and refunds 60% of the unit costs back to the
   * location's resource pool, then removes the queue entry.
   *
   * @param userId - ID of the user attempting the cancellation
   * @param queueId - ID of the queue entry to cancel
   */
  static async cancelProduction(userId: number, queueId: number): Promise<void> {
    const client = await pool.connect();

    try {
      await client.query('BEGIN');

      const result = await client.query(
        'SELECT * FROM shipyard_queue WHERE id = $1',
        [queueId]
      );

      if (result.rows.length === 0) {
        throw new Error('Queue item not found');
      }

      const queue = result.rows[0];
      const target = this.resolveQueueTarget(queue);

      const ownerResult = await client.query(
        `SELECT user_id FROM ${target.table} WHERE id = $1`,
        [target.id]
      );

      if (ownerResult.rows.length === 0) {
        throw new Error('Location not found for queue item');
      }

      if (ownerResult.rows[0].user_id !== userId) {
        throw new Error('Access denied');
      }

      const refund = {
        metal: Math.floor(queue.metal_cost * 0.6),
        crystal: Math.floor(queue.crystal_cost * 0.6),
        deuterium: Math.floor(queue.deuterium_cost * 0.6),
      };

      await client.query(
        `UPDATE ${target.table}
         SET metal = metal + $1,
             crystal = crystal + $2,
             deuterium = deuterium + $3
         WHERE id = $4`,
        [refund.metal, refund.crystal, refund.deuterium, target.id]
      );

      await client.query('DELETE FROM shipyard_queue WHERE id = $1', [queueId]);

      await client.query('COMMIT');
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }
  }

  /**
   * Process finished shipyard queue entries (end_time <= NOW()).
   *
   * For each finished entry, the units are applied to the target (planet
   * or moon) and the queue entry removed. Returns the number of entries completed.
   *
   * @returns Number of completed queue entries
   */
  static async completeFinishedJobs(): Promise<number> {
    const result = await pool.query(
      'SELECT * FROM shipyard_queue WHERE end_time <= NOW()'
    );

    let completed = 0;

    for (const queue of result.rows) {
      try {
        await pool.query('BEGIN');
        const target = this.resolveQueueTarget(queue);

        await pool.query(
          `UPDATE ${target.table}
           SET ${queue.unit_type} = ${queue.unit_type} + $1
           WHERE id = $2`,
          [queue.quantity, target.id]
        );

        await pool.query('DELETE FROM shipyard_queue WHERE id = $1', [queue.id]);

        await pool.query('COMMIT');
        completed++;
        console.log(
          `Shipyard queue ${queue.id} completed (${queue.quantity} ${queue.unit_type})`
        );
      } catch (error) {
        await pool.query('ROLLBACK');
        console.error(`Error completing shipyard queue ${queue.id}:`, error);
      }
    }

    return completed;
  }

  /**
   * Decorate a raw queue row with convenience fields used by the API.
   *
   * @private
   */
  private static decorateQueueEntry(queue: any) {
    const now = Date.now();
    const end = new Date(queue.end_time).getTime();
    const start = new Date(queue.start_time).getTime();
    const totalDuration = Math.max(end - start, 1);
    const elapsed = Math.min(Math.max(now - start, 0), totalDuration);

    return {
      ...queue,
      secondsRemaining: Math.max(Math.ceil((end - now) / 1000), 0),
      progress: Math.min(elapsed / totalDuration, 1),
    };
  }

  /**
   * Resolve the storage table and id referenced by a queue row.
   *
   * Returns `{ table: 'planets' | 'moons', id }` or throws if missing.
   *
   * @private
   */
  private static resolveQueueTarget(queueRow: any): {
    table: 'planets' | 'moons';
    id: number;
  } {
    const locationType: LocationType = queueRow.location_type;
    if (locationType === 'moon') {
      if (!queueRow.moon_id) {
        throw new Error('Moon queue is missing moon reference');
      }

      return {
        table: 'moons',
        id: queueRow.moon_id,
      };
    }

    if (!queueRow.planet_id) {
      throw new Error('Planet queue is missing planet reference');
    }

    return {
      table: 'planets',
      id: queueRow.planet_id,
    };
  }
}
