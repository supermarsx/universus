import { BuildingService } from './buildingService';
import { FleetService } from './fleetService';
import { pool } from '../config/database';

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
    const result = await pool.query(
      'SELECT * FROM research_queue WHERE end_time <= NOW()'
    );

    for (const research of result.rows) {
      try {
        await pool.query('BEGIN');

        // Update research level
        await pool.query(
          `UPDATE research SET ${research.research_type} = $1 WHERE user_id = $2`,
          [research.level, research.user_id]
        );

        // Remove from queue
        await pool.query('DELETE FROM research_queue WHERE id = $1', [research.id]);

        await pool.query('COMMIT');
        console.log(`Finished research ${research.research_type} for user ${research.user_id}`);
      } catch (error) {
        await pool.query('ROLLBACK');
        console.error(`Error finishing research ${research.id}:`, error);
      }
    }
  }

  private static async checkAndFinishShipyard(): Promise<void> {
    const result = await pool.query(
      'SELECT * FROM shipyard_queue WHERE end_time <= NOW()'
    );

    for (const queue of result.rows) {
      try {
        await pool.query('BEGIN');

        // Add ships/defenses to planet
        await pool.query(
          `UPDATE planets SET ${queue.unit_type} = ${queue.unit_type} + $1 WHERE id = $2`,
          [queue.quantity, queue.planet_id]
        );

        // Remove from queue
        await pool.query('DELETE FROM shipyard_queue WHERE id = $1', [queue.id]);

        await pool.query('COMMIT');
        console.log(`Finished building ${queue.quantity} ${queue.unit_type} on planet ${queue.planet_id}`);
      } catch (error) {
        await pool.query('ROLLBACK');
        console.error(`Error finishing shipyard queue ${queue.id}:`, error);
      }
    }
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
    } catch (error) {
      await pool.query('ROLLBACK');
      throw error;
    }
  }
}
