/**
 * @module backend/services/playerRoutingService
 *
 * PlayerRoutingService routes players to optimal shard servers using a
 * pluggable load-balancing algorithm (round-robin, least-connections,
 * weighted, geographic, health-based). It also manages player assignments,
 * migrations and auto-balancing operations.
 */

import pool from '../config/database';
import serverDiscoveryService from './serverDiscoveryService';
import {
  ShardPlayer,
  PlayerRoutingRequest,
  PlayerRoutingResult,
  PlayerMigrationRequest,
  LoadBalancingAlgorithm,
  LoadBalancerConfig,
  ServerType,
  ServerRegion,
  ShardServer
} from '../types/sharding';

export class PlayerRoutingService {
  private readonly DEFAULT_CONFIG: LoadBalancerConfig = {
    algorithm: LoadBalancingAlgorithm.GEOGRAPHIC,
    health_check_interval: 30000,
    max_server_load: 0.85, // 85% capacity
    failover_enabled: true,
    geographic_regions: Object.values(ServerRegion),
    weighted_factors: {
      cpu_weight: 0.3,
      memory_weight: 0.2,
      latency_weight: 0.3,
      load_weight: 0.2
    }
  };

  private config: LoadBalancerConfig;

  constructor(config?: Partial<LoadBalancerConfig>) {
    this.config = { ...this.DEFAULT_CONFIG, ...config };
  }

  /**
   * Route a player to the best available server based on the configured
   * algorithm and optional player preferences. If the player already has
   * an active healthy assignment, that assignment will be returned.
   *
   * @param request - PlayerRoutingRequest containing user/session and preferences
   */
  async routePlayer(request: PlayerRoutingRequest): Promise<PlayerRoutingResult> {
    // Check if player already has an active assignment
    const existingAssignment = await this.getPlayerAssignment(request.user_id);
    
    if (existingAssignment && existingAssignment.is_active) {
      const server = await serverDiscoveryService.getServerById(existingAssignment.server_id);
      
      if (server && server.status === 'online' && server.health_score >= 70) {
        // Return existing server assignment
        return this.buildRoutingResult(server, this.config.algorithm);
      }
    }

    // Find optimal server using configured algorithm
    const server = await this.findOptimalServer(request);

    if (!server) {
      throw new Error('No available servers for player routing');
    }

    // Assign player to server
    await this.assignPlayerToServer(request.user_id, server.server_id, request.session_id);

    return this.buildRoutingResult(server, this.config.algorithm);
  }

  /**
   * Find optimal server based on routing algorithm
   */
  private async findOptimalServer(request: PlayerRoutingRequest): Promise<ShardServer | null> {
    switch (this.config.algorithm) {
      case LoadBalancingAlgorithm.ROUND_ROBIN:
        return this.roundRobinSelection();
      
      case LoadBalancingAlgorithm.LEAST_CONNECTIONS:
        return this.leastConnectionsSelection();
      
      case LoadBalancingAlgorithm.WEIGHTED:
        return this.weightedSelection();
      
      case LoadBalancingAlgorithm.GEOGRAPHIC:
        return this.geographicSelection(request.preferred_region);
      
      case LoadBalancingAlgorithm.HEALTH_BASED:
        return this.healthBasedSelection();
      
      default:
        return this.leastConnectionsSelection();
    }
  }

  /**
   * Round Robin algorithm - distribute players evenly
   */
  private async roundRobinSelection(): Promise<ShardServer | null> {
    const servers = await serverDiscoveryService.getHealthyServers(ServerType.GAME);
    
    if (servers.length === 0) return null;

    // Get last assigned server index
    const lastIndex = await this.getLastRoundRobinIndex();
    const nextIndex = (lastIndex + 1) % servers.length;

    await this.setLastRoundRobinIndex(nextIndex);

    return servers[nextIndex];
  }

  /**
   * Least Connections algorithm - route to server with fewest players
   */
  private async leastConnectionsSelection(): Promise<ShardServer | null> {
    return serverDiscoveryService.getLeastLoadedServer(ServerType.GAME);
  }

  /**
   * Weighted algorithm - consider multiple factors
   */
  private async weightedSelection(): Promise<ShardServer | null> {
    const servers = await serverDiscoveryService.getHealthyServers(ServerType.GAME);
    
    if (servers.length === 0) return null;

    const weights = this.config.weighted_factors!;
    let bestServer: ShardServer | null = null;
    let bestScore = -Infinity;

    for (const server of servers) {
      // Calculate weighted score (higher is better)
      const loadFactor = 1 - (server.current_load / server.capacity);
      const cpuFactor = 1 - (server.cpu_usage / 100);
      const memoryFactor = 1 - (server.memory_usage / 100);
      const healthFactor = server.health_score / 100;

      const score = 
        (loadFactor * weights.load_weight) +
        (cpuFactor * weights.cpu_weight) +
        (memoryFactor * weights.memory_weight) +
        (healthFactor * weights.latency_weight);

      if (score > bestScore) {
        bestScore = score;
        bestServer = server;
      }
    }

    return bestServer;
  }

  /**
   * Geographic algorithm - route to nearest server in preferred region
   */
  private async geographicSelection(preferredRegion?: ServerRegion): Promise<ShardServer | null> {
    // Try preferred region first
    if (preferredRegion) {
      const server = await serverDiscoveryService.getLeastLoadedServer(
        ServerType.GAME,
        preferredRegion
      );
      
      if (server && server.current_load / server.capacity < this.config.max_server_load) {
        return server;
      }
    }

    // Fall back to least loaded server globally
    return serverDiscoveryService.getLeastLoadedServer(ServerType.GAME);
  }

  /**
   * Health-based algorithm - prefer servers with best health scores
   */
  private async healthBasedSelection(): Promise<ShardServer | null> {
    const servers = await serverDiscoveryService.getHealthyServers(ServerType.GAME);
    
    if (servers.length === 0) return null;

    // Sort by health score (descending) then by load (ascending)
    servers.sort((a, b) => {
      if (b.health_score !== a.health_score) {
        return b.health_score - a.health_score;
      }
      return (a.current_load / a.capacity) - (b.current_load / b.capacity);
    });

    return servers[0];
  }

  // =====================================================
  // PLAYER ASSIGNMENT
  // =====================================================

  /**
   * Assign player to a server
   */
  async assignPlayerToServer(
    userId: number,
    serverId: string,
    sessionId?: string
  ): Promise<ShardPlayer> {
    const client = await pool.connect();
    
    try {
      await client.query('BEGIN');

      // Deactivate any existing assignments
      await client.query(
        `UPDATE shard_players SET is_active = false WHERE user_id = $1`,
        [userId]
      );

      // Create new assignment
      const result = await client.query(
        `INSERT INTO shard_players (
          user_id, server_id, session_id, is_active, last_active
        ) VALUES ($1, $2, $3, true, CURRENT_TIMESTAMP)
        RETURNING *`,
        [userId, serverId, sessionId]
      );

      // Increment server load
      await client.query(
        `UPDATE shard_servers 
         SET current_load = current_load + 1, updated_at = CURRENT_TIMESTAMP
         WHERE server_id = $1`,
        [serverId]
      );

      await client.query('COMMIT');
      
      return this.mapPlayerRow(result.rows[0]);

    } catch (error) {
      await client.query('ROLLBACK');
      console.error('Error assigning player to server:', error);
      throw error;
    } finally {
      client.release();
    }
  }

  /**
   * Remove player from server
   */
  async removePlayerFromServer(userId: number): Promise<void> {
    const client = await pool.connect();
    
    try {
      await client.query('BEGIN');

      // Get current assignment
      const assignment = await client.query(
        `SELECT server_id FROM shard_players WHERE user_id = $1 AND is_active = true`,
        [userId]
      );

      if (assignment.rows.length > 0) {
        const serverId = assignment.rows[0].server_id;

        // Deactivate assignment
        await client.query(
          `UPDATE shard_players SET is_active = false WHERE user_id = $1`,
          [userId]
        );

        // Decrement server load
        await client.query(
          `UPDATE shard_servers 
           SET current_load = GREATEST(0, current_load - 1), updated_at = CURRENT_TIMESTAMP
           WHERE server_id = $1`,
          [serverId]
        );
      }

      await client.query('COMMIT');

    } catch (error) {
      await client.query('ROLLBACK');
      console.error('Error removing player from server:', error);
      throw error;
    } finally {
      client.release();
    }
  }

  /**
   * Get player's current server assignment
   */
  async getPlayerAssignment(userId: number): Promise<ShardPlayer | null> {
    const result = await pool.query(
      `SELECT * FROM shard_players WHERE user_id = $1 AND is_active = true ORDER BY assigned_at DESC LIMIT 1`,
      [userId]
    );

    if (result.rows.length === 0) {
      return null;
    }

    return this.mapPlayerRow(result.rows[0]);
  }

  /**
   * Get all players on a server
   */
  async getServerPlayers(serverId: string): Promise<ShardPlayer[]> {
    const result = await pool.query(
      `SELECT * FROM shard_players WHERE server_id = $1 AND is_active = true ORDER BY assigned_at`,
      [serverId]
    );

    return result.rows.map(row => this.mapPlayerRow(row));
  }

  // =====================================================
  // PLAYER MIGRATION
  // =====================================================

  /**
   * Migrate player from one server to another
   */
  async migratePlayer(request: PlayerMigrationRequest): Promise<PlayerRoutingResult> {
    const client = await pool.connect();
    
    try {
      await client.query('BEGIN');

      console.log(`Migrating player ${request.user_id}: ${request.from_server_id} -> ${request.to_server_id}`);

      // Verify target server exists and is healthy
      const targetServer = await serverDiscoveryService.getServerById(request.to_server_id);
      
      if (!targetServer || targetServer.status !== 'online') {
        throw new Error('Target server is not available');
      }

      if (targetServer.current_load >= targetServer.capacity) {
        throw new Error('Target server is at capacity');
      }

      // Deactivate current assignment
      await client.query(
        `UPDATE shard_players 
         SET is_active = false, 
             metadata = metadata || $1::jsonb
         WHERE user_id = $2 AND server_id = $3`,
        [JSON.stringify({ migration_reason: request.reason, migrated_at: new Date() }), request.user_id, request.from_server_id]
      );

      // Create new assignment
      await client.query(
        `INSERT INTO shard_players (
          user_id, server_id, session_id, is_active, last_active, metadata
        ) VALUES ($1, $2, $3, true, CURRENT_TIMESTAMP, $4)`,
        [
          request.user_id,
          request.to_server_id,
          request.preserve_session ? request.session_id : null,
          JSON.stringify({ migrated_from: request.from_server_id, migration_reason: request.reason })
        ]
      );

      // Update server loads
      await client.query(
        `UPDATE shard_servers 
         SET current_load = GREATEST(0, current_load - 1), updated_at = CURRENT_TIMESTAMP
         WHERE server_id = $1`,
        [request.from_server_id]
      );

      await client.query(
        `UPDATE shard_servers 
         SET current_load = current_load + 1, updated_at = CURRENT_TIMESTAMP
         WHERE server_id = $1`,
        [request.to_server_id]
      );

      await client.query('COMMIT');

      return this.buildRoutingResult(targetServer, LoadBalancingAlgorithm.HEALTH_BASED);

    } catch (error) {
      await client.query('ROLLBACK');
      console.error('Error migrating player:', error);
      throw error;
    } finally {
      client.release();
    }
  }

  /**
   * Auto-balance players across servers
   */
  async autoBalancePlayers(): Promise<number> {
    const servers = await serverDiscoveryService.getHealthyServers(ServerType.GAME);
    
    if (servers.length < 2) {
      console.log('Not enough servers for auto-balancing');
      return 0;
    }

    // Calculate average load
    const totalLoad = servers.reduce((sum, s) => sum + s.current_load, 0);
    const totalCapacity = servers.reduce((sum, s) => sum + s.capacity, 0);
    const averageLoadPercentage = totalLoad / totalCapacity;

    let migratedCount = 0;

    // Find overloaded servers (>90% capacity)
    const overloadedServers = servers.filter(s => 
      s.current_load / s.capacity > 0.90
    );

    // Find underloaded servers (<50% capacity)
    const underloadedServers = servers.filter(s =>
      s.current_load / s.capacity < 0.50
    ).sort((a, b) => a.current_load - b.current_load);

    for (const overloaded of overloadedServers) {
      const playersToMove = Math.floor(overloaded.current_load * 0.1); // Move 10% of players
      const players = await this.getServerPlayers(overloaded.server_id);

      for (let i = 0; i < playersToMove && i < players.length; i++) {
        const targetServer = underloadedServers[0];
        
        if (targetServer) {
          try {
            await this.migratePlayer({
              user_id: players[i].user_id,
              from_server_id: overloaded.server_id,
              to_server_id: targetServer.server_id,
              reason: 'auto_balance',
              preserve_session: true
            });

            migratedCount++;
          } catch (error) {
            console.error(`Failed to migrate player ${players[i].user_id}:`, error);
          }
        }
      }
    }

    console.log(`Auto-balanced ${migratedCount} players`);
    return migratedCount;
  }

  // =====================================================
  // STATISTICS
  // =====================================================

  /**
   * Get routing statistics
   */
  async getRoutingStatistics() {
    const result = await pool.query(`
      SELECT 
        COUNT(DISTINCT user_id) as total_players,
        COUNT(DISTINCT server_id) as active_servers,
        COUNT(*) FILTER (WHERE is_active = true) as active_sessions,
        AVG(connection_quality) as avg_connection_quality
      FROM shard_players
    `);

    const stats = result.rows[0];

    // Get server distribution
    const distribution = await pool.query(`
      SELECT 
        server_id,
        COUNT(*) as player_count
      FROM shard_players
      WHERE is_active = true
      GROUP BY server_id
      ORDER BY player_count DESC
    `);

    return {
      total_players: parseInt(stats.total_players),
      active_servers: parseInt(stats.active_servers),
      active_sessions: parseInt(stats.active_sessions),
      average_connection_quality: parseFloat(stats.avg_connection_quality) || 0,
      server_distribution: distribution.rows.map(row => ({
        server_id: row.server_id,
        player_count: parseInt(row.player_count)
      }))
    };
  }

  /**
   * Build a PlayerRoutingResult from a ShardServer record.
   *
   * This composes the minimal routing payload returned to callers and
   * normalizes fields used by the client to connect (host/ports) and for
   * diagnostics (estimated latency, chosen algorithm).
   *
   * @private
   * @param {ShardServer} server - Server record returned from discovery
   * @param {LoadBalancingAlgorithm} algorithm - Algorithm used to pick this server
   * @returns {PlayerRoutingResult} Normalized routing payload
   */
  private buildRoutingResult(server: ShardServer, algorithm: LoadBalancingAlgorithm): PlayerRoutingResult {
    return {
      server_id: server.server_id,
      server_name: server.server_name,
      host_address: server.host_address,
      port: server.port,
      websocket_port: server.websocket_port,
      region: server.region,
      estimated_latency: server.response_time_ms,
      routing_algorithm: algorithm
    };
  }

  /**
   * Read the persisted round-robin index used for round-robin selection.
   *
   * The implementation stores a small index in the `metadata` JSONB column
   * of a representative `shard_servers` row. Returns -1 when no persisted
   * index is found.
   *
   * @private
   * @returns {Promise<number>} Persisted index or -1 when unset
   */
  private async getLastRoundRobinIndex(): Promise<number> {
    const result = await pool.query(
      `SELECT metadata->>'round_robin_index' as idx FROM shard_servers WHERE server_type = 'game' LIMIT 1`
    );

    if (result.rows.length > 0 && result.rows[0].idx) {
      return parseInt(result.rows[0].idx);
    }

    return -1;
  }

  /**
   * Persist the round-robin index back into the database.
   *
   * @private
   * @param {number} index - Index to persist
   */
  private async setLastRoundRobinIndex(index: number): Promise<void> {
    await pool.query(
      `UPDATE shard_servers 
       SET metadata = metadata || $1::jsonb 
       WHERE server_type = 'game' 
       LIMIT 1`,
      [JSON.stringify({ round_robin_index: index })]
    );
  }

  /**
   * Map a raw DB row from `shard_players` into the `ShardPlayer` DTO.
   *
   * @private
   * @param {any} row - Raw database row
   * @returns {ShardPlayer} Normalized player assignment object
   */
  private mapPlayerRow(row: any): ShardPlayer {
    return {
      id: row.id,
      user_id: row.user_id,
      server_id: row.server_id,
      session_id: row.session_id,
      assigned_at: row.assigned_at,
      last_active: row.last_active,
      connection_quality: row.connection_quality,
      preferred_region: row.preferred_region,
      is_active: row.is_active,
      metadata: row.metadata
    };
  }

  /**
   * Update load balancer configuration
   */
  updateConfig(newConfig: Partial<LoadBalancerConfig>): void {
    this.config = { ...this.config, ...newConfig };
    console.log('Load balancer config updated:', this.config);
  }

  /**
   * Get current configuration
   */
  getConfig(): LoadBalancerConfig {
    return { ...this.config };
  }
}

export default new PlayerRoutingService();
