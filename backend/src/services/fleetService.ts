/**
 * @module backend/services/fleetService
 *
 * FleetService handles fleet dispatching, mission processing and fleet state
 * transitions. It performs validations, persists fleet movements and invokes
 * combat/espionage/harvest handlers as missions complete.
 */
import { pool } from '../config/database';
import { PlanetService } from './planetService';
import { CombatService } from './combatService';
import { SHIPS, DEFENSES } from '../config/gameConfig';
import { PoolClient } from 'pg';
import { Fleet, Planet } from '../types';
import { getRealtimeHandler } from '../socket';
import notificationService from './notificationService';
import fleetScheduler from './fleetScheduler';
import { CombatResult } from '../services/combatService';
import moonService from './moonService';
import { ResearchService } from './researchService';
import { MessagingService } from './messagingService';
import { gameConfig } from './gameConfigAdapter';

interface FleetParticipant {
  fleet: Fleet;
  ships: { [key: string]: number };
}

const PLANET_SHIP_KEYS = [
  'small_cargo',
  'large_cargo',
  'light_fighter',
  'heavy_fighter',
  'cruiser',
  'battleship',
  'bomber',
  'destroyer',
  'colony_ship',
  'recycler',
  'espionage_probe',
  'deathstar',
];

const PLANET_DEFENSE_KEYS = [
  'rocket_launcher',
  'light_laser',
  'heavy_laser',
  'gauss_cannon',
  'ion_cannon',
  'plasma_turret',
  'small_shield_dome',
  'large_shield_dome',
];

const PLANET_BUILDING_KEYS = [
  'metal_mine',
  'crystal_mine',
  'deuterium_synthesizer',
  'solar_plant',
  'fusion_reactor',
  'shipyard',
  'robotics_factory',
  'nanite_factory',
  'research_lab',
  'missile_silo',
];

const messagingService = new MessagingService(pool);

/**
 * Coordinator for fleet operations (dispatch, arrival processing, returns).
 * Public APIs are mostly static helpers used by controllers and schedulers.
 */
export class FleetService {
  /**
   * Dispatch a fleet from an origin planet to a target coordinate.
   * Validates ships, fuel and cargo, persists the fleet and schedules arrival.
   *
   * @returns The created Fleet record
   */
  static async dispatchFleet(
    userId: number,
    originPlanetId: number,
    targetGalaxy: number,
    targetSystem: number,
    targetPosition: number,
    missionType: string,
    ships: { [key: string]: number },
    cargo: { metal: number; crystal: number; deuterium: number },
    acsGroupId?: number
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

      await this.validateMissionRequest({
        userId,
        missionType,
        ships,
        targetGalaxy,
        targetSystem,
        targetPosition,
      });

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
          departure_time, arrival_time, return_time, ships, cargo_metal, cargo_crystal, cargo_deuterium, status, acs_group_id)
         VALUES ($1, $2, $3, $4, $5, $6, NOW(), $7, $8, $9, $10, $11, $12, 'outbound', $13)
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
          acsGroupId ?? null,
        ]
      );

      const fleetRecord = fleetResult.rows[0] || null;

      await client.query('COMMIT');

      if (fleetRecord) {
        this.emitFleetEvent(userId, {
          action: 'dispatch',
          fleet: fleetRecord,
        });
        this.scheduleArrivalEvent(fleetRecord.id, fleetRecord.arrival_time);
      }

      return fleetRecord;
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }
  }

  /**
   * Compute a heuristic distance value between two coordinates. Used to
   * translate to travel time based on ship speeds.
   */
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

  /**
   * Return fleets owned by a user along with ETA metadata suitable for the UI.
   */
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
      const arrival = row.arrival_time ? new Date(row.arrival_time).getTime() : null;
      const returnTime = row.return_time ? new Date(row.return_time).getTime() : null;
      const etaMs = arrival ? Math.max(0, arrival - now) : null;
      const returnEtaMs = returnTime ? Math.max(0, returnTime - now) : null;

      return {
        ...row,
        ships: typeof row.ships === 'string' ? JSON.parse(row.ships) : row.ships,
        arrivalTimestamp: arrival,
        returnTimestamp: returnTime,
        etaMs,
        returnEtaMs,
        secondsUntilArrival: etaMs !== null ? Math.max(0, Math.ceil(etaMs / 1000)) : null,
        secondsUntilReturn: returnEtaMs !== null ? Math.max(0, Math.ceil(returnEtaMs / 1000)) : null,
      };
    });
  }

  /**
   * Fetch recent combat reports involving the specified user.
   */
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
      attackerAllies: this.safeParse(row.attacker_allies) || [],
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

  /**
   * Process an arriving fleet: route to the correct mission handler and
   * commit mission side-effects (combat, colonization, harvest, espionage).
   * This method is invoked by the scheduler when an arrival event fires.
   */
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
      const targetLocation = this.formatLocation(
        fleet.target_galaxy,
        fleet.target_system,
        fleet.target_position
      );

      let missionResult: MissionOutcome | null = null;

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
        case 'colonize':
          missionResult = await this.handleColonizeMission(fleet, client);
          break;
        case 'espionage':
          missionResult = await this.handleEspionageMission(fleet, client);
          break;
        case 'harvest':
          missionResult = await this.handleHarvestMission(fleet, client);
          break;
      }

      await client.query('COMMIT');

      this.handleMissionOutcome(fleet, missionResult, targetLocation);
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }
  }

  private static async handleMissionOutcome(
    fleet: Fleet,
    outcome: MissionOutcome | null,
    targetLocation: string
  ): Promise<void> {
    if (!outcome) {
      this.emitFleetEvent(fleet.user_id, {
        action: 'arrival',
        fleetId: fleet.id,
      });

      await notificationService.notifyFleetArrived(fleet.user_id, fleet.id, targetLocation);
      return;
    }

    switch (outcome.type) {
      case 'colonize':
        this.emitFleetEvent(fleet.user_id, {
          action: 'colonize',
          status: outcome.success ? 'success' : 'failed',
          fleetId: fleet.id,
          planetId: outcome.newPlanet?.id ?? null,
          planet: outcome.newPlanet ?? null,
        });

        await notificationService.notifyColonizationResult(
          fleet.user_id,
          targetLocation,
          outcome.success,
          outcome.newPlanet?.id
        );
        break;
      case 'espionage':
        this.emitFleetEvent(fleet.user_id, {
          action: 'espionage',
          success: outcome.success,
          detected: outcome.detected,
          intelLevel: outcome.intelLevel,
        });
        break;
      case 'harvest':
        this.emitFleetEvent(fleet.user_id, {
          action: 'harvest',
          fleetId: fleet.id,
          collected: outcome.collected,
          empty: outcome.empty ?? false,
        });

        await notificationService.notifyFleetArrived(fleet.user_id, fleet.id, targetLocation);
        break;
    }
  }

  private static async handleAttackMission(fleet: Fleet, client: PoolClient): Promise<void> {
    const targetResult = await client.query(
      'SELECT * FROM planets WHERE galaxy = $1 AND system = $2 AND position = $3',
      [fleet.target_galaxy, fleet.target_system, fleet.target_position]
    );

    if (targetResult.rows.length === 0) {
      await this.returnFleet(fleet, client);
      return;
    }

    const targetPlanet = targetResult.rows[0];
    const participants = await this.collectAttackParticipants(fleet, client);
    const participantUserIds = participants.map((p) => p.fleet.user_id);

    const attackerTechRows = await client.query(
      'SELECT * FROM research WHERE user_id = ANY($1)',
      [participantUserIds]
    );
    const defenderTech = await client.query(
      'SELECT * FROM research WHERE user_id = $1',
      [targetPlanet.user_id]
    );

    const defenderShips: { [key: string]: number } = {};
    const defenderDefenses: { [key: string]: number } = {};

    for (const key of Object.keys(SHIPS)) {
      if (targetPlanet[key] > 0) {
        defenderShips[key] = targetPlanet[key];
      }
    }

    for (const key of Object.keys(DEFENSES)) {
      if (targetPlanet[key] > 0) {
        defenderDefenses[key] = targetPlanet[key];
      }
    }

    const aggregateShips = this.combineFleetShips(participants);
    const combatResult = await CombatService.simulateBattle(
      aggregateShips,
      defenderShips,
      defenderDefenses,
      this.mergeTechLevels(attackerTechRows.rows),
      defenderTech.rows[0] || {},
      {
        metal: targetPlanet.metal,
        crystal: targetPlanet.crystal,
        deuterium: targetPlanet.deuterium,
      },
      targetPlanet.id,
      // Use fleet id as the deterministic seed for reproducible combat outcomes
      fleet.id
    );

    await this.applyDefenderLosses(targetPlanet.id, targetPlanet, combatResult, client);

    if (combatResult.winner === 'attacker') {
      await client.query(
        `UPDATE planets 
         SET metal = metal - $1, crystal = crystal - $2, deuterium = deuterium - $3
         WHERE id = $4`,
        [combatResult.loot.metal, combatResult.loot.crystal, combatResult.loot.deuterium, targetPlanet.id]
      );
    }

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

    await moonService.tryCreateMoonFromDebris(
      targetPlanet.id,
      targetPlanet.user_id,
      combatResult.debris.metal,
      combatResult.debris.crystal
    );

    await this.updateAttackerFleetsAfterCombat(participants, combatResult, client);

    const usernameMap = await this.fetchUsernames([
      ...participantUserIds,
      targetPlanet.user_id,
    ]);

    const attackerAlliesMeta = participants
      .filter((participant) => participant.fleet.user_id !== fleet.user_id)
      .map((participant) => ({
        userId: participant.fleet.user_id,
        username: usernameMap[participant.fleet.user_id] || 'Unknown Commander',
      }));

    const reportId = await CombatService.saveCombatReport(
      fleet.user_id,
      targetPlanet.user_id,
      {
        galaxy: fleet.target_galaxy,
        system: fleet.target_system,
        position: fleet.target_position,
      },
      combatResult,
      attackerAlliesMeta
    );

    const attackerUsername =
      usernameMap[fleet.user_id] ||
      (await pool
        .query('SELECT username FROM users WHERE id = $1', [fleet.user_id])
        .then((r) => r.rows[0]?.username || 'Commander'));
    const defenderUsername =
      usernameMap[targetPlanet.user_id] ||
      (await pool
        .query('SELECT username FROM users WHERE id = $1', [targetPlanet.user_id])
        .then((r) => r.rows[0]?.username || 'Unknown'));
    const planetInfo = await pool.query('SELECT name FROM planets WHERE id = $1', [targetPlanet.id]);

    if (planetInfo.rows.length > 0) {
      await CombatService.sendCombatNotifications(
        fleet.user_id,
        targetPlanet.user_id,
        attackerUsername,
        defenderUsername,
        planetInfo.rows[0].name,
        reportId,
        combatResult
      );
    }

    const summary = this.buildCombatSummary(reportId, fleet, targetPlanet.user_id, combatResult, attackerAlliesMeta);
    const location = this.formatLocation(fleet.target_galaxy, fleet.target_system, fleet.target_position);

    const notifiedAttackers = new Set<number>();
    for (const participant of participants) {
      if (!notifiedAttackers.has(participant.fleet.user_id)) {
        notifiedAttackers.add(participant.fleet.user_id);
        this.emitFleetEvent(participant.fleet.user_id, {
          action: 'combat',
          role: 'attacker',
          report: summary,
        });
        await notificationService.notifyCombatReport(
          participant.fleet.user_id,
          reportId,
          combatResult.winner,
          location
        );
      }
    }

    if (targetPlanet.user_id) {
      this.emitFleetEvent(targetPlanet.user_id, {
        action: 'combat',
        role: 'defender',
        report: summary,
      });
      await notificationService.notifyCombatReport(
        targetPlanet.user_id,
        reportId,
        combatResult.winner,
        location
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

    this.scheduleReturnEvent(fleet.id, fleet.return_time);
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

  private static async handleColonizeMission(fleet: Fleet, client: PoolClient): Promise<ColonizeMissionResult> {
    const ships = this.safeParse(fleet.ships);
    const colonyShips = ships.colony_ship || 0;

    if (colonyShips <= 0) {
      await this.returnFleet(fleet, client);
      return { type: 'colonize', success: false, failureReason: 'invalid' };
    }

    const [planetCount, limit] = await Promise.all([
      PlanetService.getPlanetCountByUserId(fleet.user_id),
      gameConfig.getColonizationLimit(),
    ]);

    if (planetCount >= limit) {
      await this.returnFleet(fleet, client);
      return { type: 'colonize', success: false, failureReason: 'limit_reached' };
    }

    const targetPlanet = await PlanetService.getPlanetByCoordinates(
      fleet.target_galaxy,
      fleet.target_system,
      fleet.target_position
    );

    if (targetPlanet) {
      await this.returnFleet(fleet, client);
      return { type: 'colonize', success: false, failureReason: 'slot_taken' };
    }

    const newPlanet = await PlanetService.createColonizedPlanet(
      {
        userId: fleet.user_id,
        galaxy: fleet.target_galaxy,
        system: fleet.target_system,
        position: fleet.target_position,
        initialMetal: 500 + (fleet.cargo_metal || 0),
        initialCrystal: 300 + (fleet.cargo_crystal || 0),
        initialDeuterium: 100 + (fleet.cargo_deuterium || 0),
      },
      client
    );

    await client.query('DELETE FROM fleets WHERE id = $1', [fleet.id]);

    return { type: 'colonize', success: true, newPlanet };
  }

  private static async handleEspionageMission(
    fleet: Fleet,
    client: PoolClient
  ): Promise<EspionageMissionResult> {
    const ships = this.safeParse(fleet.ships);
    const probes = ships.espionage_probe || 0;

    if (probes <= 0) {
      await this.returnFleet(fleet, client);
      return { type: 'espionage', success: false, detected: false, intelLevel: 'none' };
    }

    const targetResult = await client.query(
      `SELECT p.*, u.username AS owner_username
       FROM planets p
       LEFT JOIN users u ON u.id = p.user_id
       WHERE p.galaxy = $1 AND p.system = $2 AND p.position = $3
       FOR UPDATE`,
      [fleet.target_galaxy, fleet.target_system, fleet.target_position]
    );

    if (targetResult.rows.length === 0) {
      await client.query('DELETE FROM fleets WHERE id = $1', [fleet.id]);
      return { type: 'espionage', success: false, detected: false, intelLevel: 'none' };
    }

    const targetPlanet = targetResult.rows[0];
    const [spyResearch, defenderResearch] = await Promise.all([
      ResearchService.getUserResearch(fleet.user_id),
      targetPlanet.user_id ? ResearchService.getUserResearch(targetPlanet.user_id) : null,
    ]);

    const attackerEspionage = spyResearch?.espionage_technology || 0;
    const defenderEspionage = defenderResearch?.espionage_technology || 0;

    const detailScore = attackerEspionage + Math.log2(probes + 1);
    const defenseScore = defenderEspionage;
    const detailDelta = detailScore - defenseScore;

    let intelLevel: EspionageIntelLevel = 'minimal';
    if (detailDelta >= 3) {
      intelLevel = 'full';
    } else if (detailDelta >= 0) {
      intelLevel = 'standard';
    }

    const detectionChance = Math.max(
      0.05,
      Math.min(0.95, 0.5 + (defenseScore - detailScore) * 0.05)
    );
    const detected = Math.random() < detectionChance;

    const report = this.buildEspionageReport(targetPlanet, intelLevel);
    report.detected = detected;
    report.probes = probes;
    report.intelLevel = intelLevel;

    if (targetPlanet.user_id) {
      await messagingService.sendEspionageReport(fleet.user_id, targetPlanet.user_id, report);
    }

    await client.query('DELETE FROM fleets WHERE id = $1', [fleet.id]);

    return {
      type: 'espionage',
      success: true,
      detected,
      intelLevel,
      reportSummary: {
        resources: report.resources,
        intelLevel,
        detected,
      },
    };
  }

  private static async handleHarvestMission(
    fleet: Fleet,
    client: PoolClient
  ): Promise<HarvestMissionResult> {
    const ships = this.safeParse(fleet.ships);
    const recyclerCount = ships.recycler || 0;

    if (recyclerCount <= 0) {
      await this.returnFleet(fleet, client);
      return { type: 'harvest', collected: { metal: 0, crystal: 0 }, empty: true };
    }

    const debrisResult = await client.query(
      `SELECT * FROM debris_fields
       WHERE galaxy = $1 AND system = $2 AND position = $3
       FOR UPDATE`,
      [fleet.target_galaxy, fleet.target_system, fleet.target_position]
    );

    if (debrisResult.rows.length === 0) {
      await this.returnFleet(fleet, client);
      return { type: 'harvest', collected: { metal: 0, crystal: 0 }, empty: true };
    }

    const debris = debrisResult.rows[0];
    const recyclerCapacity = (SHIPS.recycler?.cargo || 0) * recyclerCount;
    let remainingCapacity = recyclerCapacity;

    const collected = { metal: 0, crystal: 0 };

    if (debris.metal > 0) {
      collected.metal = Math.min(debris.metal, remainingCapacity);
      remainingCapacity -= collected.metal;
    }

    if (remainingCapacity > 0 && debris.crystal > 0) {
      collected.crystal = Math.min(debris.crystal, remainingCapacity);
      remainingCapacity -= collected.crystal;
    }

    const updatedMetal = Math.max(0, debris.metal - collected.metal);
    const updatedCrystal = Math.max(0, debris.crystal - collected.crystal);

    if (updatedMetal === 0 && updatedCrystal === 0) {
      await client.query('DELETE FROM debris_fields WHERE id = $1', [debris.id]);
    } else {
      await client.query(
        'UPDATE debris_fields SET metal = $1, crystal = $2 WHERE id = $3',
        [updatedMetal, updatedCrystal, debris.id]
      );
    }

    await client.query(
      `UPDATE fleets 
         SET status = 'returning',
             cargo_metal = cargo_metal + $1,
             cargo_crystal = cargo_crystal + $2
       WHERE id = $3`,
      [collected.metal, collected.crystal, fleet.id]
    );

    this.scheduleReturnEvent(fleet.id, fleet.return_time);

    return {
      type: 'harvest',
      collected,
      empty: collected.metal === 0 && collected.crystal === 0,
    };
  }

  private static buildEspionageReport(targetPlanet: any, intelLevel: EspionageIntelLevel) {
    const report: any = {
      coordinates: {
        galaxy: targetPlanet.galaxy,
        system: targetPlanet.system,
        position: targetPlanet.position,
      },
      planetName: targetPlanet.name,
      ownerId: targetPlanet.user_id,
      ownerName: targetPlanet.owner_username,
      resources: {
        metal: targetPlanet.metal,
        crystal: targetPlanet.crystal,
        deuterium: targetPlanet.deuterium,
        energy: targetPlanet.energy,
      },
    };

    if (intelLevel !== 'minimal' && intelLevel !== 'none') {
      report.ships = this.extractPlanetCounts(targetPlanet, PLANET_SHIP_KEYS);
    }

    if (intelLevel === 'full') {
      report.defenses = this.extractPlanetCounts(targetPlanet, PLANET_DEFENSE_KEYS);
      report.buildings = this.extractPlanetCounts(targetPlanet, PLANET_BUILDING_KEYS);
    }

    return report;
  }

  private static extractPlanetCounts(source: any, keys: string[]): Record<string, number> {
    const result: Record<string, number> = {};
    keys.forEach((key) => {
      const value = Number(source[key] || 0);
      if (value > 0) {
        result[key] = value;
      }
    });
    return result;
  }

  private static async returnFleet(fleet: Fleet, client: PoolClient): Promise<void> {
    await client.query(
      `UPDATE fleets SET status = 'returning' WHERE id = $1`,
      [fleet.id]
    );
    this.scheduleReturnEvent(fleet.id, fleet.return_time);
  }

  static async recallFleet(userId: number, fleetId: number): Promise<void> {
    const result = await pool.query(
      `UPDATE fleets 
       SET status = 'returning', return_time = NOW()
       WHERE id = $1 AND user_id = $2
       RETURNING return_time`,
      [fleetId, userId]
    );
    if ((result.rowCount ?? 0) > 0) {
      this.unscheduleFleetEvents(fleetId, 'arrival');
      this.scheduleReturnEvent(fleetId, result.rows[0].return_time);
    }

    this.emitFleetEvent(userId, {
      action: 'recall',
      fleetId,
    });
  }

  /**
   * Cancel an outbound fleet if the requesting user owns it.
   * Returns the updated fleet row or null when no fleet is affected.
   */
  static async cancelFleet(userId: number, fleetId: number): Promise<any> {
    const result = await pool.query(
      `UPDATE fleets
       SET status = 'cancelled'
       WHERE id = $1 AND user_id = $2
       RETURNING *`,
      [fleetId, userId]
    );

    if ((result.rowCount ?? 0) > 0) {
      // Unschedule any pending events for this fleet and notify the user
      this.unscheduleFleetEvents(fleetId);
      this.emitFleetEvent(userId, {
        action: 'cancel',
        fleetId,
      });
      return result.rows[0];
    }

    return null;
  }

  private static emitFleetEvent(userId: number, payload: any): void {
    const handler = getRealtimeHandler();
    if (!handler) return;

    handler.emitFleetUpdate(userId, payload);
  }

  /**
   * Finalize a returning fleet: add ships/resources back to the origin
   * planet and emit realtime events/notifications.
   */
  static async completeFleetReturn(fleetId: number): Promise<void> {
    const client = await pool.connect();

    try {
      await client.query('BEGIN');

      const fleetResult = await client.query('SELECT * FROM fleets WHERE id = $1', [fleetId]);
      if (fleetResult.rows.length === 0) {
        await client.query('ROLLBACK');
        return;
      }

      const fleet = fleetResult.rows[0];
      if (fleet.status !== 'returning') {
        await client.query('ROLLBACK');
        return;
      }

      const planetResult = await client.query(
        'SELECT galaxy, system, position, name FROM planets WHERE id = $1',
        [fleet.origin_planet_id]
      );

      const ships = this.safeParse(fleet.ships);
      for (const [shipType, count] of Object.entries(ships)) {
        if (!count) continue;
        await client.query(
          `UPDATE planets SET ${shipType} = ${shipType} + $1 WHERE id = $2`,
          [count, fleet.origin_planet_id]
        );
      }

      await client.query(
        `UPDATE planets 
         SET metal = metal + $1, crystal = crystal + $2, deuterium = deuterium + $3
         WHERE id = $4`,
        [fleet.cargo_metal, fleet.cargo_crystal, fleet.cargo_deuterium, fleet.origin_planet_id]
      );

      await client.query('DELETE FROM fleets WHERE id = $1', [fleetId]);
      await client.query('COMMIT');

      const planet = planetResult.rows[0];
      const location = planet
        ? `${planet.name || 'Planet'} (${planet.galaxy}:${planet.system}:${planet.position})`
        : `Planet ${fleet.origin_planet_id}`;

      await notificationService.notifyFleetReturned(fleet.user_id, fleet.id, location);
      this.emitFleetEvent(fleet.user_id, {
        action: 'return',
        fleetId,
        location,
      });
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }
  }

  private static async validateMissionRequest(params: MissionValidationParams): Promise<void> {
    const mission = params.missionType;

    switch (mission) {
      case 'colonize': {
        const colonyShips = Number(params.ships.colony_ship || 0);
        if (colonyShips < 1) {
          throw new Error('Colonize mission requires at least one Colony Ship');
        }

        const hasOtherShips = Object.entries(params.ships).some(
          ([type, count]) => type !== 'colony_ship' && Number(count) > 0
        );

        if (hasOtherShips) {
          throw new Error('Colonize missions can only include Colony Ships');
        }

        const [planetCount, limit] = await Promise.all([
          PlanetService.getPlanetCountByUserId(params.userId),
          gameConfig.getColonizationLimit(),
        ]);

        if (planetCount >= limit) {
          throw new Error('Planet limit reached. Research Astrophysics to unlock more colonies.');
        }

        const existing = await PlanetService.getPlanetByCoordinates(
          params.targetGalaxy,
          params.targetSystem,
          params.targetPosition
        );

        if (existing) {
          throw new Error('Target coordinates already contain a planet');
        }

        break;
      }
      case 'espionage': {
        const probes = Number(params.ships.espionage_probe || 0);
        if (probes < 1) {
          throw new Error('Espionage mission requires at least one Espionage Probe');
        }

        const hasOtherShips = Object.entries(params.ships).some(
          ([type, count]) => type !== 'espionage_probe' && Number(count) > 0
        );

        if (hasOtherShips) {
          throw new Error('Espionage missions only allow probes');
        }

        const target = await PlanetService.getPlanetByCoordinates(
          params.targetGalaxy,
          params.targetSystem,
          params.targetPosition
        );

        if (!target) {
          throw new Error('No planet at those coordinates to scout');
        }

        break;
      }
      case 'harvest': {
        const recyclerCount = Number(params.ships.recycler || 0);
        if (recyclerCount < 1) {
          throw new Error('Harvest mission requires at least one Recycler');
        }

        const debrisResult = await pool.query(
          'SELECT 1 FROM debris_fields WHERE galaxy = $1 AND system = $2 AND position = $3',
          [params.targetGalaxy, params.targetSystem, params.targetPosition]
        );

        if (debrisResult.rows.length === 0) {
          throw new Error('No debris field detected at those coordinates');
        }

        break;
      }
      default:
        break;
    }
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

  /**
   * Safely parse a JSON string or return the object as-is.
   * Useful for fields that may be stored as JSON text in the DB or as objects.
   *
   * @private
   * @param {any} value - Value that may be a JSON string or already an object
   * @returns {any} Parsed object or empty object on failure
   */

  private static async collectAttackParticipants(fleet: Fleet, client: PoolClient): Promise<FleetParticipant[]> {
    const participants: FleetParticipant[] = [
      { fleet, ships: this.safeParse(fleet.ships) }
    ];

    if (!fleet.acs_group_id) {
      return participants;
    }

    const alliedResult = await client.query(
      `SELECT * FROM fleets 
       WHERE acs_group_id = $1
         AND status = 'outbound'
         AND id <> $2
         AND target_galaxy = $3
         AND target_system = $4
         AND target_position = $5
         AND arrival_time <= NOW()
       FOR UPDATE`,
      [
        fleet.acs_group_id,
        fleet.id,
        fleet.target_galaxy,
        fleet.target_system,
        fleet.target_position,
      ]
    );

    alliedResult.rows.forEach((row) => {
      participants.push({ fleet: row, ships: this.safeParse(row.ships) });
    });

    return participants;
  }

  /**
   * Assemble participants in an attack including ACS members if present.
   *
   * @private
   * @param {Fleet} fleet - The initiating fleet record
   * @param {PoolClient} client - Transactional PG client (for FOR UPDATE queries)
   * @returns {Promise<FleetParticipant[]>} Array of participating fleets with parsed ships
   */

  private static combineFleetShips(participants: FleetParticipant[]): { [key: string]: number } {
    const totals: { [key: string]: number } = {};
    participants.forEach((participant) => {
      Object.entries(participant.ships).forEach(([type, count]) => {
        totals[type] = (totals[type] || 0) + (count as number);
      });
    });
    return totals;
  }

  /**
   * Sum ship counts across multiple participants into a single ship map.
   *
   * @private
   * @param {FleetParticipant[]} participants - Fleets participating in the action
   * @returns {{ [key: string]: number }} Aggregated ship counts by type
   */

  private static mergeTechLevels(rows: any[]): any {
    const merged: Record<string, number> = {};
    rows.forEach((row) => {
      if (!row) return;
      Object.entries(row).forEach(([key, value]) => {
        if (typeof value === 'number') {
          merged[key] = Math.max(merged[key] ?? 0, value);
        }
      });
    });
    return merged;
  }

  /**
   * Merge multiple technology level records, taking the max per technology.
   *
   * @private
   * @param {any[]} rows - Array of tech level maps
   * @returns {Record<string, number>} Merged tech levels
   */

  private static async fetchUsernames(userIds: Array<number | null | undefined>): Promise<Record<number, string>> {
    const ids = Array.from(
      new Set(
        userIds.filter((id): id is number => typeof id === 'number')
      )
    );

    if (!ids.length) {
      return {};
    }

    const result = await pool.query(
      'SELECT id, username FROM users WHERE id = ANY($1)',
      [ids]
    );

    const map: Record<number, string> = {};
    result.rows.forEach((row) => {
      map[row.id] = row.username;
    });
    return map;
  }

  /**
   * Resolve a list of user ids to a map of username strings.
   *
   * @private
   * @param {(number | null | undefined)[]} userIds - Array of user ids (may include null/undefined)
   * @returns {Promise<Record<number,string>>} Map from id to username
   */

  private static async updateAttackerFleetsAfterCombat(
    participants: FleetParticipant[],
    result: CombatResult,
    client: PoolClient
  ): Promise<void> {
    const totalLosses = result.attackerLosses || {};
    const totals = this.combineFleetShips(participants);
    const lossAllocations = this.allocateLosses(participants, totalLosses, totals);
    const lootPool = result.winner === 'attacker' ? result.loot : { metal: 0, crystal: 0, deuterium: 0 };
    const lootShares = this.splitLoot(lootPool, participants.length);

    for (let i = 0; i < participants.length; i++) {
      const participant = participants[i];
      const losses = lossAllocations[i] || {};
      const survivors: { [key: string]: number } = {};
      Object.entries(participant.ships).forEach(([type, count]) => {
        const remaining = (count as number) - (losses[type] || 0);
        if (remaining > 0) {
          survivors[type] = remaining;
        }
      });

      const loot = lootShares[i] || { metal: 0, crystal: 0, deuterium: 0 };

      await client.query(
        `UPDATE fleets 
         SET ships = $1,
             cargo_metal = cargo_metal + $2,
             cargo_crystal = cargo_crystal + $3,
             cargo_deuterium = cargo_deuterium + $4,
             status = 'returning'
         WHERE id = $5`,
        [
          JSON.stringify(survivors),
          loot.metal,
          loot.crystal,
          loot.deuterium,
          participant.fleet.id,
        ]
      );

      this.scheduleReturnEvent(participant.fleet.id, participant.fleet.return_time);
    }
  }

  /**
   * Apply losses to attacker fleets after combat and allocate loot.
   *
   * This function updates fleet records in-place using the provided client
   * within a transaction.
   *
   * @private
   * @param {FleetParticipant[]} participants - Fleets engaged as attackers
   * @param {CombatResult} result - Result object from the combat simulation
   * @param {PoolClient} client - PG client used for transactional updates
   */

  private static allocateLosses(
    participants: FleetParticipant[],
    totalLosses: { [key: string]: number },
    totals: { [key: string]: number }
  ): Array<{ [key: string]: number }> {
    const allocations = participants.map(() => ({} as { [key: string]: number }));
    const allocated: Record<string, number> = {};
    const types = Object.keys(totalLosses);

    participants.forEach((participant, index) => {
      types.forEach((type) => {
        const totalLoss = totalLosses[type] || 0;
        if (totalLoss === 0) {
          allocations[index][type] = 0;
          return;
        }
        const fleetCount = participant.ships[type] || 0;
        const totalCount = totals[type] || 0;
        if (fleetCount === 0 || totalCount === 0) {
          allocations[index][type] = 0;
          return;
        }

        let loss: number;
        if (index === participants.length - 1) {
          const remaining = totalLoss - (allocated[type] || 0);
          loss = Math.min(fleetCount, Math.max(remaining, 0));
        } else {
          loss = Math.min(
            fleetCount,
            Math.round((totalLoss * fleetCount) / totalCount)
          );
          allocated[type] = (allocated[type] || 0) + loss;
        }

        allocations[index][type] = loss;
      });
    });

    return allocations;
  }

  private static splitLoot(
    loot: { metal: number; crystal: number; deuterium: number },
    parts: number
  ): Array<{ metal: number; crystal: number; deuterium: number }> {
    if (parts <= 0) return [];
    const shares = Array.from({ length: parts }, () => ({ metal: 0, crystal: 0, deuterium: 0 }));

    (['metal', 'crystal', 'deuterium'] as const).forEach((resource) => {
      let remaining = loot[resource];
      for (let i = 0; i < parts; i++) {
        const value = Math.floor(remaining / (parts - i));
        shares[i][resource] = value;
        remaining -= value;
      }
    });

    return shares;
  }

  private static buildCombatSummary(
    reportId: number,
    fleet: Fleet,
    defenderId: number | null,
    result: CombatResult,
    attackerAllies: Array<{ userId: number; username: string }> = []
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
      attackerAllies,
    };
  }

  private static formatLocation(galaxy: number, system: number, position: number): string {
    return `${galaxy}:${system}:${position}`;
  }

  private static scheduleArrivalEvent(fleetId: number, when?: Date | string | null): void {
    if (!when) return;
    fleetScheduler.scheduleArrival(fleetId, when).catch((err) => {
      console.error('[FleetService] Failed to schedule fleet arrival', fleetId, err);
    });
  }

  private static scheduleReturnEvent(fleetId: number, when?: Date | string | null): void {
    if (!when) return;
    fleetScheduler.scheduleReturn(fleetId, when).catch((err) => {
      console.error('[FleetService] Failed to schedule fleet return', fleetId, err);
    });
  }

  private static unscheduleFleetEvents(fleetId: number, type?: 'arrival' | 'return'): void {
    fleetScheduler.unschedule(fleetId, type).catch((err) => {
      console.error('[FleetService] Failed to unschedule fleet', fleetId, err);
    });
  }

  /**
   * Move a fleet to a moon destination via Jump Gate.
   * Strips all resources from the fleet and clears any active orders.
   * Returns true when the move was successful.
   */
  static async moveFleetToMoon(
    userId: number,
    fromMoonId: number,
    fleetId: number,
    toMoonId: number
  ): Promise<boolean> {
    const client = await pool.connect();
    try {
      await client.query('BEGIN');

      // Get fleet and validate
      const fleetResult = await client.query(
        'SELECT * FROM fleets WHERE id = $1 FOR UPDATE',
        [fleetId]
      );
      const fleet = fleetResult.rows[0];
      if (!fleet || fleet.user_id !== userId) {
        throw new Error('Fleet not found or access denied');
      }
      if (fleet.status === 'cancelled') {
        throw new Error('Fleet is cancelled');
      }

      // Get moons
      const fromMoonResult = await client.query(
        'SELECT id, planet_id, user_id FROM moons WHERE id = $1 FOR UPDATE',
        [fromMoonId]
      );
      const toMoonResult = await client.query(
        'SELECT id, planet_id, user_id FROM moons WHERE id = $1 FOR UPDATE',
        [toMoonId]
      );
      const fromMoon = fromMoonResult.rows[0];
      const toMoon = toMoonResult.rows[0];

      if (!fromMoon || !toMoon) throw new Error('Invalid moon(s)');
      if (fromMoon.user_id !== userId || toMoon.user_id !== userId) {
        throw new Error('Moon access denied');
      }
      if (fromMoon.id === toMoon.id) {
        throw new Error('Destination moon must be different');
      }
      if (fleet.origin_planet_id !== fromMoon.planet_id) {
        throw new Error('Fleet is not stationed at the source moon');
      }

      const ships = this.safeParse(fleet.ships);
      const shipKeys = PLANET_SHIP_KEYS;
      const shipCounts = shipKeys.map((key) => Math.max(0, Number(ships[key] || 0)));

      if (shipCounts.every((count) => count === 0)) {
        throw new Error('Fleet has no ships to jump');
      }

      const assignments = shipKeys.map((key, index) => `${key} = ${key} + $${index + 1}`);
      await client.query(
        `UPDATE moons SET ${assignments.join(', ')} WHERE id = $${shipCounts.length + 1}`,
        [...shipCounts, toMoon.id]
      );

      // Jump Gate drops transported resources at origin before instant transfer.
      await client.query(
        `UPDATE moons
         SET metal = COALESCE(metal, 0) + $1,
             crystal = COALESCE(crystal, 0) + $2,
             deuterium = COALESCE(deuterium, 0) + $3
         WHERE id = $4`,
        [
          Math.max(0, Number(fleet.cargo_metal || 0)),
          Math.max(0, Number(fleet.cargo_crystal || 0)),
          Math.max(0, Number(fleet.cargo_deuterium || 0)),
          fromMoon.id,
        ]
      );

      // Strip resources and clear fleet record to avoid further processing
      await client.query(
        `UPDATE fleets
         SET cargo_metal = 0, cargo_crystal = 0, cargo_deuterium = 0
         WHERE id = $1`,
        [fleetId]
      );

      await client.query('DELETE FROM fleets WHERE id = $1', [fleetId]);

      await client.query('COMMIT');
      FleetService.unscheduleFleetEvents(fleetId);
      return true;
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }
  }

  private static async applyDefenderLosses(
    planetId: number,
    targetPlanet: any,
    result: CombatResult,
    client: PoolClient
  ): Promise<void> {
    const losses = result.defenderLosses || {};
    const updatePayload: Record<string, number> = {};

    Object.entries(losses).forEach(([unit, rawLoss]) => {
      const loss = Math.max(0, Number(rawLoss || 0));
      if (loss <= 0) return;

      const current = Math.max(0, Number(targetPlanet[unit] || 0));
      if (unit in DEFENSES) {
        // OGame-like defense rebuild chance after battle (default 70%).
        let rebuilt = 0;
        for (let i = 0; i < loss; i++) {
          if (Math.random() < 0.7) rebuilt++;
        }
        const effectiveLoss = Math.max(0, loss - rebuilt);
        updatePayload[unit] = Math.max(0, current - effectiveLoss);
      } else {
        updatePayload[unit] = Math.max(0, current - loss);
      }
    });

    const updates = Object.entries(updatePayload);
    if (!updates.length) return;

    const setters = updates.map(([key], index) => `${key} = $${index + 1}`);
    const values = updates.map(([, value]) => value);

    await client.query(
      `UPDATE planets SET ${setters.join(', ')} WHERE id = $${values.length + 1}`,
      [...values, planetId]
    );
  }
}

interface MissionValidationParams {
  userId: number;
  missionType: string;
  ships: { [key: string]: number };
  targetGalaxy: number;
  targetSystem: number;
  targetPosition: number;
}

type MissionOutcome = ColonizeMissionResult | EspionageMissionResult | HarvestMissionResult | null;

interface ColonizeMissionResult {
  type: 'colonize';
  success: boolean;
  newPlanet?: Planet;
  failureReason?: 'slot_taken' | 'limit_reached' | 'invalid';
}

type EspionageIntelLevel = 'none' | 'minimal' | 'standard' | 'full';

interface EspionageMissionResult {
  type: 'espionage';
  success: boolean;
  detected: boolean;
  intelLevel: EspionageIntelLevel;
  reportSummary?: {
    resources?: Record<string, number>;
    intelLevel: EspionageIntelLevel;
    detected: boolean;
  };
}

interface HarvestMissionResult {
  type: 'harvest';
  collected: { metal: number; crystal: number };
  empty?: boolean;
}

fleetScheduler.registerCallbacks({
  onArrival: (fleetId: number) => FleetService.processFleetArrival(fleetId),
  onReturn: (fleetId: number) => FleetService.completeFleetReturn(fleetId),
});
