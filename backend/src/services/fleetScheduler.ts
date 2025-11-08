import redis from '../config/redis';
import { pool } from '../config/database';

type FleetEventType = 'arrival' | 'return';

interface FleetSchedulerCallbacks {
  onArrival: (fleetId: number) => Promise<void>;
  onReturn: (fleetId: number) => Promise<void>;
}

interface ScheduledEvent {
  type: FleetEventType;
  fleetId: number;
  score: number;
}

const ZSET_KEY = 'fleets:schedule';

class FleetScheduler {
  private static instance: FleetScheduler;
  private callbacks: FleetSchedulerCallbacks | null = null;
  private timer: NodeJS.Timeout | null = null;
  private active = false;

  static getInstance(): FleetScheduler {
    if (!FleetScheduler.instance) {
      FleetScheduler.instance = new FleetScheduler();
    }
    return FleetScheduler.instance;
  }

  registerCallbacks(callbacks: FleetSchedulerCallbacks): void {
    this.callbacks = callbacks;
  }

  async start(): Promise<void> {
    if (this.active || !redis) {
      return;
    }

    this.active = true;
    await this.bootstrapFromDatabase();
    await this.scheduleNextTick();
    console.log('[FleetScheduler] Initialized with Redis-backed event queue');
  }

  async reboot(): Promise<void> {
    await this.bootstrapFromDatabase();
    await this.scheduleNextTick();
  }

  async scheduleArrival(fleetId: number, arrivalTime: Date | string | number | null): Promise<void> {
    if (!arrivalTime) return;
    await this.addEvent({ type: 'arrival', fleetId, score: this.toScore(arrivalTime) });
  }

  async scheduleReturn(fleetId: number, returnTime: Date | string | number | null): Promise<void> {
    if (!returnTime) return;
    await this.addEvent({ type: 'return', fleetId, score: this.toScore(returnTime) });
  }

  async unschedule(fleetId: number, type?: FleetEventType): Promise<void> {
    if (!redis) return;
    if (type) {
      await redis.zrem(ZSET_KEY, `${type}:${fleetId}`);
    } else {
      await redis.zrem(ZSET_KEY, `arrival:${fleetId}`, `return:${fleetId}`);
    }
    await this.scheduleNextTick();
  }

  private async addEvent(event: ScheduledEvent): Promise<void> {
    if (!redis) return;
    await redis.zadd(ZSET_KEY, event.score, `${event.type}:${event.fleetId}`);
    await this.scheduleNextTick();
  }

  private async bootstrapFromDatabase(): Promise<void> {
    if (!redis) return;
    await redis.del(ZSET_KEY);

    const result = await pool.query(
      `SELECT id, status, arrival_time, return_time
       FROM fleets
       WHERE status IN ('outbound', 'returning')`
    );

    if (!result.rows.length) return;

    const pipeline = redis.multi();
    result.rows.forEach((row) => {
      if (row.status === 'outbound' && row.arrival_time) {
        pipeline.zadd(ZSET_KEY, new Date(row.arrival_time).getTime(), `arrival:${row.id}`);
      } else if (row.status === 'returning' && row.return_time) {
        pipeline.zadd(ZSET_KEY, new Date(row.return_time).getTime(), `return:${row.id}`);
      }
    });

    await pipeline.exec();
  }

  private async scheduleNextTick(): Promise<void> {
    if (!redis) return;
    if (this.timer) {
      clearTimeout(this.timer);
      this.timer = null;
    }

    const entries = await redis.zrange(ZSET_KEY, 0, 0, 'WITHSCORES');
    if (!entries || entries.length < 2) {
      return;
    }

    const score = Number(entries[1]);
    const delay = Math.max(0, score - Date.now());
    const timeout = Math.min(delay, 0x7fffffff); // max setTimeout
    this.timer = setTimeout(() => this.processDueEvents(), timeout);
  }

  private async processDueEvents(): Promise<void> {
    if (!redis || !this.callbacks) return;

    const now = Date.now();
    const entries = await redis.zrangebyscore(ZSET_KEY, 0, now, 'WITHSCORES');
    if (!entries.length) {
      await this.scheduleNextTick();
      return;
    }

    const events: Array<{ member: string; type: FleetEventType; fleetId: number }> = [];
    for (let i = 0; i < entries.length; i += 2) {
      const member = entries[i];
      const [type, id] = member.split(':');
      const fleetId = parseInt(id, 10);
      if (!fleetId || (type !== 'arrival' && type !== 'return')) continue;
      events.push({ member, type, fleetId });
    }

    if (!events.length) {
      await this.scheduleNextTick();
      return;
    }

    const pipeline = redis.multi();
    events.forEach((event) => pipeline.zrem(ZSET_KEY, event.member));
    await pipeline.exec();

    for (const event of events) {
      try {
        if (event.type === 'arrival') {
          await this.callbacks.onArrival(event.fleetId);
        } else {
          await this.callbacks.onReturn(event.fleetId);
        }
      } catch (error) {
        console.error('[FleetScheduler] Failed processing fleet event', event, error);
      }
    }

    await this.scheduleNextTick();
  }

  private toScore(value: Date | string | number): number {
    if (typeof value === 'number') return value;
    if (value instanceof Date) return value.getTime();
    return new Date(value).getTime();
  }
}

export default FleetScheduler.getInstance();
