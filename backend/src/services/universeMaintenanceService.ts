// =====================================================
// UNIVERSE MAINTENANCE SERVICE
// Automated universe management and balancing
// =====================================================

import pool from '../config/database';
import { MaintenanceResult, MaintenanceTaskType } from '../types/universe';

export class UniverseMaintenanceService {
  
  /**
   * Run population balance maintenance
   */
  async runPopulationBalance(universeId: number): Promise<MaintenanceResult> {
    const startTime = Date.now();
    const actions: string[] = [];
    const metrics: Record<string, any> = {};
    
    try {
      const client = await pool.connect();
      
      try {
        // Get current population stats
        const statsResult = await client.query(
          `SELECT 
            current_players,
            target_bot_count,
            (SELECT COUNT(*) FROM generated_bots WHERE universe_id = $1 AND is_active = TRUE) as active_bots
           FROM universe_seeds WHERE id = $1`,
          [universeId]
        );
        
        const stats = statsResult.rows[0];
        metrics.currentPlayers = stats.current_players;
        metrics.activeBots = parseInt(stats.active_bots);
        metrics.targetBots = stats.target_bot_count;
        
        // Adjust bot population if needed
        const botDifference = stats.target_bot_count - parseInt(stats.active_bots);
        
        if (Math.abs(botDifference) > 10) {
          actions.push(`Bot population adjustment needed: ${botDifference > 0 ? '+' : ''}${botDifference}`);
        }
        
        return {
          success: true,
          taskType: MaintenanceTaskType.POPULATION_BALANCE,
          actionsPerformed: actions,
          metricsChanged: metrics,
          duration: Math.floor((Date.now() - startTime) / 1000),
          message: `Population balance check completed`
        };
        
      } finally {
        client.release();
      }
      
    } catch (error) {
      console.error('Error in population balance:', error);
      return {
        success: false,
        taskType: MaintenanceTaskType.POPULATION_BALANCE,
        actionsPerformed: [],
        metricsChanged: {},
        duration: Math.floor((Date.now() - startTime) / 1000),
        message: error instanceof Error ? error.message : 'Failed'
      };
    }
  }
  
  /**
   * Start automatic maintenance scheduler
   */
  startAutomaticMaintenance(universeId: number): void {
    // Run maintenance every hour
    setInterval(async () => {
      console.log(`[Universe ${universeId}] Running maintenance tasks...`);
      
      await this.runPopulationBalance(universeId);
      
      console.log(`[Universe ${universeId}] Maintenance completed`);
    }, 3600000); // 1 hour
    
    console.log(`[Universe ${universeId}] Automatic maintenance started`);
  }
}

export default new UniverseMaintenanceService();
