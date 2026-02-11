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

const MIN_DESTROY_FLIGHT_MS = 30 * 60 * 1000;

interface RipAttackRow {
  id: number;
  attacker_id: number;
  source_moon_id: number;
  target_moon_id: number;
  num_rips: number;
  status: 'scheduled' | 'resolved' | 'failed';
}

class DestroyMoonService {
  private calculateTravelMs(
    source: { galaxy: number; system: number; position: number },
    target: { galaxy: number; system: number; position: number },
    speedPercent: number = 100
  ): number {
    const clampedSpeed = Math.max(10, Math.min(100, speedPercent));
    const galaxyDelta = Math.abs(source.galaxy - target.galaxy);
    const systemDelta = Math.abs(source.system - target.system);
    const positionDelta = Math.abs(source.position - target.position);

    const distanceScore = galaxyDelta * 20000 + systemDelta * 95 + positionDelta * 5;
    const scaled = (distanceScore * (110 / clampedSpeed)) * 1000;
    return Math.max(MIN_DESTROY_FLIGHT_MS, Math.round(scaled));
  }

  async scheduleDestruction(
    attackerId: number,
    sourceMoonId: number,
    targetMoonId: number,
    numDeathstars: number,
    speedPercent: number = 100
  ): Promise<{
    attackId: number;
    eta: string;
    travelSeconds: number;
    chancePreview: number;
    lossChancePreview: number;
  }> {
    if (numDeathstars < 1) {
      throw new Error('No Deathstars sent');
    }

    const client = await pool.connect();
    try {
      await client.query('BEGIN');

      const sourceResult = await client.query(
        `SELECT m.id, m.user_id, m.deathstar, p.galaxy, p.system, p.position
         FROM moons m
         JOIN planets p ON p.id = m.planet_id
         WHERE m.id = $1
         FOR UPDATE`,
        [sourceMoonId]
      );
      const targetResult = await client.query(
        `SELECT m.id, m.user_id, m.diameter, p.galaxy, p.system, p.position
         FROM moons m
         JOIN planets p ON p.id = m.planet_id
         WHERE m.id = $1
         FOR UPDATE`,
        [targetMoonId]
      );

      const source = sourceResult.rows[0];
      const target = targetResult.rows[0];

      if (!source) throw new Error('Source moon not found');
      if (!target) throw new Error('Target moon not found');
      if (source.user_id !== attackerId) throw new Error('Moon access denied');
      if (source.id === target.id) throw new Error('Target moon must be different');
      if (target.user_id === attackerId) throw new Error('Cannot destroy your own moon');

      const available = Number(source.deathstar || 0);
      if (available < numDeathstars) {
        throw new Error('Insufficient Deathstars at moon');
      }

      const travelMs = this.calculateTravelMs(source, target, speedPercent);
      const eta = new Date(Date.now() + travelMs);
      const chancePreview = calculateDestructionChance(Number(target.diameter), numDeathstars);
      const lossChancePreview = calculateDeathstarLossChance(Number(target.diameter));

      await client.query(
        'UPDATE moons SET deathstar = deathstar - $1 WHERE id = $2',
        [numDeathstars, sourceMoonId]
      );

      const insert = await client.query(
        `INSERT INTO rip_attack (
           attacker_id,
           source_moon_id,
           target_moon_id,
           num_rips,
           p_destroy,
           p_lose,
           status,
           scheduled_for
         ) VALUES ($1, $2, $3, $4, $5, $6, 'scheduled', $7)
         RETURNING id, scheduled_for`,
        [attackerId, sourceMoonId, targetMoonId, numDeathstars, chancePreview, lossChancePreview, eta.toISOString()]
      );

      await client.query('COMMIT');
      return {
        attackId: insert.rows[0].id,
        eta: insert.rows[0].scheduled_for,
        travelSeconds: Math.floor(travelMs / 1000),
        chancePreview,
        lossChancePreview,
      };
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }
  }

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
      await pool.query('DELETE FROM building_queue WHERE moon_id = $1', [moonId]).catch(() => undefined);
      await pool.query('DELETE FROM construction_queue WHERE moon_id = $1', [moonId]);
      await pool.query('DELETE FROM shipyard_queue WHERE moon_id = $1', [moonId]);
      await pool.query('DELETE FROM moons WHERE id = $1', [moonId]);
    } else {
      for (let i = 0; i < numDeathstars; i++) {
        if (Math.random() * 100 < lossChance) deathstarsLost++;
      }
    }
    return { destroyed, deathstarsLost, chance, lossChance };
  }

  async processDueAttacks(limit: number = 20): Promise<void> {
    const client = await pool.connect();
    try {
      await client.query('BEGIN');
      const due = await client.query(
        `SELECT id, attacker_id, source_moon_id, target_moon_id, num_rips, status
         FROM rip_attack
         WHERE status = 'scheduled' AND scheduled_for <= NOW()
         ORDER BY scheduled_for ASC
         LIMIT $1
         FOR UPDATE SKIP LOCKED`,
        [limit]
      );
      await client.query('COMMIT');

      for (const row of due.rows as RipAttackRow[]) {
        await this.resolveScheduledAttack(row);
      }
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }
  }

  private async resolveScheduledAttack(attack: RipAttackRow): Promise<void> {
    const client = await pool.connect();
    try {
      await client.query('BEGIN');

      const targetResult = await client.query(
        'SELECT id, diameter FROM moons WHERE id = $1 FOR UPDATE',
        [attack.target_moon_id]
      );
      const sourceResult = await client.query(
        'SELECT id FROM moons WHERE id = $1 FOR UPDATE',
        [attack.source_moon_id]
      );

      if (targetResult.rows.length === 0) {
        await client.query(
          `UPDATE rip_attack
           SET status = 'failed', resolved_ts = NOW(), error_message = 'Target moon no longer exists'
           WHERE id = $1`,
          [attack.id]
        );
        await client.query(
          'UPDATE moons SET deathstar = deathstar + $1 WHERE id = $2',
          [attack.num_rips, attack.source_moon_id]
        ).catch(() => undefined);
        await client.query('COMMIT');
        return;
      }

      const targetMoon = targetResult.rows[0];
      const chance = calculateDestructionChance(Number(targetMoon.diameter), attack.num_rips);
      const lossChance = calculateDeathstarLossChance(Number(targetMoon.diameter));
      const destroyed = Math.random() * 100 < chance;
      let deathstarsLost = 0;

      if (!destroyed) {
        for (let i = 0; i < attack.num_rips; i++) {
          if (Math.random() * 100 < lossChance) deathstarsLost++;
        }
      }

      if (destroyed) {
        await client.query('DELETE FROM construction_queue WHERE moon_id = $1', [attack.target_moon_id]);
        await client.query('DELETE FROM shipyard_queue WHERE moon_id = $1', [attack.target_moon_id]);
        await client.query('DELETE FROM moons WHERE id = $1', [attack.target_moon_id]);
      }

      const survivors = Math.max(0, attack.num_rips - deathstarsLost);
      if (sourceResult.rows.length > 0 && survivors > 0) {
        await client.query(
          'UPDATE moons SET deathstar = deathstar + $1 WHERE id = $2',
          [survivors, attack.source_moon_id]
        );
      }

      await client.query(
        `UPDATE rip_attack
         SET status = 'resolved',
             success = $2,
             deathstars_lost = $3,
             p_destroy = $4,
             p_lose = $5,
             resolved_ts = NOW(),
             error_message = NULL
         WHERE id = $1`,
        [attack.id, destroyed, deathstarsLost, chance, lossChance]
      );

      await client.query('COMMIT');
    } catch (error: any) {
      await client.query('ROLLBACK');
      await pool.query(
        `UPDATE rip_attack
         SET status = 'failed',
             resolved_ts = NOW(),
             error_message = $2
         WHERE id = $1`,
        [attack.id, error?.message || 'Moon destruction failed']
      ).catch(() => undefined);
    } finally {
      client.release();
    }
  }
}

export default new DestroyMoonService();
