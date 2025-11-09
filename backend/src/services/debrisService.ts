/**
 * @module backend/services/debrisService
 *
 * Debris management for combat outcomes: generation of debris fields,
 * scheduled decay/cleanup, salvage calculations and event logging.
 * The service exposes generators and helpers used by combat resolution
 * routines and background cleanup jobs.
 */

import pool from '../config/database';
import { 
  CombatDebris, 
  DebrisType,
  DebrisTypeValues,
  CreateDebrisRequest, 
  DebrisGenerationResult,
  SalvageResources,
  DebrisGenerationConfig,
  DebrisFieldInfo,
  DebrisSystemStats
} from '../types/debris';

export class DebrisService {
  
  // =====================================================
  // DEBRIS GENERATION
  // =====================================================
  
  /**
   * Generate debris field from combat
   */
  async generateDebrisFromCombat(request: CreateDebrisRequest): Promise<DebrisGenerationResult> {
    const client = await pool.connect();
    
    try {
      await client.query('BEGIN');
      
      const { 
        galaxy, 
        system, 
        position, 
        destroyed_ships: destroyedShips, 
        total_value: totalValue, 
        debris_rate: debrisRate = 0.3,
        combat_id: combatId,
        attacker_id: attackerId,
        defender_id: defenderId
      } = request;
      
      // Calculate debris amounts
      const metalAmount = Math.floor(totalValue * debrisRate * 0.5);
      const crystalAmount = Math.floor(totalValue * debrisRate * 0.3);
      const deuteriumAmount = Math.floor(totalValue * debrisRate * 0.2);
      
      // Determine debris type based on value
      const debrisType = this.determineDebrisType(totalValue);
      
      // Calculate hazard level and other properties
      const hazardLevel = this.calculateHazardLevel(totalValue);
      const spreadRadius = this.calculateSpreadRadius(totalValue);
      const decayRate = 0.05; // 5% decay per hour
      const lifetimeHours = 72; // 3 days default lifetime
      
      // Create debris field
      const debrisResult = await client.query(
        `INSERT INTO combat_debris (
          galaxy, system, position,
          debris_type,
          total_metal, total_crystal, total_deuterium,
          created_by_combat_id,
          decay_rate,
          expires_at,
          hazard_level,
          spread_radius,
          metadata
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW() + INTERVAL '1 hour' * $10, $11, $12, $13)
        RETURNING *`,
        [
          galaxy, system, position,
          debrisType,
          metalAmount, crystalAmount, deuteriumAmount,
          combatId,
          decayRate,
          lifetimeHours,
          hazardLevel,
          spreadRadius,
          JSON.stringify({ destroyedShips, attackerId, defenderId })
        ]
      );
      
      if (debrisResult.rows.length === 0) {
        throw new Error('Failed to create debris field');
      }
      const debris: CombatDebris = this.mapDebrisRow(debrisResult.rows[0]);
      
      // Generate ship components (10% chance per destroyed ship type)
      let componentsGenerated = 0;
      if (destroyedShips && typeof destroyedShips === 'object') {
        for (const [shipType, count] of Object.entries(destroyedShips)) {
          const shipCount = typeof count === 'number' ? count : parseInt(count as string) || 0;
          for (let i = 0; i < shipCount; i++) {
            if (Math.random() < 0.1) { // 10% chance per ship
              await this.generateComponent(client, debris.id, shipType, 1);
              componentsGenerated++;
            }
          }
        }
      }
      
      // Create debris event record
      await client.query(
        `INSERT INTO debris_events (
          event_type, debris_id, galaxy, system, position,
          attacker_id, defender_id,
          ships_destroyed,
          total_destroyed_value,
          debris_generated_metal,
          debris_generated_crystal,
          debris_generated_deuterium,
          debris_generation_rate,
          rare_components_generated
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)`,
        [
          'combat',
          debris.id,
          galaxy, system, position,
          attackerId,
          defenderId,
          JSON.stringify(destroyedShips),
          totalValue,
          metalAmount,
          crystalAmount,
          deuteriumAmount,
          debrisRate,
          componentsGenerated
        ]
      );
      
      await client.query('COMMIT');
      
      return {
        debris_id: debris.id,
        metal_amount: metalAmount,
        crystal_amount: crystalAmount,
        deuterium_amount: deuteriumAmount,
        debris_type: debrisType,
        total_value: totalValue
      };
      
    } catch (error) {
      await client.query('ROLLBACK');
      console.error('Error generating debris:', error);
      return {
        debris_id: 0,
        metal_amount: 0,
        crystal_amount: 0,
        deuterium_amount: 0,
        debris_type: 'light' as DebrisType,
        total_value: 0,
        error: 'Failed to generate debris field'
      };
    } finally {
      client.release();
    }
  }
  
  /**
   * Generate individual component in debris field
   */
  private async generateComponent(
    client: any, 
    debrisId: number, 
    shipType: string, 
    techLevel: number
  ): Promise<void> {
    const rarityRoll = Math.random() * 100;
    let rarity = 'common';
    
    if (rarityRoll < 1) rarity = 'legendary';
    else if (rarityRoll < 6) rarity = 'rare';
    else if (rarityRoll < 21) rarity = 'uncommon';
    
    const componentTypes = ['engine', 'weapon', 'armor', 'electronics', 'advanced_material', 'research_data'];
    const componentType = componentTypes[Math.floor(Math.random() * componentTypes.length)];
    
    const baseValue = this.getComponentBaseValue(rarity);
    
    await client.query(
      `INSERT INTO ship_components (
        component_type, component_name, quality_grade,
        source_ship_type, tech_level,
        recycle_value_metal, recycle_value_crystal, recycle_value_deuterium,
        market_value, description
      ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)`,
      [
        componentType,
        `${rarity.charAt(0).toUpperCase() + rarity.slice(1)} ${componentType} from ${shipType}`,
        rarity,
        shipType,
        techLevel,
        baseValue,
        Math.floor(baseValue * 0.7),
        Math.floor(baseValue * 0.3),
        baseValue * 2,
        `Salvaged component from destroyed ${shipType}`
      ]
    );
  }
  
  // =====================================================
  // DEBRIS QUERIES
  // =====================================================
  
  /**
   * Get debris field by ID
   */
  async getDebrisById(debrisId: number): Promise<CombatDebris | null> {
    const result = await pool.query(
      'SELECT * FROM combat_debris WHERE id = $1',
      [debrisId]
    );
    
    return result.rows.length > 0 ? this.mapDebrisRow(result.rows[0]) : null;
  }
  
  /**
   * Get all active debris fields
   */
  async getActiveDebrisFields(limit: number = 100): Promise<DebrisFieldInfo[]> {
    const result = await pool.query(
      `SELECT * FROM v_active_debris_fields
       ORDER BY total_value DESC
       LIMIT $1`,
      [limit]
    );
    
    return result.rows.map(row => this.mapDebrisFieldInfo(row));
  }
  
  /**
   * Get debris fields in specific location
   */
  async getDebrisAtLocation(galaxy: number, system: number, position: number): Promise<CombatDebris[]> {
    const result = await pool.query(
      `SELECT * FROM combat_debris
       WHERE galaxy = $1 AND system = $2 AND position = $3
       AND is_active = TRUE
       AND expires_at > NOW()
       ORDER BY created_at DESC`,
      [galaxy, system, position]
    );
    
    return result.rows.map(row => this.mapDebrisRow(row));
  }
  
  /**
   * Search debris fields with filters
   */
  async searchDebrisFields(filters: any): Promise<DebrisFieldInfo[]> {
    let query = 'SELECT * FROM v_active_debris_fields WHERE 1=1';
    const params: any[] = [];
    let paramIndex = 1;
    
    if (filters.galaxy) {
      query += ` AND galaxy = $${paramIndex++}`;
      params.push(filters.galaxy);
    }
    
    if (filters.system) {
      query += ` AND system = $${paramIndex++}`;
      params.push(filters.system);
    }
    
    if (filters.minValue) {
      query += ` AND total_value >= $${paramIndex++}`;
      params.push(filters.minValue);
    }
    
    if (filters.onlyUnclaimed) {
      query += ' AND is_claimed = FALSE';
    }
    
    query += ' ORDER BY total_value DESC LIMIT 100';
    
    const result = await pool.query(query, params);
    return result.rows.map(row => this.mapDebrisFieldInfo(row));
  }
  
  // =====================================================
  // DEBRIS DECAY & CLEANUP
  // =====================================================
  
  /**
   * Apply decay to all active debris fields
   */
  async applyDebrisDecay(): Promise<number> {
    const result = await pool.query(
      `UPDATE combat_debris
       SET 
         total_metal = GREATEST(0, FLOOR(total_metal * (1 - decay_rate))),
         total_crystal = GREATEST(0, FLOOR(total_crystal * (1 - decay_rate))),
         total_deuterium = GREATEST(0, FLOOR(total_deuterium * (1 - decay_rate)))
       WHERE is_active = TRUE
         AND decay_start < NOW() - INTERVAL '1 hour'
       RETURNING id`
    );
    
    return (result.rowCount ?? 0);
  }
  
  /**
   * Clean up expired or empty debris fields
   */
  async cleanupExpiredDebris(): Promise<number> {
    const client = await pool.connect();
    
    try {
      await client.query('BEGIN');
      
      // Get expired debris fields
      const expiredResult = await client.query(
        `SELECT id FROM combat_debris
         WHERE is_active = TRUE
         AND (expires_at < NOW() OR (total_metal + total_crystal + total_deuterium) < 100)`
      );
      
      const expiredIds = expiredResult.rows.map(row => row.id);
      
      if (expiredIds.length > 0) {
        // Mark as inactive
        await client.query(
          `UPDATE combat_debris
           SET is_active = FALSE
           WHERE id = ANY($1)`,
          [expiredIds]
        );
        
        // Log cleanup
        await client.query(
          `INSERT INTO debris_cleanup (debris_id, cleanup_type, scheduled_at, executed_at, status, cleanup_reason)
           SELECT id, 'automatic', NOW(), NOW(), 'completed', 'Expired or empty'
           FROM combat_debris
           WHERE id = ANY($1)`,
          [expiredIds]
        );
      }
      
      await client.query('COMMIT');
      return expiredIds.length;
      
    } catch (error) {
      await client.query('ROLLBACK');
      console.error('Error cleaning up debris:', error);
      return 0;
    } finally {
      client.release();
    }
  }
  
  /**
   * Start automatic debris cleanup scheduler
   */
  startAutomaticCleanup(intervalMinutes: number = 60): void {
    setInterval(async () => {
      console.log('[Debris Cleanup] Running automatic cleanup...');
      
      // Apply decay
      const decayedCount = await this.applyDebrisDecay();
      console.log(`[Debris Cleanup] Decayed ${decayedCount} debris fields`);
      
      // Cleanup expired
      const cleanedCount = await this.cleanupExpiredDebris();
      console.log(`[Debris Cleanup] Cleaned up ${cleanedCount} expired debris fields`);
      
    }, intervalMinutes * 60 * 1000);
    
    console.log(`[Debris Cleanup] Auto-cleanup started (every ${intervalMinutes} minutes)`);
  }
  
  // =====================================================
  // STATISTICS
  // =====================================================
  
  /**
   * Get debris system statistics
   */
  async getDebrisSystemStats(): Promise<DebrisSystemStats> {
    const result = await pool.query(
      `SELECT 
         COUNT(*) as total_debris_fields,
         SUM(CASE WHEN is_active = TRUE THEN 1 ELSE 0 END) as active_fields,
         SUM(CASE WHEN is_active = FALSE THEN 1 ELSE 0 END) as expired_fields,
         SUM(CASE WHEN is_active = TRUE THEN total_metal + total_crystal + total_deuterium ELSE 0 END) as total_value_available,
         AVG(CASE WHEN is_active = TRUE THEN total_metal + total_crystal + total_deuterium ELSE NULL END) as avg_field_value
       FROM combat_debris`
    );
    
    const salvageResult = await pool.query(
      `SELECT 
         COUNT(*) as total_salvage_operations,
         SUM(CASE WHEN status IN ('en_route', 'salvaging', 'returning') THEN 1 ELSE 0 END) as active_salvage_ops
       FROM debris_salvage`
    );
    
    const componentResult = await pool.query(
      `SELECT 
         COUNT(*) as total_components_generated,
         SUM(CASE WHEN quality_grade = 'legendary' THEN 1 ELSE 0 END) as legendary_components_found
       FROM ship_components`
    );
    
    const debrisStats = result.rows[0] || {};
    const salvageStats = salvageResult.rows[0] || {};
    const componentStats = componentResult.rows[0] || {};

    return {
      totalDebrisFields: parseInt(debrisStats.total_debris_fields) || 0,
      activeFields: parseInt(debrisStats.active_fields) || 0,
      expiredFields: parseInt(debrisStats.expired_fields) || 0,
      totalValueAvailable: parseInt(debrisStats.total_value_available) || 0,
      avgFieldValue: parseFloat(debrisStats.avg_field_value) || 0,
      totalSalvageOperations: parseInt(salvageStats.total_salvage_operations) || 0,
      activeSalvageOps: parseInt(salvageStats.active_salvage_ops) || 0,
      totalComponentsGenerated: parseInt(componentStats.total_components_generated) || 0,
      legendaryComponentsFound: parseInt(componentStats.legendary_components_found) || 0
    };
  }
  
  // =====================================================
  // UTILITY METHODS
  // =====================================================
  
  private determineDebrisType(totalValue: number): DebrisType {
    if (totalValue > 10000000) return DebrisTypeValues.WRECKAGE;
    if (totalValue > 1000000) return DebrisTypeValues.HEAVY;
    if (totalValue > 100000) return DebrisTypeValues.COMPONENTS;
    return DebrisTypeValues.LIGHT;
  }
  
  private calculateHazardLevel(totalValue: number): number {
    if (totalValue > 5000000) return 8;
    if (totalValue > 1000000) return 5;
    if (totalValue > 100000) return 3;
    return 1;
  }
  
  private calculateSpreadRadius(totalValue: number): number {
    if (totalValue > 10000000) return 500;
    if (totalValue > 1000000) return 200;
    return 100;
  }
  
  private getComponentBaseValue(rarity: string): number {
    const values: Record<string, number> = {
      'common': 1000,
      'uncommon': 5000,
      'rare': 20000,
      'legendary': 100000
    };
    return values[rarity] || 1000;
  }
  
  private mapDebrisRow(row: any): CombatDebris {
    return {
      id: row.id,
      galaxy: row.galaxy,
      system: row.system,
      position: row.position,
      debris_type: row.debris_type,
      total_metal: parseInt(row.total_metal),
      total_crystal: parseInt(row.total_crystal),
      total_deuterium: parseInt(row.total_deuterium),
      total_rare_materials: parseInt(row.total_rare_materials) || 0,
      created_at: row.created_at,
      created_by_combat_id: row.created_by_combat_id,
      decay_start: row.decay_start,
      decay_rate: parseFloat(row.decay_rate),
      expires_at: row.expires_at,
      is_active: row.is_active,
      is_claimed: row.is_claimed,
      claimed_by: row.claimed_by,
      claimed_at: row.claimed_at,
      hazard_level: row.hazard_level,
      radiation_level: row.radiation_level,
      spread_radius: row.spread_radius,
      metadata: row.metadata
    };
  }
  
  private mapDebrisFieldInfo(row: any): DebrisFieldInfo {
    const baseDebris = this.mapDebrisRow(row);
    return {
      ...baseDebris,
      debris: baseDebris,
      resource_count: parseInt(row.resource_count) || 0,
      total_resources: parseInt(row.total_resources) || 0,
      claimed_by_user: row.claimed_by_user,
      claimant_username: row.claimant_username,
      total_value: parseInt(row.total_value),
      hours_remaining: parseFloat(row.hours_remaining),
      nearby_salvagers: row.nearby_salvagers || 0
    };
  }
}

export default new DebrisService();
