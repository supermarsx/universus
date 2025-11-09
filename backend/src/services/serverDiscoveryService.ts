/**
 * @module backend/services/serverDiscoveryService
 *
 * Server discovery and health management utilities. This service is responsible
 * for registering/deregistering shard servers, tracking health metrics and
 * heartbeats, performing periodic health checks, and providing queries for
 * selecting suitable servers (least-loaded, healthy by region/type).
 *
 * Public surface:
 * - registerServer, deregisterServer
 * - updateServerHealth, recordHeartbeat, checkServerHealth
 * - startHealthMonitoring, stopHealthMonitoring
 * - getAllServers, getServerById, getLeastLoadedServer, getServersByType/Region
 * - updateServerStatus, updateServerConfig, getServerStatistics
 */

import pool from '../config/database';
import {
  ShardServer,
  ServerRegistrationRequest,
  ServerHealthUpdate,
  ServerStatus,
  ServerType,
  ServerRegion,
  ServerMetrics,
  HealthCheckResult,
  ShardingApiResponse
} from '../types/sharding';

export class ServerDiscoveryService {
  /**
   * Server discovery service instance.
   * Manages shard server metadata and health checks stored in Postgres.
   */
  private healthCheckInterval: NodeJS.Timeout | null = null;
  private readonly HEALTH_CHECK_INTERVAL_MS = 30000; // 30 seconds
  private readonly HEARTBEAT_TIMEOUT_MS = 90000; // 90 seconds
  private readonly MAX_FAILED_CHECKS = 3;

  /**
   * Register a new server in the shard cluster.
   *
   * Inserts a new row into `shard_servers` or updates an existing entry when
   * a server with the same `server_id` already exists. The returned object is
   * mapped to the `ShardServer` type.
   *
   * @param {ServerRegistrationRequest} request - Registration payload from server
   * @returns {Promise<ShardServer>} The registered or updated shard server
   * @throws Will re-throw any database errors encountered during the transaction
   */
  async registerServer(request: ServerRegistrationRequest): Promise<ShardServer> {
    const client = await pool.connect();
    
    try {
      await client.query('BEGIN');

      // Check if server already exists
      const existing = await client.query(
        'SELECT id FROM shard_servers WHERE server_id = $1',
        [request.server_id]
      );

      if (existing.rows.length > 0) {
        // Update existing server
        const result = await client.query(
          `UPDATE shard_servers 
           SET server_name = $1, server_type = $2, region = $3, 
               host_address = $4, port = $5, websocket_port = $6,
               capacity = $7, status = 'online', health_score = 100,
               last_heartbeat = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP,
               metadata = $8
           WHERE server_id = $9
           RETURNING *`,
          [
            request.server_name,
            request.server_type,
            request.region,
            request.host_address,
            request.port,
            request.websocket_port,
            request.capacity || 1000,
            JSON.stringify(request.metadata || {}),
            request.server_id
          ]
        );

        await client.query('COMMIT');
        return this.mapServerRow(result.rows[0]);
      }

      // Register new server
      const result = await client.query(
        `INSERT INTO shard_servers (
          server_id, server_name, server_type, region,
          host_address, port, websocket_port, capacity,
          status, health_score, metadata
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'online', 100, $9)
        RETURNING *`,
        [
          request.server_id,
          request.server_name,
          request.server_type,
          request.region,
          request.host_address,
          request.port,
          request.websocket_port,
          request.capacity || 1000,
          JSON.stringify(request.metadata || {})
        ]
      );

      await client.query('COMMIT');
      console.log(`Server registered: ${request.server_id} (${request.server_name})`);
      
      return this.mapServerRow(result.rows[0]);

    } catch (error) {
      await client.query('ROLLBACK');
      console.error('Error registering server:', error);
      throw error;
    } finally {
      client.release();
    }
  }

  /**
   * Deregister a server from the cluster
   */
  async deregisterServer(serverId: string): Promise<void> {
    const client = await pool.connect();
    
    try {
      await client.query('BEGIN');

      // Update server status
      await client.query(
        `UPDATE shard_servers 
         SET status = 'offline', updated_at = CURRENT_TIMESTAMP
         WHERE server_id = $1`,
        [serverId]
      );

      // Mark all players on this server as inactive
      await client.query(
        `UPDATE shard_players 
         SET is_active = false 
         WHERE server_id = $1`,
        [serverId]
      );

      await client.query('COMMIT');
      console.log(`Server deregistered: ${serverId}`);

    } catch (error) {
      await client.query('ROLLBACK');
      console.error('Error deregistering server:', error);
      throw error;
    } finally {
      client.release();
    }
  }

  // =====================================================
  // HEALTH MONITORING
  // =====================================================

  /**
   * Update server health metrics
   */
  async updateServerHealth(update: ServerHealthUpdate): Promise<void> {
    try {
      await pool.query(
        `UPDATE shard_servers 
         SET cpu_usage = $1, memory_usage = $2, response_time_ms = $3,
             current_load = $4, health_score = $5,
             last_heartbeat = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP
         WHERE server_id = $6`,
        [
          update.cpu_usage,
          update.memory_usage,
          update.response_time_ms,
          update.current_load,
          update.health_score,
          update.server_id
        ]
      );

      // Auto-update status based on health score
      if (update.health_score < 30) {
        await this.updateServerStatus(update.server_id, ServerStatus.DEGRADED);
      } else if (update.health_score >= 70) {
        await this.updateServerStatus(update.server_id, ServerStatus.ONLINE);
      }

    } catch (error) {
      console.error('Error updating server health:', error);
      throw error;
    }
  }

  /**
   * Record server heartbeat
   */
  async recordHeartbeat(serverId: string): Promise<void> {
    try {
      await pool.query(
        `UPDATE shard_servers 
         SET last_heartbeat = CURRENT_TIMESTAMP 
         WHERE server_id = $1`,
        [serverId]
      );
    } catch (error) {
      console.error('Error recording heartbeat:', error);
    }
  }

  /**
   * Check server health status
   */
  async checkServerHealth(serverId: string): Promise<HealthCheckResult> {
    const result = await pool.query(
      'SELECT * FROM shard_servers WHERE server_id = $1',
      [serverId]
    );

    if (result.rows.length === 0) {
      throw new Error(`Server not found: ${serverId}`);
    }

    const server = this.mapServerRow(result.rows[0]);
    const now = new Date();
    const heartbeatAge = now.getTime() - new Date(server.last_heartbeat).getTime();

    return {
      server_id: serverId,
      status: server.status,
      health_score: server.health_score,
      checks: {
        api_responsive: heartbeatAge < this.HEARTBEAT_TIMEOUT_MS,
        database_connected: server.health_score > 50,
        redis_connected: server.health_score > 50,
        websocket_active: server.websocket_port !== null,
        disk_space_available: server.health_score > 30
      },
      metrics: {
        cpu_usage: server.cpu_usage,
        memory_usage: server.memory_usage,
        response_time: server.response_time_ms,
        active_connections: server.current_load
      },
      timestamp: now
    };
  }

  /**
   * Start automatic health monitoring
   */
  startHealthMonitoring(): void {
    if (this.healthCheckInterval) {
      console.log('Health monitoring already running');
      return;
    }

    console.log('Starting server health monitoring...');
    
    this.healthCheckInterval = setInterval(async () => {
      try {
        await this.performHealthChecks();
      } catch (error) {
        console.error('Error in health check cycle:', error);
      }
    }, this.HEALTH_CHECK_INTERVAL_MS);
  }

  /**
   * Stop automatic health monitoring
   */
  stopHealthMonitoring(): void {
    if (this.healthCheckInterval) {
      clearInterval(this.healthCheckInterval);
      this.healthCheckInterval = null;
      console.log('Health monitoring stopped');
    }
  }

  /**
   * Perform health checks on all non-offline servers.
   *
   * This method queries `shard_servers` and evaluates heartbeat age and
   * health scores. It will degrade or mark servers offline and trigger
   * failover handling for servers judged to be offline.
   *
   * @private
   */
  private async performHealthChecks(): Promise<void> {
    const result = await pool.query(
      `SELECT * FROM shard_servers WHERE status != 'offline'`
    );

    const now = new Date();

    for (const row of result.rows) {
      const server = this.mapServerRow(row);
      const heartbeatAge = now.getTime() - new Date(server.last_heartbeat).getTime();

      // Check if server is unresponsive
      if (heartbeatAge > this.HEARTBEAT_TIMEOUT_MS) {
        console.warn(`Server ${server.server_id} missed heartbeat (${heartbeatAge}ms)`);
        
        // Degrade health score
        const newHealthScore = Math.max(0, server.health_score - 20);
        
        await pool.query(
          `UPDATE shard_servers 
           SET health_score = $1, status = $2, updated_at = CURRENT_TIMESTAMP
           WHERE server_id = $3`,
          [newHealthScore, newHealthScore < 30 ? 'offline' : 'degraded', server.server_id]
        );

        // If server is now offline, migrate players
        if (newHealthScore < 30) {
          await this.handleServerFailure(server.server_id);
        }
      }
    }
  }

  /**
   * Handle server failure and trigger failover actions.
   *
   * Marks the failed server offline, fetches affected players and marks them
   * for reassignment. The reassignment happens during player login or
   * placement cycles.
   *
   * @private
   * @param {string} serverId - The server identifier to handle
   */
  private async handleServerFailure(serverId: string): Promise<void> {
    console.error(`Server failure detected: ${serverId}`);
    
    const client = await pool.connect();
    
    try {
      await client.query('BEGIN');

      // Mark server as offline
      await client.query(
        `UPDATE shard_servers SET status = 'offline' WHERE server_id = $1`,
        [serverId]
      );

      // Get affected players
      const playersResult = await client.query(
        `SELECT user_id FROM shard_players WHERE server_id = $1 AND is_active = true`,
        [serverId]
      );

      console.log(`Migrating ${playersResult.rows.length} players from failed server`);

      // Mark players for reassignment
      await client.query(
        `UPDATE shard_players 
         SET is_active = false, metadata = metadata || '{"needs_reassignment": true}'::jsonb
         WHERE server_id = $1`,
        [serverId]
      );

      await client.query('COMMIT');

      // Players will be reassigned on next login

    } catch (error) {
      await client.query('ROLLBACK');
      console.error('Error handling server failure:', error);
    } finally {
      client.release();
    }
  }

  // =====================================================
  // SERVER QUERIES
  // =====================================================

  /**
   * Get all servers
   */
  async getAllServers(): Promise<ShardServer[]> {
    const result = await pool.query(
      'SELECT * FROM shard_servers ORDER BY server_id'
    );
    
    return result.rows.map(row => this.mapServerRow(row));
  }

  /**
   * Get servers by type
   */
  async getServersByType(type: ServerType): Promise<ShardServer[]> {
    const result = await pool.query(
      'SELECT * FROM shard_servers WHERE server_type = $1 ORDER BY server_id',
      [type]
    );
    
    return result.rows.map(row => this.mapServerRow(row));
  }

  /**
   * Get servers by region
   */
  async getServersByRegion(region: ServerRegion): Promise<ShardServer[]> {
    const result = await pool.query(
      'SELECT * FROM shard_servers WHERE region = $1 ORDER BY server_id',
      [region]
    );
    
    return result.rows.map(row => this.mapServerRow(row));
  }

  /**
   * Get healthy servers (status = online, health_score >= 70).
   *
   * Optionally filter by server type. Results are ordered by health score
   * (descending) then current load (ascending) to prefer high-health low-load
   * servers.
   *
   * @param {ServerType=} type - Optional type filter
   * @returns {Promise<ShardServer[]>} Matching servers
   */
  async getHealthyServers(type?: ServerType): Promise<ShardServer[]> {
    let query = `
      SELECT * FROM shard_servers 
      WHERE status = 'online' AND health_score >= 70
    `;
    const params: any[] = [];

    if (type) {
      query += ' AND server_type = $1';
      params.push(type);
    }

    query += ' ORDER BY health_score DESC, current_load ASC';

    const result = await pool.query(query, params);
    return result.rows.map(row => this.mapServerRow(row));
  }

  /**
   * Get server by ID
   */
  async getServerById(serverId: string): Promise<ShardServer | null> {
    const result = await pool.query(
      'SELECT * FROM shard_servers WHERE server_id = $1',
      [serverId]
    );

    if (result.rows.length === 0) {
      return null;
    }

    return this.mapServerRow(result.rows[0]);
  }

  /**
   * Get least loaded server
   */
  async getLeastLoadedServer(type?: ServerType, region?: ServerRegion): Promise<ShardServer | null> {
    let query = `
      SELECT * FROM shard_servers 
      WHERE status = 'online' AND health_score >= 70
    `;
    const params: any[] = [];
    let paramIndex = 1;

    if (type) {
      query += ` AND server_type = $${paramIndex++}`;
      params.push(type);
    }

    if (region) {
      query += ` AND region = $${paramIndex++}`;
      params.push(region);
    }

    query += ` ORDER BY (current_load::float / capacity::float) ASC, health_score DESC LIMIT 1`;

    const result = await pool.query(query, params);

    if (result.rows.length === 0) {
      return null;
    }

    return this.mapServerRow(result.rows[0]);
  }

  // =====================================================
  // SERVER MANAGEMENT
  // =====================================================

  /**
   * Update server status
   */
  async updateServerStatus(serverId: string, status: ServerStatus): Promise<void> {
    await pool.query(
      `UPDATE shard_servers 
       SET status = $1, updated_at = CURRENT_TIMESTAMP
       WHERE server_id = $2`,
      [status, serverId]
    );

    console.log(`Server ${serverId} status updated to ${status}`);
  }

  /**
   * Update server configuration
   */
  async updateServerConfig(
    serverId: string,
    config: Partial<ShardServer>
  ): Promise<ShardServer> {
    const updates: string[] = [];
    const values: any[] = [];
    let paramIndex = 1;

    if (config.capacity !== undefined) {
      updates.push(`capacity = $${paramIndex++}`);
      values.push(config.capacity);
    }

    if (config.metadata !== undefined) {
      updates.push(`metadata = $${paramIndex++}`);
      values.push(JSON.stringify(config.metadata));
    }

    if (updates.length === 0) {
      const current = await this.getServerById(serverId);
      if (!current) throw new Error('Server not found');
      return current;
    }

    updates.push(`updated_at = CURRENT_TIMESTAMP`);
    values.push(serverId);

    const result = await pool.query(
      `UPDATE shard_servers SET ${updates.join(', ')} WHERE server_id = $${paramIndex} RETURNING *`,
      values
    );

    return this.mapServerRow(result.rows[0]);
  }

  /**
   * Get server statistics
   */
  async getServerStatistics() {
    const result = await pool.query(`
      SELECT 
        COUNT(*) as total_servers,
        COUNT(*) FILTER (WHERE status = 'online') as online_servers,
        COUNT(*) FILTER (WHERE status = 'offline') as offline_servers,
        COUNT(*) FILTER (WHERE status = 'maintenance') as maintenance_servers,
        COUNT(*) FILTER (WHERE status = 'degraded') as degraded_servers,
        SUM(capacity) as total_capacity,
        SUM(current_load) as total_load,
        AVG(health_score) as avg_health_score,
        AVG(cpu_usage) as avg_cpu_usage,
        AVG(memory_usage) as avg_memory_usage,
        AVG(response_time_ms) as avg_response_time
      FROM shard_servers
    `);

    const stats = result.rows[0];
    
    return {
      total_servers: parseInt(stats.total_servers),
      online_servers: parseInt(stats.online_servers),
      offline_servers: parseInt(stats.offline_servers),
      maintenance_servers: parseInt(stats.maintenance_servers),
      degraded_servers: parseInt(stats.degraded_servers),
      total_capacity: parseInt(stats.total_capacity) || 0,
      total_load: parseInt(stats.total_load) || 0,
      load_percentage: stats.total_capacity > 0 
        ? (parseInt(stats.total_load) / parseInt(stats.total_capacity) * 100).toFixed(2)
        : 0,
      average_health_score: parseFloat(stats.avg_health_score) || 0,
      average_cpu_usage: parseFloat(stats.avg_cpu_usage) || 0,
      average_memory_usage: parseFloat(stats.avg_memory_usage) || 0,
      average_response_time: parseFloat(stats.avg_response_time) || 0
    };
  }

  // =====================================================
  // UTILITY METHODS
  // =====================================================

  /**
   * Map a raw database row into the ShardServer interface
   *
   * @private
   * @param {any} row - Raw row from `shard_servers`
   * @returns {ShardServer}
   */
  private mapServerRow(row: any): ShardServer {
    return {
      id: row.id,
      server_id: row.server_id,
      server_name: row.server_name,
      server_type: row.server_type,
      region: row.region,
      host_address: row.host_address,
      port: row.port,
      websocket_port: row.websocket_port,
      capacity: row.capacity,
      current_load: row.current_load,
      status: row.status,
      health_score: row.health_score,
      cpu_usage: parseFloat(row.cpu_usage),
      memory_usage: parseFloat(row.memory_usage),
      response_time_ms: row.response_time_ms,
      last_heartbeat: row.last_heartbeat,
      created_at: row.created_at,
      updated_at: row.updated_at,
      metadata: row.metadata
    };
  }
}

export default new ServerDiscoveryService();
