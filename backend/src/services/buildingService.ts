import { pool } from '../config/database';
import { PlanetService } from './planetService';
import {
  calculateBuildingCost,
  BUILDINGS,
} from '../config/gameConfig';
import { gameConfig } from './gameConfigAdapter';

export class BuildingService {
  static async startConstruction(
    userId: number,
    planetId: number,
    buildingType: string
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

      // Check if building exists in config
      if (!BUILDINGS[buildingType]) {
        throw new Error('Invalid building type');
      }

      // Check if already building something
      const queueResult = await client.query(
        'SELECT COUNT(*) FROM construction_queue WHERE planet_id = $1',
        [planetId]
      );
      
      if (parseInt(queueResult.rows[0].count) > 0) {
        throw new Error('Already constructing a building on this planet');
      }

      const currentLevel = planet[buildingType] || 0;

      // Check requirements
      const config = BUILDINGS[buildingType];
      if (config.requirements) {
        if (config.requirements.buildings) {
          for (const [reqBuilding, reqLevel] of Object.entries(config.requirements.buildings)) {
            if ((planet[reqBuilding] || 0) < reqLevel) {
              throw new Error(`Requires ${reqBuilding} level ${reqLevel}`);
            }
          }
        }
        
        if (config.requirements.research) {
          const researchResult = await client.query(
            'SELECT * FROM research WHERE user_id = $1',
            [userId]
          );
          const research = researchResult.rows[0] || {};
          
          for (const [reqResearch, reqLevel] of Object.entries(config.requirements.research)) {
            if ((research[reqResearch] || 0) < reqLevel) {
              throw new Error(`Requires ${reqResearch} level ${reqLevel}`);
            }
          }
        }
      }

      // Calculate cost
      const cost = calculateBuildingCost(buildingType, currentLevel);

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

      // Calculate build time using configuration
      const buildTime = await gameConfig.calculateBuildingTime(
        buildingType,
        currentLevel + 1,
        planet.robotics_factory || 0,
        planet.nanite_factory || 0
      );
      
      const gameSpeed = parseFloat(process.env.GAME_SPEED || '1');
      const buildTimeSeconds = buildTime / gameSpeed;

      const endTime = new Date(Date.now() + buildTimeSeconds * 1000);

      // Add to construction queue
      const result = await client.query(
        `INSERT INTO construction_queue 
         (planet_id, building_type, level, start_time, end_time, metal_cost, crystal_cost, deuterium_cost)
         VALUES ($1, $2, $3, NOW(), $4, $5, $6, $7)
         RETURNING *`,
        [planetId, buildingType, currentLevel + 1, endTime, cost.metal, cost.crystal, cost.deuterium]
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

  static async finishConstruction(constructionId: number): Promise<void> {
    const client = await pool.connect();
    
    try {
      await client.query('BEGIN');

      // Get construction details
      const result = await client.query(
        'SELECT * FROM construction_queue WHERE id = $1',
        [constructionId]
      );

      if (result.rows.length === 0) {
        throw new Error('Construction not found');
      }

      const construction = result.rows[0];

      // Check if finished
      if (new Date(construction.end_time) > new Date()) {
        throw new Error('Construction not yet finished');
      }

      // Update building level
      await client.query(
        `UPDATE planets SET ${construction.building_type} = $1 WHERE id = $2`,
        [construction.level, construction.planet_id]
      );

      // Remove from queue
      await client.query(
        'DELETE FROM construction_queue WHERE id = $1',
        [constructionId]
      );

      await client.query('COMMIT');
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }
  }

  static async checkAndFinishConstructions(): Promise<void> {
    const result = await pool.query(
      'SELECT * FROM construction_queue WHERE end_time <= NOW()'
    );

    for (const construction of result.rows) {
      try {
        await this.finishConstruction(construction.id);
        console.log(`Finished construction ${construction.id} on planet ${construction.planet_id}`);
      } catch (error) {
        console.error(`Error finishing construction ${construction.id}:`, error);
      }
    }
  }

  static async getConstructionQueue(planetId: number): Promise<any[]> {
    const result = await pool.query(
      'SELECT * FROM construction_queue WHERE planet_id = $1 ORDER BY start_time',
      [planetId]
    );
    return result.rows;
  }

  static async cancelConstruction(
    userId: number,
    constructionId: number
  ): Promise<void> {
    const client = await pool.connect();
    
    try {
      await client.query('BEGIN');

      const result = await client.query(
        `SELECT cq.*, p.user_id 
         FROM construction_queue cq 
         JOIN planets p ON cq.planet_id = p.id 
         WHERE cq.id = $1`,
        [constructionId]
      );

      if (result.rows.length === 0) {
        throw new Error('Construction not found');
      }

      const construction = result.rows[0];

      if (construction.user_id !== userId) {
        throw new Error('Access denied');
      }

      // Refund 60% of resources
      const refund = {
        metal: Math.floor(construction.metal_cost * 0.6),
        crystal: Math.floor(construction.crystal_cost * 0.6),
        deuterium: Math.floor(construction.deuterium_cost * 0.6),
      };

      await client.query(
        `UPDATE planets 
         SET metal = metal + $1, crystal = crystal + $2, deuterium = deuterium + $3
         WHERE id = $4`,
        [refund.metal, refund.crystal, refund.deuterium, construction.planet_id]
      );

      await client.query(
        'DELETE FROM construction_queue WHERE id = $1',
        [constructionId]
      );

      await client.query('COMMIT');
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }
  }
}
