import { BuildingService } from './buildingService';
import { FleetService } from './fleetService';
import { ShipyardService } from './shipyardService';
import { pool } from '../config/database';
import notificationService from './notificationService';
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

    // Check and resolve fleet arrivals
    await this.checkFleetArrivals();
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

  private static async checkFleetArrivals(): Promise<void> {
    const result = await pool.query(
      `SELECT * FROM fleets WHERE arrival_time <= NOW() AND status = 'outbound'`
    );

    for (const fleet of result.rows) {
      try {
        await FleetService.processFleetArrival(fleet.id);
        console.log(`Processed fleet ${fleet.id} arrival`);
      } catch (error) {
        console.error(`Error handling fleet ${fleet.id} arrival:`, error);
      }
    }

    // Check for returning fleets
    const returningResult = await pool.query(
      `SELECT * FROM fleets WHERE return_time <= NOW() AND status = 'returning'`
    );

    for (const fleet of returningResult.rows) {
      try {
        await this.returnFleetToOrigin(fleet);
        console.log(`Fleet ${fleet.id} returned home`);
      } catch (error) {
        console.error(`Error returning fleet ${fleet.id}:`, error);
      }
    }
  }

  private static async returnFleetToOrigin(fleet: any): Promise<void> {
    const planetResult = await pool.query(
      'SELECT galaxy, system, position, name FROM planets WHERE id = $1',
      [fleet.origin_planet_id]
    );

    await pool.query('BEGIN');

    try {
      // Return ships to origin planet
      const ships = fleet.ships;
      for (const [shipType, count] of Object.entries(ships)) {
        await pool.query(
          `UPDATE planets SET ${shipType} = ${shipType} + $1 WHERE id = $2`,
          [count, fleet.origin_planet_id]
        );
      }

      // Return cargo
      await pool.query(
        `UPDATE planets 
         SET metal = metal + $1, crystal = crystal + $2, deuterium = deuterium + $3
         WHERE id = $4`,
        [fleet.cargo_metal, fleet.cargo_crystal, fleet.cargo_deuterium, fleet.origin_planet_id]
      );

      // Delete fleet
      await pool.query('DELETE FROM fleets WHERE id = $1', [fleet.id]);

      await pool.query('COMMIT');

      const planet = planetResult.rows[0];
      const location = planet
        ? `${planet.name || 'Planet'} (${planet.galaxy}:${planet.system}:${planet.position})`
        : `Planet ${fleet.origin_planet_id}`;

      await notificationService.notifyFleetReturned(fleet.user_id, fleet.id, location);
    } catch (error) {
      await pool.query('ROLLBACK');
      throw error;
    }
  }
}
