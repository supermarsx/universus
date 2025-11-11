/**
 * @module backend/services/galaxyService
 *
 * GalaxyService provides read-only views and intel for galaxy/system slots.
 * It builds a sensor snapshot for a requested system, combining planet,
 * moon and debris data with the requesting player's sensor capabilities.
 */
import { pool } from '../config/database';
import { redis } from '../config/redis';
import { PlanetService } from './planetService';
import { ResearchService } from './researchService';
import { GameConfigAdapter } from './gameConfigAdapter';

interface GalaxyRequestContext {
  userId: number;
  galaxy: number;
  system: number;
  originPlanetId?: number;
}

interface RawPlanetRow {
  id: number;
  name: string | null;
  galaxy: number;
  system: number;
  position: number;
  planet_type: string | null;
  temperature: number | null;
  user_id: number | null;
  username: string | null;
  alliance_id: number | null;
  alliance_name: string | null;
  alliance_tag: string | null;
  last_seen: string | null;
}

interface RawDebrisRow {
  id: number;
  galaxy: number;
  system: number;
  position: number;
  metal: number;
  crystal: number;
  expires_at: string | null;
}

interface RawMoonRow {
  id: number;
  planet_id: number;
  diameter: number;
  total_fields: number;
  position: number;
}

interface GalaxySlotIntel {
  position: number;
  hasPlanet: boolean;
  intelQuality: 'full' | 'partial' | 'minimal';
  planet?: {
    id: number;
    name: string | null;
    type: string | null;
    temperature: number | null;
  };
  moon?: {
    id: number;
    diameter: number;
  };
  owner?: {
    id: number;
    username: string | null;
    alliance?: {
      id: number;
      name: string | null;
      tag: string | null;
    };
    lastSeen: string | null;
    activity: {
      label: string;
      minutesSince: number | null;
    };
    relation: 'self' | 'ally' | 'neutral';
  };
  debris?: {
    metal: number;
    crystal: number;
    expiresAt: string | null;
  } | null;
  markers: {
    canColonize: boolean;
    hasDebris: boolean;
  };
}

interface GalaxySnapshot {
  coordinates: {
    galaxy: number;
    system: number;
  };
  pagination: {
    galaxyCount: number;
    systemsPerGalaxy: number;
    positionsPerSystem: number;
    hasPreviousSystem: boolean;
    hasNextSystem: boolean;
    previousSystem: number | null;
    nextSystem: number | null;
  };
  intel: {
    sensorRange: number;
    espionageLevel: number;
    originPlanetId: number | null;
    originPlanetName: string | null;
    sensorSources: {
      espionage: number;
      phalanx: number;
      sensorArray: number;
    };
  };
  planets: GalaxySlotIntel[];
}

interface RawSystemData {
  planets: RawPlanetRow[];
  debris: RawDebrisRow[];
  moons: RawMoonRow[];
}

const CACHE_TTL_SECONDS = 15;

export class GalaxyService {
  private static get config() { return GameConfigAdapter.getInstance(); }

  /**
   * Build a complete snapshot for a given system as visible to a user.
   * The snapshot includes pagination metadata, sensor intel and per-slot data.
   *
   * @param context - Request context containing user and coordinates
   */
  static async getSystemSnapshot(context: GalaxyRequestContext): Promise<GalaxySnapshot> {
    const [galaxyCount, systemsPerGalaxy, positionsPerSystem] = await Promise.all([
      this.config.getGalaxyCount(),
      this.config.getSystemsPerGalaxy(),
      this.config.getPositionsPerSystem(),
    ]);

    const normalizedGalaxy = Math.min(Math.max(context.galaxy, 1), galaxyCount);
    const normalizedSystem = Math.min(Math.max(context.system, 1), systemsPerGalaxy);

    const [rawData, researchRow, requesterAllianceId] = await Promise.all([
      this.fetchRawSystemData(normalizedGalaxy, normalizedSystem),
      ResearchService.getUserResearch(context.userId),
      this.getUserAllianceId(context.userId),
    ]);
    const espionageLevel = researchRow?.espionage_technology || 0;

    let originPlanet = null;
    if (context.originPlanetId) {
      const planet = await PlanetService.getPlanetById(context.originPlanetId);
      if (planet && planet.user_id === context.userId) {
        originPlanet = planet;
      }
    }

    const sensorRange = this.calculateSensorRange(espionageLevel, originPlanet);
    const sensorSources = {
      espionage: espionageLevel,
      phalanx: originPlanet?.sensor_phalanx || 0,
      sensorArray: originPlanet?.sensor_array || 0,
    };
    const slots = this.buildSlots({
      rawData,
      positionsPerSystem,
      originPlanet,
      sensorRange,
      userId: context.userId,
      requesterAllianceId,
    });

    return {
      coordinates: {
        galaxy: normalizedGalaxy,
        system: normalizedSystem,
      },
      pagination: {
        galaxyCount,
        systemsPerGalaxy,
        positionsPerSystem,
        hasPreviousSystem: normalizedSystem > 1,
        hasNextSystem: normalizedSystem < systemsPerGalaxy,
        previousSystem: normalizedSystem > 1 ? normalizedSystem - 1 : null,
        nextSystem: normalizedSystem < systemsPerGalaxy ? normalizedSystem + 1 : null,
      },
      intel: {
        sensorRange,
        espionageLevel,
        originPlanetId: originPlanet ? originPlanet.id : null,
        originPlanetName: originPlanet?.name || null,
        sensorSources,
      },
      planets: slots,
    };
  }

    /**
     * Build the system snapshot visible to a requesting user.
     *
     * @param {GalaxyRequestContext} context - Context including user and target coordinates
     * @returns {Promise<GalaxySnapshot>} The assembled snapshot with pagination and slot intel
     */

  /**
   * Read raw rows for planets, debris and moons for a system. Results are
   * cached briefly in Redis to reduce DB load for rapid UI paging.
   *
   * @param galaxy - Galaxy number
   * @param system - System number
   */
  private static async fetchRawSystemData(galaxy: number, system: number): Promise<RawSystemData> {
    const cacheKey = `galaxy:raw:${galaxy}:${system}`;

    if (redis) {
      const cached = await redis.get(cacheKey);
      if (cached) {
        return JSON.parse(cached);
      }
    }

    const [planetsResult, debrisResult, moonsResult] = await Promise.all([
      pool.query(
        `SELECT 
            p.id,
            p.name,
            p.galaxy,
            p.system,
            p.position,
            p.planet_type,
            p.temperature,
            p.user_id,
            u.username,
            u.alliance_id,
            a.name AS alliance_name,
            a.tag AS alliance_tag,
            COALESCE(u.last_login, u.created_at) AS last_seen
         FROM planets p
         LEFT JOIN users u ON p.user_id = u.id
         LEFT JOIN alliances a ON u.alliance_id = a.id
         WHERE p.galaxy = $1 AND p.system = $2
         ORDER BY p.position`,
        [galaxy, system]
      ),
      pool.query(
        `SELECT id, galaxy, system, position, metal, crystal, expires_at
         FROM debris_fields
         WHERE galaxy = $1 AND system = $2 AND (metal > 0 OR crystal > 0)
         ORDER BY position`,
        [galaxy, system]
      ),
      pool.query(
        `SELECT 
            m.id,
            m.planet_id,
            m.diameter,
            m.total_fields,
            p.position
         FROM moons m
         JOIN planets p ON p.id = m.planet_id
         WHERE p.galaxy = $1 AND p.system = $2`,
        [galaxy, system]
      ),
    ]);

    const payload: RawSystemData = {
      planets: planetsResult.rows as RawPlanetRow[],
      debris: debrisResult.rows as RawDebrisRow[],
      moons: moonsResult.rows as RawMoonRow[],
    };

    if (redis) {
      await redis.set(cacheKey, JSON.stringify(payload), 'EX', CACHE_TTL_SECONDS);
    }

    return payload;
  }

  /**
   * Convenience helper to fetch a user's alliance id (or null).
   * @param userId - User id to look up
   */
  private static async getUserAllianceId(userId: number): Promise<number | null> {
    const result = await pool.query(
      'SELECT alliance_id FROM users WHERE id = $1',
      [userId]
    );

    return result.rows[0]?.alliance_id || null;
  }

  /**
   * Convert raw system rows into the public-facing slot intel structure.
   * This composes planet, moon, debris and owner decoration logic.
   */
  private static buildSlots(params: {
    rawData: RawSystemData;
    positionsPerSystem: number;
    originPlanet: any | null;
    sensorRange: number;
    userId: number;
    requesterAllianceId: number | null;
  }): GalaxySlotIntel[] {
    const debrisMap = new Map<number, RawDebrisRow>();
    params.rawData.debris.forEach((row) => debrisMap.set(row.position, row));

    const planetMap = new Map<number, RawPlanetRow>();
    params.rawData.planets.forEach((row) => planetMap.set(row.position, row));
    const moonByPlanetId = new Map<number, RawMoonRow>();
    (params.rawData.moons || []).forEach((row) => moonByPlanetId.set(row.planet_id, row));

    const slots: GalaxySlotIntel[] = [];

    for (let position = 1; position <= params.positionsPerSystem; position++) {
      const planetRow = planetMap.get(position) || null;
      const debrisRow = debrisMap.get(position) || null;

      const intelQuality = this.determineIntelQuality(
        planetRow,
        params.originPlanet,
        params.sensorRange
      );

      const slot: GalaxySlotIntel = {
        position,
        hasPlanet: Boolean(planetRow),
        intelQuality,
        planet: planetRow
          ? {
              id: planetRow.id,
              name: planetRow.name,
              type: planetRow.planet_type,
              temperature: planetRow.temperature,
            }
          : undefined,
        owner: planetRow
          ? this.decorateOwner(planetRow, params.userId, params.requesterAllianceId)
          : undefined,
        moon: planetRow && moonByPlanetId.has(planetRow.id)
          ? {
              id: moonByPlanetId.get(planetRow.id)!.id,
              diameter: moonByPlanetId.get(planetRow.id)!.diameter,
            }
          : undefined,
        debris: debrisRow
          ? {
              metal: debrisRow.metal,
              crystal: debrisRow.crystal,
              expiresAt: debrisRow.expires_at,
            }
          : null,
        markers: {
          canColonize: !planetRow,
          hasDebris: Boolean(debrisRow),
        },
      };

      if (intelQuality === 'minimal') {
        // Remove sensitive owner data when intel is minimal
        slot.owner = planetRow && planetRow.user_id === params.userId ? slot.owner : undefined;
        if (slot.planet && planetRow && planetRow.user_id !== params.userId) {
          slot.planet.name = null;
          slot.planet.type = null;
        }
      } else if (intelQuality === 'partial' && slot.owner && planetRow && planetRow.user_id !== params.userId) {
        // Strip alliance info on partial intel
        slot.owner.alliance = undefined;
      }

      slots.push(slot);
    }

    return slots;
  }

  /**
   * Build the owner metadata object for a planet row, including relation
   * (self/ally/neutral) and recent activity classification.
   */
  private static decorateOwner(row: RawPlanetRow, userId: number, requesterAllianceId: number | null) {
    if (!row.user_id) return undefined;

    const lastSeen = row.last_seen ? new Date(row.last_seen) : null;
    const minutesSince = lastSeen
      ? Math.floor((Date.now() - lastSeen.getTime()) / 60000)
      : null;

    const relation: 'self' | 'ally' | 'neutral' =
      row.user_id === userId
        ? 'self'
        : requesterAllianceId && row.alliance_id && row.alliance_id === requesterAllianceId
        ? 'ally'
        : 'neutral';

    return {
      id: row.user_id,
      username: row.username,
      alliance: row.alliance_id
        ? {
            id: row.alliance_id,
            name: row.alliance_name,
            tag: row.alliance_tag,
          }
        : undefined,
      lastSeen: row.last_seen,
      activity: this.classifyActivity(minutesSince),
      relation,
    };
  }

  /**
   * Convert minutes-since-last-seen into an activity label used by the UI.
   */
  private static classifyActivity(minutesSince: number | null) {
    if (minutesSince === null) {
      return {
        label: 'unknown',
        minutesSince: null,
      };
    }

    if (minutesSince <= 10) {
      return { label: 'active', minutesSince };
    }

    if (minutesSince <= 60) {
      return { label: 'recent', minutesSince };
    }

    if (minutesSince <= 1440) {
      return { label: 'idle', minutesSince };
    }

    return { label: 'inactive', minutesSince };
  }

  /**
   * Determine the intel quality ('full'|'partial'|'minimal') for a slot
   * relative to the origin sensor source and sensor ranges.
   */
  private static determineIntelQuality(
    planetRow: RawPlanetRow | null,
    originPlanet: any | null,
    sensorRange: number
  ): 'full' | 'partial' | 'minimal' {
    if (!planetRow) return 'full'; // empty slots always visible
    if (!originPlanet) return 'minimal';
    if (planetRow.galaxy !== originPlanet.galaxy) return 'minimal';

    const systemDelta = Math.abs(planetRow.system - originPlanet.system);
    if (systemDelta <= sensorRange) {
      return 'full';
    }

    if (systemDelta <= sensorRange * 2) {
      return 'partial';
    }

    return 'minimal';
  }

  /**
   * Calculate the effective sensor range considering espionage tech and
   * installed phalanx/sensor arrays on the origin planet.
   */
  private static calculateSensorRange(espionageLevel: number, originPlanet: any | null): number {
    const baseRange = 1;
    const espionageBonus = Math.floor(espionageLevel / 2);
    const sensorArrayLevel = originPlanet?.sensor_array || originPlanet?.sensor_phalanx || 0;
    const sensorBonus = Math.floor(sensorArrayLevel / 2);
    return Math.max(1, baseRange + espionageBonus + sensorBonus);
  }
}

export default GalaxyService;
