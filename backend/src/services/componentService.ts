// =====================================================
// COMPONENT SERVICE
// Handles ship component recycling, trading, and storage
// =====================================================

import pool from '../config/database';
import { 
  ShipComponent,
  PlayerComponentInventory,
  ComponentType,
  QualityGrade,
  RecycleComponentRequest,
  ComponentRecycleResult,
  SalvageResources,
  ComponentCollection,
  ComponentBonus
} from '../types/debris';

export class ComponentService {
  
  // =====================================================
  // COMPONENT QUERIES
  // =====================================================
  
  /**
   * Get component by ID
   */
  async getComponentById(componentId: number): Promise<ShipComponent | null> {
    const result = await pool.query(
      'SELECT * FROM ship_components WHERE id = $1',
      [componentId]
    );
    
    return result.rows.length > 0 ? this.mapComponent(result.rows[0]) : null;
  }
  
  /**
   * Get components by type
   */
  async getComponentsByType(componentType: ComponentType, limit: number = 100): Promise<ShipComponent[]> {
    const result = await pool.query(
      `SELECT * FROM ship_components
       WHERE component_type = $1
       ORDER BY market_value DESC
       LIMIT $2`,
      [componentType, limit]
    );
    
    return result.rows.map(row => this.mapComponent(row));
  }
  
  /**
   * Get components by rarity
   */
  async getComponentsByRarity(rarity: QualityGrade, limit: number = 100): Promise<ShipComponent[]> {
    const result = await pool.query(
      `SELECT * FROM ship_components
       WHERE quality_grade = $1
       ORDER BY market_value DESC
       LIMIT $2`,
      [rarity, limit]
    );
    
    return result.rows.map(row => this.mapComponent(row));
  }
  
  /**
   * Search components with filters
   */
  async searchComponents(filters: {
    type?: ComponentType;
    rarity?: QualityGrade;
    minValue?: number;
    maxValue?: number;
    tradeable?: boolean;
    sourceShip?: string;
    minTechLevel?: number;
  }): Promise<ShipComponent[]> {
    let query = 'SELECT * FROM ship_components WHERE 1=1';
    const params: any[] = [];
    let paramIndex = 1;
    
    if (filters.type) {
      query += ` AND component_type = $${paramIndex++}`;
      params.push(filters.type);
    }
    
    if (filters.rarity) {
      query += ` AND quality_grade = $${paramIndex++}`;
      params.push(filters.rarity);
    }
    
    if (filters.minValue) {
      query += ` AND market_value >= $${paramIndex++}`;
      params.push(filters.minValue);
    }
    
    if (filters.maxValue) {
      query += ` AND market_value <= $${paramIndex++}`;
      params.push(filters.maxValue);
    }
    
    if (filters.tradeable !== undefined) {
      query += ` AND is_tradeable = $${paramIndex++}`;
      params.push(filters.tradeable);
    }
    
    if (filters.sourceShip) {
      query += ` AND source_ship_type = $${paramIndex++}`;
      params.push(filters.sourceShip);
    }
    
    if (filters.minTechLevel) {
      query += ` AND tech_level >= $${paramIndex++}`;
      params.push(filters.minTechLevel);
    }
    
    query += ' ORDER BY market_value DESC LIMIT 100';
    
    const result = await pool.query(query, params);
    return result.rows.map(row => this.mapComponent(row));
  }
  
  // =====================================================
  // PLAYER INVENTORY
  // =====================================================
  
  /**
   * Get player's component inventory
   */
  async getPlayerInventory(userId: number): Promise<PlayerComponentInventory[]> {
    const result = await pool.query(
      `SELECT pci.*, sc.component_name, sc.quality_grade, sc.market_value, sc.component_type
       FROM player_component_inventory pci
       JOIN ship_components sc ON pci.component_id = sc.id
       WHERE pci.user_id = $1
       ORDER BY sc.quality_grade DESC, sc.market_value DESC`,
      [userId]
    );
    
    return result.rows.map(row => this.mapPlayerInventory(row));
  }
  
  /**
   * Get player's equipped components
   */
  async getPlayerEquippedComponents(userId: number): Promise<PlayerComponentInventory[]> {
    const result = await pool.query(
      `SELECT pci.*, sc.*
       FROM player_component_inventory pci
       JOIN ship_components sc ON pci.component_id = sc.id
       WHERE pci.user_id = $1 AND pci.is_equipped = TRUE
       ORDER BY sc.component_type`,
      [userId]
    );
    
    return result.rows.map(row => this.mapPlayerInventory(row));
  }
  
  /**
   * Add component to player inventory
   */
  async addComponentToInventory(
    userId: number, 
    componentId: number, 
    quantity: number = 1,
    source: string = 'salvage'
  ): Promise<boolean> {
    try {
      await pool.query(
        `INSERT INTO player_component_inventory (
          user_id, component_id, quantity, acquired_from, acquired_at
        ) VALUES ($1, $2, $3, $4, NOW())
        ON CONFLICT (user_id, component_id) DO UPDATE
        SET quantity = player_component_inventory.quantity + $3`,
        [userId, componentId, quantity, source]
      );
      
      return true;
    } catch (error) {
      console.error('Error adding component to inventory:', error);
      return false;
    }
  }
  
  /**
   * Remove component from player inventory
   */
  async removeComponentFromInventory(
    userId: number, 
    componentId: number, 
    quantity: number = 1
  ): Promise<boolean> {
    const client = await pool.connect();
    
    try {
      await client.query('BEGIN');
      
      // Get current quantity
      const result = await client.query(
        'SELECT quantity FROM player_component_inventory WHERE user_id = $1 AND component_id = $2',
        [userId, componentId]
      );
      
      if (result.rows.length === 0) {
        throw new Error('Component not in inventory');
      }
      
      const quantityValue = result.rows[0]?.quantity || '0';
      const currentQuantity = parseInt(quantityValue);
      
      if (currentQuantity < quantity) {
        throw new Error('Insufficient component quantity');
      }
      
      if (currentQuantity === quantity) {
        // Remove entry
        await client.query(
          'DELETE FROM player_component_inventory WHERE user_id = $1 AND component_id = $2',
          [userId, componentId]
        );
      } else {
        // Decrease quantity
        await client.query(
          'UPDATE player_component_inventory SET quantity = quantity - $1 WHERE user_id = $2 AND component_id = $3',
          [quantity, userId, componentId]
        );
      }
      
      await client.query('COMMIT');
      return true;
      
    } catch (error) {
      await client.query('ROLLBACK');
      console.error('Error removing component:', error);
      return false;
    } finally {
      client.release();
    }
  }
  
  // =====================================================
  // COMPONENT RECYCLING
  // =====================================================
  
  /**
   * Recycle component for resources
   */
  async recycleComponent(request: RecycleComponentRequest): Promise<ComponentRecycleResult> {
    const client = await pool.connect();
    
    try {
      await client.query('BEGIN');
      
      const { component_id: componentId, user_id: userId, recycle_all: recycleAll = false } = request;
      
      // Get component details
      const componentResult = await client.query(
        'SELECT * FROM ship_components WHERE id = $1',
        [componentId]
      );
      
      if (componentResult.rows.length === 0) {
        throw new Error('Component not found');
      }
      
      const component = componentResult.rows[0];
      if (!component) {
        throw new Error('Component data is invalid');
      }
      
      // Check if player has this component
      const inventoryResult = await client.query(
        'SELECT * FROM player_component_inventory WHERE user_id = $1 AND component_id = $2',
        [userId, componentId]
      );
      
      if (inventoryResult.rows.length === 0) {
        throw new Error('Component not in inventory');
      }
      
      const inventoryItem = inventoryResult.rows[0];
      if (!inventoryItem) {
        throw new Error('Inventory data is invalid');
      }
      const recycleQuantity = recycleAll ? inventoryItem.quantity : 1;
      
      // Calculate resources from recycling
      const efficiency = parseFloat(component.recycle_efficiency) || 0.8;
      
      const metalGained = Math.floor(component.recycle_value_metal * efficiency * recycleQuantity);
      const crystalGained = Math.floor(component.recycle_value_crystal * efficiency * recycleQuantity);
      const deuteriumGained = Math.floor(component.recycle_value_deuterium * efficiency * recycleQuantity);
      
      // Calculate experience (10% of total value)
      const totalValue = metalGained + crystalGained + deuteriumGained;
      const experienceGained = Math.floor(totalValue * 0.1);
      
      // Remove components from inventory
      await this.removeComponentFromInventory(userId, componentId, recycleQuantity);
      
      // Add resources to user
      await client.query(
        `UPDATE users
         SET metal = metal + $1,
             crystal = crystal + $2,
             deuterium = deuterium + $3,
             salvage_experience = salvage_experience + $4
         WHERE id = $5`,
        [metalGained, crystalGained, deuteriumGained, experienceGained, userId]
      );
      
      // Update salvage statistics
      await client.query(
        `UPDATE salvage_statistics
         SET components_recycled = components_recycled + $1,
             updated_at = NOW()
         WHERE user_id = $2`,
        [recycleQuantity, userId]
      );
      
      await client.query('COMMIT');
      
      return {
        resources_gained: {
          metal: metalGained,
          crystal: crystalGained,
          deuterium: deuteriumGained,
          rare_materials: 0
        },
        recycle_efficiency: efficiency,
        message: `Recycled ${recycleQuantity}x ${component.component_name} for ${totalValue} resources`
      };
      
    } catch (error) {
      await client.query('ROLLBACK');
      console.error('Error recycling component:', error);
      return {
        resources_gained: { metal: 0, crystal: 0, deuterium: 0, rare_materials: 0 },
        recycle_efficiency: 0,
        error: error instanceof Error ? error.message : 'Failed to recycle component'
      };
    } finally {
      client.release();
    }
  }
  
  /**
   * Bulk recycle components by rarity
   */
  async bulkRecycleByRarity(userId: number, rarity: QualityGrade): Promise<ComponentRecycleResult> {
    const client = await pool.connect();
    
    try {
      await client.query('BEGIN');
      
      // Get all components of this rarity in user's inventory
      const inventoryResult = await client.query(
        `SELECT pci.*, sc.*
         FROM player_component_inventory pci
         JOIN ship_components sc ON pci.component_id = sc.id
         WHERE pci.user_id = $1 AND sc.quality_grade = $2`,
        [userId, rarity]
      );
      
      if (inventoryResult.rows.length === 0) {
        throw new Error(`No ${rarity} components to recycle`);
      }
      
      let totalMetal = 0;
      let totalCrystal = 0;
      let totalDeuterium = 0;
      let totalExperience = 0;
      let totalRecycled = 0;
      
      for (const row of inventoryResult.rows) {
        const efficiency = parseFloat(row.recycle_efficiency) || 0.8;
        const quantity = parseInt(row.quantity);
        
        const metal = Math.floor(row.recycle_value_metal * efficiency * quantity);
        const crystal = Math.floor(row.recycle_value_crystal * efficiency * quantity);
        const deuterium = Math.floor(row.recycle_value_deuterium * efficiency * quantity);
        
        totalMetal += metal;
        totalCrystal += crystal;
        totalDeuterium += deuterium;
        totalRecycled += quantity;
        
        // Remove from inventory
        await client.query(
          'DELETE FROM player_component_inventory WHERE user_id = $1 AND component_id = $2',
          [userId, row.component_id]
        );
      }
      
      const totalValue = totalMetal + totalCrystal + totalDeuterium;
      totalExperience = Math.floor(totalValue * 0.1);
      
      // Add resources to user
      await client.query(
        `UPDATE users
         SET metal = metal + $1,
             crystal = crystal + $2,
             deuterium = deuterium + $3,
             salvage_experience = salvage_experience + $4
         WHERE id = $5`,
        [totalMetal, totalCrystal, totalDeuterium, totalExperience, userId]
      );
      
      await client.query('COMMIT');
      
      return {
        resources_gained: {
          metal: totalMetal,
          crystal: totalCrystal,
          deuterium: totalDeuterium,
          rare_materials: 0
        },
        recycle_efficiency: 0.8,
        message: `Recycled ${totalRecycled} ${rarity} components for ${totalValue} resources`
      };
      
    } catch (error) {
      await client.query('ROLLBACK');
      console.error('Error bulk recycling:', error);
      return {
        resources_gained: { metal: 0, crystal: 0, deuterium: 0, rare_materials: 0 },
        recycle_efficiency: 0,
        error: error instanceof Error ? error.message : 'Failed to bulk recycle components'
      };
    } finally {
      client.release();
    }
  }
  
  // =====================================================
  // COMPONENT EQUIPMENT
  // =====================================================
  
  /**
   * Equip component to ship
   */
  async equipComponent(
    userId: number, 
    componentId: number, 
    shipType: string
  ): Promise<boolean> {
    try {
      // Unequip any existing component of same type on this ship
      await pool.query(
        `UPDATE player_component_inventory pci
         SET is_equipped = FALSE, equipped_to_ship = NULL
         FROM ship_components sc
         WHERE pci.component_id = sc.id
         AND pci.user_id = $1
         AND pci.equipped_to_ship = $2
         AND sc.component_type = (
           SELECT component_type FROM ship_components WHERE id = $3
         )`,
        [userId, shipType, componentId]
      );
      
      // Equip new component
      const result = await pool.query(
        `UPDATE player_component_inventory
         SET is_equipped = TRUE, equipped_to_ship = $1
         WHERE user_id = $2 AND component_id = $3 AND quantity > 0
         RETURNING id`,
        [shipType, userId, componentId]
      );
      
      return (result.rowCount || 0) > 0;
      
    } catch (error) {
      console.error('Error equipping component:', error);
      return false;
    }
  }
  
  /**
   * Unequip component from ship
   */
  async unequipComponent(userId: number, componentId: number): Promise<boolean> {
    try {
      const result = await pool.query(
        `UPDATE player_component_inventory
         SET is_equipped = FALSE, equipped_to_ship = NULL
         WHERE user_id = $1 AND component_id = $2
         RETURNING id`,
        [userId, componentId]
      );
      
      return (result.rowCount || 0) > 0;
      
    } catch (error) {
      console.error('Error unequipping component:', error);
      return false;
    }
  }
  
  /**
   * Get ship bonuses from equipped components
   */
  async getShipBonuses(userId: number, shipType: string): Promise<ComponentBonus> {
    const result = await pool.query(
      `SELECT sc.bonus_stats
       FROM player_component_inventory pci
       JOIN ship_components sc ON pci.component_id = sc.id
       WHERE pci.user_id = $1 
       AND pci.is_equipped = TRUE 
       AND pci.equipped_to_ship = $2`,
      [userId, shipType]
    );
    
    const bonuses: ComponentBonus = {};
    
    for (const row of result.rows) {
      if (row.bonus_stats) {
        const stats = row.bonus_stats;
        
        if (stats.speed) bonuses.speed = (bonuses.speed || 0) + stats.speed;
        if (stats.attack) bonuses.attack = (bonuses.attack || 0) + stats.attack;
        if (stats.defense) bonuses.defense = (bonuses.defense || 0) + stats.defense;
        if (stats.cargo) bonuses.cargo = (bonuses.cargo || 0) + stats.cargo;
        if (stats.fuel) bonuses.fuel = (bonuses.fuel || 0) + stats.fuel;
        if (stats.research) bonuses.research = (bonuses.research || 0) + stats.research;
        if (stats.production) bonuses.production = (bonuses.production || 0) + stats.production;
      }
    }
    
    return bonuses;
  }
  
  // =====================================================
  // COMPONENT TRADING
  // =====================================================
  
  /**
   * Get component market value
   */
  async getComponentMarketValue(componentId: number): Promise<number> {
    const result = await pool.query(
      'SELECT market_value FROM ship_components WHERE id = $1 AND is_tradeable = TRUE',
      [componentId]
    );
    
    return result.rows.length > 0 ? parseInt(result.rows[0].market_value) : 0;
  }
  
  /**
   * Trade component (sell to NPC market)
   */
  async sellComponent(userId: number, componentId: number, quantity: number = 1): Promise<{
    success: boolean;
    creditsEarned: number;
    message: string;
  }> {
    const client = await pool.connect();
    
    try {
      await client.query('BEGIN');
      
      // Get component market value
      const componentResult = await client.query(
        'SELECT * FROM ship_components WHERE id = $1 AND is_tradeable = TRUE',
        [componentId]
      );
      
      if (componentResult.rows.length === 0) {
        throw new Error('Component not tradeable');
      }
      
      const component = componentResult.rows[0];
      if (!component) {
        throw new Error('Component data is invalid');
      }
      const marketValue = parseInt(component.market_value);
      const totalCredits = marketValue * quantity;
      
      // Remove from inventory
      const removed = await this.removeComponentFromInventory(userId, componentId, quantity);
      
      if (!removed) {
        throw new Error('Failed to remove component from inventory');
      }
      
      // Add credits to user (assuming credits are stored as crystal)
      await client.query(
        'UPDATE users SET crystal = crystal + $1 WHERE id = $2',
        [totalCredits, userId]
      );
      
      // Update statistics
      await client.query(
        `UPDATE salvage_statistics
         SET components_sold = components_sold + $1,
             total_market_value = total_market_value + $2,
             updated_at = NOW()
         WHERE user_id = $3`,
        [quantity, totalCredits, userId]
      );
      
      await client.query('COMMIT');
      
      return {
        success: true,
        creditsEarned: totalCredits,
        message: `Sold ${quantity}x ${component.component_name} for ${totalCredits} credits`
      };
      
    } catch (error) {
      await client.query('ROLLBACK');
      console.error('Error selling component:', error);
      return {
        success: false,
        creditsEarned: 0,
        message: error instanceof Error ? error.message : 'Failed to sell component'
      };
    } finally {
      client.release();
    }
  }
  
  // =====================================================
  // STATISTICS
  // =====================================================
  
  /**
   * Get component statistics
   */
  async getComponentStatistics(): Promise<{
    totalComponents: number;
    componentsByRarity: Record<string, number>;
    componentsByType: Record<string, number>;
    avgMarketValue: number;
    totalMarketValue: number;
  }> {
    const result = await pool.query(
      `SELECT 
         COUNT(*) as total_components,
         AVG(market_value) as avg_market_value,
         SUM(market_value) as total_market_value
       FROM ship_components`
    );
    
    const rarityResult = await pool.query(
      `SELECT quality_grade, COUNT(*) as count
       FROM ship_components
       GROUP BY quality_grade`
    );
    
    const typeResult = await pool.query(
      `SELECT component_type, COUNT(*) as count
       FROM ship_components
       GROUP BY component_type`
    );
    
    const componentsByRarity: Record<string, number> = {};
    for (const row of rarityResult.rows) {
      componentsByRarity[row.quality_grade] = parseInt(row.count);
    }
    
    const componentsByType: Record<string, number> = {};
    for (const row of typeResult.rows) {
      componentsByType[row.component_type] = parseInt(row.count);
    }
    
    const statsRow = result.rows[0] || {};
    return {
      totalComponents: parseInt(statsRow.total_components) || 0,
      componentsByRarity,
      componentsByType,
      avgMarketValue: parseFloat(statsRow.avg_market_value) || 0,
      totalMarketValue: parseInt(statsRow.total_market_value) || 0
    };
  }
  
  /**
   * Get player component value
   */
  async getPlayerComponentValue(userId: number): Promise<number> {
    const result = await pool.query(
      `SELECT SUM(sc.market_value * pci.quantity) as total_value
       FROM player_component_inventory pci
       JOIN ship_components sc ON pci.component_id = sc.id
       WHERE pci.user_id = $1`,
      [userId]
    );
    
    const totalValue = result.rows[0]?.total_value;
    return totalValue !== undefined ? parseInt(totalValue) : 0;
  }
  
  // =====================================================
  // UTILITY METHODS
  // =====================================================
  
  private mapComponent(row: any): ShipComponent {
    return {
      id: row.id,
      component_type: row.component_type,
      component_name: row.component_name,
      component_subtype: row.component_subtype,
      quality_grade: row.quality_grade,
      condition_percent: parseInt(row.condition_percent),
      source_ship_type: row.source_ship_type,
      tech_level: parseInt(row.tech_level),
      recycle_value_metal: parseInt(row.recycle_value_metal),
      recycle_value_crystal: parseInt(row.recycle_value_crystal),
      recycle_value_deuterium: parseInt(row.recycle_value_deuterium),
      recycle_efficiency: parseFloat(row.recycle_efficiency),
      market_value: parseInt(row.market_value),
      is_tradeable: row.is_tradeable,
      is_unique: row.is_unique,
      required_tech: row.required_tech,
      bonus_stats: row.bonus_stats,
      description: row.description,
      created_at: row.created_at
    };
  }
  
  private mapPlayerInventory(row: any): PlayerComponentInventory {
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
}

export default new ComponentService();
