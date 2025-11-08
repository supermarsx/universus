import { BuildingService } from './buildingService';
import { ShipyardService } from './shipyardService';
import { ResearchService } from './researchService';

export class GameLoopService {
  private static intervalId: NodeJS.Timeout | null = null;

  static start(): void {
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

  static stop(): void {
    if (this.intervalId) {
      clearInterval(this.intervalId);
      this.intervalId = null;
      console.log('Game loop stopped');
    }
  }

  private static async tick(): Promise<void> {
    // Check and finish constructions
    await BuildingService.checkAndFinishConstructions();

    // Check and finish research
    await this.checkAndFinishResearch();

    // Check and finish shipyard queues
    await this.checkAndFinishShipyard();
  }

  private static async checkAndFinishResearch(): Promise<void> {
    const completed = await ResearchService.completeFinishedResearch();
    if (completed > 0) {
      console.log(`Finished ${completed} research projects`);
    }
  }

  private static async checkAndFinishShipyard(): Promise<void> {
    await ShipyardService.completeFinishedJobs();
  }

}
