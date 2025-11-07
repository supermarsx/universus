// =====================================================
// BOT GENERATION SERVICE
// Generate bots from templates for universe seeding
// =====================================================

import pool from '../../config/database';
import { BotGenerationTemplate, GenerateBotsRequest, BotGenerationResult, BotPersonality, BotSkillLevel } from '../../types/universe';
import { BotService } from './botService';

export class BotGenerationService {
  private botService: BotService;
  
  constructor() {
    this.botService = new BotService();
  }
  
  /**
   * Generate bots for a universe
   */
  async generateBotsForUniverse(request: GenerateBotsRequest): Promise<BotGenerationResult> {
    const client = await pool.connect();
    
    try {
      await client.query('BEGIN');
      
      const { universeId, botCount, personalities, skillLevels, distributeEvenly = true } = request;
      
      // Get bot templates
      let templatesQuery = 'SELECT * FROM bot_generation_templates WHERE universe_id = $1';
      const params: any[] = [universeId];
      
      if (personalities && personalities.length > 0) {
        templatesQuery += ' AND bot_personality = ANY($2)';
        params.push(personalities);
      }
      
      const templatesResult = await client.query(templatesQuery, params);
      const templates = templatesResult.rows;
      
      if (templates.length === 0) {
        throw new Error('No bot templates found for universe');
      }
      
      const botsGenerated: any[] = [];
      
      for (let i = 0; i < botCount; i++) {
        // Select template
        if (templates.length === 0) {
          throw new Error('No templates available for bot generation');
        }
        const template = templates[i % templates.length];
        
        // Generate bot using existing bot service
        const botName = await this.generateBotName(client, template.bot_personality);
        
        // Create bot user account
        const botUser = await this.createBotUser(client, botName, template);
        
        // Place bot in universe
        const placement = await this.placeBotInUniverse(client, universeId, botUser.id, template);
        
        // Record generated bot
        const botRecord = await client.query(
          `INSERT INTO generated_bots (
            universe_id, user_id, template_id, bot_name, bot_personality, 
            skill_level, galaxy, system, position
          ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
          RETURNING *`,
          [
            universeId, botUser.id, template.id, botName, template.bot_personality,
            template.skill_level, placement.galaxy, placement.system, placement.position
          ]
        );
        
        botsGenerated.push(botRecord.rows[0]);
        
        // Update template count
        await client.query(
          'UPDATE bot_generation_templates SET current_bots_generated = current_bots_generated + 1 WHERE id = $1',
          [template.id]
        );
      }
      
      await client.query('COMMIT');
      
      return {
        success: true,
        botsGenerated: botCount,
        message: `Successfully generated ${botCount} bots for universe`
      };
      
    } catch (error) {
      await client.query('ROLLBACK');
      console.error('Error generating bots:', error);
      return {
        success: false,
        botsGenerated: 0,
        message: error instanceof Error ? error.message : 'Failed to generate bots'
      };
    } finally {
      client.release();
    }
  }
  
  private async generateBotName(client: any, personality: string): Promise<string> {
    const result = await client.query('SELECT get_next_bot_name($1) as name', [personality]);
    if (!result.rows[0] || !result.rows[0].name) {
      throw new Error('Failed to generate bot name');
    }
    return result.rows[0].name;
  }
  
  private async createBotUser(client: any, botName: string, template: any): Promise<any> {
    // Create bot user account
    const result = await client.query(
      `INSERT INTO users (username, email, password, is_bot, bot_personality)
       VALUES ($1, $2, $3, $4, $5)
       RETURNING *`,
      [botName, `${botName}@bot.local`, 'bot', true, template.bot_personality]
    );
    if (!result.rows[0]) {
      throw new Error('Failed to create bot user');
    }
    return result.rows[0];
  }
  
  private async placeBotInUniverse(client: any, universeId: number, userId: number, template: any): Promise<any> {
    // Simple placement in random galaxy/system
    const galaxyResult = await client.query(
      'SELECT galaxy_number FROM galaxy_seeds WHERE universe_id = $1 ORDER BY RANDOM() LIMIT 1',
      [universeId]
    );
    
    if (!galaxyResult.rows[0] || !galaxyResult.rows[0].galaxy_number) {
      throw new Error('Failed to get galaxy for bot placement');
    }
    const galaxy = galaxyResult.rows[0].galaxy_number;
    const system = Math.floor(Math.random() * 450) + 25;
    const position = Math.floor(Math.random() * 15) + 1;
    
    return { galaxy, system, position };
  }
}

export default new BotGenerationService();
