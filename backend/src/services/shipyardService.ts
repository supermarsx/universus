import { pool } from '../config/database';
import { PlanetService } from './planetService';
import { SHIPS, DEFENSES } from '../config/gameConfig';
import { gameConfig } from './gameConfigAdapter';

export class ShipyardService {
  static async startProduction(
    userId: number,
    planetId: number,
    unitType: string,
    quantity: number
  ): Promise<any> {
    const client = await pool.connect();
    
    try {
      await client.query('BEGIN');

      // Verify planet ownership
      const planetResult = await client.query(
        'SELECT * FROM planets WHERE id = $1 AND user_id = $2',
        [planetId, userId]
      );
      
      if (planetResult.rows.length === 0) {
        throw new Error('Planet not found or access denied');
      }

      const planet = planetResult.rows[0];

      // Check if shipyard exists
      if (planet.shipyard === 0) {
        throw new Error('Shipyard required');
      }

      // Get unit config
      const unitConfig = SHIPS[unitType] || DEFENSES[unitType];
      if (!unitConfig) {
        throw new Error('Invalid unit type');
      }

      // Calculate total cost
      const totalCost = {
        metal: unitConfig.cost.metal * quantity,
        crystal: unitConfig.cost.crystal * quantity,
        deuterium: unitConfig.cost.deuterium * quantity,
      };

      // Check if can afford
      await PlanetService.updateResources(planetId);
      const updatedPlanet = await PlanetService.getPlanetById(planetId);
      if (!updatedPlanet) throw new Error('Planet not found');

      if (
        updatedPlanet.metal < totalCost.metal ||
        updatedPlanet.crystal < totalCost.crystal ||
        updatedPlanet.deuterium < totalCost.deuterium
      ) {
        throw new Error('Insufficient resources');
      }

      // Deduct resources
      await client.query(
        `UPDATE planets 
         SET metal = metal - $1, crystal = crystal - $2, deuterium = deuterium - $3
         WHERE id = $4`,
        [totalCost.metal, totalCost.crystal, totalCost.deuterium, planetId]
      );

      // Calculate build time using configuration
      const shipyardLevel = planet.shipyard || 1;
      const naniteLevel = planet.nanite_factory || 0;
      
      const singleBuildTime = await gameConfig.calculateShipBuildTime(
        unitType,
        shipyardLevel,
        naniteLevel
      );
      
      const gameSpeed = parseFloat(process.env.GAME_SPEED || '1');
      const totalBuildTime = (singleBuildTime * quantity) / gameSpeed;

      const endTime = new Date(Date.now() + totalBuildTime * 1000);

      // Add to shipyard queue
      const result = await client.query(
        `INSERT INTO shipyard_queue 
         (planet_id, unit_type, quantity, start_time, end_time, metal_cost, crystal_cost, deuterium_cost)
         VALUES ($1, $2, $3, NOW(), $4, $5, $6, $7)
         RETURNING *`,
        [planetId, unitType, quantity, endTime, totalCost.metal, totalCost.crystal, totalCost.deuterium]
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

  static async getQueue(planetId: number): Promise<any[]> {
    const result = await pool.query(
      'SELECT * FROM shipyard_queue WHERE planet_id = $1 ORDER BY start_time',
      [planetId]
    );
    return result.rows;
  }

  static async cancelProduction(userId: number, queueId: number): Promise<void> {
    const client = await pool.connect();
    
    try {
      await client.query('BEGIN');

      const result = await client.query(
        `SELECT sq.*, p.user_id 
         FROM shipyard_queue sq 
         JOIN planets p ON sq.planet_id = p.id 
         WHERE sq.id = $1`,
        [queueId]
      );

      if (result.rows.length === 0) {
        throw new Error('Queue item not found');
      }

      const queue = result.rows[0];

      if (queue.user_id !== userId) {
        throw new Error('Access denied');
      }

      // Refund 60% of resources
      const refund = {
        metal: Math.floor(queue.metal_cost * 0.6),
        crystal: Math.floor(queue.crystal_cost * 0.6),
        deuterium: Math.floor(queue.deuterium_cost * 0.6),
      };

      await client.query(
        `UPDATE planets 
         SET metal = metal + $1, crystal = crystal + $2, deuterium = deuterium + $3
         WHERE id = $4`,
        [refund.metal, refund.crystal, refund.deuterium, queue.planet_id]
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
}
