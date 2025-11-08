import { pool } from '../config/database';
import {
  moonConfig,
  getMoonChanceFromDebris,
  rollForMoon,
  calculateMoonDiameter,
} from '../config/moonConfig';

export interface Moon {
  id: number;
  planet_id: number;
  user_id: number;
  name: string;
  diameter: number;
  total_fields: number;
  used_fields: number;
  lunar_base: number;
  sensor_phalanx: number;
  jump_gate: number;
  moon_robotics_factory: number;
  moon_shipyard: number;
  moon_nanite_factory: number;
  metal: number;
  crystal: number;
  deuterium: number;
  energy: number;
  last_resource_update: Date;
  rocket_launcher: number;
  light_laser: number;
  heavy_laser: number;
  gauss_cannon: number;
  ion_cannon: number;
  plasma_turret: number;
  small_shield_dome: number;
  large_shield_dome: number;
  small_cargo: number;
  large_cargo: number;
  light_fighter: number;
  heavy_fighter: number;
  cruiser: number;
  battleship: number;
  colony_ship: number;
  recycler: number;
  espionage_probe: number;
  bomber: number;
  destroyer: number;
  deathstar: number;
}

class MoonService {
  async getMoonById(moonId: number): Promise<Moon | null> {
    const result = await pool.query('SELECT * FROM moons WHERE id = $1', [moonId]);
    return result.rows[0] || null;
  }

  async getMoonByPlanetId(planetId: number): Promise<Moon | null> {
    const result = await pool.query('SELECT * FROM moons WHERE planet_id = $1', [planetId]);
    return result.rows[0] || null;
  }

  async tryCreateMoonFromDebris(
    planetId: number,
    userId: number,
    debrisMetal: number,
    debrisCrystal: number
  ): Promise<Moon | null> {
    const existing = await this.getMoonByPlanetId(planetId);
    if (existing) {
      return existing;
    }

    const chance = getMoonChanceFromDebris(debrisMetal, debrisCrystal);
    if (chance <= 0) {
      return null;
    }

    if (!rollForMoon(chance)) {
      return null;
    }

    const diameter = calculateMoonDiameter(chance);
    const totalFields = moonConfig.BASE_FIELDS;

    const result = await pool.query(
      `INSERT INTO moons (
        planet_id,
        user_id,
        name,
        diameter,
        total_fields,
        used_fields,
        lunar_base,
        sensor_phalanx,
        jump_gate,
        moon_robotics_factory,
        moon_shipyard,
        moon_nanite_factory
      ) VALUES ($1, $2, $3, $4, $5, 0, 0, 0, 0, 0, 0, 0)
      RETURNING *`,
      [planetId, userId, 'Moon', diameter, totalFields]
    );

    return result.rows[0] || null;
  }

  async listMoonsByUser(userId: number): Promise<Moon[]> {
    const result = await pool.query('SELECT * FROM moons WHERE user_id = $1 ORDER BY id', [userId]);
    return result.rows;
  }

  async deductResources(
    moonId: number,
    resources: { metal?: number; crystal?: number; deuterium?: number }
  ): Promise<void> {
    const metal = Math.max(0, resources.metal || 0);
    const crystal = Math.max(0, resources.crystal || 0);
    const deuterium = Math.max(0, resources.deuterium || 0);

    await pool.query(
      `UPDATE moons
       SET metal = metal - $1,
           crystal = crystal - $2,
           deuterium = deuterium - $3
       WHERE id = $4`,
      [metal, crystal, deuterium, moonId]
    );
  }

  async addResources(
    moonId: number,
    resources: { metal?: number; crystal?: number; deuterium?: number }
  ): Promise<void> {
    await pool.query(
      `UPDATE moons
       SET metal = metal + $1,
           crystal = crystal + $2,
           deuterium = deuterium + $3
       WHERE id = $4`,
      [resources.metal || 0, resources.crystal || 0, resources.deuterium || 0, moonId]
    );
  }
}

export default new MoonService();
