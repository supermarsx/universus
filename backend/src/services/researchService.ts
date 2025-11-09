/**
 * @module backend/services/researchService
 *
 * Research management: starting, cancelling and querying research queues.
 * This service coordinates planet resource checks, queue insertion, and
 * emits realtime events when research state changes.
 */

import { pool } from '../config/database';
import { PlanetService } from './planetService';
import { calculateResearchCost, RESEARCH, ResearchConfig } from '../config/gameConfig';
import { gameConfig } from './gameConfigAdapter';
import notificationService from './notificationService';
import { getRealtimeHandler } from '../socket';

interface ResearchOverview {
  planetId: number | null;
  researchLabLevel: number;
  technologies: any[];
  currentResearch: any | null;
  queue: any[];
}

export class ResearchService {
  static async startResearch(
    userId: number,
    planetId: number,
    researchType: string
  ): Promise<any> {
    const client = await pool.connect();
    
    try {
      await client.query('BEGIN');

      const queueCount = await client.query(
        'SELECT COUNT(*) FROM research_queue WHERE user_id = $1',
        [userId]
      );

      if (parseInt(queueCount.rows[0].count) > 0) {
        throw new Error('Already researching something');
      }

      const planet = await this.getUserPlanet(userId, planetId, client);
      if (!planet) {
        throw new Error('Planet not found');
      }

      if ((planet.research_lab || 0) === 0) {
        throw new Error('Research lab required');
      }

      const currentResearch = await this.getResearchRow(userId, client);
      const config = RESEARCH[researchType];
      if (!config) {
        throw new Error('Invalid research type');
      }

      this.ensureRequirements(researchType, config, planet, currentResearch);

      const currentLevel = currentResearch[researchType] || 0;
      const cost = calculateResearchCost(researchType, currentLevel);

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

      await client.query(
        `UPDATE planets 
         SET metal = metal - $1, crystal = crystal - $2, deuterium = deuterium - $3
         WHERE id = $4`,
        [cost.metal, cost.crystal, cost.deuterium, planetId]
      );

      const labLevel = planet.research_lab || 1;
      const researchTime = await gameConfig.calculateResearchTime(
        researchType,
        currentLevel + 1,
        labLevel
      );
      
      const gameSpeed = parseFloat(process.env.GAME_SPEED || '1');
      const researchTimeSeconds = researchTime / gameSpeed;
      const endTime = new Date(Date.now() + researchTimeSeconds * 1000);

      const result = await client.query(
        `INSERT INTO research_queue 
         (user_id, planet_id, research_type, level, start_time, end_time, metal_cost, crystal_cost, deuterium_cost)
         VALUES ($1, $2, $3, $4, NOW(), $5, $6, $7, $8)
         RETURNING *`,
        [userId, planetId, researchType, currentLevel + 1, endTime, cost.metal, cost.crystal, cost.deuterium]
      );

      await client.query('COMMIT');

      const queueEntry = this.decorateQueueEntry(result.rows[0]);
      this.emitResearchEvent(userId, 'researchUpdate', {
        planetId,
        researchType,
        level: currentLevel + 1,
        endTime: queueEntry?.end_time,
      });

      return queueEntry;
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }
  }

  static async getResearchOverview(userId: number, planetId?: number): Promise<ResearchOverview> {
    const researchRow = await this.getResearchRow(userId);
    const queue = await this.getResearchQueue(userId);
    let planet = null;

    if (planetId) {
      planet = await this.getUserPlanet(userId, planetId);
      if (!planet) {
        throw new Error('Planet not found');
      }
    }

    return {
      planetId: planet ? planet.id : null,
      researchLabLevel: planet ? planet.research_lab || 0 : 0,
      technologies: this.buildTechnologyList(researchRow, planet),
      currentResearch: queue.length > 0 ? queue[0] : null,
      queue,
    };
  }

  static async getUserResearch(userId: number): Promise<any> {
    return this.getResearchRow(userId);
  }

  static async getResearchQueue(userId: number): Promise<any[]> {
    const result = await pool.query(
      'SELECT * FROM research_queue WHERE user_id = $1 ORDER BY start_time',
      [userId]
    );
    return result.rows.map((row) => this.decorateQueueEntry(row));
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

      this.emitResearchEvent(userId, 'researchUpdate', {
        planetId: research.planet_id,
        researchType: research.research_type,
        level: research.level,
        cancelled: true,
      });
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }
  }

  static async completeFinishedResearch(): Promise<number> {
    const result = await pool.query(
      'SELECT * FROM research_queue WHERE end_time <= NOW()'
    );

    let completed = 0;

    for (const entry of result.rows) {
      const client = await pool.connect();
      try {
        await client.query('BEGIN');

        await client.query(
          `UPDATE research SET ${entry.research_type} = $1 WHERE user_id = $2`,
          [entry.level, entry.user_id]
        );

        await client.query('DELETE FROM research_queue WHERE id = $1', [entry.id]);

        await client.query('COMMIT');
        completed++;

        await notificationService.notifyResearchComplete(
          entry.user_id,
          this.formatTechnologyName(entry.research_type)
        );

        this.emitResearchEvent(entry.user_id, 'researchComplete', {
          planetId: entry.planet_id,
          researchType: entry.research_type,
          level: entry.level,
        });
      } catch (error) {
        await client.query('ROLLBACK');
        console.error(`Error completing research queue ${entry.id}:`, error);
      } finally {
        client.release();
      }
    }

    return completed;
  }

  private static async getResearchRow(userId: number, client: any = pool): Promise<any> {
    const result = await client.query(
      'SELECT * FROM research WHERE user_id = $1',
      [userId]
    );
    return result.rows[0] || {};
  }

  private static async getUserPlanet(userId: number, planetId: number, client: any = pool): Promise<any | null> {
    const result = await client.query(
      'SELECT * FROM planets WHERE id = $1 AND user_id = $2',
      [planetId, userId]
    );
    return result.rows[0] || null;
  }

  private static buildTechnologyList(researchRow: any, planet?: any) {
    return Object.entries(RESEARCH).map(([type, config]) => {
      const currentLevel = researchRow[type] || 0;
      const cost = calculateResearchCost(type, currentLevel);
      return {
        type,
        name: config.displayName || this.formatTechnologyName(type),
        description: config.description || '',
        category: config.category || 'general',
        level: currentLevel,
        nextLevel: currentLevel + 1,
        cost,
        requirements: config.requirements || {},
        requirementsMet: this.requirementsMet(config.requirements, planet, researchRow),
      };
    });
  }

  private static ensureRequirements(
    researchType: string,
    config: ResearchConfig,
    planet: any,
    researchRow: any
  ): void {
    if (!config.requirements) return;

    if (config.requirements.buildings) {
      for (const [reqBuilding, reqLevel] of Object.entries(config.requirements.buildings)) {
        if ((planet[reqBuilding] || 0) < reqLevel) {
          throw new Error(`Requires ${reqBuilding} level ${reqLevel}`);
        }
      }
    }

    if (config.requirements.research) {
      for (const [reqResearch, reqLevel] of Object.entries(config.requirements.research)) {
        if ((researchRow[reqResearch] || 0) < reqLevel) {
          throw new Error(`Requires ${reqResearch} level ${reqLevel}`);
        }
      }
    }
  }

  private static requirementsMet(
    requirements: ResearchConfig['requirements'] | undefined,
    planet: any,
    researchRow: any
  ): boolean {
    if (!requirements) return true;

    if (requirements.buildings) {
      for (const [reqBuilding, reqLevel] of Object.entries(requirements.buildings)) {
        if ((planet?.[reqBuilding] || 0) < reqLevel) {
          return false;
        }
      }
    }

    if (requirements.research) {
      for (const [reqResearch, reqLevel] of Object.entries(requirements.research)) {
        if ((researchRow[reqResearch] || 0) < reqLevel) {
          return false;
        }
      }
    }

    return true;
  }

  private static decorateQueueEntry(queue: any) {
    if (!queue) return null;
    const now = Date.now();
    const start = new Date(queue.start_time).getTime();
    const end = new Date(queue.end_time).getTime();
    const total = Math.max(end - start, 1);
    const elapsed = Math.min(Math.max(now - start, 0), total);

    return {
      ...queue,
      progress: Math.min(elapsed / total, 1),
      secondsRemaining: Math.max(Math.ceil((end - now) / 1000), 0),
    };
  }

  private static emitResearchEvent(
    userId: number,
    event: 'researchUpdate' | 'researchComplete',
    payload: any
  ): void {
    const handler = getRealtimeHandler();
    if (!handler) return;

    if (event === 'researchUpdate') {
      handler.emitResearchUpdate(userId, payload);
    } else {
      handler.emitResearchComplete(userId, payload);
    }
  }

  private static formatTechnologyName(key: string): string {
    const config = RESEARCH[key];
    if (config?.displayName) return config.displayName;
    return key
      .split('_')
      .map((segment) => segment.charAt(0).toUpperCase() + segment.slice(1))
      .join(' ');
  }
}
