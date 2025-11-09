/**
 * @module backend/services/phalanxService
 *
 * Phalanx scanning service: verifies ownership and sensor ranges, calculates
 * scan costs, and returns scan results including inbound/outbound fleet
 * information. Intended to be used by moon-phalanx scanning endpoints.
 */

import { pool } from '../config/database';
import moonService from './moonService';
import { moonConfig } from '../config/moonConfig';

interface PhalanxScanParams {
  userId: number;
  moonId: number;
  targetGalaxy: number;
  targetSystem: number;
  targetPosition: number;
}

class PhalanxService {
  async performScan(params: PhalanxScanParams) {
    const client = await pool.connect();

    try {
      await client.query('BEGIN');

      const moon = await moonService.getMoonById(params.moonId);
      if (!moon || moon.user_id !== params.userId) {
        throw new Error('Moon not found or access denied');
      }

      const sensorLevel = moon.sensor_phalanx || 0;
      if (sensorLevel <= 0) {
        throw new Error('Sensor Phalanx required on this moon');
      }

      const originPlanetResult = await client.query(
        'SELECT id, galaxy, system, position, name FROM planets WHERE id = $1',
        [moon.planet_id]
      );

      if (originPlanetResult.rows.length === 0) {
        throw new Error('Moon origin planet not found');
      }

      const originPlanet = originPlanetResult.rows[0];
      if (originPlanet.galaxy !== params.targetGalaxy) {
        throw new Error('Sensor Phalanx can only scan within the same galaxy');
      }

      const range = Math.max(0, sensorLevel * sensorLevel - 1);
      const systemDelta = Math.abs(originPlanet.system - params.targetSystem);
      if (systemDelta > range) {
        throw new Error(`Target system out of range (Level ${sensorLevel} → ±${range} systems)`);
      }

      const cost = moonConfig.PHALANX_SCAN_COST || 5000;
      if (moon.deuterium < cost) {
        throw new Error('Insufficient deuterium on moon to power scan');
      }

      await moonService.deductResources(moon.id, { deuterium: cost });

      const targetPlanetResult = await client.query(
        `SELECT p.id, p.name, p.user_id, u.username
         FROM planets p
         LEFT JOIN users u ON u.id = p.user_id
         WHERE p.galaxy = $1 AND p.system = $2 AND p.position = $3`,
        [params.targetGalaxy, params.targetSystem, params.targetPosition]
      );

      const targetPlanet = targetPlanetResult.rows[0] || null;

      const inboundFleets = await client.query(
         `SELECT f.id,
                f.user_id,
                u.username,
                f.mission_type,
                f.origin_planet_id,
                op.galaxy AS origin_galaxy,
                op.system AS origin_system,
                op.position AS origin_position,
                f.target_galaxy,
                f.target_system,
                f.target_position,
                f.departure_time,
                f.arrival_time,
                f.return_time,
                f.status
         FROM fleets f
         LEFT JOIN users u ON u.id = f.user_id
         LEFT JOIN planets op ON op.id = f.origin_planet_id
         WHERE f.target_galaxy = $1
           AND f.target_system = $2
           AND f.target_position = $3
           AND f.status = 'outbound'
         ORDER BY f.arrival_time ASC`,
        [params.targetGalaxy, params.targetSystem, params.targetPosition]
      );

      let outboundRows: any[] = [];
      if (targetPlanet) {
        const outbound = await client.query(
          `SELECT f.id,
                  f.user_id,
                  u.username,
                  f.mission_type,
                  f.origin_planet_id,
                  op.galaxy AS origin_galaxy,
                  op.system AS origin_system,
                  op.position AS origin_position,
                  f.target_galaxy,
                  f.target_system,
                  f.target_position,
                  f.departure_time,
                  f.arrival_time,
                  f.return_time,
                  f.status
           FROM fleets f
           LEFT JOIN users u ON u.id = f.user_id
           LEFT JOIN planets op ON op.id = f.origin_planet_id
           WHERE f.origin_planet_id = $1
             AND f.status IN ('outbound', 'returning')
           ORDER BY f.arrival_time ASC`,
          [targetPlanet.id]
        );
        outboundRows = outbound.rows;
      }

      await client.query('COMMIT');

      const now = Date.now();
      const mapFleet = (row: any) => {
        const arrivalSource =
          row.status === 'returning' && row.return_time ? row.return_time : row.arrival_time;
        return {
          id: row.id,
          ownerId: row.user_id,
          owner: row.username,
          mission: row.mission_type,
          status: row.status,
        origin: row.origin_galaxy
          ? {
              galaxy: row.origin_galaxy,
              system: row.origin_system,
              position: row.origin_position,
            }
          : null,
        target: {
          galaxy: row.target_galaxy ?? params.targetGalaxy,
          system: row.target_system ?? params.targetSystem,
          position: row.target_position ?? params.targetPosition,
        },
        arrivalTime: arrivalSource,
        departureTime: row.departure_time,
        etaSeconds: arrivalSource
          ? Math.max(0, Math.floor((new Date(arrivalSource).getTime() - now) / 1000))
          : null,
      };
      };

      return {
        target: {
          galaxy: params.targetGalaxy,
          system: params.targetSystem,
          position: params.targetPosition,
          planetId: targetPlanet?.id || null,
          planetName: targetPlanet?.name || null,
          ownerName: targetPlanet?.username || null,
        },
        sensor: {
          level: sensorLevel,
          range,
          cost,
        },
        fleets: {
          inbound: inboundFleets.rows.map(mapFleet),
          outbound: outboundRows.map(mapFleet),
        },
      };
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }
  }
}

export default new PhalanxService();
