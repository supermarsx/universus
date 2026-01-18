import { pool } from '../config/database';
import moonService from './moonService';

function calculateDestructionChance(diameter: number, numDeathstars: number): number {
  // Spec: (A * sqrt(n)) * max(0, 100 - sqrt(d)) / 100, with A=1
  return (Math.sqrt(numDeathstars) * Math.max(0, 100 - Math.sqrt(diameter))) / 100;
}

function calculateDeathstarLossChance(diameter: number): number {
  // Spec: B * sqrt(d) / 2, with B=1
  return Math.sqrt(diameter) / 2;
}

class DestroyMoonService {
  async attemptDestruction(attackerId: number, moonId: number, numDeathstars: number): Promise<{
    destroyed: boolean;
    deathstarsLost: number;
    chance: number;
    lossChance: number;
    error?: string;
  }> {
    const moon = await moonService.getMoonById(moonId);
    if (!moon) return { destroyed: false, deathstarsLost: 0, chance: 0, lossChance: 0, error: 'Moon not found' };
    if (numDeathstars < 1) return { destroyed: false, deathstarsLost: 0, chance: 0, lossChance: 0, error: 'No Deathstars sent' };

    const planetResult = await pool.query('SELECT galaxy, system, position FROM planets WHERE id = $1', [moon.planet_id]);
    const planet = planetResult.rows[0];
    if (!planet) return { destroyed: false, deathstarsLost: 0, chance: 0, lossChance: 0, error: 'Moon planet not found' };

    const chance = calculateDestructionChance(moon.diameter, numDeathstars);
    const lossChance = calculateDeathstarLossChance(moon.diameter);
    const destroyed = Math.random() * 100 < chance;
    let deathstarsLost = 0;
    if (destroyed) {
      // Log destruction
      await pool.query('INSERT INTO rip_attack (attacker_id, moon_galaxy, moon_system, moon_position, success, deathstars_lost) VALUES ($1, $2, $3, $4, $5, $6)', [attackerId, planet.galaxy, planet.system, planet.position, true, deathstarsLost]);
      // Cancel queues
      await pool.query('DELETE FROM building_queues WHERE moon_id = $1', [moonId]);
      await pool.query('DELETE FROM shipyard_queues WHERE moon_id = $1', [moonId]);
      // Delete moon (ships/defenses deleted with it)
      await pool.query('DELETE FROM moons WHERE id = $1', [moonId]);
    } else {
      // Chance to lose Deathstars
      for (let i = 0; i < numDeathstars; i++) {
        if (Math.random() * 100 < lossChance) deathstarsLost++;
      }
      // Log failed attempt
      await pool.query('INSERT INTO rip_attack (attacker_id, moon_galaxy, moon_system, moon_position, success, deathstars_lost) VALUES ($1, $2, $3, $4, $5, $6)', [attackerId, planet.galaxy, planet.system, planet.position, false, deathstarsLost]);
    }
    return { destroyed, deathstarsLost, chance, lossChance };
  }
}

export default new DestroyMoonService();
