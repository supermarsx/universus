import { PoolClient } from 'pg';
import { Planet } from '../types';
import { PlanetService } from './planetService';
import { Moon } from './moonService';

export type LocationType = 'planet' | 'moon';

export interface LocationRequest {
  planetId?: number;
  moonId?: number;
  locationType?: LocationType;
  expectedPlanetId?: number;
  refreshResources?: boolean;
}

export interface LocationContext<RecordType = any> {
  type: LocationType;
  planetId: number;
  moonId: number | null;
  ownerId: number;
  record: RecordType;
  resourceTable: 'planets' | 'moons';
  primaryId: number;
  roboticsLevel: number;
  shipyardLevel: number;
  naniteLevel: number;
  totalFields?: number;
  usedFields?: number;
}

function assertPlanetId(id?: number): asserts id is number {
  if (!id) {
    throw new Error('Planet id is required for this operation');
  }
}

function assertMoonId(id?: number): asserts id is number {
  if (!id) {
    throw new Error('Moon id is required for this operation');
  }
}

export async function resolveLocation(
  client: PoolClient,
  userId: number,
  request: LocationRequest
): Promise<LocationContext> {
  const type: LocationType =
    request.locationType ?? (request.moonId ? 'moon' : 'planet');

  if (type === 'planet') {
    const targetPlanetId = request.planetId ?? request.expectedPlanetId;
    assertPlanetId(targetPlanetId);

    let planet: Planet | null = null;
    if (request.refreshResources === false) {
      planet = await PlanetService.getPlanetById(targetPlanetId);
    } else {
      planet = await PlanetService.updateResources(targetPlanetId);
    }

    if (!planet || planet.user_id !== userId) {
      throw new Error('Planet not found or access denied');
    }

    return {
      type,
      planetId: planet.id,
      moonId: null,
      ownerId: planet.user_id,
      record: planet,
      resourceTable: 'planets',
      primaryId: planet.id,
      roboticsLevel: planet.robotics_factory || 0,
      shipyardLevel: planet.shipyard || 0,
      naniteLevel: planet.nanite_factory || 0,
    };
  }

  assertMoonId(request.moonId);
  const moonResult = await client.query('SELECT * FROM moons WHERE id = $1', [
    request.moonId,
  ]);

  if (moonResult.rows.length === 0) {
    throw new Error('Moon not found');
  }

  const moon: Moon = moonResult.rows[0];

  if (moon.user_id !== userId) {
    throw new Error('Moon not found or access denied');
  }

  if (
    request.expectedPlanetId &&
    moon.planet_id !== request.expectedPlanetId
  ) {
    throw new Error('Moon does not belong to the specified planet');
  }

  return {
    type,
    planetId: moon.planet_id,
    moonId: moon.id,
    ownerId: moon.user_id,
    record: moon,
    resourceTable: 'moons',
    primaryId: moon.id,
    roboticsLevel: moon.moon_robotics_factory || 0,
    shipyardLevel: moon.moon_shipyard || 0,
    naniteLevel: moon.moon_nanite_factory || 0,
    totalFields: moon.total_fields,
    usedFields: moon.used_fields,
  };
}

/**
 * Assert that a planet id value is provided; throws if not.
 *
 * @param id - Optional planet id value
 */
export function ensurePlanetId(id?: number): asserts id is number {
  if (!id) throw new Error('Planet id is required for this operation');
}

/**
 * Assert that a moon id value is provided; throws if not.
 *
 * @param id - Optional moon id value
 */
export function ensureMoonId(id?: number): asserts id is number {
  if (!id) throw new Error('Moon id is required for this operation');
}
