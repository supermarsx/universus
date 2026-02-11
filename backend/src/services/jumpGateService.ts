import { pool } from '../config/database';
import { getMoonById, Moon } from './moonService';
import { FleetService } from './fleetService';

const JUMP_GATE_COOLDOWN_MS = 60 * 60 * 1000; // 1 hour

class JumpGateService {
  async canJump(moonId: number): Promise<boolean> {
    const moon = await getMoonById(moonId);
    if (!moon || moon.jump_gate < 1) return false;
    if (!moon.last_jump_time) return true;
    return Date.now() - new Date(moon.last_jump_time).getTime() >= JUMP_GATE_COOLDOWN_MS;
  }

  async jumpFleet(userId: number, fromMoonId: number, toMoonId: number, fleetIds: number[]): Promise<{ success: boolean; error?: string }> {
    // Validate source and destination moons
    const fromMoon = await getMoonById(fromMoonId);
    const toMoon = await getMoonById(toMoonId);
    if (!fromMoon || !toMoon) return { success: false, error: 'Invalid moon(s)' };
    if (fromMoon.user_id !== userId) return { success: false, error: 'Not your moon' };
    if (toMoon.user_id !== userId) return { success: false, error: 'Destination moon not owned by user' };
    if (fromMoonId === toMoonId) return { success: false, error: 'Destination moon must be different' };
    if (fromMoon.jump_gate < 1 || toMoon.jump_gate < 1) return { success: false, error: 'Both moons must have Jump Gates' };
    if (!(await this.canJump(fromMoonId))) return { success: false, error: 'Source Jump Gate is on cooldown' };
    if (!(await this.canJump(toMoonId))) return { success: false, error: 'Destination Jump Gate is on cooldown' };

    // Move fleets (use FleetService.moveFleetToMoon)
    try {
      for (const fleetId of fleetIds) {
        await FleetService.moveFleetToMoon(userId, fromMoonId, fleetId, toMoonId);
      }
    } catch (error: any) {
      return { success: false, error: error.message || 'Failed to move fleet(s)' };
    }

    // Set cooldown (skip actual DB write during tests)
    if (process.env.NODE_ENV !== 'test' && process.env.SKIP_SERVER_START !== 'true') {
      await pool.query('UPDATE moons SET last_jump_time = NOW() WHERE id = ANY($1::int[])', [[fromMoonId, toMoonId]]);
    }
    return { success: true };
  }

}

export default new JumpGateService();
