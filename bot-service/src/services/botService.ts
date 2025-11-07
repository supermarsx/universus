import { pool } from '../config/database';
import { QueryResult } from 'pg';

/**
 * Bot Personality Presets
 * Each personality has unique behavior parameters that influence decision-making
 */
const BOT_PERSONALITIES = {
  aggressive_conqueror: {
    aggression_level: 90,
    expansion_priority: 80,
    military_focus: 95,
    economy_focus: 40,
    research_focus: 50,
    diplomacy_focus: 20,
    risk_tolerance: 85,
    preferred_ship_type: 'cruiser',
    attack_frequency_hours: 6,
    resource_threshold_attack: 50000,
    fleet_size_preference: 'large',
    alliance_behavior: 'aggressive',
    think_interval_minutes: 10
  },
  strategic_builder: {
    aggression_level: 30,
    expansion_priority: 70,
    military_focus: 50,
    economy_focus: 85,
    research_focus: 75,
    diplomacy_focus: 60,
    risk_tolerance: 40,
    preferred_ship_type: 'battleship',
    attack_frequency_hours: 48,
    resource_threshold_attack: 200000,
    fleet_size_preference: 'medium',
    alliance_behavior: 'defensive',
    think_interval_minutes: 20
  },
  diplomatic_negotiator: {
    aggression_level: 15,
    expansion_priority: 50,
    military_focus: 30,
    economy_focus: 70,
    research_focus: 65,
    diplomacy_focus: 95,
    risk_tolerance: 25,
    preferred_ship_type: 'colony_ship',
    attack_frequency_hours: 96,
    resource_threshold_attack: 300000,
    fleet_size_preference: 'small',
    alliance_behavior: 'cooperative',
    think_interval_minutes: 30
  },
  resource_hoarder: {
    aggression_level: 10,
    expansion_priority: 60,
    military_focus: 35,
    economy_focus: 95,
    research_focus: 55,
    diplomacy_focus: 40,
    risk_tolerance: 15,
    preferred_ship_type: 'recycler',
    attack_frequency_hours: 120,
    resource_threshold_attack: 500000,
    fleet_size_preference: 'small',
    alliance_behavior: 'neutral',
    think_interval_minutes: 25
  },
  speed_rusher: {
    aggression_level: 95,
    expansion_priority: 85,
    military_focus: 90,
    economy_focus: 60,
    research_focus: 70,
    diplomacy_focus: 25,
    risk_tolerance: 90,
    preferred_ship_type: 'light_fighter',
    attack_frequency_hours: 4,
    resource_threshold_attack: 30000,
    fleet_size_preference: 'medium',
    alliance_behavior: 'aggressive',
    think_interval_minutes: 8
  },
  tech_enthusiast: {
    aggression_level: 35,
    expansion_priority: 55,
    military_focus: 45,
    economy_focus: 75,
    research_focus: 95,
    diplomacy_focus: 55,
    risk_tolerance: 50,
    preferred_ship_type: 'bomber',
    attack_frequency_hours: 72,
    resource_threshold_attack: 150000,
    fleet_size_preference: 'medium',
    alliance_behavior: 'neutral',
    think_interval_minutes: 18
  },
  alliance_focused: {
    aggression_level: 50,
    expansion_priority: 65,
    military_focus: 65,
    economy_focus: 65,
    research_focus: 60,
    diplomacy_focus: 90,
    risk_tolerance: 55,
    preferred_ship_type: 'battlecruiser',
    attack_frequency_hours: 24,
    resource_threshold_attack: 100000,
    fleet_size_preference: 'medium',
    alliance_behavior: 'cooperative',
    think_interval_minutes: 15
  },
  solo_survivor: {
    aggression_level: 40,
    expansion_priority: 75,
    military_focus: 70,
    economy_focus: 80,
    research_focus: 70,
    diplomacy_focus: 30,
    risk_tolerance: 35,
    preferred_ship_type: 'destroyer',
    attack_frequency_hours: 36,
    resource_threshold_attack: 120000,
    fleet_size_preference: 'medium',
    alliance_behavior: 'neutral',
    think_interval_minutes: 22
  }
};

export interface BotProfile {
  id: number;
  user_id: number;
  personality_type: string;
  is_active: boolean;
  difficulty_level: number;
  aggression_level: number;
  expansion_priority: number;
  military_focus: number;
  economy_focus: number;
  research_focus: number;
  diplomacy_focus: number;
  risk_tolerance: number;
  preferred_ship_type: string;
  attack_frequency_hours: number;
  resource_threshold_attack: number;
  fleet_size_preference: string;
  alliance_behavior: string;
  total_attacks_launched: number;
  total_resources_plundered: number;
  total_ships_built: number;
  total_research_completed: number;
  win_rate: number;
  last_action_at: Date | null;
  next_think_at: Date | null;
  think_interval_minutes: number;
  current_strategy: any;
  created_at: Date;
  updated_at: Date;
}

export interface BotActionLog {
  id: number;
  bot_id: number;
  action_type: string;
  action_details: any;
  decision_factors: any;
  success: boolean;
  resources_spent: any;
  resources_gained: any;
  execution_time_ms: number;
  created_at: Date;
}

export interface BotTarget {
  id: number;
  bot_id: number;
  target_user_id: number;
  target_planet_id: number;
  threat_level: number;
  resource_potential: number;
  defense_strength: number;
  last_espionage_at: Date | null;
  espionage_data: any;
  attack_priority: number;
  last_attack_at: Date | null;
  total_attacks: number;
  successful_attacks: number;
  next_attack_available_at: Date | null;
  created_at: Date;
  updated_at: Date;
}

export class BotService {
  /**
   * Create a new bot with specified personality and difficulty
   */
  static async createBot(
    username: string,
    email: string,
    personality_type: keyof typeof BOT_PERSONALITIES,
    difficulty_level: number = 5
  ): Promise<BotProfile> {
    const client = await pool.connect();
    
    try {
      await client.query('BEGIN');
      
      // Create user account for bot
      const userResult = await client.query(
        `INSERT INTO users (username, email, password_hash, dark_matter)
         VALUES ($1, $2, $3, $4)
         RETURNING id`,
        [username, email, 'BOT_ACCOUNT_NO_PASSWORD_' + Date.now(), 1000]
      );
      
      const userId = userResult.rows[0].id;
      
      // Create home planet for bot
      const galaxyPos = Math.floor(Math.random() * 9) + 1;
      const systemPos = Math.floor(Math.random() * 499) + 1;
      const planetPos = Math.floor(Math.random() * 15) + 1;
      
      await client.query(
        `INSERT INTO planets (user_id, name, galaxy, system, position, metal, crystal, deuterium)
         VALUES ($1, $2, $3, $4, $5, 500, 300, 100)`,
        [userId, `${username}'s Planet`, galaxyPos, systemPos, planetPos]
      );
      
      // Get personality preset
      const preset = BOT_PERSONALITIES[personality_type];
      
      // Adjust parameters based on difficulty level
      const difficultyMultiplier = difficulty_level / 5;
      
      // Create bot profile
      const botResult = await client.query(
        `INSERT INTO bot_profiles (
          user_id, personality_type, difficulty_level,
          aggression_level, expansion_priority, military_focus, economy_focus,
          research_focus, diplomacy_focus, risk_tolerance,
          preferred_ship_type, attack_frequency_hours, resource_threshold_attack,
          fleet_size_preference, alliance_behavior, think_interval_minutes,
          next_think_at
        ) VALUES (
          $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16,
          NOW() + INTERVAL '5 minutes'
        ) RETURNING *`,
        [
          userId,
          personality_type,
          difficulty_level,
          Math.min(100, preset.aggression_level * difficultyMultiplier),
          Math.min(100, preset.expansion_priority * difficultyMultiplier),
          Math.min(100, preset.military_focus * difficultyMultiplier),
          Math.min(100, preset.economy_focus * difficultyMultiplier),
          Math.min(100, preset.research_focus * difficultyMultiplier),
          Math.min(100, preset.diplomacy_focus * difficultyMultiplier),
          Math.min(100, preset.risk_tolerance * difficultyMultiplier),
          preset.preferred_ship_type,
          preset.attack_frequency_hours / difficultyMultiplier,
          preset.resource_threshold_attack,
          preset.fleet_size_preference,
          preset.alliance_behavior,
          preset.think_interval_minutes
        ]
      );
      
      await client.query('COMMIT');
      
      return botResult.rows[0] as BotProfile;
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }
  }

  /**
   * Get all bots with optional filtering
   */
  static async getAllBots(filters?: {
    is_active?: boolean;
    personality_type?: string;
    min_difficulty?: number;
    max_difficulty?: number;
  }): Promise<BotProfile[]> {
    let query = `
      SELECT bp.*, u.username, u.email
      FROM bot_profiles bp
      JOIN users u ON bp.user_id = u.id
      WHERE 1=1
    `;
    
    const params: any[] = [];
    let paramIndex = 1;
    
    if (filters?.is_active !== undefined) {
      query += ` AND bp.is_active = $${paramIndex++}`;
      params.push(filters.is_active);
    }
    
    if (filters?.personality_type) {
      query += ` AND bp.personality_type = $${paramIndex++}`;
      params.push(filters.personality_type);
    }
    
    if (filters?.min_difficulty) {
      query += ` AND bp.difficulty_level >= $${paramIndex++}`;
      params.push(filters.min_difficulty);
    }
    
    if (filters?.max_difficulty) {
      query += ` AND bp.difficulty_level <= $${paramIndex++}`;
      params.push(filters.max_difficulty);
    }
    
    query += ' ORDER BY bp.created_at DESC';
    
    const result = await pool.query(query, params);
    return result.rows as BotProfile[];
  }

  /**
   * Get bot by ID
   */
  static async getBotById(botId: number): Promise<BotProfile | null> {
    const result = await pool.query(
      'SELECT * FROM bot_profiles WHERE id = $1',
      [botId]
    );
    
    if (!result.rows[0]) {
      return null;
    }
    return result.rows[0] as BotProfile;
  }

  /**
   * Update bot configuration
   */
  static async updateBot(botId: number, updates: Partial<BotProfile>): Promise<BotProfile> {
    const allowedFields = [
      'is_active', 'difficulty_level', 'aggression_level', 'expansion_priority',
      'military_focus', 'economy_focus', 'research_focus', 'diplomacy_focus',
      'risk_tolerance', 'attack_frequency_hours', 'resource_threshold_attack',
      'think_interval_minutes'
    ];
    
    const setClause: string[] = [];
    const values: any[] = [];
    let paramIndex = 1;
    
    for (const [key, value] of Object.entries(updates)) {
      if (allowedFields.includes(key)) {
        setClause.push(`${key} = $${paramIndex++}`);
        values.push(value);
      }
    }
    
    if (setClause.length === 0) {
      throw new Error('No valid fields to update');
    }
    
    values.push(botId);
    
    const result = await pool.query(
      `UPDATE bot_profiles SET ${setClause.join(', ')} WHERE id = $${paramIndex} RETURNING *`,
      values
    );
    
    if (!result.rows[0]) {
      throw new Error('Bot not found after update');
    }
    return result.rows[0] as BotProfile;
  }

  /**
   * Delete bot and associated user account
   */
  static async deleteBot(botId: number): Promise<void> {
    const client = await pool.connect();
    
    try {
      await client.query('BEGIN');
      
      // Get user_id
      const botResult = await client.query(
        'SELECT user_id FROM bot_profiles WHERE id = $1',
        [botId]
      );
      
      if (botResult.rows.length === 0) {
        throw new Error('Bot not found');
      }
      
      if (!botResult.rows[0]) {
        throw new Error('Bot not found');
      }
      const userId = botResult.rows[0].user_id;
      
      // Delete user (cascades to bot_profiles and other related data)
      await client.query('DELETE FROM users WHERE id = $1', [userId]);
      
      await client.query('COMMIT');
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }
  }

  /**
   * Log bot action
   */
  static async logAction(
    botId: number,
    actionType: string,
    actionDetails: any,
    decisionFactors: any = {},
    success: boolean = true,
    resourcesSpent: any = {},
    resourcesGained: any = {},
    executionTimeMs: number = 0
  ): Promise<BotActionLog> {
    const result = await pool.query(
      `INSERT INTO bot_actions_log (
        bot_id, action_type, action_details, decision_factors,
        success, resources_spent, resources_gained, execution_time_ms
      ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING *`,
      [
        botId,
        actionType,
        JSON.stringify(actionDetails),
        JSON.stringify(decisionFactors),
        success,
        JSON.stringify(resourcesSpent),
        JSON.stringify(resourcesGained),
        executionTimeMs
      ]
    );
    
    if (!result.rows[0]) {
      throw new Error('Failed to log bot action');
    }
    return result.rows[0] as BotActionLog;
  }

  /**
   * Get bot action history
   */
  static async getActionHistory(
    botId: number,
    limit: number = 100,
    actionType?: string
  ): Promise<BotActionLog[]> {
    let query = 'SELECT * FROM bot_actions_log WHERE bot_id = $1';
    const params: any[] = [botId];
    
    if (actionType) {
      query += ' AND action_type = $2';
      params.push(actionType);
    }
    
    query += ' ORDER BY created_at DESC LIMIT $' + (params.length + 1);
    params.push(limit);
    
    const result = await pool.query(query, params);
    return result.rows as BotActionLog[];
  }

  /**
   * Get bot statistics for a date range
   */
  static async getStatistics(
    botId: number,
    startDate?: Date,
    endDate?: Date
  ): Promise<any> {
    const params: any[] = [botId];
    let query = 'SELECT * FROM bot_stats WHERE bot_id = $1';
    
    if (startDate) {
      params.push(startDate);
      query += ` AND stat_date >= $${params.length}`;
    }
    
    if (endDate) {
      params.push(endDate);
      query += ` AND stat_date <= $${params.length}`;
    }
    
    query += ' ORDER BY stat_date DESC';
    
    const result = await pool.query(query, params);
    return result.rows;
  }

  /**
   * Get bots that need to think (make decisions)
   */
  static async getBotsNeedingThink(): Promise<BotProfile[]> {
    const result = await pool.query(
      `SELECT * FROM bot_profiles 
       WHERE is_active = true 
       AND (next_think_at IS NULL OR next_think_at <= NOW())
       ORDER BY next_think_at ASC NULLS FIRST
       LIMIT 50`
    );
    
    return result.rows as BotProfile[];
  }

  /**
   * Update bot's next think time
   */
  static async updateNextThinkTime(botId: number, thinkIntervalMinutes: number): Promise<void> {
    await pool.query(
      `UPDATE bot_profiles 
       SET last_action_at = NOW(),
           next_think_at = NOW() + INTERVAL '${thinkIntervalMinutes} minutes'
       WHERE id = $1`,
      [botId]
    );
  }

  /**
   * Bulk create bots
   */
  static async bulkCreateBots(count: number, personality_type: keyof typeof BOT_PERSONALITIES, difficulty_level: number = 5): Promise<number> {
    let created = 0;
    
    for (let i = 0; i < count; i++) {
      try {
        const username = `Bot_${personality_type}_${Date.now()}_${i}`;
        const email = `bot_${personality_type}_${Date.now()}_${i}@bot.local`;
        
        await this.createBot(username, email, personality_type, difficulty_level);
        created++;
      } catch (error) {
        console.error(`Failed to create bot ${i + 1}:`, error);
      }
    }
    
    return created;
  }

  /**
   * Get bot leaderboard
   */
  static async getLeaderboard(limit: number = 20): Promise<any[]> {
    const result = await pool.query(
      `SELECT * FROM bot_leaderboard 
       WHERE is_active = true 
       ORDER BY total_resources_plundered DESC, win_rate DESC 
       LIMIT $1`,
      [limit]
    );
    
    return result.rows;
  }

  /**
   * Add or update bot target
   */
  static async addTarget(
    botId: number,
    targetUserId: number,
    targetPlanetId: number,
    threatLevel: number = 5,
    resourcePotential: number = 0
  ): Promise<BotTarget> {
    const result = await pool.query(
      `INSERT INTO bot_targets (
        bot_id, target_user_id, target_planet_id, threat_level, resource_potential
      ) VALUES ($1, $2, $3, $4, $5)
      ON CONFLICT (bot_id, target_planet_id) 
      DO UPDATE SET 
        threat_level = EXCLUDED.threat_level,
        resource_potential = EXCLUDED.resource_potential,
        updated_at = NOW()
      RETURNING *`,
      [botId, targetUserId, targetPlanetId, threatLevel, resourcePotential]
    );
    
    if (!result.rows[0]) {
      throw new Error('Failed to add bot target');
    }
    return result.rows[0] as BotTarget;
  }

  /**
   * Get bot targets
   */
  static async getTargets(botId: number, limit: number = 10): Promise<BotTarget[]> {
    const result = await pool.query(
      `SELECT * FROM bot_targets 
       WHERE bot_id = $1 
       AND (next_attack_available_at IS NULL OR next_attack_available_at <= NOW())
       ORDER BY attack_priority DESC, resource_potential DESC
       LIMIT $2`,
      [botId, limit]
    );
    
    return result.rows as BotTarget[];
  }
}
