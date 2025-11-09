/**
 * @module backend/services/planetService
 *
 * Planet service: reading and updating planet state, resource production
 * calculations, and utility helpers used throughout gameplay services.
 */

import { pool } from '../config/database';
import { PoolClient } from 'pg';
import { Planet } from '../types';
import {
  calculateBuildingCost,
  calculateBuildingTime,
  calculateStorageCapacity,
  BUILDINGS,
} from '../config/gameConfig';
import { gameConfig } from './gameConfigAdapter';

export class PlanetService {
  /**
   * Get all planets owned by a user.
   *
   * @param userId - Owner's user id
   * @returns Array of Planet rows
   */
  static async getPlanetsByUserId(userId: number): Promise<Planet[]> {
    const result = await pool.query(
      'SELECT * FROM planets WHERE user_id = $1 ORDER BY id',
      [userId]
    );
    return result.rows;
  }

  /**
   * Count planets owned by a user.
   *
   * @param userId - Owner's user id
   * @returns Number of planets
   */
  static async getPlanetCountByUserId(userId: number): Promise<number> {
    const result = await pool.query('SELECT COUNT(*) FROM planets WHERE user_id = $1', [userId]);
    return parseInt(result.rows[0]?.count || '0', 10);
  }

  /**
   * Fetch a planet by its primary id.
   *
   * @param planetId - Planet id
   * @returns Planet row or null
   */
  static async getPlanetById(planetId: number): Promise<Planet | null> {
    const result = await pool.query(
      'SELECT * FROM planets WHERE id = $1',
      [planetId]
    );
    return result.rows.length > 0 ? result.rows[0] : null;
  }

  /**
   * Recalculate and persist resource values for a planet based on
   * elapsed time and configured production rates.
   *
   * @param planetId - Planet id to update
   * @returns Updated Planet object
   */
  static async updateResources(planetId: number): Promise<Planet> {
    const planet = await this.getPlanetById(planetId);
    if (!planet) throw new Error('Planet not found');

    const now = new Date();
    const lastUpdate = new Date(planet.last_resource_update);
    const hoursPassed = (now.getTime() - lastUpdate.getTime()) / (1000 * 60 * 60);

    if (hoursPassed > 0) {
      // Calculate production per hour using configuration
      const gameSpeed = parseFloat(process.env.GAME_SPEED || '1');
      
      const metalProduction = await gameConfig.calculateResourceProduction(
        'metal_mine',
        planet.metal_mine,
        gameSpeed
      );
      const crystalProduction = await gameConfig.calculateResourceProduction(
        'crystal_mine',
        planet.crystal_mine,
        gameSpeed
      );
      const deuteriumProduction = await gameConfig.calculateResourceProduction(
        'deuterium_synthesizer',
        planet.deuterium_synthesizer,
        gameSpeed
      );

      // Calculate storage capacities
      const metalCapacity = calculateStorageCapacity(planet.metal_storage);
      const crystalCapacity = calculateStorageCapacity(planet.crystal_storage);
      const deuteriumCapacity = calculateStorageCapacity(planet.deuterium_tank);

      // Update resources with caps
      const newMetal = Math.min(
        planet.metal + metalProduction * hoursPassed,
        metalCapacity
      );
      const newCrystal = Math.min(
        planet.crystal + crystalProduction * hoursPassed,
        crystalCapacity
      );
      const newDeuterium = Math.min(
        planet.deuterium + deuteriumProduction * hoursPassed,
        deuteriumCapacity
      );

      await pool.query(
        `UPDATE planets 
         SET metal = $1, crystal = $2, deuterium = $3, last_resource_update = $4
         WHERE id = $5`,
        [newMetal, newCrystal, newDeuterium, now, planetId]
      );

      planet.metal = newMetal;
      planet.crystal = newCrystal;
      planet.deuterium = newDeuterium;
      planet.last_resource_update = now;
    }

    return planet;
  }

  /**
   * Calculate the per-hour resource production for a planet.
   *
   * @param planet - Planet record used to compute production
   * @returns Object with metal, crystal, deuterium and energy production
   */
  static async getResourceProduction(planet: Planet): Promise<{
    metal: number;
    crystal: number;
    deuterium: number;
    energy: number;
  }> {
    const gameSpeed = parseFloat(process.env.GAME_SPEED || '1');

    const metalProd = await gameConfig.calculateResourceProduction('metal_mine', planet.metal_mine, gameSpeed);
    const crystalProd = await gameConfig.calculateResourceProduction('crystal_mine', planet.crystal_mine, gameSpeed);
    const deuteriumProd = await gameConfig.calculateResourceProduction(
      'deuterium_synthesizer',
      planet.deuterium_synthesizer,
      gameSpeed
    );

    const solarProd = await gameConfig.calculateResourceProduction('solar_plant', planet.solar_plant, gameSpeed);
    const fusionProd = await gameConfig.calculateResourceProduction(
      'fusion_reactor',
      planet.fusion_reactor,
      gameSpeed
    );

    const energyProduction = solarProd + fusionProd;
    const energyConsumption =
      planet.metal_mine * 10 +
      planet.crystal_mine * 10 +
      planet.deuterium_synthesizer * 20;

    return {
      metal: metalProd,
      crystal: crystalProd,
      deuterium: deuteriumProd,
      energy: energyProduction - energyConsumption,
    };
  }

  /**
   * Check whether a planet can afford a specified cost after updating
   * its current resource state.
   *
   * @param planetId - Planet id
   * @param cost - Cost object with metal/crystal/deuterium
   * @returns Boolean indicating affordability
   */
  static async canAfford(
    planetId: number,
    cost: { metal: number; crystal: number; deuterium: number }
  ): Promise<boolean> {
    const planet = await this.updateResources(planetId);
    return (
      planet.metal >= cost.metal &&
      planet.crystal >= cost.crystal &&
      planet.deuterium >= cost.deuterium
    );
  }

  /**
   * Deduct resources from a planet (atomic DB update).
   *
   * @param planetId - Planet id
   * @param cost - Cost object with metal/crystal/deuterium to subtract
   */
  static async deductResources(
    planetId: number,
    cost: { metal: number; crystal: number; deuterium: number }
  ): Promise<void> {
    await pool.query(
      `UPDATE planets 
       SET metal = metal - $1, crystal = crystal - $2, deuterium = deuterium - $3
       WHERE id = $4`,
      [cost.metal, cost.crystal, cost.deuterium, planetId]
    );
  }

  /**
   * Add resources to a planet (atomic DB update).
   *
   * @param planetId - Planet id
   * @param resources - Amounts to add
   */
  static async addResources(
    planetId: number,
    resources: { metal: number; crystal: number; deuterium: number }
  ): Promise<void> {
    await pool.query(
      `UPDATE planets 
       SET metal = metal + $1, crystal = crystal + $2, deuterium = deuterium + $3
       WHERE id = $4`,
      [resources.metal, resources.crystal, resources.deuterium, planetId]
    );
  }

  /**
   * Lookup a planet by galaxy/system/position coordinates.
   *
   * @param galaxy - Galaxy number
   * @param system - System number
   * @param position - Position in the system
   * @returns Planet row or null
   */
  static async getPlanetByCoordinates(
    galaxy: number,
    system: number,
    position: number
  ): Promise<Planet | null> {
    const result = await pool.query(
      'SELECT * FROM planets WHERE galaxy = $1 AND system = $2 AND position = $3',
      [galaxy, system, position]
    );
    return result.rows.length > 0 ? result.rows[0] : null;
  }

  /**
   * Create a new colonized planet record for a user. Can accept an
   * optional client (transaction) for caller-managed transactions.
   *
   * @param params - Parameters including userId, coordinates and optional initial resources
   * @param client - Optional PoolClient to run the insert within an existing transaction
   * @returns The newly created Planet row
   */
  static async createColonizedPlanet(
    params: {
      userId: number;
      galaxy: number;
      system: number;
      position: number;
      name?: string;
      initialMetal?: number;
      initialCrystal?: number;
      initialDeuterium?: number;
    },
    client?: PoolClient
  ): Promise<Planet> {
    const {
      userId,
      galaxy,
      system,
      position,
      name,
      initialMetal = 500,
      initialCrystal = 300,
      initialDeuterium = 100,
    } = params;

    const planetName = name || `Colony ${galaxy}:${system}:${position}`;

    const executor = client ?? pool;

    const result = await executor.query(
      `INSERT INTO planets (
        user_id,
        name,
        galaxy,
        system,
        position,
        metal,
        crystal,
        deuterium,
        last_resource_update
      ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW())
      RETURNING *`,
      [userId, planetName, galaxy, system, position, initialMetal, initialCrystal, initialDeuterium]
    );

    return result.rows[0];
  }
}
