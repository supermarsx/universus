/**
 * @module backend/services/buildingService
 *
 * Building construction service. Handles building queue management,
 * requirement checks, field availability (for moons), and resource
 * consumption for construction actions.
 */

import { pool } from '../config/database';
import { calculateBuildingCost, BUILDINGS } from '../config/gameConfig';
import { gameConfig } from './gameConfigAdapter';
import { resolveLocation, LocationType } from './locationService';
import { MoonFieldService } from './moonFieldService';

interface ConstructionQueueParams {
  planetId: number;
  locationType?: LocationType;
  moonId?: number;
}

export class BuildingService {
  static async startConstruction(
    userId: number,
    buildingType: string,
    options: {
      planetId?: number;
      moonId?: number;
      locationType?: LocationType;
      expectedPlanetId?: number;
    }
  ): Promise<any> {
    const client = await pool.connect();

    try {
      await client.query('BEGIN');

      if (!BUILDINGS[buildingType]) {
        throw new Error('Invalid building type');
      }

      const location = await resolveLocation(client, userId, {
        planetId: options.planetId,
        moonId: options.moonId,
        locationType: options.locationType,
        expectedPlanetId: options.expectedPlanetId ?? options.planetId,
      });

      const queueFilters = this.buildQueueFilter(location);
      const queueResult = await client.query(
        `SELECT COUNT(*) FROM construction_queue WHERE ${queueFilters.whereClause}`,
        queueFilters.values
      );

      if (parseInt(queueResult.rows[0].count, 10) > 0) {
        throw new Error('This location is already constructing a building');
      }

      const currentLevel = location.record[buildingType] || 0;
      const targetLevel = currentLevel + 1;

      if (location.type === 'moon') {
        MoonFieldService.assertBuildingAllowed(buildingType);
        MoonFieldService.assertFieldAvailability({
          buildingType,
          nextLevel: targetLevel,
          totalFields: location.totalFields,
          usedFields: location.usedFields,
        });
      }
      const config = BUILDINGS[buildingType];

      if (config.requirements) {
        if (config.requirements.buildings) {
          for (const [reqBuilding, reqLevel] of Object.entries(
            config.requirements.buildings
          )) {
            if ((location.record[reqBuilding] || 0) < reqLevel) {
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

          for (const [reqResearch, reqLevel] of Object.entries(
            config.requirements.research
          )) {
            if ((research[reqResearch] || 0) < reqLevel) {
              throw new Error(`Requires ${reqResearch} level ${reqLevel}`);
            }
          }
        }
      }

      const cost = calculateBuildingCost(buildingType, currentLevel);

      if (
        location.record.metal < cost.metal ||
        location.record.crystal < cost.crystal ||
        location.record.deuterium < cost.deuterium
      ) {
        throw new Error('Insufficient resources');
      }

      await client.query(
        `UPDATE ${location.resourceTable}
         SET metal = metal - $1,
             crystal = crystal - $2,
             deuterium = deuterium - $3
         WHERE id = $4`,
        [cost.metal, cost.crystal, cost.deuterium, location.primaryId]
      );

      const buildTime = await gameConfig.calculateBuildingTime(
        buildingType,
        targetLevel,
        location.roboticsLevel,
        location.naniteLevel
      );
      const gameSpeed = parseFloat(process.env.GAME_SPEED || '1');
      const buildTimeSeconds = buildTime / gameSpeed;
      const endTime = new Date(Date.now() + buildTimeSeconds * 1000);

      const insertResult = await client.query(
        `INSERT INTO construction_queue
           (planet_id, moon_id, location_type, building_type, level,
            start_time, end_time, metal_cost, crystal_cost, deuterium_cost)
         VALUES ($1, $2, $3, $4, $5, NOW(), $6, $7, $8, $9)
         RETURNING *`,
        [
          location.planetId,
          location.moonId,
          location.type,
          buildingType,
          targetLevel,
          endTime,
          cost.metal,
          cost.crystal,
          cost.deuterium,
        ]
      );

      await client.query('COMMIT');
      return insertResult.rows[0] || null;
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

      const result = await client.query(
        'SELECT * FROM construction_queue WHERE id = $1',
        [constructionId]
      );

      if (result.rows.length === 0) {
        throw new Error('Construction not found');
      }

      const construction = result.rows[0];

      if (new Date(construction.end_time) > new Date()) {
        throw new Error('Construction not yet finished');
      }

      const target = this.resolveQueueTarget(construction);

      await client.query(
        `UPDATE ${target.table} SET ${construction.building_type} = $1 WHERE id = $2`,
        [construction.level, target.id]
      );

      if (construction.location_type === 'moon') {
        const { usedFields, totalFields } = MoonFieldService.calculateFieldAdjustments(
          construction.building_type,
          construction.level
        );
        if (usedFields !== 0 || totalFields !== 0) {
          await client.query(
            `UPDATE moons
             SET used_fields = used_fields + $1,
                 total_fields = total_fields + $2
             WHERE id = $3`,
            [usedFields, totalFields, target.id]
          );
        }
      }

      await client.query('DELETE FROM construction_queue WHERE id = $1', [
        constructionId,
      ]);

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
        console.log(
          `Finished construction ${construction.id} on ${construction.location_type}`
        );
      } catch (error) {
        console.error(
          `Error finishing construction ${construction.id}:`,
          error
        );
      }
    }
  }

  static async getConstructionQueue(
    params: ConstructionQueueParams
  ): Promise<any[]> {
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
      `SELECT * FROM construction_queue WHERE ${whereClause} ORDER BY start_time`,
      values
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
        'SELECT * FROM construction_queue WHERE id = $1',
        [constructionId]
      );

      if (result.rows.length === 0) {
        throw new Error('Construction not found');
      }

      const construction = result.rows[0];
      const target = this.resolveQueueTarget(construction);

      const ownerResult = await client.query(
        `SELECT user_id FROM ${target.table} WHERE id = $1`,
        [target.id]
      );

      if (ownerResult.rows.length === 0) {
        throw new Error('Location not found for construction');
      }

      if (ownerResult.rows[0].user_id !== userId) {
        throw new Error('Access denied');
      }

      const refund = {
        metal: Math.floor(construction.metal_cost * 0.6),
        crystal: Math.floor(construction.crystal_cost * 0.6),
        deuterium: Math.floor(construction.deuterium_cost * 0.6),
      };

      await client.query(
        `UPDATE ${target.table}
         SET metal = metal + $1,
             crystal = crystal + $2,
             deuterium = deuterium + $3
         WHERE id = $4`,
        [refund.metal, refund.crystal, refund.deuterium, target.id]
      );

      await client.query('DELETE FROM construction_queue WHERE id = $1', [
        constructionId,
      ]);

      await client.query('COMMIT');
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }
  }

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

  private static buildQueueFilter(location: {
    type: LocationType;
    planetId: number;
    moonId: number | null;
  }): { whereClause: string; values: Array<number | string> } {
    const values: Array<number | string> = [location.type, location.planetId];
    let whereClause = 'location_type = $1 AND planet_id = $2';

    if (location.type === 'moon') {
      if (!location.moonId) {
        throw new Error('Moon queue requires moon id');
      }
      values.push(location.moonId);
      whereClause += ' AND moon_id = $3';
    }

    return { whereClause, values };
  }
}
