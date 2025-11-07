import { pool } from '../config/database';
import { PlanetService } from './planetService';
import { calculateResearchCost, RESEARCH } from '../config/gameConfig';
import { gameConfig } from './gameConfigAdapter';

export class ResearchService {
  static async startResearch(
    userId: number,
    planetId: number,
    researchType: string
  ): Promise<any> {
    const client = await pool.connect();
    
    try {
      await client.query('BEGIN');

      // Check if already researching
      const existingResult = await client.query(
        'SELECT COUNT(*) FROM research_queue WHERE user_id = $1',
        [userId]
      );

      if (parseInt(existingResult.rows[0].count) > 0) {
        throw new Error('Already researching something');
      }

      // Verify planet ownership and research lab
      const planetResult = await client.query(
        'SELECT * FROM planets WHERE id = $1 AND user_id = $2',
        [planetId, userId]
      );

      if (planetResult.rows.length === 0) {
        throw new Error('Planet not found');
      }

      const planet = planetResult.rows[0];

      if (planet.research_lab === 0) {
        throw new Error('Research lab required');
      }

      // Get current research levels
      const researchResult = await client.query(
        'SELECT * FROM research WHERE user_id = $1',
        [userId]
      );

      const currentResearch = researchResult.rows[0] || {};
      const currentLevel = currentResearch[researchType] || 0;

      // Check requirements
      const config = RESEARCH[researchType];
      if (!config) {
        throw new Error('Invalid research type');
      }

      if (config.requirements) {
        if (config.requirements.buildings) {
          for (const [reqBuilding, reqLevel] of Object.entries(config.requirements.buildings)) {
            if ((planet[reqBuilding] || 0) < reqLevel) {
              throw new Error(`Requires ${reqBuilding} level ${reqLevel}`);
            }
          }
        }

        if (config.requirements.research) {
          for (const [reqResearch, reqLevel] of Object.entries(config.requirements.research)) {
            if ((currentResearch[reqResearch] || 0) < reqLevel) {
              throw new Error(`Requires ${reqResearch} level ${reqLevel}`);
            }
          }
        }
      }

      // Calculate cost
      const cost = calculateResearchCost(researchType, currentLevel);

      // Check if can afford
      await PlanetService.updateResources(planetId);
      const updatedPlanet = await PlanetService.getPlanetById(planetId);
      if (!updatedPlanet) throw new Error('Planet not found');

      if (
        updatedPlanet.metal < cost.metal ||
        updatedPlanet.crystal < cost.crystal ||
        updatedPlanet.deuterium < cost.deuterium
      ) {
        throw new Error('Insufficient resources');
      }

      // Deduct resources
      await client.query(
        `UPDATE planets 
         SET metal = metal - $1, crystal = crystal - $2, deuterium = deuterium - $3
         WHERE id = $4`,
        [cost.metal, cost.crystal, cost.deuterium, planetId]
      );

      // Calculate research time
      // Calculate research time using configuration
      const labLevel = planet.research_lab || 1;
      const researchTime = await gameConfig.calculateResearchTime(
        researchType,
        currentLevel + 1,
        labLevel
      );
      
      const gameSpeed = parseFloat(process.env.GAME_SPEED || '1');
      const researchTimeSeconds = researchTime / gameSpeed;

      const endTime = new Date(Date.now() + researchTimeSeconds * 1000);

      // Add to research queue
      const result = await client.query(
        `INSERT INTO research_queue 
         (user_id, planet_id, research_type, level, start_time, end_time, metal_cost, crystal_cost, deuterium_cost)
         VALUES ($1, $2, $3, $4, NOW(), $5, $6, $7, $8)
         RETURNING *`,
        [userId, planetId, researchType, currentLevel + 1, endTime, cost.metal, cost.crystal, cost.deuterium]
      );

      await client.query('COMMIT');

      return result.rows[0] || null;
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }
  }

  static async getUserResearch(userId: number): Promise<any> {
    const result = await pool.query(
      'SELECT * FROM research WHERE user_id = $1',
      [userId]
    );
    return result.rows[0] || {};
  }

  static async getResearchQueue(userId: number): Promise<any[]> {
    const result = await pool.query(
      'SELECT * FROM research_queue WHERE user_id = $1 ORDER BY start_time',
      [userId]
    );
    return result.rows;
  }

  static async cancelResearch(userId: number, queueId: number): Promise<void> {
    const client = await pool.connect();
    
    try {
      await client.query('BEGIN');

      const result = await client.query(
        'SELECT * FROM research_queue WHERE id = $1 AND user_id = $2',
        [queueId, userId]
      );

      if (result.rows.length === 0) {
        throw new Error('Research not found');
      }

      const research = result.rows[0];

      // Refund 60% of resources
      const refund = {
        metal: Math.floor(research.metal_cost * 0.6),
        crystal: Math.floor(research.crystal_cost * 0.6),
        deuterium: Math.floor(research.deuterium_cost * 0.6),
      };

      await client.query(
        `UPDATE planets 
         SET metal = metal + $1, crystal = crystal + $2, deuterium = deuterium + $3
         WHERE id = $4`,
        [refund.metal, refund.crystal, refund.deuterium, research.planet_id]
      );

      await client.query('DELETE FROM research_queue WHERE id = $1', [queueId]);

      await client.query('COMMIT');
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }
  }
}
