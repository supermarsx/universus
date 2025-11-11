// =====================================================
// GLOBAL LEADERBOARD SERVICE
// Cross-server ranking and leaderboard aggregation
// =====================================================

import pool from '../config/database';
import crossServerCommunication from './crossServerCommunicationService';
import {
  ShardLeaderboard,
  LeaderboardEntry,
  LeaderboardSnapshot,
  GlobalLeaderboardRequest,
  LeaderboardCategory,
  LeaderboardPeriod,
  PaginatedResponse
} from '../types/sharding';

export class GlobalLeaderboardService {
  private updateInterval: NodeJS.Timeout | null = null;
  private readonly UPDATE_INTERVAL_MS = 60000; // 1 minute

  // =====================================================
  // LEADERBOARD UPDATES
  // =====================================================

  /**
   * Update player's leaderboard entry
   */
  async updatePlayerEntry(
    userId: number,
    serverId: string,
    category: LeaderboardCategory,
    score: number,
    allianceId?: number
  ): Promise<void> {
    const client = await pool.connect();
    
    try {
      await client.query('BEGIN');

      // Get current entry if exists
      const current = await client.query(
        `SELECT rank FROM shard_leaderboards 
         WHERE user_id = $1 AND server_id = $2 AND category = $3`,
        [userId, serverId, category]
      );

      const previousRank = current.rows.length > 0 ? current.rows[0].rank : null;

      // Upsert entry
      await client.query(
        `INSERT INTO shard_leaderboards (
          user_id, server_id, category, score, alliance_id, previous_rank
        ) VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (user_id, server_id, category) 
        DO UPDATE SET 
          score = $4,
          previous_rank = shard_leaderboards.rank,
          alliance_id = $5,
          last_updated = CURRENT_TIMESTAMP`,
        [userId, serverId, category, score, allianceId, previousRank]
      );

      await client.query('COMMIT');

      // Recalculate ranks for this category
      await this.recalculateRanks(category);

      // Broadcast update to other servers
      await crossServerCommunication.publishLeaderboardUpdate(category, [
        { user_id: userId, server_id: serverId, score }
      ]);

    } catch (error) {
      await client.query('ROLLBACK');
      console.error('Error updating leaderboard entry:', error);
      throw error;
    } finally {
      client.release();
    }
  }

  /**
   * Recalculate ranks for a category
   */
  async recalculateRanks(category: LeaderboardCategory): Promise<void> {
    await pool.query(`
      WITH ranked AS (
        SELECT 
          id,
          ROW_NUMBER() OVER (ORDER BY score DESC, last_updated ASC) as new_rank
        FROM shard_leaderboards
        WHERE category = $1
      )
      UPDATE shard_leaderboards l
      SET 
        rank = r.new_rank,
        rank_change = COALESCE(l.previous_rank, 0) - r.new_rank,
        last_updated = CURRENT_TIMESTAMP
      FROM ranked r
      WHERE l.id = r.id
    `, [category]);
  }

  /**
   * Aggregate leaderboards across all servers
   */
  async aggregateLeaderboards(category: LeaderboardCategory): Promise<LeaderboardEntry[]> {
    const result = await pool.query(`
      SELECT 
        l.user_id,
        u.username,
        l.server_id,
        l.score,
        l.rank,
        l.rank_change,
        l.alliance_id,
        a.alliance_name
      FROM shard_leaderboards l
      JOIN users u ON l.user_id = u.id
      LEFT JOIN alliances a ON l.alliance_id = a.id
      WHERE l.category = $1
      ORDER BY l.score DESC, l.last_updated ASC
      LIMIT 1000
    `, [category]);

    // Recalculate global ranks
    const entries: LeaderboardEntry[] = result.rows.map((row, index) => ({
      rank: index + 1,
      user_id: row.user_id,
      username: row.username,
      server_id: row.server_id,
      score: parseInt(row.score),
      rank_change: row.rank_change,
      alliance_name: row.alliance_name,
      metadata: {}
    }));

    return entries;
  }

  // =====================================================
  // LEADERBOARD QUERIES
  // =====================================================

  /**
   * Get global leaderboard
   */
  async getGlobalLeaderboard(
    request: GlobalLeaderboardRequest
  ): Promise<PaginatedResponse<LeaderboardEntry>> {
    const limit = request.limit || 50;
    const offset = request.offset || 0;

    let query = `
      SELECT 
        l.user_id,
        u.username,
        l.server_id,
        l.score,
        l.rank,
        l.rank_change,
        l.alliance_id,
        a.alliance_name,
        l.metadata
      FROM shard_leaderboards l
      JOIN users u ON l.user_id = u.id
      LEFT JOIN alliances a ON l.alliance_id = a.id
      WHERE l.category = $1
    `;

    const params: any[] = [request.category];
    let paramIndex = 2;

    if (request.server_id) {
      query += ` AND l.server_id = $${paramIndex++}`;
      params.push(request.server_id);
    }

    if (request.alliance_id) {
      query += ` AND l.alliance_id = $${paramIndex++}`;
      params.push(request.alliance_id);
    }

    // Count total
    const countResult = await pool.query(
      query.replace('SELECT l.user_id, u.username, l.server_id, l.score, l.rank, l.rank_change, l.alliance_id, a.alliance_name, l.metadata', 'SELECT COUNT(*) as total'),
      params
    );

    const total = parseInt(countResult.rows[0].total);

    // Get paginated results
    query += ` ORDER BY l.score DESC, l.last_updated ASC LIMIT $${paramIndex++} OFFSET $${paramIndex}`;
    params.push(limit, offset);

    const result = await pool.query(query, params);

    const entries: LeaderboardEntry[] = result.rows.map(row => ({
      rank: row.rank,
      user_id: row.user_id,
      username: row.username,
      server_id: row.server_id,
      score: parseInt(row.score),
      rank_change: row.rank_change,
      alliance_name: row.alliance_name,
      metadata: row.metadata
    }));

    return {
      data: entries,
      total,
      page: Math.floor(offset / limit) + 1,
      per_page: limit,
      total_pages: Math.ceil(total / limit)
    };
  }

  /**
   * Get player's rank in leaderboard
   */
  async getPlayerRank(
    userId: number,
    category: LeaderboardCategory
  ): Promise<LeaderboardEntry | null> {
    const result = await pool.query(`
      SELECT 
        l.user_id,
        u.username,
        l.server_id,
        l.score,
        l.rank,
        l.rank_change,
        l.alliance_id,
        a.alliance_name
      FROM shard_leaderboards l
      JOIN users u ON l.user_id = u.id
      LEFT JOIN alliances a ON l.alliance_id = a.id
      WHERE l.user_id = $1 AND l.category = $2
    `, [userId, category]);

    if (result.rows.length === 0) {
      return null;
    }

    const row = result.rows[0];
    return {
      rank: row.rank,
      user_id: row.user_id,
      username: row.username,
      server_id: row.server_id,
      score: parseInt(row.score),
      rank_change: row.rank_change,
      alliance_name: row.alliance_name
    };
  }

  /**
   * Get top players by category
   */
  async getTopPlayers(
    category: LeaderboardCategory,
    limit: number = 10
  ): Promise<LeaderboardEntry[]> {
    const result = await pool.query(`
      SELECT 
        l.user_id,
        u.username,
        l.server_id,
        l.score,
        l.rank,
        l.rank_change,
        l.alliance_id,
        a.alliance_name
      FROM shard_leaderboards l
      JOIN users u ON l.user_id = u.id
      LEFT JOIN alliances a ON l.alliance_id = a.id
      WHERE l.category = $1
      ORDER BY l.score DESC
      LIMIT $2
    `, [category, limit]);

    return result.rows.map(row => ({
      rank: row.rank,
      user_id: row.user_id,
      username: row.username,
      server_id: row.server_id,
      score: parseInt(row.score),
      rank_change: row.rank_change,
      alliance_name: row.alliance_name
    }));
  }

  // =====================================================
  // TIME-BASED LEADERBOARDS
  // =====================================================

  /**
   * Create daily snapshot
   */
  async createDailySnapshot(): Promise<void> {
    const today = new Date();
    today.setHours(0, 0, 0, 0);

    const client = await pool.connect();
    
    try {
      await client.query('BEGIN');

      // Create snapshots for all categories
      const categories = Object.values(LeaderboardCategory);

      for (const category of categories) {
        await client.query(`
          INSERT INTO shard_leaderboard_snapshots (
            snapshot_date, period, user_id, category, score, rank, server_id
          )
          SELECT 
            $1, 'daily', user_id, category, score, rank, server_id
          FROM shard_leaderboards
          WHERE category = $2
        `, [today, category]);
      }

      await client.query('COMMIT');
      console.log('Daily leaderboard snapshots created');

    } catch (error) {
      await client.query('ROLLBACK');
      console.error('Error creating daily snapshot:', error);
      throw error;
    } finally {
      client.release();
    }
  }

  /**
   * Get historical leaderboard
   */
  async getHistoricalLeaderboard(
    category: LeaderboardCategory,
    period: LeaderboardPeriod,
    date?: Date
  ): Promise<LeaderboardEntry[]> {
    const targetDate = date || new Date();
    
    const result = await pool.query(`
      SELECT 
        s.user_id,
        u.username,
        s.server_id,
        s.score,
        s.rank
      FROM shard_leaderboard_snapshots s
      JOIN users u ON s.user_id = u.id
      WHERE s.category = $1 AND s.period = $2 AND s.snapshot_date = $3
      ORDER BY s.rank ASC
      LIMIT 100
    `, [category, period, targetDate]);

    return result.rows.map(row => ({
      rank: row.rank,
      user_id: row.user_id,
      username: row.username,
      server_id: row.server_id,
      score: parseInt(row.score),
      rank_change: 0
    }));
  }

  // =====================================================
  // AUTOMATED UPDATES
  // =====================================================

  /**
   * Start automatic leaderboard updates
   */
  startAutomaticUpdates(): void {
    if (process.env.NODE_ENV === 'test' || process.env.SKIP_SERVER_START === 'true') {
      console.log('Global leaderboard automatic updates skipped (test mode or SKIP_SERVER_START=true)');
      return;
    }

    if (this.updateInterval) {
      console.log('Leaderboard updates already running');
      return;
    }

    console.log('Starting automatic leaderboard updates...');
    
    this.updateInterval = setInterval(async () => {
      try {
        // Recalculate all category ranks
        const categories = Object.values(LeaderboardCategory);
        for (const category of categories) {
          await this.recalculateRanks(category);
        }
        
        console.log('Leaderboard ranks updated');
      } catch (error) {
        console.error('Error in leaderboard update cycle:', error);
      }
    }, this.UPDATE_INTERVAL_MS);

    // Schedule daily snapshots at midnight
    this.scheduleDailySnapshots();
  }

  /**
   * Stop automatic updates
   */
  stopAutomaticUpdates(): void {
    if (this.updateInterval) {
      clearInterval(this.updateInterval);
      this.updateInterval = null;
      console.log('Leaderboard updates stopped');
    }
  }

  /**
   * Schedule daily snapshots
   */
  private scheduleDailySnapshots(): void {
    const now = new Date();
    const tomorrow = new Date(now);
    tomorrow.setDate(tomorrow.getDate() + 1);
    tomorrow.setHours(0, 0, 0, 0);
    
    const msUntilMidnight = tomorrow.getTime() - now.getTime();

    setTimeout(async () => {
      await this.createDailySnapshot();
      
      // Schedule next day's snapshot
      setInterval(async () => {
        await this.createDailySnapshot();
      }, 24 * 60 * 60 * 1000); // Every 24 hours
      
    }, msUntilMidnight);

    console.log(`Daily snapshots scheduled (next in ${msUntilMidnight / 1000 / 60} minutes)`);
  }

  // =====================================================
  // STATISTICS
  // =====================================================

  /**
   * Get leaderboard statistics
   */
  async getStatistics() {
    const result = await pool.query(`
      SELECT 
        COUNT(DISTINCT user_id) as total_players,
        COUNT(DISTINCT server_id) as active_servers,
        COUNT(*) as total_entries,
        AVG(score) as average_score,
        MAX(score) as highest_score
      FROM shard_leaderboards
    `);

    const stats = result.rows[0];

    // Get category breakdown
    const categoryStats = await pool.query(`
      SELECT 
        category,
        COUNT(*) as entries,
        AVG(score) as avg_score,
        MAX(score) as max_score
      FROM shard_leaderboards
      GROUP BY category
    `);

    return {
      total_players: parseInt(stats.total_players),
      active_servers: parseInt(stats.active_servers),
      total_entries: parseInt(stats.total_entries),
      average_score: parseFloat(stats.average_score) || 0,
      highest_score: parseInt(stats.highest_score) || 0,
      categories: categoryStats.rows.map(row => ({
        category: row.category,
        entries: parseInt(row.entries),
        average_score: parseFloat(row.avg_score),
        max_score: parseInt(row.max_score)
      }))
    };
  }
}

export default new GlobalLeaderboardService();
