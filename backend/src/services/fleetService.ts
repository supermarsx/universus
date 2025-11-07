import { pool } from '../config/database';
import { PlanetService } from './planetService';
import { CombatService } from './combatService';
import { SHIPS } from '../config/gameConfig';
import { PoolClient } from 'pg';
import { Fleet } from '../types';

export class FleetService {
  static async dispatchFleet(
    userId: number,
    originPlanetId: number,
    targetGalaxy: number,
    targetSystem: number,
    targetPosition: number,
    missionType: string,
    ships: { [key: string]: number },
    cargo: { metal: number; crystal: number; deuterium: number }
  ): Promise<Fleet> {
    const client = await pool.connect();
    
    try {
      await client.query('BEGIN');

      // Verify origin planet
      const originResult = await client.query(
        'SELECT * FROM planets WHERE id = $1 AND user_id = $2',
        [originPlanetId, userId]
      );

      if (originResult.rows.length === 0) {
        throw new Error('Origin planet not found');
      }

      const originPlanet = originResult.rows[0];

      // Verify ships are available
      for (const [shipType, count] of Object.entries(ships)) {
        if (originPlanet[shipType] < count) {
          throw new Error(`Insufficient ${shipType}`);
        }
      }

      // Calculate fuel consumption and travel time
      const distance = this.calculateDistance(
        originPlanet.galaxy,
        originPlanet.system,
        originPlanet.position,
        targetGalaxy,
        targetSystem,
        targetPosition
      );

      const speed = this.calculateFleetSpeed(ships);
      const travelTime = Math.ceil((distance / speed) * 3600); // in seconds

      // Calculate fuel needed
      let fuelNeeded = 0;
      for (const [shipType, count] of Object.entries(ships)) {
        const shipConfig = SHIPS[shipType];
        if (shipConfig) {
          fuelNeeded += shipConfig.fuelConsumption * count * (distance / 100);
        }
      }

      // Check fuel
      if (originPlanet.deuterium < fuelNeeded + cargo.deuterium) {
        throw new Error('Insufficient deuterium for fuel');
      }

      // Calculate cargo capacity
      let cargoCapacity = 0;
      for (const [shipType, count] of Object.entries(ships)) {
        const shipConfig = SHIPS[shipType];
        if (shipConfig) {
          cargoCapacity += shipConfig.cargo * count;
        }
      }

      cargoCapacity -= fuelNeeded;

      const totalCargo = cargo.metal + cargo.crystal + cargo.deuterium;
      if (totalCargo > cargoCapacity) {
        throw new Error('Insufficient cargo capacity');
      }

      // Deduct ships and resources from origin
      for (const [shipType, count] of Object.entries(ships)) {
        await client.query(
          `UPDATE planets SET ${shipType} = ${shipType} - $1 WHERE id = $2`,
          [count, originPlanetId]
        );
      }

      await client.query(
        `UPDATE planets 
         SET metal = metal - $1, crystal = crystal - $2, deuterium = deuterium - $3
         WHERE id = $4`,
        [cargo.metal, cargo.crystal, fuelNeeded + cargo.deuterium, originPlanetId]
      );

      // Create fleet record
      const arrivalTime = new Date(Date.now() + travelTime * 1000);
      const returnTime = new Date(arrivalTime.getTime() + travelTime * 1000);

      const fleetResult = await client.query(
        `INSERT INTO fleets 
         (user_id, mission_type, origin_planet_id, target_galaxy, target_system, target_position,
          departure_time, arrival_time, return_time, ships, cargo_metal, cargo_crystal, cargo_deuterium, status)
         VALUES ($1, $2, $3, $4, $5, $6, NOW(), $7, $8, $9, $10, $11, $12, 'outbound')
         RETURNING *`,
        [
          userId,
          missionType,
          originPlanetId,
          targetGalaxy,
          targetSystem,
          targetPosition,
          arrivalTime,
          returnTime,
          JSON.stringify(ships),
          cargo.metal,
          cargo.crystal,
          cargo.deuterium,
        ]
      );

      await client.query('COMMIT');

      return fleetResult.rows[0] || null;
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }
  }

  static calculateDistance(
    g1: number,
    s1: number,
    p1: number,
    g2: number,
    s2: number,
    p2: number
  ): number {
    if (g1 !== g2) {
      return Math.abs(g1 - g2) * 20000;
    } else if (s1 !== s2) {
      return Math.abs(s1 - s2) * 5 * 19 + 2700;
    } else {
      return Math.abs(p1 - p2) * 5 + 1000;
    }
  }

  private static calculateFleetSpeed(ships: { [key: string]: number }): number {
    let minSpeed = Infinity;
    
    for (const [shipType, count] of Object.entries(ships)) {
      if (count > 0) {
        const shipConfig = SHIPS[shipType];
        if (shipConfig && shipConfig.baseSpeed < minSpeed) {
          minSpeed = shipConfig.baseSpeed;
        }
      }
    }

    return minSpeed === Infinity ? 0 : minSpeed;
  }

  static async getUserFleets(userId: number): Promise<Fleet[]> {
    const result = await pool.query(
      'SELECT * FROM fleets WHERE user_id = $1 ORDER BY arrival_time',
      [userId]
    );
    return result.rows;
  }

  static async processFleetArrival(fleetId: number): Promise<void> {
    const client = await pool.connect();
    
    try {
      await client.query('BEGIN');

      const fleetResult = await client.query(
        'SELECT * FROM fleets WHERE id = $1',
        [fleetId]
      );

      if (fleetResult.rows.length === 0) {
        throw new Error('Fleet not found');
      }

      const fleet = fleetResult.rows[0];

      switch (fleet.mission_type) {
        case 'attack':
          await this.handleAttackMission(fleet, client);
          break;
        case 'transport':
          await this.handleTransportMission(fleet, client);
          break;
        case 'deploy':
          await this.handleDeployMission(fleet, client);
          break;
      }

      await client.query('COMMIT');
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }
  }

  private static async handleAttackMission(fleet: Fleet, client: PoolClient): Promise<void> {
    // Find target planet
    const targetResult = await client.query(
      'SELECT * FROM planets WHERE galaxy = $1 AND system = $2 AND position = $3',
      [fleet.target_galaxy, fleet.target_system, fleet.target_position]
    );

    if (targetResult.rows.length === 0) {
      // No target, return fleet
      await this.returnFleet(fleet, client);
      return;
    }

    const targetPlanet = targetResult.rows[0];

    // Get attacker and defender tech levels
    const attackerTech = await client.query(
      'SELECT * FROM research WHERE user_id = $1',
      [fleet.user_id]
    );
    const defenderTech = await client.query(
      'SELECT * FROM research WHERE user_id = $1',
      [targetPlanet.user_id]
    );

    // Get defender ships and defenses
    const defenderShips: { [key: string]: number } = {};
    const defenderDefenses: { [key: string]: number } = {};

    for (const key of Object.keys(SHIPS)) {
      if (targetPlanet[key] > 0) {
        defenderShips[key] = targetPlanet[key];
      }
    }

    // Simulate combat
    const combatResult = await CombatService.simulateBattle(
      fleet.ships,
      defenderShips,
      defenderDefenses,
      attackerTech.rows[0] || {},
      defenderTech.rows[0] || {},
      {
        metal: targetPlanet.metal,
        crystal: targetPlanet.crystal,
        deuterium: targetPlanet.deuterium,
      }
    );

    // Save combat report
    const reportId = await CombatService.saveCombatReport(
      fleet.user_id,
      targetPlanet.user_id,
      {
        galaxy: fleet.target_galaxy,
        system: fleet.target_system,
        position: fleet.target_position,
      },
      combatResult
    );

    // Send combat notifications
    const attackerInfo = await pool.query('SELECT username FROM users WHERE id = $1', [fleet.user_id]);
    const defenderInfo = await pool.query('SELECT username FROM users WHERE id = $1', [targetPlanet.user_id]);
    const planetInfo = await pool.query('SELECT name FROM planets WHERE id = $1', [targetPlanet.id]);
    
    if (attackerInfo.rows.length > 0 && defenderInfo.rows.length > 0 && planetInfo.rows.length > 0) {
      await CombatService.sendCombatNotifications(
        fleet.user_id,
        targetPlanet.user_id,
        attackerInfo.rows[0].username,
        defenderInfo.rows[0].username,
        planetInfo.rows[0].name,
        reportId,
        combatResult
      );
    }

    // Update planets based on combat result
    if (combatResult.winner === 'attacker') {
      // Deduct resources from defender
      await client.query(
        `UPDATE planets 
         SET metal = metal - $1, crystal = crystal - $2, deuterium = deuterium - $3
         WHERE id = $4`,
        [combatResult.loot.metal, combatResult.loot.crystal, combatResult.loot.deuterium, targetPlanet.id]
      );

      // Update fleet cargo
      await client.query(
        `UPDATE fleets 
         SET cargo_metal = cargo_metal + $1, 
             cargo_crystal = cargo_crystal + $2, 
             cargo_deuterium = cargo_deuterium + $3,
             status = 'returning'
         WHERE id = $4`,
        [combatResult.loot.metal, combatResult.loot.crystal, combatResult.loot.deuterium, fleet.id]
      );
    }

    // Create debris field
    if (combatResult.debris.metal > 0 || combatResult.debris.crystal > 0) {
      await client.query(
        `INSERT INTO debris_fields (galaxy, system, position, metal, crystal)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (galaxy, system, position) 
         DO UPDATE SET metal = debris_fields.metal + $4, crystal = debris_fields.crystal + $5`,
        [
          fleet.target_galaxy,
          fleet.target_system,
          fleet.target_position,
          combatResult.debris.metal,
          combatResult.debris.crystal,
        ]
      );
    }

    // Update fleet status
    await client.query(
      `UPDATE fleets SET status = 'returning' WHERE id = $1`,
      [fleet.id]
    );
  }

  private static async handleTransportMission(fleet: Fleet, client: PoolClient): Promise<void> {
    const targetResult = await client.query(
      'SELECT * FROM planets WHERE galaxy = $1 AND system = $2 AND position = $3',
      [fleet.target_galaxy, fleet.target_system, fleet.target_position]
    );

    if (targetResult.rows.length > 0) {
      const targetPlanet = targetResult.rows[0];

      await client.query(
        `UPDATE planets 
         SET metal = metal + $1, crystal = crystal + $2, deuterium = deuterium + $3
         WHERE id = $4`,
        [fleet.cargo_metal, fleet.cargo_crystal, fleet.cargo_deuterium, targetPlanet.id]
      );
    }

    await client.query(
      `UPDATE fleets SET status = 'returning', cargo_metal = 0, cargo_crystal = 0, cargo_deuterium = 0 WHERE id = $1`,
      [fleet.id]
    );
  }

  private static async handleDeployMission(fleet: Fleet, client: PoolClient): Promise<void> {
    const targetResult = await client.query(
      'SELECT * FROM planets WHERE galaxy = $1 AND system = $2 AND position = $3 AND user_id = $4',
      [fleet.target_galaxy, fleet.target_system, fleet.target_position, fleet.user_id]
    );

    if (targetResult.rows.length > 0) {
      const targetPlanet = targetResult.rows[0];

      // Move ships to target
      for (const [shipType, count] of Object.entries(fleet.ships)) {
        await client.query(
          `UPDATE planets SET ${shipType} = ${shipType} + $1 WHERE id = $2`,
          [count, targetPlanet.id]
        );
      }

      // Delete fleet
      await client.query('DELETE FROM fleets WHERE id = $1', [fleet.id]);
    } else {
      await this.returnFleet(fleet, client);
    }
  }

  private static async returnFleet(fleet: Fleet, client: PoolClient): Promise<void> {
    await client.query(
      `UPDATE fleets SET status = 'returning' WHERE id = $1`,
      [fleet.id]
    );
  }

  static async recallFleet(userId: number, fleetId: number): Promise<void> {
    await pool.query(
      `UPDATE fleets SET status = 'returning', arrival_time = NOW() WHERE id = $1 AND user_id = $2`,
      [fleetId, userId]
    );
  }
}
