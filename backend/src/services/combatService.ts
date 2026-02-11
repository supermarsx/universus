/**
 * @module backend/services/combatService
 *
 * CombatService runs deterministic combat simulations used by the game.
 * It converts ship and defense counts into combat units, simulates rounds,
 * computes losses, debris and loot, and persists combat reports. The
 * simulation is intentionally simplified for determinism and performance.
 */
import { pool } from '../config/database';
import { SHIPS, DEFENSES } from '../config/gameConfig';
import { combatTracker } from './millisecondCombatTracker';
import notificationService from './notificationService';
import { getRealtimeHandler } from '../socket';
import { CombatAlertType } from '../types/realtime';
import { gameConfig } from './gameConfigAdapter';

// Lightweight seeded PRNG (Mulberry32)
function mulberry32(seed: number) {
  return function() {
    let t = seed += 0x6D2B79F5;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

interface CombatUnit {
  type: string;
  count: number;
  shield: number;
  weapon: number;
  hull: number;
  maxShield?: number;
  maxHull?: number;
  rapidFire?: { [key: string]: number };
}

export interface CombatResult {
  winner: 'attacker' | 'defender' | 'draw';
  rounds: any[];
  attackerLosses: { [key: string]: number };
  defenderLosses: { [key: string]: number };
  loot: { metal: number; crystal: number; deuterium: number };
  debris: { metal: number; crystal: number };
  combatId?: number;
}

export class CombatService {
  /**
   * Simulate a full fleet/defense battle.
   *
   * By default this runs the TypeScript implementation. If `CORE_ENGINE=rust`
   * the method will delegate to the Rust backend via gRPC. The Rust response is
   * expected to return a JSON-serialized CombatResult (string) in `json_result`.
   *
   * This method executes the configured number of rounds or until one side
   * is eliminated. It returns a detailed CombatResult describing rounds,
   * losses, debris and loot. Optionally logs rounds to the millisecond
   * combat tracker when a planetId is provided.
   *
   * @param attackerShips - mapping of ship keys to counts for attacker
   * @param defenderShips - mapping of ship keys to counts for defender
   * @param defenderDefenses - mapping of defense keys to counts for defender
   * @param attackerTech - research/tech levels for attacker
   * @param defenderTech - research/tech levels for defender
   * @param planetResources - resources available on the defended planet
   * @param planetId - optional planet id used for logging/tracking
   * @param seed - optional numeric seed for deterministic simulations
   */
  static async simulateBattle(
    attackerShips: { [key: string]: number },
    defenderShips: { [key: string]: number },
    defenderDefenses: { [key: string]: number },
    attackerTech: any,
    defenderTech: any,
    planetResources: { metal: number; crystal: number; deuterium: number },
    planetId?: number,
    seed?: number
  ): Promise<CombatResult> {
    // Rust-first delegation: use Rust unless explicitly disabled.
    const defaultEngine = process.env.NODE_ENV === 'test' ? 'ts' : 'rust';
    const coreEngine = (process.env.CORE_ENGINE || defaultEngine).toLowerCase();
    const coreTransport = (process.env.CORE_TRANSPORT || 'grpc').toLowerCase();
    if (coreEngine !== 'ts' && coreEngine !== 'typescript' && coreEngine !== 'js') {
      const rustRequest = {
        battle_id: String(planetId || 'local'),
        attacker_ships: attackerShips || {},
        defender_ships: defenderShips || {},
        defender_defenses: defenderDefenses || {},
        attacker_tech: this.normalizeTechMap(attackerTech),
        defender_tech: this.normalizeTechMap(defenderTech),
        planet_metal: Number(planetResources?.metal || 0),
        planet_crystal: Number(planetResources?.crystal || 0),
        planet_deuterium: Number(planetResources?.deuterium || 0),
        seed: typeof seed === 'number' ? String(seed) : undefined,
        universe: process.env.CORE_UNIVERSE || 'default',
      };

      if (coreTransport === 'napi') {
        try {
          const { simulateBattleNapi } = require('../coreAdapter/rustCoreNapiClient');
          const result = await simulateBattleNapi(rustRequest);
          if (result && result.winner) return result as CombatResult;
          throw new Error('Rust N-API returned invalid combat result payload');
        } catch (error) {
          console.error('Rust N-API call failed, falling back to gRPC/TS implementation:', error);
        }
      }

      try {
        const { simulateBattleRust } = require('../coreAdapter/rustCoreClient');
        const result = await simulateBattleRust(rustRequest);
        if (result && result.winner) return result as CombatResult;
        throw new Error('Rust core returned invalid combat result payload');
      } catch (error) {
        console.error('Rust core call failed, falling back to TS implementation:', error);
        // fallback to JS implementation
      }
    }
    // Start combat tracking with millisecond precision
    let combatId: number | undefined;
    if (planetId) {
      try {
        combatId = await combatTracker.executeCombatAtArrival(planetId, combatTracker['getCurrentTimeMicros']());
      } catch (error) {
        console.error('Failed to start combat tracking:', error);
      }
    }

    // Seeded RNG for deterministic behavior. Preference: explicit seed -> combatId -> timestamp
    const rngSeed = typeof seed === 'number' ? seed : (typeof combatId === 'number' ? combatId : Date.now());
    const rand = mulberry32(Math.floor(rngSeed));

    // Convert to combat units
    const attackerUnits: CombatUnit[] = this.prepareCombatUnits(
      attackerShips,
      {},
      attackerTech
    );
    const defenderUnits: CombatUnit[] = this.prepareCombatUnits(
      defenderShips,
      defenderDefenses,
      defenderTech
    );

    const rounds: any[] = [];
    
    // Get max rounds from configuration
    const combatConfig = await gameConfig.getCombatConfig();
    const maxRounds = combatConfig.maxRounds;

    for (let round = 1; round <= maxRounds; round++) {
      if (attackerUnits.length === 0 || defenderUnits.length === 0) {
        break;
      }

      const roundResult = this.simulateRound(attackerUnits, defenderUnits, rand);
      rounds.push(roundResult);

      // Log round with millisecond precision
      if (combatId) {
        try {
          await combatTracker.logCombatRound(combatId, round, {
            attackerShips: this.getShipCounts(attackerUnits),
            defenderShips: this.getShipCounts(defenderUnits),
            attackerDamage: roundResult.attackerDamage || 0,
            defenderDamage: roundResult.defenderDamage || 0
          });
        } catch (error) {
          console.error('Failed to log combat round:', error);
        }
      }

      // Regenerate shields (preserve tech-modified shield values)
      this.regenerateShields(attackerUnits);
      this.regenerateShields(defenderUnits);
    }

    // Determine winner
    let winner: 'attacker' | 'defender' | 'draw';
    if (attackerUnits.length === 0 && defenderUnits.length === 0) {
      winner = 'draw';
    } else if (defenderUnits.length === 0) {
      winner = 'attacker';
    } else {
      winner = 'defender';
    }

    // Calculate losses
    const attackerLosses = this.calculateLosses(attackerShips, attackerUnits);
    const defenderLosses = this.calculateLosses(
      { ...defenderShips, ...defenderDefenses },
      defenderUnits
    );

    // Calculate debris
    const debris = this.calculateDebris(attackerLosses, defenderLosses);

    // Calculate loot
    const loot =
      winner === 'attacker'
        ? this.calculateLoot(planetResources, attackerUnits)
        : { metal: 0, crystal: 0, deuterium: 0 };

    // Complete combat tracking
    if (combatId) {
      try {
        await combatTracker.completeCombat(combatId, winner, {
          attackerLosses,
          defenderLosses,
          debris,
          loot,
          totalRounds: rounds.length
        });
      } catch (error) {
        console.error('Failed to complete combat tracking:', error);
      }
    }

    return {
      winner,
      rounds,
      attackerLosses,
      defenderLosses,
      loot,
      debris,
      combatId,
    };
  }

  /**
   * Convert an array of CombatUnit into a counts map keyed by unit type.
   * @private
   */
  private static getShipCounts(units: CombatUnit[]): { [key: string]: number } {
    const counts: { [key: string]: number } = {};
    for (const unit of units) {
      counts[unit.type] = (counts[unit.type] || 0) + (unit.count || 1);
    }
    return counts;
  }

  private static normalizeTechMap(tech: any): Record<string, number> {
    const normalized: Record<string, number> = {};
    for (const [key, value] of Object.entries(tech || {})) {
      const n = Number(value);
      if (Number.isFinite(n)) {
        normalized[key] = Math.max(0, Math.trunc(n));
      }
    }
    return normalized;
  }

  private static prepareCombatUnits(
    ships: { [key: string]: number },
    defenses: { [key: string]: number },
    tech: any
  ): CombatUnit[] {
    const units: CombatUnit[] = [];

    for (const [type, count] of Object.entries(ships)) {
      if (count > 0 && SHIPS[type]) {
        const ship = SHIPS[type];
        for (let i = 0; i < count; i++) {
          const maxShield = ship.shieldPower * (1 + (tech.shielding_technology || 0) * 0.1);
          const maxHull = ship.structurePoints * (1 + (tech.armor_technology || 0) * 0.1);
          units.push({
            type,
            count: 1,
            shield: maxShield,
            weapon: ship.weaponPower * (1 + (tech.weapons_technology || 0) * 0.1),
            hull: maxHull,
            maxShield,
            maxHull,
            rapidFire: ship.rapidFire,
          });
        }
      }
    }

    for (const [type, count] of Object.entries(defenses)) {
      if (count > 0 && DEFENSES[type]) {
        const defense = DEFENSES[type];
        for (let i = 0; i < count; i++) {
          const maxShield = defense.shieldPower * (1 + (tech.shielding_technology || 0) * 0.1);
          const maxHull = defense.structurePoints * (1 + (tech.armor_technology || 0) * 0.1);
          units.push({
            type,
            count: 1,
            shield: maxShield,
            weapon: defense.weaponPower * (1 + (tech.weapons_technology || 0) * 0.1),
            hull: maxHull,
            maxShield,
            maxHull,
            rapidFire: defense.rapidFire,
          });
        }
      }
    }

    return units;
  }

  private static simulateRound(
    attackers: CombatUnit[],
    defenders: CombatUnit[],
    rand: () => number
  ): any {
    /**
     * Simulate a single combat round where attackers then defenders fire.
     * Returns an object with shot counts and destroyed counts for the round.
     * @private
     */
    const roundData = {
      attackerShots: 0,
      defenderShots: 0,
      attackerDestroyed: 0,
      defenderDestroyed: 0,
    };

    // Attackers shoot
    for (const attacker of attackers) {
      if (defenders.length === 0) break;
      
      const target = defenders[Math.floor(rand() * defenders.length)];
      this.shoot(attacker, target, defenders, rand);
      roundData.attackerShots++;
    }

    // Count destroyed defenders
    roundData.defenderDestroyed = this.removeDestroyed(defenders);

    // Defenders shoot back
    for (const defender of defenders) {
      if (attackers.length === 0) break;
      
      const target = attackers[Math.floor(rand() * attackers.length)];
      this.shoot(defender, target, attackers, rand);
      roundData.defenderShots++;
    }

    // Count destroyed attackers
    roundData.attackerDestroyed = this.removeDestroyed(attackers);

    return roundData;
  }

  private static shoot(
    shooter: CombatUnit,
    target: CombatUnit,
    targetArray: CombatUnit[],
    rand: () => number
  ): void {
    /**
     * Apply shooter damage to a single target, handling shields, hull and
     * rapid-fire behavior. Mutates the target and may recursively trigger
     * additional shots when rapid fire activates.
     * @private
     */
    let damage = shooter.weapon;

    // Check if damage can penetrate shield
    if (damage < target.shield * 0.01) {
      return; // Shot bounces off
    }

    // Apply to shield first
    if (target.shield > 0) {
      const shieldDamage = Math.min(damage, target.shield);
      target.shield -= shieldDamage;
      damage -= shieldDamage;
    }

    // Apply remaining damage to hull
    if (damage > 0) {
      target.hull -= damage;

      // Check for explosion chance
      if (target.hull <= 0) {
        target.hull = 0;
      } else if (typeof target.maxHull === 'number' && target.hull < target.maxHull * 0.7) {
        // Chance of explosion based on remaining hull relative to 70% threshold
        const explosionChance = 1 - (target.hull / (target.maxHull * 0.7));
        if (rand() < explosionChance) {
          target.hull = 0;
        }
      }
    }

    // Rapid fire
    if (shooter.rapidFire && shooter.rapidFire[target.type]) {
      const rfChance = 1 - 1 / shooter.rapidFire[target.type];
      if (rand() < rfChance && targetArray.length > 0) {
        const newTarget = targetArray[Math.floor(rand() * targetArray.length)];
        this.shoot(shooter, newTarget, targetArray, rand);
      }
    }
  }

  private static removeDestroyed(units: CombatUnit[]): number {
    /**
     * Remove and count destroyed units (hull <= 0) from the provided array.
     * @private
     */
    let destroyed = 0;
    for (let i = units.length - 1; i >= 0; i--) {
      if (units[i].hull <= 0) {
        units.splice(i, 1);
        destroyed++;
      }
    }
    return destroyed;
  }

  private static regenerateShields(units: CombatUnit[]): void {
    /**
     * Reset shields to their full (tech-modified) value between rounds.
     * @private
     */
    for (const unit of units) {
      if (typeof unit.maxShield === 'number') {
        unit.shield = unit.maxShield;
      } else {
        const config = SHIPS[unit.type] || DEFENSES[unit.type];
        if (config) unit.shield = config.shieldPower;
      }
    }
  }

  private static calculateLosses(
    initial: { [key: string]: number },
    remaining: CombatUnit[]
  ): { [key: string]: number } {
    /**
     * Compute losses by comparing initial counts to remaining combat units.
     * @private
     */
    const losses: { [key: string]: number } = {};
    const remainingCounts: { [key: string]: number } = {};

    for (const unit of remaining) {
      remainingCounts[unit.type] = (remainingCounts[unit.type] || 0) + 1;
    }

    for (const [type, count] of Object.entries(initial)) {
      const lost = count - (remainingCounts[type] || 0);
      if (lost > 0) {
        losses[type] = lost;
      }
    }

    return losses;
  }

  private static calculateDebris(
    attackerLosses: { [key: string]: number },
    defenderLosses: { [key: string]: number }
  ): { metal: number; crystal: number } {
    /**
     * Convert losses into debris amounts using configured cost fractions.
     * @private
     */
    let metal = 0;
    let crystal = 0;

    const addDebris = (losses: { [key: string]: number }) => {
      for (const [type, count] of Object.entries(losses)) {
        const config = SHIPS[type] || DEFENSES[type];
        if (config) {
          metal += Math.floor(config.cost.metal * count * 0.3);
          crystal += Math.floor(config.cost.crystal * count * 0.3);
        }
      }
    };

    addDebris(attackerLosses);
    addDebris(defenderLosses);

    return { metal, crystal };
  }

  private static calculateLoot(
    planetResources: { metal: number; crystal: number; deuterium: number },
    attackerUnits: CombatUnit[]
  ): { metal: number; crystal: number; deuterium: number } {
    /**
     * Estimate loot collected by attacker units from planet resources given
     * attacker cargo capacity and the 50% looting cap.
     * @private
     */
    // Calculate cargo capacity
    let cargoCapacity = 0;
    for (const unit of attackerUnits) {
      const ship = SHIPS[unit.type];
      if (ship) {
        cargoCapacity += ship.cargo;
      }
    }

    // Max 50% of resources can be looted
    const maxLoot = {
      metal: Math.floor(planetResources.metal * 0.5),
      crystal: Math.floor(planetResources.crystal * 0.5),
      deuterium: Math.floor(planetResources.deuterium * 0.5),
    };

    // Distribute cargo capacity proportionally
    const totalAvailable = maxLoot.metal + maxLoot.crystal + maxLoot.deuterium;
    if (totalAvailable === 0 || cargoCapacity === 0) {
      return { metal: 0, crystal: 0, deuterium: 0 };
    }

    const capacityUsed = Math.min(cargoCapacity, totalAvailable);
    
    return {
      metal: Math.floor((maxLoot.metal / totalAvailable) * capacityUsed),
      crystal: Math.floor((maxLoot.crystal / totalAvailable) * capacityUsed),
      deuterium: Math.floor((maxLoot.deuterium / totalAvailable) * capacityUsed),
    };
  }

  static async saveCombatReport(
    attackerId: number,
    defenderId: number | null,
    coordinates: { galaxy: number; system: number; position: number },
    result: CombatResult,
    attackerAllies: Array<{ userId: number; username: string }> = []
  ): Promise<number> {
    const reportResult = await pool.query(
      `INSERT INTO combat_reports 
       (attacker_id, defender_id, planet_galaxy, planet_system, planet_position,
        rounds, winner, attacker_losses, defender_losses, loot_metal, loot_crystal, 
        loot_deuterium, debris_metal, debris_crystal, attacker_allies)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
       RETURNING id`,
      [
        attackerId,
        defenderId,
        coordinates.galaxy,
        coordinates.system,
        coordinates.position,
        JSON.stringify(result.rounds),
        result.winner,
        JSON.stringify(result.attackerLosses),
        JSON.stringify(result.defenderLosses),
        result.loot.metal,
        result.loot.crystal,
        result.loot.deuterium,
        result.debris.metal,
        result.debris.crystal,
        JSON.stringify(attackerAllies),
      ]
    );

    if (reportResult.rows.length === 0) {
      throw new Error('Failed to save combat report');
    }
    return reportResult.rows[0].id;
  }

  static async sendCombatNotifications(
    attackerId: number,
    defenderId: number | null,
    attackerUsername: string,
    defenderUsername: string,
    planetName: string,
    combatId: number,
    result: CombatResult
  ): Promise<void> {
    try {
      // Notify defender of attack
      if (defenderId) {
        await notificationService.notifyUnderAttack(
          defenderId,
          attackerUsername,
          planetName,
          combatId
        );
      }

      // Broadcast combat alert via Socket.io
      const handler = getRealtimeHandler();
      if (handler && defenderId) {
        await handler.broadcastCombatAlert({
          combatId,
          alertType: CombatAlertType.COMBAT_STARTED,
          attackerId,
          attackerUsername,
          defenderId,
          defenderUsername,
          severity: 5,
          data: { planetName, winner: result.winner },
          timestamp: new Date()
        });

        // Send combat ended alert
        await handler.broadcastCombatAlert({
          combatId,
          alertType: CombatAlertType.COMBAT_ENDED,
          attackerId,
          attackerUsername,
          defenderId,
          defenderUsername,
          severity: result.winner === 'attacker' ? 4 : 3,
          data: {
            winner: result.winner,
            attackerLosses: result.attackerLosses,
            defenderLosses: result.defenderLosses,
            loot: result.loot
          },
          timestamp: new Date()
        });
      }
    } catch (error) {
      console.error('Failed to send combat notifications:', error);
    }
  }
}

