/**
 * @module backend/services/leaderboardService
 *
 * Leaderboard calculations and cache management. Provides APIs to rebuild
 * leaderboards, calculate player/alliance scores and interact with cached
 * ranking data in Redis. Designed for periodic rebuild jobs as well as
 * on-demand score retrieval.
 */

import { Pool } from 'pg';
import Redis from 'ioredis';

/**
 * Interface representing a player's leaderboard entry
 */
export interface PlayerScore {
  userId: number;
  username: string;
  allianceId?: number;
  totalScore: number;
  buildingScore: number;
  researchScore: number;
  fleetScore: number;
  defenseScore: number;
  rank: number;
  allianceTag?: string;
  scoreTrend?: Array<{ timestamp: string; score: number }>;
  weeklyRankChange?: number | null;
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
  scoreTrend?: Array<{ timestamp: string; score: number }>;
  weeklyRankChange?: number | null;
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
  private readonly CACHE_TTL = 600; // 10 minutes
  private readonly CACHE_TTL_MS = 10 * 60 * 1000;
  private readonly PLAYER_LEADERBOARD_KEY = 'leaderboard:players';
  private readonly ALLIANCE_LEADERBOARD_KEY = 'leaderboard:alliances';
  private readonly ALLIANCE_LOOKUP_KEY = 'leaderboard:alliances:lookup';
  private readonly META_KEY = 'leaderboard:meta';
  private readonly ALLIANCE_MEMBER_PREFIX = 'leaderboard:alliance_members:';

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
        `SELECT u.id, u.username, u.alliance_id, a.tag as alliance_tag
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
        allianceId: user.alliance_id || undefined,
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
      const snapshotRows: Array<{
        userId: number;
        username: string;
        totalScore: number;
        buildingScore: number;
        researchScore: number;
        fleetScore: number;
        defenseScore: number;
        allianceTag?: string;
      }> = [];

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
              allianceId: score.allianceId,
              allianceTag: score.allianceTag,
              buildingScore: score.buildingScore,
              researchScore: score.researchScore,
              fleetScore: score.fleetScore,
              defenseScore: score.defenseScore,
            })
          );
          snapshotRows.push({
            userId: score.userId,
            username: score.username,
            totalScore: score.totalScore,
            buildingScore: score.buildingScore,
            researchScore: score.researchScore,
            fleetScore: score.fleetScore,
            defenseScore: score.defenseScore,
            allianceTag: score.allianceTag,
          });
        } catch (error) {
          console.error(`Error calculating score for user ${user.id}:`, error);
        }
      }

      // Set expiry
      pipeline.expire(this.PLAYER_LEADERBOARD_KEY, this.CACHE_TTL);

      await pipeline.exec();
      await this.persistPlayerSnapshots(snapshotRows);
      await this.redis.hset(
        this.META_KEY,
        'players_last_build',
        Date.now().toString()
      );

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
      await this.ensurePlayerCacheFresh();

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
          userId: playerData.userId,
          username: playerData.username,
          allianceId: playerData.allianceId,
          allianceTag: playerData.allianceTag,
          totalScore: score,
          rank: offset + (i / 2) + 1,
          buildingScore: playerData.buildingScore ?? 0,
          researchScore: playerData.researchScore ?? 0,
          fleetScore: playerData.fleetScore ?? 0,
          defenseScore: playerData.defenseScore ?? 0,
        });
      }

      await this.attachScoreTrends(result);
      await this.appendWeeklyRankChanges(result);

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
      pipeline.del(this.ALLIANCE_LOOKUP_KEY);
      const snapshotRows: Array<{
        allianceId: number;
        allianceName: string;
        allianceTag: string;
        totalScore: number;
        memberCount: number;
        averageScore: number;
      }> = [];

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

        const memberValue = this.getAllianceMemberValue(alliance.id);
        pipeline.zadd(
          this.ALLIANCE_LEADERBOARD_KEY,
          totalScore,
          memberValue
        );
        pipeline.hset(
          this.ALLIANCE_LOOKUP_KEY,
          alliance.id.toString(),
          JSON.stringify({
            allianceId: alliance.id,
            allianceName: alliance.name,
            allianceTag: alliance.tag,
            totalScore,
            memberCount: alliance.member_count,
            averageScore: Math.floor(averageScore),
          })
        );
        snapshotRows.push({
          allianceId: alliance.id,
          allianceName: alliance.name,
          allianceTag: alliance.tag,
          totalScore,
          memberCount: Number(alliance.member_count) || 0,
          averageScore: Math.floor(averageScore),
        });
      }

      pipeline.expire(this.ALLIANCE_LEADERBOARD_KEY, this.CACHE_TTL);
      pipeline.expire(this.ALLIANCE_LOOKUP_KEY, this.CACHE_TTL);
      await pipeline.exec();
      await this.persistAllianceSnapshots(snapshotRows);
      await this.redis.hset(
        this.META_KEY,
        'alliances_last_build',
        Date.now().toString()
      );

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
      await this.ensureAllianceCacheFresh();

      const alliances = await this.redis.zrevrange(
        this.ALLIANCE_LEADERBOARD_KEY,
        offset,
        offset + limit - 1,
        'WITHSCORES'
      );

      const result: AllianceScore[] = [];
      const allianceIds: number[] = [];
      const scores: number[] = [];

      for (let i = 0; i < alliances.length; i += 2) {
        const memberValue = alliances[i];
        const score = parseInt(alliances[i + 1]);
        const allianceId = this.parseAllianceMemberValue(memberValue);
        if (!allianceId) continue;
        allianceIds.push(allianceId);
        scores.push(score);
      }

      if (allianceIds.length) {
        const lookupValues = await this.redis.hmget(
          this.ALLIANCE_LOOKUP_KEY,
          ...allianceIds.map(String)
        );

        lookupValues.forEach((raw, idx) => {
          if (!raw) return;
          const data = JSON.parse(raw);
          result.push({
            ...data,
            totalScore: scores[idx],
            rank: offset + idx + 1,
          });
        });
      }

      await this.attachAllianceScoreTrends(result);
      await this.appendAllianceWeeklyRankChanges(result);

      return result;
    } catch (error) {
      console.error('Error getting top alliances:', error);
      throw error;
    }
  }

  async getAllianceDetails(
    allianceId: number,
    options: { limit?: number; offset?: number } = {}
  ): Promise<{ alliance: AllianceScore; members: PlayerScore[] }> {
    await this.ensureAllianceCacheFresh();

    const raw = await this.redis.hget(
      this.ALLIANCE_LOOKUP_KEY,
      allianceId.toString()
    );

    if (!raw) {
      // Attempt rebuild once
      await this.updateAllianceLeaderboard();
    }

    const refreshedRaw = raw || await this.redis.hget(
      this.ALLIANCE_LOOKUP_KEY,
      allianceId.toString()
    );

    if (!refreshedRaw) {
      throw new Error('Alliance not found on leaderboard');
    }

    const allianceData = JSON.parse(refreshedRaw);
    const memberValue = this.getAllianceMemberValue(allianceId);
    const rank =
      ((await this.redis.zrevrank(this.ALLIANCE_LEADERBOARD_KEY, memberValue)) ??
        0) + 1;

    const members = await this.getAllianceMembers(
      allianceId,
      options.limit ?? 25,
      options.offset ?? 0
    );

    return {
      alliance: {
        ...allianceData,
        totalScore: allianceData.totalScore,
        rank,
      },
      members,
    };
  }

  async getAllianceMembers(
    allianceId: number,
    limit: number = 25,
    offset: number = 0
  ): Promise<PlayerScore[]> {
    const key = this.getAllianceMemberKey(allianceId);
    const exists = await this.redis.exists(key);
    if (!exists || (await this.redis.ttl(key)) <= 0) {
      await this.buildAllianceMemberLeaderboard(allianceId);
    }

    const members = await this.redis.zrevrange(
      key,
      offset,
      offset + limit - 1,
      'WITHSCORES'
    );

    const result: PlayerScore[] = [];
    for (let i = 0; i < members.length; i += 2) {
      const player = JSON.parse(members[i]);
      const score = parseInt(members[i + 1]);
      result.push({
        ...player,
        totalScore: score,
        rank: offset + (i / 2) + 1,
      });
    }
    return result;
  }

  private async buildAllianceMemberLeaderboard(
    allianceId: number
  ): Promise<void> {
    const key = this.getAllianceMemberKey(allianceId);
    const membersQuery = await this.db.query(
      'SELECT id FROM users WHERE alliance_id = $1',
      [allianceId]
    );

    const pipeline = this.redis.pipeline();
    pipeline.del(key);

    for (const member of membersQuery.rows) {
      try {
        const score = await this.calculatePlayerScore(member.id);
        pipeline.zadd(
          key,
          score.totalScore,
          JSON.stringify({
            userId: score.userId,
            username: score.username,
            allianceId: score.allianceId,
            allianceTag: score.allianceTag,
            buildingScore: score.buildingScore,
            researchScore: score.researchScore,
            fleetScore: score.fleetScore,
            defenseScore: score.defenseScore,
          })
        );
      } catch (error) {
        console.error(
          `[Leaderboards] Failed to calculate score for alliance member ${member.id}`,
          error
        );
      }
    }

    pipeline.expire(key, this.CACHE_TTL);
    await pipeline.exec();
  }

  private async ensurePlayerCacheFresh(): Promise<void> {
    const exists = await this.redis.exists(this.PLAYER_LEADERBOARD_KEY);
    if (!exists || (await this.isCacheStale('players'))) {
      await this.updatePlayerLeaderboard();
    }
  }

  private async ensureAllianceCacheFresh(): Promise<void> {
    const exists = await this.redis.exists(this.ALLIANCE_LEADERBOARD_KEY);
    if (!exists || (await this.isCacheStale('alliances'))) {
      await this.updateAllianceLeaderboard();
    }
  }

  async getCacheMetadata() {
    const meta = await this.redis.hgetall(this.META_KEY);
    const playersTTL = await this.redis.ttl(this.PLAYER_LEADERBOARD_KEY);
    const alliancesTTL = await this.redis.ttl(this.ALLIANCE_LEADERBOARD_KEY);

    const normalize = (timestamp?: string) =>
      timestamp ? new Date(parseInt(timestamp, 10)).toISOString() : null;

    return {
      players: {
        lastBuild: normalize(meta.players_last_build),
        ttlSeconds: playersTTL,
        stale: await this.isCacheStale('players')
      },
      alliances: {
        lastBuild: normalize(meta.alliances_last_build),
        ttlSeconds: alliancesTTL,
        stale: await this.isCacheStale('alliances')
      }
    };
  }

  private getAllianceMemberKey(allianceId: number): string {
    return `${this.ALLIANCE_MEMBER_PREFIX}${allianceId}`;
  }

  private getAllianceMemberValue(allianceId: number): string {
    return `alliance:${allianceId}`;
  }

  private parseAllianceMemberValue(value: string): number | null {
    if (!value?.startsWith('alliance:')) {
      return null;
    }
    const [, idPart] = value.split(':');
    const id = parseInt(idPart, 10);
    return Number.isNaN(id) ? null : id;
  }

  private async isCacheStale(kind: 'players' | 'alliances'): Promise<boolean> {
    const field = `${kind}_last_build`;
    const lastBuild = await this.redis.hget(this.META_KEY, field);
    if (!lastBuild) {
      return true;
    }
    const lastBuildTs = parseInt(lastBuild, 10);
    if (Number.isNaN(lastBuildTs)) {
      return true;
    }
    return Date.now() - lastBuildTs >= this.CACHE_TTL_MS;
  }

  private async persistPlayerSnapshots(rows: Array<{
    userId: number;
    username: string;
    totalScore: number;
    buildingScore: number;
    researchScore: number;
    fleetScore: number;
    defenseScore: number;
    allianceTag?: string;
  }>): Promise<void> {
    if (!rows.length) return;

    const snapshotAt = new Date();
    const columns = 9;
    const values: any[] = [];
    const placeholders = rows
      .map((row, index) => {
        const base = index * columns;
        values.push(
          snapshotAt,
          row.userId,
          row.username,
          row.totalScore,
          row.buildingScore,
          row.researchScore,
          row.fleetScore,
          row.defenseScore,
          row.allianceTag || null
        );
        return `($${base + 1}, $${base + 2}, $${base + 3}, $${base + 4}, $${base + 5}, $${base + 6}, $${base + 7}, $${base + 8}, $${base + 9})`;
      })
      .join(',');

    await this.db.query(
      `INSERT INTO player_leaderboard_snapshots (
        snapshot_at,
        user_id,
        username,
        total_score,
        building_score,
        research_score,
        fleet_score,
        defense_score,
        alliance_tag
      ) VALUES ${placeholders}`,
      values
    );
  }

  private async persistAllianceSnapshots(rows: Array<{
    allianceId: number;
    allianceName: string;
    allianceTag: string;
    totalScore: number;
    memberCount: number;
    averageScore: number;
  }>): Promise<void> {
    if (!rows.length) return;

    const snapshotAt = new Date();
    const columns = 7;
    const values: any[] = [];
    const placeholders = rows
      .map((row, index) => {
        const base = index * columns;
        values.push(
          snapshotAt,
          row.allianceId,
          row.allianceName,
          row.allianceTag,
          row.totalScore,
          row.memberCount,
          row.averageScore
        );
        return `($${base + 1}, $${base + 2}, $${base + 3}, $${base + 4}, $${base + 5}, $${base + 6}, $${base + 7})`;
      })
      .join(',');

    await this.db.query(
      `INSERT INTO alliance_leaderboard_snapshots (
        snapshot_at,
        alliance_id,
        alliance_name,
        alliance_tag,
        total_score,
        member_count,
        average_score
      ) VALUES ${placeholders}`,
      values
    );
  }

  private async attachScoreTrends(players: PlayerScore[], points = 10): Promise<void> {
    if (!players.length) return;
    const userIds = players.map((p) => p.userId);
    const trendResult = await this.db.query(
      `
        SELECT user_id, snapshot_at, total_score
        FROM (
          SELECT
            user_id,
            snapshot_at,
            total_score,
            ROW_NUMBER() OVER (PARTITION BY user_id ORDER BY snapshot_at DESC) AS row_num
          FROM player_leaderboard_snapshots
          WHERE user_id = ANY($1::int[])
        ) ranked
        WHERE row_num <= $2
        ORDER BY user_id, snapshot_at ASC
      `,
      [userIds, points]
    );

    const trendMap = new Map<number, Array<{ timestamp: string; score: number }>>();
    trendResult.rows.forEach((row) => {
      if (!trendMap.has(row.user_id)) {
        trendMap.set(row.user_id, []);
      }
      trendMap.get(row.user_id)!.push({
        timestamp: row.snapshot_at,
        score: Number(row.total_score),
      });
    });

    players.forEach((player) => {
      player.scoreTrend = trendMap.get(player.userId) || [];
    });
  }

  private async appendWeeklyRankChanges(players: PlayerScore[]): Promise<void> {
    if (!players.length) return;
    const userIds = players.map((p) => p.userId);

    const snapshotRef = await this.db.query(
      `
        SELECT snapshot_at
        FROM player_leaderboard_snapshots
        WHERE snapshot_at <= NOW() - INTERVAL '7 days'
        ORDER BY snapshot_at DESC
        LIMIT 1
      `
    );

    if (!snapshotRef.rows.length) {
      players.forEach((player) => (player.weeklyRankChange = null));
      return;
    }

    const referenceDate = snapshotRef.rows[0].snapshot_at;
    const ranksResult = await this.db.query(
      `
        SELECT user_id, rank
        FROM (
          SELECT
            user_id,
            total_score,
            RANK() OVER (ORDER BY total_score DESC) AS rank
          FROM player_leaderboard_snapshots
          WHERE snapshot_at = $1
        ) ranked
        WHERE user_id = ANY($2::int[])
      `,
      [referenceDate, userIds]
    );

    const rankMap = new Map<number, number>();
    ranksResult.rows.forEach((row) => {
      rankMap.set(row.user_id, Number(row.rank));
    });

    players.forEach((player) => {
      const previousRank = rankMap.get(player.userId);
      player.weeklyRankChange =
        typeof previousRank === 'number' ? previousRank - player.rank : null;
    });
  }

  private async attachAllianceScoreTrends(
    alliances: AllianceScore[],
    points = 10
  ): Promise<void> {
    if (!alliances.length) return;
    const allianceIds = alliances.map((a) => a.allianceId);
    const trendResult = await this.db.query(
      `
        SELECT alliance_id, snapshot_at, total_score
        FROM (
          SELECT
            alliance_id,
            snapshot_at,
            total_score,
            ROW_NUMBER() OVER (PARTITION BY alliance_id ORDER BY snapshot_at DESC) AS row_num
          FROM alliance_leaderboard_snapshots
          WHERE alliance_id = ANY($1::int[])
        ) ranked
        WHERE row_num <= $2
        ORDER BY alliance_id, snapshot_at ASC
      `,
      [allianceIds, points]
    );

    const trendMap = new Map<number, Array<{ timestamp: string; score: number }>>();
    trendResult.rows.forEach((row) => {
      if (!trendMap.has(row.alliance_id)) {
        trendMap.set(row.alliance_id, []);
      }
      trendMap.get(row.alliance_id)!.push({
        timestamp: row.snapshot_at,
        score: Number(row.total_score),
      });
    });

    alliances.forEach((entry) => {
      entry.scoreTrend = trendMap.get(entry.allianceId) || [];
    });
  }

  private async appendAllianceWeeklyRankChanges(alliances: AllianceScore[]): Promise<void> {
    if (!alliances.length) return;
    const allianceIds = alliances.map((a) => a.allianceId);

    const snapshotRef = await this.db.query(
      `
        SELECT snapshot_at
        FROM alliance_leaderboard_snapshots
        WHERE snapshot_at <= NOW() - INTERVAL '7 days'
        ORDER BY snapshot_at DESC
        LIMIT 1
      `
    );

    if (!snapshotRef.rows.length) {
      alliances.forEach((entry) => (entry.weeklyRankChange = null));
      return;
    }

    const referenceDate = snapshotRef.rows[0].snapshot_at;
    const ranksResult = await this.db.query(
      `
        SELECT alliance_id, rank
        FROM (
          SELECT
            alliance_id,
            total_score,
            RANK() OVER (ORDER BY total_score DESC) AS rank
          FROM alliance_leaderboard_snapshots
          WHERE snapshot_at = $1
        ) ranked
        WHERE alliance_id = ANY($2::int[])
      `,
      [referenceDate, allianceIds]
    );

    const rankMap = new Map<number, number>();
    ranksResult.rows.forEach((row) => {
      rankMap.set(row.alliance_id, Number(row.rank));
    });

    alliances.forEach((entry) => {
      const previousRank = rankMap.get(entry.allianceId);
      entry.weeklyRankChange =
        typeof previousRank === 'number' ? previousRank - entry.rank : null;
    });
  }
}
