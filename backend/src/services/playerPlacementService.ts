// =====================================================
// PLAYER PLACEMENT SERVICE
// Intelligent player starting position algorithms
// =====================================================

import pool from '../config/database';
import {
  PlacePlayerRequest,
  PlayerPlacementResult,
  PlayerPlacement,
  PlacementStrategy,
  LocationScore,
  Coordinates
} from '../types/universe';

export class PlayerPlacementService {
  
  // =====================================================
  // PLAYER PLACEMENT
  // =====================================================
  
  /**
   * Place a player in the universe using intelligent algorithms
   */
  async placePlayer(request: PlacePlayerRequest): Promise<PlayerPlacementResult> {
    const client = await pool.connect();
    
    try {
      await client.query('BEGIN');
      
      const {
        userId,
        universeId,
        preferredPlaystyle = 'balanced',
        allianceId,
        useCustomLocation = false,
        customGalaxy,
        customSystem
      } = request;
      
      // Check if player already placed in this universe
      const existingResult = await client.query(
        'SELECT * FROM player_placements WHERE universe_id = $1 AND user_id = $2',
        [universeId, userId]
      );
      
      if (existingResult.rows.length > 0) {
        throw new Error('Player already placed in this universe');
      }
      
      // Get universe configuration
      const universeResult = await client.query(
        'SELECT * FROM universe_seeds WHERE id = $1 AND is_seeded = TRUE',
        [universeId]
      );
      
      if (universeResult.rows.length === 0) {
        throw new Error('Universe not found or not seeded');
      }
      
      const universe = universeResult.rows[0];
      
      let placement: Coordinates;
      let qualityScore: number;
      let alternativeLocations: Array<{ galaxy: number; system: number; position: number; score: number }> = [];
      
      if (useCustomLocation && customGalaxy && customSystem) {
        // Use custom location if specified
        placement = {
          galaxy: customGalaxy,
          system: customSystem,
          position: await this.findBestPositionInSystem(client, customGalaxy, customSystem)
        };
        
        qualityScore = await this.calculateLocationQuality(client, placement, universeId);
      } else {
        // Use intelligent placement algorithm
        const placementResult = await this.findOptimalPlacement(
          client,
          universeId,
          userId,
          preferredPlaystyle,
          allianceId
        );
        
        placement = placementResult.best;
        qualityScore = placementResult.score;
        alternativeLocations = placementResult.alternatives;
      }
      
      // Get player information
      const userResult = await client.query(
        'SELECT * FROM users WHERE id = $1',
        [userId]
      );
      
      const user = userResult.rows[0];
      
      // Create placement record
      const placementInsert = await client.query(
        `INSERT INTO player_placements (
          universe_id, user_id, galaxy, system, position,
          placement_strategy, player_level_at_placement,
          player_experience_at_placement, preferred_playstyle,
          alliance_id, was_grouped_placement,
          starting_metal, starting_crystal, starting_deuterium,
          placement_quality_score
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
        RETURNING *`,
        [
          universeId, userId, placement.galaxy, placement.system, placement.position,
          allianceId ? 'alliance_grouped' : 'skill_based',
          user.level || 1,
          user.experience || 0,
          preferredPlaystyle,
          allianceId,
          !!allianceId,
          universe.starting_resources_metal,
          universe.starting_resources_crystal,
          universe.starting_resources_deuterium,
          qualityScore
        ]
      );
      
      const playerPlacement = this.mapPlayerPlacement(placementInsert.rows[0]);
      
      // Update universe player count
      await client.query(
        'UPDATE universe_seeds SET current_players = current_players + 1 WHERE id = $1',
        [universeId]
      );
      
      // Update galaxy player count
      await client.query(
        'UPDATE galaxy_seeds SET current_players = current_players + 1 WHERE universe_id = $1 AND galaxy_number = $2',
        [universeId, placement.galaxy]
      );
      
      await client.query('COMMIT');
      
      return {
        success: true,
        placement: playerPlacement,
        qualityScore,
        alternativeLocations,
        message: `Player placed at [${placement.galaxy}:${placement.system}:${placement.position}] with quality score ${qualityScore.toFixed(2)}`
      };
      
    } catch (error) {
      await client.query('ROLLBACK');
      console.error('Error placing player:', error);
      return {
        success: false,
        qualityScore: 0,
        message: error instanceof Error ? error.message : 'Failed to place player'
      };
    } finally {
      client.release();
    }
  }
  
  // =====================================================
  // PLACEMENT ALGORITHMS
  // =====================================================
  
  /**
   * Find optimal placement location using scoring algorithm
   */
  private async findOptimalPlacement(
    client: any,
    universeId: number,
    userId: number,
    playstyle: string,
    allianceId?: number
  ): Promise<{ best: Coordinates; score: number; alternatives: Array<{ galaxy: number; system: number; position: number; score: number }> }> {
    
    // Get available galaxies
    const galaxiesResult = await client.query(
      `SELECT * FROM galaxy_seeds 
       WHERE universe_id = $1 
       AND is_generated = TRUE
       AND current_players < max_players_per_galaxy
       ORDER BY galaxy_number`,
      [universeId]
    );
    
    const candidateLocations: LocationScore[] = [];
    
    // Evaluate locations across multiple galaxies
    for (const galaxy of galaxiesResult.rows) {
      // Focus on beginner galaxies (1-3) for new players
      if (galaxy.galaxy_number <= 3) {
        // Sample systems in this galaxy
        const systems = this.getSampleSystems(galaxy.system_count, 20);
        
        for (const system of systems) {
          const location: Coordinates = {
            galaxy: galaxy.galaxy_number,
            system,
            position: Math.floor(Math.random() * 15) + 1
          };
          
          const score = await this.scoreLocation(client, location, universeId, playstyle, allianceId);
          
          candidateLocations.push({
            coordinates: location,
            totalScore: score.totalScore,
            resourceScore: score.resourceScore,
            distanceScore: score.distanceScore,
            competitionScore: score.competitionScore,
            strategicScore: score.strategicScore
          });
        }
      }
    }
    
    // Sort by total score
    candidateLocations.sort((a, b) => b.totalScore - a.totalScore);
    
    // Get best location
    const best = candidateLocations[0];
    
    // Get top 5 alternatives
    const alternatives = candidateLocations.slice(1, 6).map(loc => ({
      galaxy: loc.coordinates.galaxy,
      system: loc.coordinates.system,
      position: loc.coordinates.position,
      score: loc.totalScore
    }));
    
    return {
      best: best.coordinates,
      score: best.totalScore,
      alternatives
    };
  }
  
  /**
   * Score a potential placement location
   */
  private async scoreLocation(
    client: any,
    location: Coordinates,
    universeId: number,
    playstyle: string,
    allianceId?: number
  ): Promise<{
    totalScore: number;
    resourceScore: number;
    distanceScore: number;
    competitionScore: number;
    strategicScore: number;
  }> {
    
    // Resource richness score (0-30 points)
    const resourceScore = await this.calculateResourceScore(client, location);
    
    // Distance from center score (0-25 points)
    const distanceScore = this.calculateDistanceScore(location);
    
    // Competition score (0-25 points)
    const competitionScore = await this.calculateCompetitionScore(client, location, universeId);
    
    // Strategic value score (0-20 points)
    const strategicScore = await this.calculateStrategicScore(client, location, playstyle);
    
    // Total score (0-100)
    const totalScore = resourceScore + distanceScore + competitionScore + strategicScore;
    
    return {
      totalScore,
      resourceScore,
      distanceScore,
      competitionScore,
      strategicScore
    };
  }
  
  /**
   * Calculate resource richness score
   */
  private async calculateResourceScore(client: any, location: Coordinates): Promise<number> {
    // Check if planet resources exist for this location
    const resourceResult = await client.query(
      `SELECT * FROM planet_resources 
       WHERE galaxy = $1 AND system = $2 
       LIMIT 5`,
      [location.galaxy, location.system]
    );
    
    if (resourceResult.rows.length === 0) {
      return 20; // Default moderate score if no data
    }
    
    const avgRichness = resourceResult.rows.reduce((sum: number, row: any) => {
      return sum + parseFloat(row.metal_richness) + parseFloat(row.crystal_richness) + parseFloat(row.deuterium_richness);
    }, 0) / resourceResult.rows.length;
    
    // Scale to 0-30 points
    return Math.min(30, avgRichness * 10);
  }
  
  /**
   * Calculate distance from galaxy center score
   */
  private calculateDistanceScore(location: Coordinates): number {
    // Prefer mid-range systems (not too close to center, not too far)
    const centerSystem = 250;
    const distance = Math.abs(location.system - centerSystem);
    
    // Best score at 100-200 systems from center
    if (distance >= 100 && distance <= 200) {
      return 25;
    } else if (distance < 100) {
      return 15;
    } else {
      return Math.max(0, 25 - (distance - 200) / 10);
    }
  }
  
  /**
   * Calculate competition score (fewer nearby players = better)
   */
  private async calculateCompetitionScore(
    client: any,
    location: Coordinates,
    universeId: number
  ): Promise<number> {
    const searchRadius = 50; // Systems
    
    const competitorResult = await client.query(
      `SELECT COUNT(*) as count FROM player_placements
       WHERE universe_id = $1
       AND galaxy = $2
       AND ABS(system - $3) < $4`,
      [universeId, location.galaxy, location.system, searchRadius]
    );
    
    const competitorCount = parseInt(competitorResult.rows[0].count);
    
    // Fewer competitors = higher score
    return Math.max(0, 25 - (competitorCount * 2.5));
  }
  
  /**
   * Calculate strategic value score based on playstyle
   */
  private async calculateStrategicScore(
    client: any,
    location: Coordinates,
    playstyle: string
  ): Promise<number> {
    // Get sector information
    const sectorResult = await client.query(
      `SELECT sc.* FROM sector_configurations sc
       JOIN galaxy_seeds gs ON sc.galaxy_id = gs.id
       WHERE gs.galaxy_number = $1
       AND $2 BETWEEN sc.system_start AND sc.system_end
       LIMIT 1`,
      [location.galaxy, location.system]
    );
    
    if (sectorResult.rows.length === 0) {
      return 10; // Default moderate score
    }
    
    const sector = sectorResult.rows[0];
    let score = 10;
    
    // Adjust based on playstyle
    if (playstyle === 'military' || playstyle === 'aggressive') {
      // Prefer PVP zones
      if (sector.is_pvp_zone) score += 10;
    } else if (playstyle === 'economic' || playstyle === 'peaceful') {
      // Prefer safe zones
      if (sector.is_safe_zone) score += 10;
    } else if (playstyle === 'explorer') {
      // Prefer outer systems
      if (location.system > 300) score += 10;
    }
    
    return score;
  }
  
  /**
   * Find best available position in a system
   */
  private async findBestPositionInSystem(client: any, galaxy: number, system: number): Promise<number> {
    // Get occupied positions in this system
    const occupiedResult = await client.query(
      `SELECT position FROM player_placements
       WHERE galaxy = $1 AND system = $2
       UNION
       SELECT position FROM generated_bots
       WHERE galaxy = $1 AND system = $2 AND is_active = TRUE`,
      [galaxy, system]
    );
    
    const occupiedPositions = new Set(occupiedResult.rows.map((row: any) => row.position));
    
    // Find first available position (1-15)
    for (let pos = 1; pos <= 15; pos++) {
      if (!occupiedPositions.has(pos)) {
        return pos;
      }
    }
    
    // If all occupied, return random position (will be handled by planet creation)
    return Math.floor(Math.random() * 15) + 1;
  }
  
  /**
   * Calculate location quality using database function
   */
  private async calculateLocationQuality(
    client: any,
    location: Coordinates,
    universeId: number
  ): Promise<number> {
    const result = await client.query(
      'SELECT calculate_placement_quality($1, $2, $3, $4) as quality',
      [location.galaxy, location.system, location.position, universeId]
    );
    
    return parseFloat(result.rows[0].quality) || 50;
  }
  
  // =====================================================
  // QUERY METHODS
  // =====================================================
  
  /**
   * Get player placement
   */
  async getPlayerPlacement(userId: number, universeId: number): Promise<PlayerPlacement | null> {
    const result = await pool.query(
      'SELECT * FROM player_placements WHERE user_id = $1 AND universe_id = $2',
      [userId, universeId]
    );
    
    return result.rows.length > 0 ? this.mapPlayerPlacement(result.rows[0]) : null;
  }
  
  /**
   * Get all placements in universe
   */
  async getUniversePlacements(universeId: number): Promise<PlayerPlacement[]> {
    const result = await pool.query(
      'SELECT * FROM player_placements WHERE universe_id = $1 ORDER BY placed_at DESC',
      [universeId]
    );
    
    return result.rows.map(row => this.mapPlayerPlacement(row));
  }
  
  /**
   * Get placements in galaxy
   */
  async getGalaxyPlacements(universeId: number, galaxy: number): Promise<PlayerPlacement[]> {
    const result = await pool.query(
      'SELECT * FROM player_placements WHERE universe_id = $1 AND galaxy = $2',
      [universeId, galaxy]
    );
    
    return result.rows.map(row => this.mapPlayerPlacement(row));
  }
  
  // =====================================================
  // UTILITY METHODS
  // =====================================================
  
  /**
   * Get sample systems for evaluation
   */
  private getSampleSystems(totalSystems: number, sampleSize: number): number[] {
    const systems: number[] = [];
    const step = Math.floor(totalSystems / sampleSize);
    
    for (let i = 0; i < sampleSize; i++) {
      systems.push(Math.min(totalSystems, (i * step) + Math.floor(Math.random() * step)));
    }
    
    return systems;
  }
  
  private mapPlayerPlacement(row: any): PlayerPlacement {
    return {
      id: row.id,
      universeId: row.universe_id,
      userId: row.user_id,
      galaxy: row.galaxy,
      system: row.system,
      position: row.position,
      placementStrategy: row.placement_strategy,
      placementRuleId: row.placement_rule_id,
      playerLevelAtPlacement: row.player_level_at_placement,
      playerExperienceAtPlacement: parseInt(row.player_experience_at_placement),
      preferredPlaystyle: row.preferred_playstyle,
      allianceId: row.alliance_id,
      wasGroupedPlacement: row.was_grouped_placement,
      startingMetal: parseInt(row.starting_metal),
      startingCrystal: parseInt(row.starting_crystal),
      startingDeuterium: parseInt(row.starting_deuterium),
      placementQualityScore: parseFloat(row.placement_quality_score),
      resourceRichnessScore: parseFloat(row.resource_richness_score),
      strategicValueScore: parseFloat(row.strategic_value_score),
      placedAt: row.placed_at
    };
  }
}

export default new PlayerPlacementService();
