import { BuildingService } from './buildingService';
import { ShipyardService } from './shipyardService';
import { ResearchService } from './researchService';

export class GameLoopService {
  private static intervalId: NodeJS.Timeout | null = null;
  /**
   * Start the background game loop. The loop runs periodically and performs
   * scheduled checks such as finishing constructions, research and shipyard jobs.
   * This method is idempotent—the loop will not be started twice.
   */
  static start(): void {
    if (process.env.NODE_ENV === 'test' || process.env.SKIP_SERVER_START === 'true') {
      console.log('Game loop start skipped (test mode or SKIP_SERVER_START=true)');
      return;
    }
    console.log('Starting game loop...');

    // Run every 10 seconds
    this.intervalId = setInterval(async () => {
      try {
        await this.tick();
      } catch (error) {
        console.error('Game loop error:', error);
      }
    }, 10000);
  }

  /**
   * Stop the running game loop if active.
   */
  static stop(): void {
    if (this.intervalId) {
      clearInterval(this.intervalId);
      this.intervalId = null;
      console.log('Game loop stopped');
    }
  }

  /**
   * Single tick executed by the loop. Performs the unit-of-work for scheduled
   * background tasks that must be run periodically.
   *
   * @private
   */
  private static async tick(): Promise<void> {
    // Check and finish constructions
    await BuildingService.checkAndFinishConstructions();

    // Check and finish research
    await this.checkAndFinishResearch();

    // Check and finish shipyard queues
    await this.checkAndFinishShipyard();
  }

  /**
   * Check for research jobs that have completed and finalize them.
   * Logs the number of completed research projects when applicable.
   *
   * @private
   */
  private static async checkAndFinishResearch(): Promise<void> {
    const completed = await ResearchService.completeFinishedResearch();
    if (completed > 0) {
      console.log(`Finished ${completed} research projects`);
    }
  }

  /**
   * Run shipyard job completion checks.
   *
   * @private
   */
  private static async checkAndFinishShipyard(): Promise<void> {
    await ShipyardService.completeFinishedJobs();
  }

}
