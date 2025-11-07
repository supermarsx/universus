// =====================================================
// SALVAGE SERVICE
// Handles salvage missions, collection mechanics, competition
// =====================================================

import pool from '../config/database';
import { 
  DebrisSalvageOperation,
  SalvageType,
  SalvageTypeValues,
  SalvageStatus,
  StartSalvageRequest,
  SalvageOperationResult,
  SalvageCompletionResult,
  SalvageResources,
  ComponentCollection,
  SalvageEfficiencyCalculation,
  Coordinates,
  DistanceCalculation,
  UserSalvageProfile,
  SalvageStatistics
} from '../types/debris';

export class SalvageService {
  
  // =====================================================
  // SALVAGE OPERATIONS
  // =====================================================
  
  /**
   * Start a salvage operation
   */
  async startSalvageOperation(request: StartSalvageRequest): Promise<SalvageOperationResult> {
    const client = await pool.connect();
    
    try {
      await client.query('BEGIN');
      
      const { userId, debrisId, salvageType, fleetId, shipTypes, cargoCapacity } = request;
      
      // Verify debris field exists and is active
      const debrisResult = await client.query(
        'SELECT * FROM combat_debris WHERE id = $1 AND is_active = TRUE AND expires_at > NOW()',
        [debrisId]
      );
      
      if (debrisResult.rows.length === 0) {
        throw new Error('Debris field not found or inactive');
      }
      
      const debris = debrisResult.rows[0];
      
      // Calculate efficiency
      const efficiency = await this.calculateSalvageEfficiency(userId, debrisId, salvageType);
      
      // Calculate travel time (simplified - would use fleet mechanics in production)
      const travelTime = 3600; // 1 hour default
      const salvageDuration = this.calculateSalvageDuration(salvageType, cargoCapacity);
      const totalDuration = travelTime * 2 + salvageDuration;
      
      const arrivalTime = new Date(Date.now() + travelTime * 1000);
      const returnTime = new Date(Date.now() + totalDuration * 1000);
      
      // Check for existing salvage operations (competition)
      const competitionResult = await client.query(
        'SELECT COUNT(*) as count FROM debris_salvage WHERE debris_id = $1 AND status IN ($2, $3)',
        [debrisId, 'en_route', 'salvaging']
      );
      
      const competitionCount = competitionResult.rows[0]?.count || '0';
      const isCompetitive = parseInt(competitionCount) > 0;
      
      // Create salvage operation
      const operationResult = await client.query(
        `INSERT INTO debris_salvage (
          user_id, debris_id, salvage_type, fleet_id, ship_types,
          cargo_capacity, salvage_efficiency, status,
          start_time, arrival_time, return_time,
          is_competitive
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        RETURNING *`,
        [
          userId, debrisId, salvageType, fleetId, JSON.stringify(shipTypes),
          cargoCapacity, efficiency.final_efficiency, 'en_route',
          new Date(), arrivalTime, returnTime,
          isCompetitive
        ]
      );
      
      if (operationResult.rows.length === 0) {
        throw new Error('Failed to create salvage operation');
      }
      const operation = this.mapSalvageOperation(operationResult.rows[0]);
      
      // Schedule automatic completion (in production, this would be handled by a game loop)
      setTimeout(() => {
        this.completeSalvageOperation(operation.id).catch(console.error);
      }, totalDuration * 1000);
      
      await client.query('COMMIT');
      
      return {
        operation_id: operation.id,
        estimated_duration: totalDuration,
        estimated_resources: {
          metal: Math.floor(debris.total_metal * efficiency.final_efficiency),
          crystal: Math.floor(debris.total_crystal * efficiency.final_efficiency),
          deuterium: Math.floor(debris.total_deuterium * efficiency.final_efficiency),
          rare_materials: Math.floor((debris.total_rare_materials || 0) * efficiency.final_efficiency)
        },
        success_probability: efficiency.final_efficiency,
        message: isCompetitive 
          ? `Salvage operation started - WARNING: Competition detected!`
          : `Salvage operation started to [${debris.galaxy}:${debris.system}:${debris.position}]`
      };
      
    } catch (error) {
      await client.query('ROLLBACK');
      console.error('Error starting salvage operation:', error);
      return {
        operation_id: 0,
        estimated_duration: 0,
        estimated_resources: { metal: 0, crystal: 0, deuterium: 0, rare_materials: 0 },
        success_probability: 0,
        error: error instanceof Error ? error.message : 'Failed to start salvage operation'
      };
    } finally {
      client.release();
    }
  }
  
  /**
   * Complete a salvage operation
   */
  async completeSalvageOperation(operationId: number): Promise<SalvageCompletionResult> {
    const client = await pool.connect();
    
    try {
      await client.query('BEGIN');
      
      // Get operation details
      const opResult = await client.query(
        'SELECT * FROM debris_salvage WHERE id = $1',
        [operationId]
      );
      
      if (opResult.rows.length === 0) {
        throw new Error('Salvage operation not found');
      }
      
      const operation = opResult.rows[0];
      
      if (operation.status === 'completed' || operation.status === 'failed') {
        throw new Error('Salvage operation already completed');
      }
      
      // Get debris field
      const debrisResult = await client.query(
        'SELECT * FROM combat_debris WHERE id = $1',
        [operation.debris_id]
      );
      
      if (debrisResult.rows.length === 0) {
        throw new Error('Debris field not found');
      }
      
      const debris = debrisResult.rows[0];
      
      // Check for competition/conflicts
      const competingOpsResult = await client.query(
        `SELECT user_id FROM debris_salvage 
         WHERE debris_id = $1 AND id != $2 AND status = 'salvaging'`,
        [operation.debris_id, operationId]
      );
      
      const hasConflict = competingOpsResult.rows.length > 0;
      const efficiencyModifier = hasConflict ? 0.75 : 1.0;
      
      // Calculate resources collected
      const efficiency = operation.salvage_efficiency * efficiencyModifier;
      const cargoLimit = operation.cargo_capacity;
      
      const metalCollected = Math.min(
        Math.floor(debris.total_metal * efficiency),
        Math.floor(cargoLimit * 0.5)
      );
      
      const crystalCollected = Math.min(
        Math.floor(debris.total_crystal * efficiency),
        Math.floor(cargoLimit * 0.3)
      );
      
      const deuteriumCollected = Math.min(
        Math.floor(debris.total_deuterium * efficiency),
        Math.floor(cargoLimit * 0.2)
      );
      
      const rareMaterialsCollected = Math.min(
        Math.floor((debris.total_rare_materials || 0) * efficiency),
        Math.floor(cargoLimit * 0.1)
      );
      
      // Calculate experience gained
      const totalCollected = metalCollected + crystalCollected + deuteriumCollected + rareMaterialsCollected;
      const experienceGained = Math.floor(totalCollected / 1000);
      
      // Collect components (random chance)
      const componentsCollected = await this.collectComponents(
        client, 
        operation.debris_id, 
        operation.user_id,
        operation.salvage_type
      );
      
      // Update debris field (subtract collected resources)
      await client.query(
        `UPDATE combat_debris
         SET total_metal = GREATEST(0, total_metal - $1),
             total_crystal = GREATEST(0, total_crystal - $2),
             total_deuterium = GREATEST(0, total_deuterium - $3),
             total_rare_materials = GREATEST(0, total_rare_materials - $4)
         WHERE id = $5`,
        [metalCollected, crystalCollected, deuteriumCollected, rareMaterialsCollected, operation.debris_id]
      );
      
      // Update salvage operation
      await client.query(
        `UPDATE debris_salvage
         SET status = 'completed',
             completion_time = NOW(),
             resources_collected = $1,
             components_collected = $2,
             total_value = $3,
             experience_gained = $4,
             success_rate = $5,
             conflict_occurred = $6
         WHERE id = $7`,
        [
          JSON.stringify({ 
            metal: metalCollected, 
            crystal: crystalCollected, 
            deuterium: deuteriumCollected,
            rare_materials: rareMaterialsCollected
          } ),
          JSON.stringify(componentsCollected),
          totalCollected,
          experienceGained,
          efficiency,
          hasConflict,
          operationId
        ]
      );
      
      // Add resources to user
      await client.query(
        `UPDATE users
         SET metal = metal + $1,
             crystal = crystal + $2,
             deuterium = deuterium + $3,
             rare_materials = COALESCE(rare_materials, 0) + $4,
             salvage_experience = salvage_experience + $5
         WHERE id = $6`,
        [metalCollected, crystalCollected, deuteriumCollected, rareMaterialsCollected, experienceGained, operation.user_id]
      );
      
      // Update user salvage statistics
      await this.updateUserSalvageStats(
        client,
        operation.user_id,
        { metal: metalCollected, crystal: crystalCollected, deuterium: deuteriumCollected, rare_materials: rareMaterialsCollected },
        Object.keys(componentsCollected).length,
        experienceGained
      );
      
      await client.query('COMMIT');
      
      return {
        success: true,
        resources_collected: { 
          metal: metalCollected, 
          crystal: crystalCollected, 
          deuterium: deuteriumCollected,
          rare_materials: rareMaterialsCollected
        },
        components_collected: componentsCollected,
        experienceGained: experienceGained,
        efficiencyAchieved: efficiency,
        conflicts: hasConflict,
        message: `Salvage completed! Collected ${totalCollected} resources and ${Object.keys(componentsCollected).length} components`
      };
      
    } catch (error) {
      await client.query('ROLLBACK');
      console.error('Error completing salvage operation:', error);
      return {
        success: false,
        resources_collected: { metal: 0, crystal: 0, deuterium: 0, rare_materials: 0 },
        components_collected: {},
        experienceGained: 0,
        efficiencyAchieved: 0,
        conflicts: false,
        message: error instanceof Error ? error.message : 'Failed to complete salvage operation'
      };
    } finally {
      client.release();
    }
  }
  
  /**
   * Cancel a salvage operation
   */
  async cancelSalvageOperation(operationId: number, userId: number): Promise<boolean> {
    const result = await pool.query(
      `UPDATE debris_salvage
       SET status = 'failed', notes = 'Cancelled by user'
       WHERE id = $1 AND user_id = $2 AND status IN ('planned', 'en_route')
       RETURNING id`,
      [operationId, userId]
    );
    
    return (result.rowCount || 0) > 0;
  }
  
  // =====================================================
  // SALVAGE EFFICIENCY
  // =====================================================
  
  /**
   * Calculate salvage efficiency for a user/debris combination
   */
  async calculateSalvageEfficiency(
    userId: number, 
    debrisId: number, 
    salvageType: SalvageType
  ): Promise<SalvageEfficiencyCalculation> {
    const baseEfficiency = 0.7; // 70% base efficiency
    
    // Get user's salvage tech level
    const userResult = await pool.query(
      'SELECT salvage_tech_level FROM users WHERE id = $1',
      [userId]
    );
    
    const techLevel = userResult.rows[0]?.salvage_tech_level || 1;
    const techBonus = Math.min(techLevel * 0.01, 0.30); // 1% per level, max 30%
    
    // Get debris hazard level
    const debrisResult = await pool.query(
      'SELECT hazard_level FROM combat_debris WHERE id = $1',
      [debrisId]
    );
    
    const hazardLevel = debrisResult.rows[0]?.hazard_level || 0;
    const hazardPenalty = hazardLevel > 5 ? 0.2 : hazardLevel > 3 ? 0.1 : 0;
    
    // Check competition
    const competitionResult = await pool.query(
      `SELECT COUNT(*) as count FROM debris_salvage
       WHERE debris_id = $1 AND status IN ('en_route', 'salvaging')`,
      [debrisId]
    );
    
    const competitionCountRaw = competitionResult.rows[0]?.count || '0';
    const competitionCount = parseInt(competitionCountRaw);
    const competitionPenalty = competitionCount > 3 ? 0.15 : competitionCount > 1 ? 0.05 : 0;
    
    // Mission type modifiers
    const typeModifiers: Record<string, number> = {
      [SalvageTypeValues.AUTOMATED]: 0.8,
      [SalvageTypeValues.MANUAL]: 1.0,
      [SalvageTypeValues.ALLIANCE]: 1.1,
      [SalvageTypeValues.COMMERCIAL]: 0.9,
      [SalvageTypeValues.DEEP_SPACE]: 0.85,
      [SalvageTypeValues.EMERGENCY]: 0.75
    };
    
    const weatherFactor = typeModifiers[salvageType] || 1.0;
    
    // Calculate final efficiency
    const finalEfficiency = Math.max(0.3, Math.min(1.5,
      (baseEfficiency + techBonus - hazardPenalty - competitionPenalty) * weatherFactor
    ));
    
    return {
      base_efficiency: baseEfficiency,
      tech_bonus: techBonus,
      hazard_penalty: hazardPenalty,
      competition_penalty: competitionPenalty,
      weather_factor: weatherFactor,
      final_efficiency: finalEfficiency
    };
  }
  
  /**
   * Calculate salvage operation duration
   */
  private calculateSalvageDuration(salvageType: SalvageType, cargoCapacity: number): number {
    const baseDuration = 1800; // 30 minutes base
    
    const typeMultipliers: Record<string, number> = {
      [SalvageTypeValues.AUTOMATED]: 0.7,
      [SalvageTypeValues.MANUAL]: 1.2,
      [SalvageTypeValues.ALLIANCE]: 0.8,
      [SalvageTypeValues.COMMERCIAL]: 1.0,
      [SalvageTypeValues.DEEP_SPACE]: 1.5,
      [SalvageTypeValues.EMERGENCY]: 0.5
    };
    
    const typeMultiplier = typeMultipliers[salvageType] || 1.0;
    const cargoFactor = Math.log10(cargoCapacity / 1000 + 1);
    
    return Math.floor(baseDuration * typeMultiplier * cargoFactor);
  }
  
  // =====================================================
  // COMPONENT COLLECTION
  // =====================================================
  
  /**
   * Collect components from debris field
   */
  private async collectComponents(
    client: any,
    debrisId: number,
    userId: number,
    salvageType: SalvageType
  ): Promise<ComponentCollection> {
    const componentsCollected: ComponentCollection = {};
    
    // Get available components
    const componentsResult = await client.query(
      `SELECT * FROM ship_components
       WHERE id IN (
         SELECT component_id FROM debris_resources
         WHERE debris_id = $1 AND is_collected = FALSE
       )
       LIMIT 10`,
      [debrisId]
    );
    
    // Collection chance based on salvage type
    const collectionChances: Record<string, number> = {
      [SalvageTypeValues.AUTOMATED]: 0.3,
      [SalvageTypeValues.MANUAL]: 0.6,
      [SalvageTypeValues.ALLIANCE]: 0.5,
      [SalvageTypeValues.COMMERCIAL]: 0.4,
      [SalvageTypeValues.DEEP_SPACE]: 0.7,
      [SalvageTypeValues.EMERGENCY]: 0.2
    };
    
    const collectionChance = collectionChances[salvageType] || 0.5;
    
    for (const component of componentsResult.rows) {
      if (Math.random() < collectionChance) {
        // Add to user's inventory
        await client.query(
          `INSERT INTO player_component_inventory (user_id, component_id, quantity, acquired_from, acquired_at)
           VALUES ($1, $2, 1, 'salvage', NOW())
           ON CONFLICT (user_id, component_id) DO UPDATE
           SET quantity = player_component_inventory.quantity + 1`,
          [userId, component.id]
        );
        
        // Add to collection
        if (!componentsCollected[component.id]) {
          componentsCollected[component.id] = {
            quantity: 0,
            quality: component.quality_grade
          };
        }
        componentsCollected[component.id].quantity += 1;
      }
    }
    
    return componentsCollected;
  }
  
  // =====================================================
  // USER STATISTICS
  // =====================================================
  
  /**
   * Update user salvage statistics
   */
  private async updateUserSalvageStats(
    client: any,
    userId: number,
    resources: SalvageResources,
    componentsFound: number,
    experience: number
  ): Promise<void> {
    const totalValue = resources.metal + resources.crystal + resources.deuterium + (resources.rare_materials || 0);
    
    await client.query(
      `INSERT INTO salvage_statistics (
        user_id, total_salvage_missions, successful_missions,
        total_metal_collected, total_crystal_collected, total_deuterium_collected,
        total_components_found, total_salvage_value, salvage_experience_points
      ) VALUES ($1, 1, 1, $2, $3, $4, $5, $6, $7)
      ON CONFLICT (user_id) DO UPDATE SET
        total_salvage_missions = salvage_statistics.total_salvage_missions + 1,
        successful_missions = salvage_statistics.successful_missions + 1,
        total_metal_collected = salvage_statistics.total_metal_collected + $2,
        total_crystal_collected = salvage_statistics.total_crystal_collected + $3,
        total_deuterium_collected = salvage_statistics.total_deuterium_collected + $4,
        total_components_found = salvage_statistics.total_components_found + $5,
        total_salvage_value = salvage_statistics.total_salvage_value + $6,
        salvage_experience_points = salvage_statistics.salvage_experience_points + $7,
        last_salvage_at = NOW(),
        updated_at = NOW()`,
      [userId, resources.metal, resources.crystal, resources.deuterium, componentsFound, totalValue, experience]
    );
  }
  
  /**
   * Get user salvage profile
   */
  async getUserSalvageProfile(userId: number): Promise<UserSalvageProfile | null> {
    const userResult = await pool.query(
      'SELECT * FROM users WHERE id = $1',
      [userId]
    );
    
    if (userResult.rows.length === 0) return null;
    
    const statsResult = await pool.query(
      'SELECT * FROM salvage_statistics WHERE user_id = $1',
      [userId]
    );
    
    const operationsResult = await pool.query(
      `SELECT * FROM debris_salvage
       WHERE user_id = $1
       ORDER BY start_time DESC
       LIMIT 10`,
      [userId]
    );
    
    const inventoryResult = await pool.query(
      `SELECT * FROM player_component_inventory
       WHERE user_id = $1`,
      [userId]
    );
    
    const claimsResult = await pool.query(
      `SELECT * FROM debris_claims
       WHERE user_id = $1 AND is_active = TRUE`,
      [userId]
    );
    
    const rankResult = await pool.query(
      `SELECT COUNT(*) + 1 as rank
       FROM salvage_statistics
       WHERE total_salvage_value > (
         SELECT total_salvage_value FROM salvage_statistics WHERE user_id = $1
       )`,
      [userId]
    );
    
    const stats = statsResult.rows[0] || this.getDefaultStats(userId);
    const currentLevel = this.calculateLevel(stats.salvage_experience_points);
    
    return {
      user_id: userId,
      username: userResult.rows[0].username,
      stats: this.mapSalvageStats(stats),
      recent_operations: operationsResult.rows.map(row => this.mapSalvageOperation(row)),
      component_inventory: inventoryResult.rows.map(row => this.mapComponentInventory(row)),
      active_claims: claimsResult.rows.map(row => this.mapDebrisClaim(row)),
      rank: rankResult.rows[0] ? parseInt(rankResult.rows[0].rank) : 0,
      next_level_experience: this.getNextLevelExperience(currentLevel)
    };
  }
  
  /**
   * Get salvage leaderboard
   */
  async getSalvageLeaderboard(limit: number = 100): Promise<any[]> {
    const result = await pool.query(
      `SELECT * FROM v_top_salvagers LIMIT $1`,
      [limit]
    );
    
    return result.rows;
  }
  
  // =====================================================
  // QUERY METHODS
  // =====================================================
  
  /**
   * Get user's active salvage operations
   */
  async getUserActiveSalvageOperations(userId: number): Promise<DebrisSalvageOperation[]> {
    const result = await pool.query(
      `SELECT * FROM debris_salvage
       WHERE user_id = $1 AND status IN ('en_route', 'salvaging', 'returning')
       ORDER BY start_time DESC`,
      [userId]
    );
    
    return result.rows.map(row => this.mapSalvageOperation(row));
  }
  
  /**
   * Get salvage operation by ID
   */
  async getSalvageOperationById(operationId: number): Promise<DebrisSalvageOperation | null> {
    const result = await pool.query(
      'SELECT * FROM debris_salvage WHERE id = $1',
      [operationId]
    );
    
    return result.rows.length > 0 ? this.mapSalvageOperation(result.rows[0]) : null;
  }
  
  // =====================================================
  // UTILITY METHODS
  // =====================================================
  
  private calculateLevel(experience: number): number {
    return Math.floor(Math.sqrt(experience / 100)) + 1;
  }
  
  private getNextLevelExperience(currentLevel: number): number {
    return (currentLevel * currentLevel * 100) - ((currentLevel - 1) * (currentLevel - 1) * 100);
  }
  
  private getDefaultStats(userId: number): any {
    return {
      user_id: userId,
      total_salvage_missions: 0,
      successful_missions: 0,
      failed_missions: 0,
      total_metal_collected: 0,
      total_crystal_collected: 0,
      total_deuterium_collected: 0,
      total_components_found: 0,
      total_salvage_value: 0,
      salvage_experience_points: 0,
      salvage_level: 1
    };
  }
  
  private mapSalvageOperation(row: any): DebrisSalvageOperation {
    return {
      id: row.id,
      user_id: row.user_id,
      debris_id: row.debris_id,
      salvage_type: row.salvage_type,
      fleet_id: row.fleet_id,
      ship_types: row.ship_types,
      cargo_capacity: parseInt(row.cargo_capacity),
      salvage_efficiency: parseFloat(row.salvage_efficiency),
      status: row.status,
      start_time: row.start_time,
      arrival_time: row.arrival_time,
      completion_time: row.completion_time,
      return_time: row.return_time,
      resources_collected: row.resources_collected,
      components_collected: row.components_collected,
      total_value: parseInt(row.total_value),
      experience_gained: parseInt(row.experience_gained),
      success_rate: parseFloat(row.success_rate),
      hazards_encountered: row.hazards_encountered,
      alliance_id: row.alliance_id,
      is_competitive: row.is_competitive,
      ranking: row.ranking,
      notes: row.notes
    };
  }
  
  private mapSalvageStats(row: any): SalvageStatistics {
    return {
      id: row.id,
      user_id: row.user_id,
      total_salvage_missions: parseInt(row.total_salvage_missions),
      successful_missions: parseInt(row.successful_missions),
      failed_missions: parseInt(row.failed_missions),
      total_metal_collected: parseInt(row.total_metal_collected),
      total_crystal_collected: parseInt(row.total_crystal_collected),
      total_deuterium_collected: parseInt(row.total_deuterium_collected),
      total_rare_materials: parseInt(row.total_rare_materials) || 0,
      total_components_found: parseInt(row.total_components_found),
      legendary_components: parseInt(row.legendary_components),
      total_salvage_value: parseInt(row.total_salvage_value),
      fastest_salvage_time: row.fastest_salvage_time,
      largest_single_haul: parseInt(row.largest_single_haul),
      salvage_efficiency_avg: parseFloat(row.salvage_efficiency_avg),
      competitive_wins: parseInt(row.competitive_wins),
      alliance_contributions: parseInt(row.alliance_contributions),
      salvage_experience_points: parseInt(row.salvage_experience_points),
      salvage_level: parseInt(row.salvage_level),
      salvage_rank: row.salvage_rank,
      last_salvage_at: row.last_salvage_at,
      updated_at: row.updated_at
    };
  }
  
  private mapComponentInventory(row: any): any {
    return {
      id: row.id,
      user_id: row.user_id,
      component_id: row.component_id,
      quantity: parseInt(row.quantity),
      acquired_from: row.acquired_from,
      acquired_at: row.acquired_at,
      is_equipped: row.is_equipped,
      equipped_to_ship: row.equipped_to_ship
    };
  }
  
  private mapDebrisClaim(row: any): any {
    return {
      id: row.id,
      debris_id: row.debris_id,
      user_id: row.user_id,
      alliance_id: row.alliance_id,
      claim_type: row.claim_type,
      claim_start: row.claim_start,
      claim_duration: row.claim_duration,
      claim_expires: row.claim_expires,
      is_active: row.is_active,
      priority_level: row.priority_level,
      claim_reason: row.claim_reason
    };
  }
}

export default new SalvageService();
