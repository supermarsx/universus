import { LeaderboardService } from './leaderboardService';
import { pool } from '../config/database';
import { redis } from '../config/redis';
import { getRealtimeHandler } from '../socket';

export class LeaderboardScheduler {
  private static instance: LeaderboardScheduler | null = null;
  private readonly leaderboardService: LeaderboardService;
  private intervalId: NodeJS.Timeout | null = null;
  private lastRun: Date | null = null;
  private intervalMs: number;

  private constructor(intervalMs: number) {
    this.intervalMs = intervalMs;
    this.leaderboardService = new LeaderboardService(pool, redis);
  }

  static start(intervalMs: number = 60 * 60 * 1000): LeaderboardScheduler {
    if (!LeaderboardScheduler.instance) {
      LeaderboardScheduler.instance = new LeaderboardScheduler(intervalMs);
      // Do not start automatic loops during tests or when SKIP_SERVER_START is set
      if (process.env.NODE_ENV === 'test' || process.env.SKIP_SERVER_START === 'true') {
        console.log('[LeaderboardScheduler] Start skipped (test mode or SKIP_SERVER_START)');
      } else {
        LeaderboardScheduler.instance.startLoop();
      }
    }
    return LeaderboardScheduler.instance;
  }

  static getStatus() {
    if (!LeaderboardScheduler.instance) {
      return {
        running: false,
        lastRun: null,
        intervalMs: null,
      };
    }

    return {
      running: Boolean(LeaderboardScheduler.instance.intervalId),
      lastRun: LeaderboardScheduler.instance.lastRun,
      intervalMs: LeaderboardScheduler.instance.intervalMs,
    };
  }

  static async triggerRebuild(): Promise<void> {
    if (!LeaderboardScheduler.instance) {
      LeaderboardScheduler.start();
    } else {
      await LeaderboardScheduler.instance.rebuild();
    }
  }

  private startLoop(): void {
    this.rebuild().catch((error) =>
      console.error('[LeaderboardScheduler] Initial rebuild failed:', error)
    );

    this.intervalId = setInterval(() => {
      this.rebuild().catch((error) =>
        console.error('[LeaderboardScheduler] Scheduled rebuild failed:', error)
      );
    }, this.intervalMs);
  }

  private async rebuild(): Promise<void> {
    this.lastRun = new Date();
    const { playersUpdated, alliancesUpdated } = await this.leaderboardService.rebuildLeaderboards();
    console.log(
      `[LeaderboardScheduler] Rebuilt leaderboards. Players: ${playersUpdated}, Alliances: ${alliancesUpdated}`
    );

    const handler = getRealtimeHandler();
    handler?.broadcastLeaderboardUpdate();
  }
}

export default LeaderboardScheduler;
