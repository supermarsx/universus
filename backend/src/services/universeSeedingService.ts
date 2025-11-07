// =====================================================
// UNIVERSE SEEDING SERVICE
// Orchestrates complete universe generation and seeding
// =====================================================

import pool from '../config/database';
import {
  UniverseSeed,
  GalaxySeed,
  CreateUniverseRequest,
  SeedUniverseRequest,
  UniverseSeedingResult,
  UniverseType,
  GalaxyType,
  DifficultyCurve
} from '../types/universe';

export class UniverseSeedingService {
  
  // =====================================================
  // UNIVERSE CREATION
  // =====================================================
  
  /**
   * Create a new universe configuration
   */
  async createUniverse(request: CreateUniverseRequest): Promise<{ success: boolean; universeId?: number; message: string }> {
    const client = await pool.connect();
    
    try {
      await client.query('BEGIN');
      
      const {
        universeName,
        universeType,
        galaxyCount = 9,
        systemsPerGalaxy = 499,
        maxPlayers = 10000,
        botPercentage = 30,
        resourceMultiplier = 1.0,
        difficultyCurve = DifficultyCurve.PROGRESSIVE,
        configuration = {}
      } = request;
      
      // Calculate target bot count
      const targetBotCount = Math.floor((maxPlayers * botPercentage) / 100);
      
      // Create universe seed
      const result = await client.query(
        `INSERT INTO universe_seeds (
          universe_name, universe_type, galaxy_count, systems_per_galaxy,
          max_players, bot_percentage, target_bot_count,
          resource_multiplier, difficulty_curve, configuration
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING *`,
        [
          universeName, universeType, galaxyCount, systemsPerGalaxy,
          maxPlayers, botPercentage, targetBotCount,
          resourceMultiplier, difficultyCurve, JSON.stringify(configuration)
        ]
      );
      
      const universe = result.rows[0] ? this.mapUniverseSeed(result.rows[0]) : null;
      
      await client.query('COMMIT');
      
      return {
        success: true,
        universeId: universe.id,
        message: `Universe "${universeName}" created successfully`
      };
      
    } catch (error) {
      await client.query('ROLLBACK');
      console.error('Error creating universe:', error);
      return {
        success: false,
        message: error instanceof Error ? error.message : 'Failed to create universe'
      };
    } finally {
      client.release();
    }
  }
  
  /**
   * Complete universe seeding process
   */
  async seedUniverse(request: SeedUniverseRequest): Promise<UniverseSeedingResult> {
    const startTime = Date.now();
    const client = await pool.connect();
    
    try {
      await client.query('BEGIN');
      
      const { universeId, generateGalaxies, generateBots, generateAlliances, distributeResources } = request;
      
      // Update seeding status
      await client.query(
        'UPDATE universe_seeds SET seeding_started_at = NOW(), is_seeded = FALSE WHERE id = $1',
        [universeId]
      );
      
      let galaxiesGenerated = 0;
      let botsGenerated = 0;
      let alliancesCreated = 0;
      let resourcePatternsApplied = 0;
      const errors: string[] = [];
      
      // Step 1: Generate Galaxies
      if (generateGalaxies) {
        try {
          galaxiesGenerated = await this.generateGalaxiesForUniverse(client, universeId);
        } catch (error) {
          errors.push(`Galaxy generation failed: ${error instanceof Error ? error.message : 'Unknown error'}`);
        }
      }
      
      // Step 2: Distribute Resources
      if (distributeResources && galaxiesGenerated > 0) {
        try {
          resourcePatternsApplied = await this.distributeUniverseResources(client, universeId);
        } catch (error) {
          errors.push(`Resource distribution failed: ${error instanceof Error ? error.message : 'Unknown error'}`);
        }
      }
      
      // Step 3: Generate Bot Templates
      if (generateBots) {
        try {
          await this.createBotTemplates(client, universeId);
        } catch (error) {
          errors.push(`Bot template creation failed: ${error instanceof Error ? error.message : 'Unknown error'}`);
        }
      }
      
      // Step 4: Create Alliance Seeds
      if (generateAlliances) {
        try {
          alliancesCreated = await this.createAllianceSeeds(client, universeId);
        } catch (error) {
          errors.push(`Alliance seeding failed: ${error instanceof Error ? error.message : 'Unknown error'}`);
        }
      }
      
      // Step 5: Create Maintenance Tasks
      await this.createMaintenanceTasks(client, universeId);
      
      // Mark universe as seeded
      await client.query(
        `UPDATE universe_seeds 
         SET is_seeded = TRUE, 
             seeding_completed_at = NOW(),
             seed_version = seed_version + 1
         WHERE id = $1`,
        [universeId]
      );
      
      await client.query('COMMIT');
      
      const duration = Math.floor((Date.now() - startTime) / 1000);
      
      return {
        success: errors.length === 0,
        universeId,
        seedVersion: 1,
        galaxiesGenerated,
        botsGenerated,
        alliancesCreated,
        resourcePatternsApplied,
        seedingDuration: duration,
        message: errors.length === 0 
          ? `Universe seeded successfully in ${duration} seconds`
          : `Universe seeding completed with ${errors.length} errors`,
        errors: errors.length > 0 ? errors : undefined
      };
      
    } catch (error) {
      await client.query('ROLLBACK');
      console.error('Error seeding universe:', error);
      
      const duration = Math.floor((Date.now() - startTime) / 1000);
      
      return {
        success: false,
        galaxiesGenerated: 0,
        botsGenerated: 0,
        alliancesCreated: 0,
        resourcePatternsApplied: 0,
        seedingDuration: duration,
        message: error instanceof Error ? error.message : 'Failed to seed universe',
        errors: [error instanceof Error ? error.message : 'Unknown error']
      };
    } finally {
      client.release();
    }
  }
  
  // =====================================================
  // GALAXY GENERATION
  // =====================================================
  
  /**
   * Generate all galaxies for a universe
   */
  private async generateGalaxiesForUniverse(client: any, universeId: number): Promise<number> {
    // Get universe configuration
    const universeResult = await client.query(
      'SELECT * FROM universe_seeds WHERE id = $1',
      [universeId]
    );
    
    if (universeResult.rows.length === 0) {
      throw new Error('Universe not found');
    }
    
    const universe = universeResult.rows[0];
    const galaxyTypes = this.determineGalaxyTypes(universe.galaxy_count, universe.universe_type);
    
    let generatedCount = 0;
    
    for (let i = 1; i <= universe.galaxy_count; i++) {
      const galaxyType = galaxyTypes[i - 1];
      
      // Create galaxy seed
      await client.query(
        `INSERT INTO galaxy_seeds (
          universe_id, galaxy_number, galaxy_name, galaxy_type,
          system_count, sector_divisions,
          metal_abundance, crystal_abundance, deuterium_abundance,
          rare_materials_chance, base_difficulty, npc_strength_multiplier,
          max_players_per_galaxy, has_safe_zones, has_pvp_zones,
          has_resource_zones, is_generated, generated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, NOW())`,
        [
          universeId, i, `Galaxy ${i}`, galaxyType,
          universe.systems_per_galaxy, 10,
          this.getAbundanceForType(galaxyType, 'metal'),
          this.getAbundanceForType(galaxyType, 'crystal'),
          this.getAbundanceForType(galaxyType, 'deuterium'),
          this.getRareMaterialsChance(galaxyType),
          this.getBaseDifficulty(i, universe.galaxy_count),
          this.getNpcMultiplier(galaxyType),
          Math.floor(universe.max_players / universe.galaxy_count),
          i <= 3, // First 3 galaxies have safe zones
          i >= 4, // Galaxies 4+ have PVP zones
          true
        ]
      );
      
      // Get the generated galaxy ID
      const galaxyResult = await client.query(
        'SELECT id FROM galaxy_seeds WHERE universe_id = $1 AND galaxy_number = $2',
        [universeId, i]
      );
      
      const galaxyId = galaxyResult.rows[0]?.id;
      
      if (!galaxyId) {
        throw new Error(`Failed to retrieve galaxy ID for galaxy ${i}`);
      }
      
      // Create sector configurations for this galaxy
      await this.createSectorConfigurations(client, galaxyId, i, universe.galaxy_count);
      
      generatedCount++;
    }
    
    return generatedCount;
  }
  
  /**
   * Create sector configurations for a galaxy
   */
  private async createSectorConfigurations(
    client: any,
    galaxyId: number,
    galaxyNumber: number,
    totalGalaxies: number
  ): Promise<void> {
    const sectorsPerGalaxy = 10;
    const systemsPerSector = 50; // Approximately 500 systems / 10 sectors
    
    for (let sector = 1; sector <= sectorsPerGalaxy; sector++) {
      const systemStart = (sector - 1) * systemsPerSector + 1;
      const systemEnd = sector * systemsPerSector;
      
      // Calculate difficulty tier (1-10)
      const difficultyTier = Math.min(10, galaxyNumber + Math.floor(sector / 2));
      
      // Determine zone types
      const isSafeZone = galaxyNumber === 1 && sector <= 3;
      const isBeginnerZone = galaxyNumber <= 2 && sector <= 5;
      const isPvpZone = galaxyNumber >= 4 && sector >= 6;
      const isEndgameZone = galaxyNumber >= 8 && sector >= 8;
      
      await client.query(
        `INSERT INTO sector_configurations (
          galaxy_id, sector_number, sector_name,
          difficulty_tier, recommended_level,
          system_start, system_end,
          metal_multiplier, crystal_multiplier, deuterium_multiplier,
          is_safe_zone, is_pvp_zone, is_beginner_zone, is_endgame_zone,
          npc_density, npc_strength_level
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)`,
        [
          galaxyId, sector, `Sector ${sector}`,
          difficultyTier, difficultyTier * 5,
          systemStart, systemEnd,
          1.0 + (sector * 0.1), // Resource multipliers increase with sector
          1.0 + (sector * 0.1),
          1.0 + (sector * 0.1),
          isSafeZone, isPvpZone, isBeginnerZone, isEndgameZone,
          0.3 + (sector * 0.05), // NPC density increases
          difficultyTier
        ]
      );
    }
  }
  
  // =====================================================
  // RESOURCE DISTRIBUTION
  // =====================================================
  
  /**
   * Distribute resources across all galaxies
   */
  private async distributeUniverseResources(client: any, universeId: number): Promise<number> {
    const galaxiesResult = await client.query(
      'SELECT * FROM galaxy_seeds WHERE universe_id = $1',
      [universeId]
    );
    
    let patternsApplied = 0;
    
    for (const galaxy of galaxiesResult.rows) {
      // Create resource distribution patterns for each galaxy
      await client.query(
        `INSERT INTO resource_distribution_patterns (
          galaxy_id, pattern_name, pattern_type, resource_type,
          base_abundance, variation_percentage, cluster_size, cluster_density,
          is_applied, applied_at
        ) VALUES 
        ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW()),
        ($1, $10, $11, $12, $13, $14, $15, $16, $9, NOW()),
        ($1, $17, $18, $19, $20, $21, $22, $23, $9, NOW())`,
        [
          galaxy.id,
          'Metal Clusters', 'clustered', 'metal', galaxy.metal_abundance, 0.3, 10, 1.5, true,
          'Crystal Veins', 'radial', 'crystal', galaxy.crystal_abundance, 0.25, 8, 1.3, true,
          'Deuterium Fields', 'strategic', 'deuterium', galaxy.deuterium_abundance, 0.2, 6, 1.2, true
        ]
      );
      
      patternsApplied += 3;
    }
    
    return patternsApplied;
  }
  
  // =====================================================
  // BOT TEMPLATES
  // =====================================================
  
  /**
   * Create bot generation templates
   */
  private async createBotTemplates(client: any, universeId: number): Promise<void> {
    const personalities = ['aggressive', 'defensive', 'economic', 'explorer', 'researcher', 'diplomatic', 'opportunist', 'balanced'];
    const skillLevels = ['novice', 'intermediate', 'advanced', 'expert'];
    
    for (const personality of personalities) {
      for (const skillLevel of skillLevels) {
        await client.query(
          `INSERT INTO bot_generation_templates (
            universe_id, template_name, bot_personality, skill_level,
            skill_randomness, aggression_level, expansion_rate,
            trading_activity, alliance_participation, resource_focus,
            combat_willingness, generation_weight
          ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)`,
          [
            universeId,
            `${personality}-${skillLevel}`,
            personality,
            skillLevel,
            0.2,
            this.getAggressionForPersonality(personality),
            this.getExpansionForSkill(skillLevel),
            this.getTradingActivity(personality),
            personality !== 'aggressive',
            this.getResourceFocus(personality),
            this.getCombatWillingness(personality),
            100
          ]
        );
      }
    }
  }
  
  // =====================================================
  // ALLIANCE SEEDING
  // =====================================================
  
  /**
   * Create alliance seeds
   */
  private async createAllianceSeeds(client: any, universeId: number): Promise<number> {
    const allianceCount = 20; // Create 20 seed alliances
    let created = 0;
    
    const allianceTypes = ['military', 'economic', 'research', 'balanced'];
    const formationStrategies = ['pre_seeded', 'bot_alliance', 'mixed'];
    
    for (let i = 1; i <= allianceCount; i++) {
      const allianceType = allianceTypes[i % allianceTypes.length];
      const formationStrategy = formationStrategies[i % formationStrategies.length];
      
      await client.query(
        `INSERT INTO alliance_seeds (
          universe_id, alliance_name, alliance_tag, alliance_type,
          formation_strategy, target_member_count, bot_member_percentage,
          home_galaxy
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)`,
        [
          universeId,
          `Alliance ${i}`,
          `A${i.toString().padStart(2, '0')}`,
          allianceType,
          formationStrategy,
          50,
          50.0,
          (i % 9) + 1
        ]
      );
      
      created++;
    }
    
    return created;
  }
  
  // =====================================================
  // MAINTENANCE TASKS
  // =====================================================
  
  /**
   * Create automated maintenance tasks
   */
  private async createMaintenanceTasks(client: any, universeId: number): Promise<void> {
    const tasks = [
      { name: 'Population Balance', type: 'population_balance', frequency: 6 },
      { name: 'Resource Balance', type: 'resource_balance', frequency: 12 },
      { name: 'Bot Management', type: 'bot_management', frequency: 24 },
      { name: 'Inactive Cleanup', type: 'cleanup', frequency: 24 },
      { name: 'Analytics Collection', type: 'analytics', frequency: 1 },
      { name: 'Performance Monitoring', type: 'performance', frequency: 1 }
    ];
    
    for (const task of tasks) {
      await client.query(
        `INSERT INTO universe_maintenance_tasks (
          universe_id, task_name, task_type, run_frequency_hours,
          is_active, next_run_at
        ) VALUES ($1, $2, $3, $4, $5, NOW() + INTERVAL '1 hour' * $4)`,
        [universeId, task.name, task.type, task.frequency, true]
      );
    }
  }
  
  // =====================================================
  // QUERY METHODS
  // =====================================================
  
  /**
   * Get universe by ID
   */
  async getUniverseById(universeId: number): Promise<UniverseSeed | null> {
    const result = await pool.query(
      'SELECT * FROM universe_seeds WHERE id = $1',
      [universeId]
    );
    
    return result.rows.length > 0 ? this.mapUniverseSeed(result.rows[0]) : null;
  }
  
  /**
   * Get all universes
   */
  async getAllUniverses(): Promise<UniverseSeed[]> {
    const result = await pool.query(
      'SELECT * FROM universe_seeds ORDER BY created_at DESC'
    );
    
    return result.rows.map(row => this.mapUniverseSeed(row));
  }
  
  /**
   * Get seeded universes
   */
  async getSeededUniverses(): Promise<UniverseSeed[]> {
    const result = await pool.query(
      'SELECT * FROM universe_seeds WHERE is_seeded = TRUE ORDER BY created_at DESC'
    );
    
    return result.rows.map(row => this.mapUniverseSeed(row));
  }
  
  /**
   * Get galaxies for universe
   */
  async getGalaxiesForUniverse(universeId: number): Promise<GalaxySeed[]> {
    const result = await pool.query(
      'SELECT * FROM galaxy_seeds WHERE universe_id = $1 ORDER BY galaxy_number',
      [universeId]
    );
    
    return result.rows.map(row => this.mapGalaxySeed(row));
  }
  
  // =====================================================
  // HELPER METHODS
  // =====================================================
  
  private determineGalaxyTypes(galaxyCount: number, universeType: UniverseType): GalaxyType[] {
    const types: GalaxyType[] = [];
    
    // First 3 galaxies are always beginner-friendly
    types.push(GalaxyType.SAFE_ZONE, GalaxyType.STANDARD, GalaxyType.STANDARD);
    
    // Middle galaxies vary by universe type
    for (let i = 3; i < galaxyCount - 2; i++) {
      if (universeType === UniverseType.COMBAT_FOCUSED) {
        types.push(i % 2 === 0 ? GalaxyType.MILITARY : GalaxyType.PVP_ZONE);
      } else if (universeType === UniverseType.RESOURCE_RICH) {
        types.push(GalaxyType.RESOURCE_RICH);
      } else if (universeType === UniverseType.RESEARCH_HEAVY) {
        types.push(GalaxyType.RESEARCH);
      } else {
        types.push(GalaxyType.STANDARD);
      }
    }
    
    // Last 2 galaxies are endgame content
    if (galaxyCount > 5) {
      types.push(GalaxyType.ENDGAME, GalaxyType.ENDGAME);
    }
    
    return types;
  }
  
  private getAbundanceForType(galaxyType: GalaxyType, resourceType: string): number {
    const baseAbundance = 1.0;
    
    if (galaxyType === GalaxyType.RESOURCE_RICH) return baseAbundance * 1.5;
    if (galaxyType === GalaxyType.WASTELAND) return baseAbundance * 0.7;
    if (galaxyType === GalaxyType.ENDGAME) return baseAbundance * 1.3;
    
    return baseAbundance;
  }
  
  private getRareMaterialsChance(galaxyType: GalaxyType): number {
    if (galaxyType === GalaxyType.ENDGAME) return 15.0;
    if (galaxyType === GalaxyType.RESOURCE_RICH) return 10.0;
    if (galaxyType === GalaxyType.RESEARCH) return 12.0;
    return 5.0;
  }
  
  private getBaseDifficulty(galaxyNumber: number, totalGalaxies: number): number {
    return Math.min(10, Math.ceil((galaxyNumber / totalGalaxies) * 10));
  }
  
  private getNpcMultiplier(galaxyType: GalaxyType): number {
    if (galaxyType === GalaxyType.MILITARY) return 1.5;
    if (galaxyType === GalaxyType.ENDGAME) return 2.0;
    if (galaxyType === GalaxyType.SAFE_ZONE) return 0.5;
    return 1.0;
  }
  
  private getAggressionForPersonality(personality: string): number {
    const levels: Record<string, number> = {
      aggressive: 9,
      defensive: 3,
      economic: 4,
      explorer: 5,
      researcher: 3,
      diplomatic: 2,
      opportunist: 7,
      balanced: 5
    };
    return levels[personality] || 5;
  }
  
  private getExpansionForSkill(skillLevel: string): number {
    const rates: Record<string, number> = {
      novice: 0.7,
      intermediate: 1.0,
      advanced: 1.3,
      expert: 1.5
    };
    return rates[skillLevel] || 1.0;
  }
  
  private getTradingActivity(personality: string): number {
    if (personality === 'economic') return 0.9;
    if (personality === 'diplomatic') return 0.7;
    if (personality === 'aggressive') return 0.2;
    return 0.5;
  }
  
  private getResourceFocus(personality: string): string {
    if (personality === 'military' || personality === 'aggressive') return 'metal';
    if (personality === 'researcher') return 'crystal';
    if (personality === 'economic') return 'balanced';
    return 'balanced';
  }
  
  private getCombatWillingness(personality: string): number {
    if (personality === 'aggressive') return 0.9;
    if (personality === 'defensive') return 0.3;
    if (personality === 'opportunist') return 0.7;
    return 0.5;
  }
  
  private mapUniverseSeed(row: any): UniverseSeed {
    return {
      id: row.id,
      universeName: row.universe_name,
      universeType: row.universe_type,
      galaxyCount: row.galaxy_count,
      systemsPerGalaxy: row.systems_per_galaxy,
      positionsPerSystem: row.positions_per_system,
      maxPlayers: row.max_players,
      currentPlayers: row.current_players || 0,
      botPercentage: parseFloat(row.bot_percentage),
      targetBotCount: row.target_bot_count || 0,
      resourceMultiplier: parseFloat(row.resource_multiplier),
      startingResourcesMetal: parseInt(row.starting_resources_metal),
      startingResourcesCrystal: parseInt(row.starting_resources_crystal),
      startingResourcesDeuterium: parseInt(row.starting_resources_deuterium),
      difficultyCurve: row.difficulty_curve,
      beginnerProtectionDays: row.beginner_protection_days,
      isSeeded: row.is_seeded,
      seedVersion: row.seed_version,
      seedingStartedAt: row.seeding_started_at,
      seedingCompletedAt: row.seeding_completed_at,
      lastMaintainedAt: row.last_maintained_at,
      createdAt: row.created_at,
      updatedAt: row.updated_at,
      createdBy: row.created_by,
      configuration: row.configuration
    };
  }
  
  private mapGalaxySeed(row: any): GalaxySeed {
    return {
      id: row.id,
      universeId: row.universe_id,
      galaxyNumber: row.galaxy_number,
      galaxyName: row.galaxy_name,
      galaxyType: row.galaxy_type,
      systemCount: row.system_count,
      sectorDivisions: row.sector_divisions,
      metalAbundance: parseFloat(row.metal_abundance),
      crystalAbundance: parseFloat(row.crystal_abundance),
      deuteriumAbundance: parseFloat(row.deuterium_abundance),
      rareMaterialsChance: parseFloat(row.rare_materials_chance),
      baseDifficulty: row.base_difficulty,
      npcStrengthMultiplier: parseFloat(row.npc_strength_multiplier),
      maxPlayersPerGalaxy: row.max_players_per_galaxy,
      currentPlayers: row.current_players || 0,
      botCount: row.bot_count || 0,
      hasSafeZones: row.has_safe_zones,
      hasPvpZones: row.has_pvp_zones,
      hasResourceZones: row.has_resource_zones,
      hasEventZones: row.has_event_zones,
      isGenerated: row.is_generated,
      generatedAt: row.generated_at,
      createdAt: row.created_at,
      updatedAt: row.updated_at
    };
  }
}

export default new UniverseSeedingService();
