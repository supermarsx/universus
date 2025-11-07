import { pool } from '../config/database';
import { BotService, BotProfile } from './botService';
import { BuildingService } from '../../../backend/src/services/buildingService';
import { ResearchService } from '../../../backend/src/services/researchService';
import { FleetService } from '../../../backend/src/services/fleetService';

/**
 * BotAIService - Implements intelligent decision-making for bot players
 * Each bot makes decisions based on their personality type and current game state
 */
export class BotAIService {
  /**
   * Main think cycle - evaluates game state and makes decisions
   */
  static async think(bot: BotProfile): Promise<void> {
    const startTime = Date.now();
    
    try {
      console.log(`[Bot AI] ${bot.id} (${bot.personality_type}) thinking...`);
      
      // Get bot's user data and planets
      const userData = await this.getBotGameState(bot.user_id);
      
      if (!userData || !userData.planets || userData.planets.length === 0) {
        console.log(`[Bot AI] ${bot.id} has no planets, skipping`);
        return;
      }
      
      // Update current strategy based on game state
      const strategy = await this.evaluateStrategy(bot, userData);
      await pool.query(
        'UPDATE bot_profiles SET current_strategy = $1 WHERE id = $2',
        [JSON.stringify(strategy), bot.id]
      );
      
      // Make decisions based on personality and strategy
      await this.makeDecisions(bot, userData, strategy);
      
      // Update think time
      await BotService.updateNextThinkTime(bot.id, bot.think_interval_minutes);
      
      const executionTime = Date.now() - startTime;
      console.log(`[Bot AI] ${bot.id} finished thinking in ${executionTime}ms`);
      
    } catch (error) {
      console.error(`[Bot AI] Error in think cycle for bot ${bot.id}:`, error);
      
      // Log failed decision
      await BotService.logAction(
        bot.id,
        'think_cycle',
        { error: String(error) },
        {},
        false,
        {},
        {},
        Date.now() - startTime
      );
    }
  }

  /**
   * Get current game state for bot
   */
  private static async getBotGameState(userId: number): Promise<any> {
    const client = await pool.connect();
    
    try {
      // Get planets
      const planetsResult = await client.query(
        'SELECT * FROM planets WHERE user_id = $1 ORDER BY id',
        [userId]
      );
      
      // Get resources for each planet
      const planets = planetsResult.rows;
      
      // Get active fleets
      const fleetsResult = await client.query(
        'SELECT * FROM fleets WHERE user_id = $1 AND status = $2',
        [userId, 'in_transit']
      );
      
      // Get construction queue
      const constructionResult = await client.query(
        'SELECT * FROM planet_buildings WHERE user_id = $1 AND is_building = true',
        [userId]
      );
      
      // Get research queue
      const researchResult = await client.query(
        'SELECT * FROM research WHERE user_id = $1',
        [userId]
      );
      
      return {
        userId,
        planets: planets,
        fleets: fleetsResult.rows,
        constructions: constructionResult.rows,
        research: researchResult.rows
      };
    } finally {
      client.release();
    }
  }

  /**
   * Evaluate and update bot strategy based on current game state
   */
  private static async evaluateStrategy(bot: BotProfile, gameState: any): Promise<any> {
    const strategy: any = {
      timestamp: new Date(),
      phase: 'early',
      priorities: [],
      goals: []
    };
    
    // Determine game phase based on development
    const totalPlanets = gameState.planets.length;
    const mainPlanet = gameState.planets[0];
    
    if (totalPlanets === 1 && mainPlanet.metal < 10000) {
      strategy.phase = 'early';
      strategy.priorities = ['economy', 'defense', 'expansion'];
    } else if (totalPlanets < 5) {
      strategy.phase = 'mid';
      strategy.priorities = ['expansion', 'military', 'economy'];
    } else {
      strategy.phase = 'late';
      strategy.priorities = ['military', 'dominance', 'optimization'];
    }
    
    // Set goals based on personality
    switch (bot.personality_type) {
      case 'aggressive_conqueror':
        strategy.goals = ['build_fleet', 'find_targets', 'launch_attacks'];
        break;
      case 'strategic_builder':
        strategy.goals = ['upgrade_infrastructure', 'expand_territory', 'prepare_defense'];
        break;
      case 'diplomatic_negotiator':
        strategy.goals = ['join_alliance', 'trade_resources', 'expand_peacefully'];
        break;
      case 'resource_hoarder':
        strategy.goals = ['maximize_production', 'stockpile_resources', 'upgrade_mines'];
        break;
      case 'speed_rusher':
        strategy.goals = ['fast_tech', 'early_raids', 'pressure_opponents'];
        break;
      case 'tech_enthusiast':
        strategy.goals = ['research_everything', 'unlock_advanced_tech', 'optimize_efficiency'];
        break;
      case 'alliance_focused':
        strategy.goals = ['support_allies', 'coordinate_attacks', 'share_intel'];
        break;
      case 'solo_survivor':
        strategy.goals = ['self_sufficiency', 'defensive_position', 'opportunistic_growth'];
        break;
    }
    
    return strategy;
  }

  /**
   * Make decisions based on strategy and personality
   */
  private static async makeDecisions(bot: BotProfile, gameState: any, strategy: any): Promise<void> {
    const decisions: string[] = [];
    
    // Economy decisions (building upgrades)
    if (bot.economy_focus > 50 && Math.random() * 100 < bot.economy_focus) {
      const economyDecision = await this.makeEconomyDecision(bot, gameState);
      if (economyDecision) decisions.push(economyDecision);
    }
    
    // Research decisions
    if (bot.research_focus > 50 && Math.random() * 100 < bot.research_focus) {
      const researchDecision = await this.makeResearchDecision(bot, gameState);
      if (researchDecision) decisions.push(researchDecision);
    }
    
    // Military decisions (ship building)
    if (bot.military_focus > 50 && Math.random() * 100 < bot.military_focus) {
      const militaryDecision = await this.makeMilitaryDecision(bot, gameState);
      if (militaryDecision) decisions.push(militaryDecision);
    }
    
    // Attack decisions
    if (bot.aggression_level > 50 && Math.random() * 100 < bot.aggression_level) {
      const attackDecision = await this.makeAttackDecision(bot, gameState);
      if (attackDecision) decisions.push(attackDecision);
    }
    
    // Expansion decisions
    if (bot.expansion_priority > 60 && Math.random() * 100 < bot.expansion_priority) {
      const expansionDecision = await this.makeExpansionDecision(bot, gameState);
      if (expansionDecision) decisions.push(expansionDecision);
    }
    
    console.log(`[Bot AI] ${bot.id} made decisions: ${decisions.join(', ')}`);
  }

  /**
   * Make economy/building decisions
   */
  private static async makeEconomyDecision(bot: BotProfile, gameState: any): Promise<string | null> {
    try {
      const mainPlanet = gameState.planets[0];
      
      // Check if already building
      const isBuilding = gameState.constructions.some(
        (c: any) => c.planet_id === mainPlanet.id
      );
      
      if (isBuilding) {
        return null; // Already building something
      }
      
      // Determine what to build based on resources and strategy
      const resources = {
        metal: mainPlanet.metal,
        crystal: mainPlanet.crystal,
        deuterium: mainPlanet.deuterium
      };
      
      // Priority: Metal Mine > Crystal Mine > Deuterium Synthesizer > Solar Plant
      const buildingPriorities = [
        { name: 'metal_mine', minResources: 5000 },
        { name: 'crystal_mine', minResources: 7000 },
        { name: 'deuterium_synthesizer', minResources: 10000 },
        { name: 'solar_plant', minResources: 4000 }
      ];
      
      for (const building of buildingPriorities) {
        if (resources.metal >= building.minResources) {
          try {
            // Attempt to start building
            await BuildingService.startConstruction(gameState.userId, mainPlanet.id, building.name);
            
            await BotService.logAction(
              bot.id,
              'build_structure',
              { building: building.name, planet_id: mainPlanet.id },
              { economy_focus: bot.economy_focus, resources },
              true
            );
            
            return `built_${building.name}`;
          } catch (error) {
            // Building failed, continue to next option
            continue;
          }
        }
      }
      
      return null;
    } catch (error) {
      console.error('[Bot AI] Economy decision error:', error);
      return null;
    }
  }

  /**
   * Make research decisions
   */
  private static async makeResearchDecision(bot: BotProfile, gameState: any): Promise<string | null> {
    try {
      // Check if already researching
      const isResearching = gameState.research.some((r: any) => r.is_researching);
      
      if (isResearching) {
        return null;
      }
      
      const mainPlanet = gameState.planets[0];
      const resources = {
        metal: mainPlanet.metal,
        crystal: mainPlanet.crystal,
        deuterium: mainPlanet.deuterium
      };
      
      // Research priorities based on personality
      let researchPriorities: string[] = [];
      
      switch (bot.personality_type) {
        case 'tech_enthusiast':
        case 'speed_rusher':
          researchPriorities = ['energy_technology', 'combustion_drive', 'computer_technology'];
          break;
        case 'aggressive_conqueror':
        case 'alliance_focused':
          researchPriorities = ['weapons_technology', 'shielding_technology', 'armor_technology'];
          break;
        case 'strategic_builder':
        case 'resource_hoarder':
          researchPriorities = ['energy_technology', 'laser_technology', 'ion_technology'];
          break;
        default:
          researchPriorities = ['energy_technology', 'computer_technology', 'espionage_technology'];
      }
      
      for (const tech of researchPriorities) {
        if (resources.metal >= 10000 && resources.crystal >= 5000) {
          try {
            await ResearchService.startResearch(gameState.userId, mainPlanet.id, tech);
            
            await BotService.logAction(
              bot.id,
              'research_technology',
              { technology: tech },
              { research_focus: bot.research_focus },
              true
            );
            
            return `researched_${tech}`;
          } catch (error) {
            continue;
          }
        }
      }
      
      return null;
    } catch (error) {
      console.error('[Bot AI] Research decision error:', error);
      return null;
    }
  }

  /**
   * Make military/ship building decisions
   */
  private static async makeMilitaryDecision(bot: BotProfile, gameState: any): Promise<string | null> {
    try {
      const mainPlanet = gameState.planets[0];
      const resources = {
        metal: mainPlanet.metal,
        crystal: mainPlanet.crystal,
        deuterium: mainPlanet.deuterium
      };
      
      // Determine ship type and quantity based on personality
      let shipType = bot.preferred_ship_type || 'light_fighter';
      let quantity = 1;
      
      switch (bot.fleet_size_preference) {
        case 'large':
          quantity = 50;
          break;
        case 'medium':
          quantity = 20;
          break;
        case 'small':
          quantity = 5;
          break;
      }
      
      // Adjust quantity based on resources
      if (resources.metal < 50000) {
        quantity = Math.floor(quantity / 2);
      }
      
      if (quantity > 0 && resources.metal >= 10000) {
        try {
          // Build ships (simplified - would need actual shipyard service)
          await BotService.logAction(
            bot.id,
            'build_ships',
            { ship_type: shipType, quantity, planet_id: mainPlanet.id },
            { military_focus: bot.military_focus, resources },
            true
          );
          
          return `built_${quantity}_${shipType}`;
        } catch (error) {
          return null;
        }
      }
      
      return null;
    } catch (error) {
      console.error('[Bot AI] Military decision error:', error);
      return null;
    }
  }

  /**
   * Make attack decisions
   */
  private static async makeAttackDecision(bot: BotProfile, gameState: any): Promise<string | null> {
    try {
      // Get potential targets
      const targets = await BotService.getTargets(bot.id, 5);
      
      if (targets.length === 0) {
        // Find new targets
        await this.scanForTargets(bot, gameState);
        return 'scanned_for_targets';
      }
      
      // Evaluate best target
      const target = targets[0];
      
      // Check if we have enough ships
      const mainPlanet = gameState.planets[0];
      const resources = {
        metal: mainPlanet.metal,
        crystal: mainPlanet.crystal,
        deuterium: mainPlanet.deuterium
      };
      
      // Decision to attack based on risk tolerance
      if (Math.random() * 100 < bot.risk_tolerance) {
        // Would launch attack here (simplified)
        await BotService.logAction(
          bot.id,
          'launch_attack',
          { target_planet_id: target.target_planet_id },
          { aggression_level: bot.aggression_level, target_resources: target.resource_potential },
          true
        );
        
        // Update target's next attack time
        await pool.query(
          `UPDATE bot_targets 
           SET last_attack_at = NOW(),
               next_attack_available_at = NOW() + INTERVAL '${bot.attack_frequency_hours} hours',
               total_attacks = total_attacks + 1
           WHERE id = $1`,
          [target.id]
        );
        
        return 'launched_attack';
      }
      
      return null;
    } catch (error) {
      console.error('[Bot AI] Attack decision error:', error);
      return null;
    }
  }

  /**
   * Make expansion decisions
   */
  private static async makeExpansionDecision(bot: BotProfile, gameState: any): Promise<string | null> {
    try {
      const currentPlanets = gameState.planets.length;
      const maxPlanets = 9; // Game limit
      
      if (currentPlanets >= maxPlanets) {
        return null; // Already at max planets
      }
      
      const mainPlanet = gameState.planets[0];
      const resources = {
        metal: mainPlanet.metal,
        crystal: mainPlanet.crystal,
        deuterium: mainPlanet.deuterium
      };
      
      // Check if we have resources for colonization
      const colonizationCost = 50000;
      
      if (resources.metal >= colonizationCost && resources.crystal >= colonizationCost) {
        // Would launch colonization mission here (simplified)
        await BotService.logAction(
          bot.id,
          'launch_colonization',
          { current_planets: currentPlanets },
          { expansion_priority: bot.expansion_priority, resources },
          true
        );
        
        return 'launched_colonization';
      }
      
      return null;
    } catch (error) {
      console.error('[Bot AI] Expansion decision error:', error);
      return null;
    }
  }

  /**
   * Scan galaxy for potential targets
   */
  private static async scanForTargets(bot: BotProfile, gameState: any): Promise<void> {
    try {
      // Find nearby planets owned by other players
      const mainPlanet = gameState.planets[0];
      
      const result = await pool.query(
        `SELECT p.*, u.username,
         (p.metal + p.crystal + p.deuterium) as total_resources
         FROM planets p
         JOIN users u ON p.user_id = u.id
         WHERE u.id != $1
         AND p.galaxy = $2
         AND ABS(p.system - $3) <= 50
         ORDER BY (p.metal + p.crystal + p.deuterium) DESC
         LIMIT 10`,
        [gameState.userId, mainPlanet.galaxy, mainPlanet.system]
      );
      
      // Add targets
      for (const planet of result.rows) {
        const threatLevel = Math.floor(Math.random() * 5) + 3; // 3-7
        const resourcePotential = planet.total_resources;
        
        await BotService.addTarget(
          bot.id,
          planet.user_id,
          planet.id,
          threatLevel,
          resourcePotential
        );
      }
      
      console.log(`[Bot AI] ${bot.id} found ${result.rows.length} targets`);
    } catch (error) {
      console.error('[Bot AI] Target scanning error:', error);
    }
  }

  /**
   * Process all active bots (called periodically by game loop or scheduler)
   */
  static async processAllBots(): Promise<void> {
    try {
      const bots = await BotService.getBotsNeedingThink();
      
      console.log(`[Bot AI] Processing ${bots.length} bots...`);
      
      for (const bot of bots) {
        try {
          await this.think(bot);
          
          // Add small delay between bots to avoid overwhelming the system
          await new Promise(resolve => setTimeout(resolve, 100));
        } catch (error) {
          console.error(`[Bot AI] Error processing bot ${bot.id}:`, error);
        }
      }
      
      console.log(`[Bot AI] Finished processing ${bots.length} bots`);
    } catch (error) {
      console.error('[Bot AI] Error in processAllBots:', error);
    }
  }
}
