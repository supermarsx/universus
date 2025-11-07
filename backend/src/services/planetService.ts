import { pool } from '../config/database';
import { Planet } from '../types';
import {
  calculateBuildingCost,
  calculateBuildingTime,
  calculateStorageCapacity,
  BUILDINGS,
} from '../config/gameConfig';
import { gameConfig } from './gameConfigAdapter';

export class PlanetService {
  static async getPlanetsByUserId(userId: number): Promise<Planet[]> {
    const result = await pool.query(
      'SELECT * FROM planets WHERE user_id = $1 ORDER BY id',
      [userId]
    );
    return result.rows;
  }

  static async getPlanetById(planetId: number): Promise<Planet | null> {
    const result = await pool.query(
      'SELECT * FROM planets WHERE id = $1',
      [planetId]
    );
    return result.rows.length > 0 ? result.rows[0] : null;
  }

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
}
