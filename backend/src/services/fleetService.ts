import { pool } from '../config/database';
import { PlanetService } from './planetService';
import { CombatService } from './combatService';
import { SHIPS } from '../config/gameConfig';
import { PoolClient } from 'pg';
import { Fleet } from '../types';
import { getRealtimeHandler } from '../socket';
import notificationService from './notificationService';
import { CombatResult } from '../services/combatService';

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

      const fleetRecord = fleetResult.rows[0] || null;

      await client.query('COMMIT');

      if (fleetRecord) {
        this.emitFleetEvent(userId, {
          action: 'dispatch',
          fleet: fleetRecord,
        });
      }

      return fleetRecord;
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

  static async getUserFleets(userId: number): Promise<any[]> {
    const result = await pool.query(
      `SELECT 
        f.*,
        p.name AS origin_planet_name,
        p.galaxy AS origin_galaxy,
        p.system AS origin_system,
        p.position AS origin_position
      FROM fleets f
      LEFT JOIN planets p ON p.id = f.origin_planet_id
      WHERE f.user_id = $1
      ORDER BY f.departure_time DESC`,
      [userId]
    );

    const now = Date.now();

    return result.rows.map((row) => {
      const arrival = new Date(row.arrival_time).getTime();
      const returnTime = row.return_time ? new Date(row.return_time).getTime() : null;

      return {
        ...row,
        ships: typeof row.ships === 'string' ? JSON.parse(row.ships) : row.ships,
        secondsUntilArrival: Math.max(0, Math.ceil((arrival - now) / 1000)),
        secondsUntilReturn: returnTime ? Math.max(0, Math.ceil((returnTime - now) / 1000)) : null,
      };
    });
  }

  static async getRecentCombatReports(userId: number, limit = 5): Promise<any[]> {
    const result = await pool.query(
      `SELECT 
        cr.*,
        au.username AS attacker_name,
        du.username AS defender_name
       FROM combat_reports cr
       LEFT JOIN users au ON au.id = cr.attacker_id
       LEFT JOIN users du ON du.id = cr.defender_id
       WHERE cr.attacker_id = $1 OR cr.defender_id = $1
       ORDER BY cr.battle_time DESC
       LIMIT $2`,
      [userId, limit]
    );

    return result.rows.map((row) => ({
      id: row.id,
      attackerId: row.attacker_id,
      defenderId: row.defender_id,
      attacker: row.attacker_name || 'Unknown',
      defender: row.defender_name || 'Unknown',
      coordinates: {
        galaxy: row.planet_galaxy,
        system: row.planet_system,
        position: row.planet_position,
      },
      winner: row.winner,
      attackerLosses: this.safeParse(row.attacker_losses),
      defenderLosses: this.safeParse(row.defender_losses),
      loot: {
        metal: row.loot_metal,
        crystal: row.loot_crystal,
        deuterium: row.loot_deuterium,
      },
      debris: {
        metal: row.debris_metal,
        crystal: row.debris_crystal,
      },
      battleTime: row.battle_time,
    }));
  }

  static async getMissionHistory(userId: number, limit = 25): Promise<any[]> {
    const result = await pool.query(
      `SELECT f.*, p.name as origin_planet_name
       FROM fleets f
       LEFT JOIN planets p ON p.id = f.origin_planet_id
       WHERE f.user_id = $1
       ORDER BY f.departure_time DESC
       LIMIT $2`,
      [userId, limit]
    );

    return result.rows.map((row) => ({
      ...row,
      ships: this.safeParse(row.ships),
      createdAt: row.departure_time,
    }));
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

      this.emitFleetEvent(fleet.user_id, {
        action: 'arrival',
        fleetId: fleet.id,
      });

      await notificationService.notifyFleetArrived(
        fleet.user_id,
        fleet.id,
        this.formatLocation(fleet.target_galaxy, fleet.target_system, fleet.target_position)
      );
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

    const summary = this.buildCombatSummary(reportId, fleet, targetPlanet.user_id, combatResult);
    this.emitFleetEvent(fleet.user_id, {
      action: 'combat',
      role: 'attacker',
      report: summary,
    });

    if (targetPlanet.user_id) {
      this.emitFleetEvent(targetPlanet.user_id, {
        action: 'combat',
        role: 'defender',
        report: summary,
      });
    }

    await notificationService.notifyCombatReport(
      fleet.user_id,
      reportId,
      combatResult.winner,
      this.formatLocation(fleet.target_galaxy, fleet.target_system, fleet.target_position)
    );

    if (targetPlanet.user_id) {
      await notificationService.notifyCombatReport(
        targetPlanet.user_id,
        reportId,
        combatResult.winner,
        this.formatLocation(fleet.target_galaxy, fleet.target_system, fleet.target_position)
      );
    }
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

    this.emitFleetEvent(userId, {
      action: 'recall',
      fleetId,
    });
  }

  private static emitFleetEvent(userId: number, payload: any): void {
    const handler = getRealtimeHandler();
    if (!handler) return;

    handler.emitFleetUpdate(userId, payload);
  }

  private static safeParse(value: any): any {
    if (!value) return {};
    if (typeof value === 'object') return value;
    try {
      return JSON.parse(value);
    } catch {
      return {};
    }
  }

  private static buildCombatSummary(
    reportId: number,
    fleet: Fleet,
    defenderId: number | null,
    result: CombatResult
  ) {
    return {
      id: reportId,
      mission: fleet.mission_type,
      target: {
        galaxy: fleet.target_galaxy,
        system: fleet.target_system,
        position: fleet.target_position,
      },
      attackerId: fleet.user_id,
      defenderId,
      winner: result.winner,
      loot: result.loot,
      attackerLosses: result.attackerLosses,
      defenderLosses: result.defenderLosses,
      timestamp: new Date().toISOString(),
    };
  }

  private static formatLocation(galaxy: number, system: number, position: number): string {
    return `${galaxy}:${system}:${position}`;
  }
}
