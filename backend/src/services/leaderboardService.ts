import { Pool } from 'pg';
import Redis from 'ioredis';

/**
 * Interface representing a player's leaderboard entry
 */
export interface PlayerScore {
  userId: number;
  username: string;
  totalScore: number;
  buildingScore: number;
  researchScore: number;
  fleetScore: number;
  defenseScore: number;
  rank: number;
  allianceTag?: string;
}

/**
 * Interface representing an alliance's leaderboard entry
 */
export interface AllianceScore {
  allianceId: number;
  allianceName: string;
  allianceTag: string;
  totalScore: number;
  memberCount: number;
  averageScore: number;
  rank: number;
}

/**
 * Leaderboard Service
 *
 * Handles all player and alliance ranking operations with Redis caching
 * for optimal performance. Scores are calculated based on:
 * - Building levels (construction cost * 1.0)
 * - Research levels (research cost * 1.0)
 * - Fleet units (unit cost * 1.0)
 * - Defense units (unit cost * 1.0)
 *
 * @class LeaderboardService
 */
export class LeaderboardService {
  private db: Pool;
  private redis: Redis;
  private readonly CACHE_TTL = 300; // 5 minutes
  private readonly PLAYER_LEADERBOARD_KEY = 'leaderboard:players';
  private readonly ALLIANCE_LEADERBOARD_KEY = 'leaderboard:alliances';

  /**
   * Creates an instance of LeaderboardService
   *
   * @param {Pool} db - PostgreSQL connection pool
   * @param {Redis} redis - Redis client for caching
   */
  constructor(db: Pool, redis: Redis) {
    this.db = db;
    this.redis = redis;
  }

  /**
   * Rebuild both player and alliance leaderboards.
   */
  async rebuildLeaderboards(): Promise<{
    playersUpdated: number;
    alliancesUpdated: number;
  }> {
    const playersUpdated = await this.updatePlayerLeaderboard();
    const alliancesUpdated = await this.updateAllianceLeaderboard();
    return { playersUpdated, alliancesUpdated };
  }

  /**
   * Calculate the total score for a specific player
   *
   * Aggregates scores from all planets owned by the player,
   * including buildings, research, fleet, and defense contributions.
   *
   * @param {number} userId - The ID of the user
   * @returns {Promise<PlayerScore>} The calculated player score breakdown
   * @throws {Error} If database query fails
   *
   * @example
   * const score = await leaderboardService.calculatePlayerScore(123);
   * console.log(`Total score: ${score.totalScore}`);
   */
  async calculatePlayerScore(userId: number): Promise<PlayerScore> {
    try {
      // Get user info
      const userQuery = await this.db.query(
        `SELECT u.id, u.username, a.tag as alliance_tag
         FROM users u
         LEFT JOIN alliances a ON u.alliance_id = a.id
         WHERE u.id = $1`,
        [userId]
      );

      if (userQuery.rows.length === 0) {
        throw new Error(`User ${userId} not found`);
      }

      const user = userQuery.rows[0];

      // Calculate building score
      const buildingScore = await this.calculateBuildingScore(userId);

      // Calculate research score
      const researchScore = await this.calculateResearchScore(userId);

      // Calculate fleet score
      const fleetScore = await this.calculateFleetScore(userId);

      // Calculate defense score
      const defenseScore = await this.calculateDefenseScore(userId);

      const totalScore =
        buildingScore + researchScore + fleetScore + defenseScore;

      return {
        userId: user.id,
        username: user.username,
        totalScore,
        buildingScore,
        researchScore,
        fleetScore,
        defenseScore,
        rank: 0, // Will be set when retrieving leaderboard
        allianceTag: user.alliance_tag || undefined,
      };
    } catch (error) {
      console.error('Error calculating player score:', error);
      throw error;
    }
  }

  /**
   * Calculate building score for a user
   *
   * @private
   * @param {number} userId - The ID of the user
   * @returns {Promise<number>} Total building score
   */
  private async calculateBuildingScore(userId: number): Promise<number> {
    const query = `
      SELECT 
        SUM(
          CASE 
            WHEN building_type = 'metal_mine' THEN metal_mine_level * 60 * POWER(1.5, metal_mine_level - 1)
            WHEN building_type = 'crystal_mine' THEN crystal_mine_level * 48 * POWER(1.6, crystal_mine_level - 1)
            WHEN building_type = 'deuterium_synthesizer' THEN deuterium_synthesizer_level * 225 * POWER(1.5, deuterium_synthesizer_level - 1)
            WHEN building_type = 'solar_plant' THEN solar_plant_level * 75 * POWER(1.5, solar_plant_level - 1)
            WHEN building_type = 'robotics_factory' THEN robotics_factory_level * 400 * POWER(2, robotics_factory_level - 1)
            WHEN building_type = 'shipyard' THEN shipyard_level * 400 * POWER(2, shipyard_level - 1)
            WHEN building_type = 'research_lab' THEN research_lab_level * 200 * POWER(2, research_lab_level - 1)
            WHEN building_type = 'alliance_depot' THEN alliance_depot_level * 20000 * POWER(2, alliance_depot_level - 1)
            WHEN building_type = 'missile_silo' THEN missile_silo_level * 20000 * POWER(2, missile_silo_level - 1)
            WHEN building_type = 'nanite_factory' THEN nanite_factory_level * 1000000 * POWER(2, nanite_factory_level - 1)
            WHEN building_type = 'terraformer' THEN terraformer_level * 0 * POWER(2, terraformer_level - 1)
            WHEN building_type = 'space_dock' THEN space_dock_level * 200 * POWER(5, space_dock_level - 1)
            ELSE 0
          END
        ) as building_score
      FROM planets
      WHERE user_id = $1
    `;

    const result = await this.db.query(query, [userId]);
    return Math.floor(result.rows[0]?.building_score || 0);
  }

  /**
   * Calculate research score for a user
   *
   * @private
   * @param {number} userId - The ID of the user
   * @returns {Promise<number>} Total research score
   */
  private async calculateResearchScore(userId: number): Promise<number> {
    const query = `
      SELECT technology, level
      FROM research
      WHERE user_id = $1
    `;

    const result = await this.db.query(query, [userId]);
    let totalScore = 0;

    // Technology base costs (simplified)
    const techCosts: { [key: string]: { metal: number; crystal: number; deuterium: number } } = {
      energy_technology: { metal: 0, crystal: 800, deuterium: 400 },
      laser_technology: { metal: 200, crystal: 100, deuterium: 0 },
      ion_technology: { metal: 1000, crystal: 300, deuterium: 100 },
      hyperspace_technology: { metal: 0, crystal: 4000, deuterium: 2000 },
      plasma_technology: { metal: 2000, crystal: 4000, deuterium: 1000 },
      combustion_drive: { metal: 400, crystal: 0, deuterium: 600 },
      impulse_drive: { metal: 2000, crystal: 4000, deuterium: 600 },
      hyperspace_drive: { metal: 10000, crystal: 20000, deuterium: 6000 },
      espionage_technology: { metal: 200, crystal: 1000, deuterium: 200 },
      computer_technology: { metal: 0, crystal: 400, deuterium: 600 },
      astrophysics: { metal: 4000, crystal: 8000, deuterium: 4000 },
      weapons_technology: { metal: 800, crystal: 200, deuterium: 0 },
      shielding_technology: { metal: 200, crystal: 600, deuterium: 0 },
      armor_technology: { metal: 1000, crystal: 0, deuterium: 0 },
    };

    result.rows.forEach((row) => {
      const baseCost = techCosts[row.technology];
      if (baseCost) {
        const level = row.level;
        const cost =
          (baseCost.metal + baseCost.crystal + baseCost.deuterium) *
          Math.pow(2, level - 1);
        totalScore += cost;
      }
    });

    return Math.floor(totalScore);
  }

  /**
   * Calculate fleet score for a user
   *
   * @private
   * @param {number} userId - The ID of the user
   * @returns {Promise<number>} Total fleet score
   */
  private async calculateFleetScore(userId: number): Promise<number> {
    const query = `
      SELECT 
        small_cargo, large_cargo, light_fighter, heavy_fighter,
        cruiser, battleship, colony_ship, recycler,
        espionage_probe, bomber, destroyer, deathstar
      FROM planets
      WHERE user_id = $1
    `;

    const result = await this.db.query(query, [userId]);
    let totalScore = 0;

    // Ship costs
    const shipCosts: { [key: string]: number } = {
      small_cargo: 4000,
      large_cargo: 12000,
      light_fighter: 3000,
      heavy_fighter: 6000,
      cruiser: 20000,
      battleship: 45000,
      colony_ship: 10000,
      recycler: 10000,
      espionage_probe: 1000,
      bomber: 50000,
      destroyer: 60000,
      deathstar: 5000000,
    };

    result.rows.forEach((planet) => {
      Object.keys(shipCosts).forEach((shipType) => {
        const count = planet[shipType] || 0;
        totalScore += count * shipCosts[shipType];
      });
    });

    return Math.floor(totalScore);
  }

  /**
   * Calculate defense score for a user
   *
   * @private
   * @param {number} userId - The ID of the user
   * @returns {Promise<number>} Total defense score
   */
  private async calculateDefenseScore(userId: number): Promise<number> {
    const query = `
      SELECT 
        rocket_launcher, light_laser, heavy_laser, gauss_cannon,
        ion_cannon, plasma_turret, small_shield_dome, large_shield_dome
      FROM planets
      WHERE user_id = $1
    `;

    const result = await this.db.query(query, [userId]);
    let totalScore = 0;

    // Defense costs
    const defenseCosts: { [key: string]: number } = {
      rocket_launcher: 2000,
      light_laser: 1500,
      heavy_laser: 6000,
      gauss_cannon: 20000,
      ion_cannon: 2000,
      plasma_turret: 50000,
      small_shield_dome: 10000,
      large_shield_dome: 50000,
    };

    result.rows.forEach((planet) => {
      Object.keys(defenseCosts).forEach((defenseType) => {
        const count = planet[defenseType] || 0;
        totalScore += count * defenseCosts[defenseType];
      });
    });

    return Math.floor(totalScore);
  }

  /**
   * Update the player leaderboard in Redis
   *
   * Recalculates all player scores and stores them in a Redis sorted set
   * for fast retrieval. This operation should be run periodically (e.g., every 5 minutes).
   *
   * @returns {Promise<number>} Number of players updated
   * @throws {Error} If update operation fails
   *
   * @example
   * const count = await leaderboardService.updatePlayerLeaderboard();
   * console.log(`Updated ${count} players`);
   */
  async updatePlayerLeaderboard(): Promise<number> {
    try {
      // Get all active users
      const usersQuery = await this.db.query(
        'SELECT id FROM users WHERE created_at > NOW() - INTERVAL \'30 days\''
      );

      const pipeline = this.redis.pipeline();

      // Clear existing leaderboard
      pipeline.del(this.PLAYER_LEADERBOARD_KEY);

      // Calculate and add scores for each player
      for (const user of usersQuery.rows) {
        try {
          const score = await this.calculatePlayerScore(user.id);
          pipeline.zadd(
            this.PLAYER_LEADERBOARD_KEY,
            score.totalScore,
            JSON.stringify({
              userId: score.userId,
              username: score.username,
              totalScore: score.totalScore,
              allianceTag: score.allianceTag,
            })
          );
        } catch (error) {
          console.error(`Error calculating score for user ${user.id}:`, error);
        }
      }

      // Set expiry
      pipeline.expire(this.PLAYER_LEADERBOARD_KEY, this.CACHE_TTL);

      await pipeline.exec();

      return usersQuery.rows.length;
    } catch (error) {
      console.error('Error updating player leaderboard:', error);
      throw error;
    }
  }

  /**
   * Get top N players from the leaderboard
   *
   * @param {number} limit - Number of top players to retrieve (default: 100)
   * @param {number} offset - Starting position (default: 0)
   * @returns {Promise<PlayerScore[]>} Array of top players with rankings
   *
   * @example
   * const top10 = await leaderboardService.getTopPlayers(10);
   * top10.forEach(player => {
   *   console.log(`${player.rank}. ${player.username}: ${player.totalScore}`);
   * });
   */
  async getTopPlayers(limit: number = 100, offset: number = 0): Promise<PlayerScore[]> {
    try {
      // Check if leaderboard exists in cache
      const exists = await this.redis.exists(this.PLAYER_LEADERBOARD_KEY);

      if (!exists) {
        // Update leaderboard if not in cache
        await this.updatePlayerLeaderboard();
      }

      // Get top players (sorted by score descending)
      const players = await this.redis.zrevrange(
        this.PLAYER_LEADERBOARD_KEY,
        offset,
        offset + limit - 1,
        'WITHSCORES'
      );

      const result: PlayerScore[] = [];

      for (let i = 0; i < players.length; i += 2) {
        const playerData = JSON.parse(players[i]);
        const score = parseInt(players[i + 1]);

        result.push({
          ...playerData,
          totalScore: score,
          rank: offset + (i / 2) + 1,
          buildingScore: 0, // Can be added if needed
          researchScore: 0,
          fleetScore: 0,
          defenseScore: 0,
        });
      }

      return result;
    } catch (error) {
      console.error('Error getting top players:', error);
      throw error;
    }
  }

  /**
   * Get player rank and surrounding players
   *
   * @param {number} userId - The ID of the user
   * @param {number} range - Number of players above and below to include (default: 5)
   * @returns {Promise<{ player: PlayerScore; neighbors: PlayerScore[] }>} Player and neighbors
   *
   * @example
   * const result = await leaderboardService.getPlayerRank(123, 5);
   * console.log(`Your rank: ${result.player.rank}`);
   */
  async getPlayerRank(
    userId: number,
    range: number = 5
  ): Promise<{ player: PlayerScore; neighbors: PlayerScore[] }> {
    try {
      const playerScore = await this.calculatePlayerScore(userId);

      // Get player rank from Redis
      const rank = await this.redis.zrevrank(
        this.PLAYER_LEADERBOARD_KEY,
        JSON.stringify({
          userId: playerScore.userId,
          username: playerScore.username,
          totalScore: playerScore.totalScore,
          allianceTag: playerScore.allianceTag,
        })
      );

      playerScore.rank = (rank ?? 0) + 1;

      // Get surrounding players
      const start = Math.max(0, (rank ?? 0) - range);
      const end = (rank ?? 0) + range;

      const neighbors = await this.getTopPlayers(end - start + 1, start);

      return {
        player: playerScore,
        neighbors,
      };
    } catch (error) {
      console.error('Error getting player rank:', error);
      throw error;
    }
  }

  /**
   * Calculate and update alliance leaderboard
   *
   * @returns {Promise<number>} Number of alliances updated
   * @throws {Error} If update operation fails
   */
  async updateAllianceLeaderboard(): Promise<number> {
    try {
      const alliancesQuery = await this.db.query(`
        SELECT 
          a.id,
          a.name,
          a.tag,
          COUNT(u.id) as member_count
        FROM alliances a
        LEFT JOIN users u ON u.alliance_id = a.id
        GROUP BY a.id, a.name, a.tag
        HAVING COUNT(u.id) > 0
      `);

      const pipeline = this.redis.pipeline();
      pipeline.del(this.ALLIANCE_LEADERBOARD_KEY);

      for (const alliance of alliancesQuery.rows) {
        // Get all member scores
        const membersQuery = await this.db.query(
          'SELECT id FROM users WHERE alliance_id = $1',
          [alliance.id]
        );

        let totalScore = 0;
        for (const member of membersQuery.rows) {
          try {
            const score = await this.calculatePlayerScore(member.id);
            totalScore += score.totalScore;
          } catch (error) {
            console.error(`Error calculating score for member ${member.id}:`, error);
          }
        }

        const averageScore = alliance.member_count > 0 ? totalScore / alliance.member_count : 0;

        pipeline.zadd(
          this.ALLIANCE_LEADERBOARD_KEY,
          totalScore,
          JSON.stringify({
            allianceId: alliance.id,
            allianceName: alliance.name,
            allianceTag: alliance.tag,
            totalScore,
            memberCount: alliance.member_count,
            averageScore: Math.floor(averageScore),
          })
        );
      }

      pipeline.expire(this.ALLIANCE_LEADERBOARD_KEY, this.CACHE_TTL);
      await pipeline.exec();

      return alliancesQuery.rows.length;
    } catch (error) {
      console.error('Error updating alliance leaderboard:', error);
      throw error;
    }
  }

  /**
   * Get top N alliances from the leaderboard
   *
   * @param {number} limit - Number of top alliances to retrieve (default: 50)
   * @param {number} offset - Starting position (default: 0)
   * @returns {Promise<AllianceScore[]>} Array of top alliances with rankings
   */
  async getTopAlliances(limit: number = 50, offset: number = 0): Promise<AllianceScore[]> {
    try {
      const exists = await this.redis.exists(this.ALLIANCE_LEADERBOARD_KEY);

      if (!exists) {
        await this.updateAllianceLeaderboard();
      }

      const alliances = await this.redis.zrevrange(
        this.ALLIANCE_LEADERBOARD_KEY,
        offset,
        offset + limit - 1,
        'WITHSCORES'
      );

      const result: AllianceScore[] = [];

      for (let i = 0; i < alliances.length; i += 2) {
        const allianceData = JSON.parse(alliances[i]);
        const score = parseInt(alliances[i + 1]);

        result.push({
          ...allianceData,
          totalScore: score,
          rank: offset + (i / 2) + 1,
        });
      }

      return result;
    } catch (error) {
      console.error('Error getting top alliances:', error);
      throw error;
    }
  }
}
