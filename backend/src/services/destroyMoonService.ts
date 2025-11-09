import { pool } from '../config/database';
import moonService from './moonService';

function calculateDestructionChance(diameter: number, numDeathstars: number): number {
  // OGame-like: chance = min(100, (numDeathstars * 100) / (diameter * diameter / 1000))
  return Math.min(100, (numDeathstars * 100) / (diameter * diameter / 1000));
}

function calculateDeathstarLossChance(diameter: number): number {
  // OGame-like: chance = min(100, 100 / Math.sqrt(diameter))
  return Math.min(100, 100 / Math.sqrt(diameter));
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
    const chance = calculateDestructionChance(moon.diameter, numDeathstars);
    const lossChance = calculateDeathstarLossChance(moon.diameter);
    const destroyed = Math.random() * 100 < chance;
    let deathstarsLost = 0;
    if (destroyed) {
      // Delete moon and all structures
      await pool.query('DELETE FROM moons WHERE id = $1', [moonId]);
    } else {
      // Chance to lose Deathstars
      for (let i = 0; i < numDeathstars; i++) {
        if (Math.random() * 100 < lossChance) deathstarsLost++;
      }
    }
    return { destroyed, deathstarsLost, chance, lossChance };
  }
}

export default new DestroyMoonService();
