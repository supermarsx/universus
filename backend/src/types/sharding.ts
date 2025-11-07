// =====================================================
// PHASE 5: SERVER SHARDING - TypeScript Types
// Enterprise multi-server architecture type definitions
// =====================================================

// =====================================================
// ENUMS
// =====================================================

export enum ServerType {
  GAME = 'game',
  CHAT = 'chat',
  LEADERBOARD = 'leaderboard',
  MARKET = 'market',
  ANALYTICS = 'analytics'
}

export enum ServerStatus {
  ONLINE = 'online',
  OFFLINE = 'offline',
  MAINTENANCE = 'maintenance',
  DEGRADED = 'degraded'
}

export enum ServerRegion {
  US_EAST = 'us-east',
  US_WEST = 'us-west',
  EU_WEST = 'eu-west',
  EU_CENTRAL = 'eu-central',
  ASIA_EAST = 'asia-east',
  ASIA_SOUTHEAST = 'asia-southeast'
}

export enum LoadBalancingAlgorithm {
  ROUND_ROBIN = 'round_robin',
  LEAST_CONNECTIONS = 'least_connections',
  WEIGHTED = 'weighted',
  GEOGRAPHIC = 'geographic',
  HEALTH_BASED = 'health_based'
}

export enum LeaderboardCategory {
  TOTAL_POINTS = 'total_points',
  FLEET_POWER = 'fleet_power',
  RESEARCH_LEVEL = 'research_level',
  RESOURCES = 'resources',
  ALLIANCE_POWER = 'alliance_power',
  COMBAT_WINS = 'combat_wins'
}

export enum LeaderboardPeriod {
  DAILY = 'daily',
  WEEKLY = 'weekly',
  MONTHLY = 'monthly',
  ALL_TIME = 'all_time'
}

export enum ChatChannelType {
  WORLD = 'world',
  ALLIANCE = 'alliance',
  SECTOR = 'sector',
  PRIVATE = 'private',
  SYSTEM = 'system'
}

export enum MessagePriority {
  LOW = 'low',
  NORMAL = 'normal',
  HIGH = 'high',
  CRITICAL = 'critical'
}

// =====================================================
// SERVER TYPES
// =====================================================

export interface ShardServer {
  id: number;
  server_id: string;
  server_name: string;
  server_type: ServerType;
  region: ServerRegion;
  host_address: string;
  port: number;
  websocket_port?: number;
  capacity: number;
  current_load: number;
  status: ServerStatus;
  health_score: number;
  cpu_usage: number;
  memory_usage: number;
  response_time_ms: number;
  last_heartbeat: Date;
  created_at: Date;
  updated_at: Date;
  metadata?: Record<string, any>;
}

export interface ServerRegistrationRequest {
  server_id: string;
  server_name: string;
  server_type: ServerType;
  region: ServerRegion;
  host_address: string;
  port: number;
  websocket_port?: number;
  capacity?: number;
  metadata?: Record<string, any>;
}

export interface ServerHealthUpdate {
  server_id: string;
  cpu_usage: number;
  memory_usage: number;
  response_time_ms: number;
  current_load: number;
  health_score: number;
}

export interface ServerMetrics {
  server_id: string;
  timestamp: Date;
  cpu_usage: number;
  memory_usage: number;
  network_latency: number;
  active_connections: number;
  requests_per_second: number;
  error_rate: number;
  uptime_percentage: number;
}

// =====================================================
// PLAYER ROUTING TYPES
// =====================================================

export interface ShardPlayer {
  id: number;
  user_id: number;
  server_id: string;
  session_id?: string;
  assigned_at: Date;
  last_active: Date;
  connection_quality: number;
  preferred_region?: string;
  is_active: boolean;
  metadata?: Record<string, any>;
}

export interface PlayerRoutingRequest {
  user_id: number;
  preferred_region?: ServerRegion;
  alliance_id?: number;
  session_id?: string;
}

export interface PlayerRoutingResult {
  server_id: string;
  server_name: string;
  host_address: string;
  port: number;
  websocket_port?: number;
  region: ServerRegion;
  estimated_latency: number;
  routing_algorithm: LoadBalancingAlgorithm;
}

export interface PlayerMigrationRequest {
  user_id: number;
  session_id?: string;
  from_server_id: string;
  to_server_id: string;
  reason: string;
  preserve_session: boolean;
}

export interface LoadBalancerConfig {
  algorithm: LoadBalancingAlgorithm;
  health_check_interval: number;
  max_server_load: number;
  failover_enabled: boolean;
  geographic_regions: ServerRegion[];
  weighted_factors?: {
    cpu_weight: number;
    memory_weight: number;
    latency_weight: number;
    load_weight: number;
  };
}

// =====================================================
// LEADERBOARD TYPES
// =====================================================

export interface ShardLeaderboard {
  id: number;
  user_id: number;
  server_id: string;
  category: LeaderboardCategory;
  score: number;
  rank: number;
  previous_rank?: number;
  rank_change: number;
  alliance_id?: number;
  last_updated: Date;
  metadata?: Record<string, any>;
}

export interface LeaderboardEntry {
  rank: number;
  user_id: number;
  username: string;
  server_id: string;
  score: number;
  rank_change: number;
  alliance_name?: string;
  metadata?: Record<string, any>;
}

export interface LeaderboardSnapshot {
  id: number;
  snapshot_date: Date;
  period: LeaderboardPeriod;
  user_id: number;
  category: LeaderboardCategory;
  score: number;
  rank: number;
  server_id: string;
}

export interface GlobalLeaderboardRequest {
  category: LeaderboardCategory;
  period?: LeaderboardPeriod;
  limit?: number;
  offset?: number;
  server_id?: string;
  alliance_id?: number;
}

// =====================================================
// CHAT TYPES
// =====================================================

export interface ShardChatMessage {
  id: number;
  channel_id: string;
  channel_type: ChatChannelType;
  user_id: number;
  server_id: string;
  message: string;
  priority: MessagePriority;
  is_system: boolean;
  metadata?: Record<string, any>;
  created_at: Date;
}

export interface ChatBroadcastRequest {
  channel_type: ChatChannelType;
  channel_id?: string;
  message: string;
  user_id?: number;
  priority?: MessagePriority;
  target_servers?: string[];
  metadata?: Record<string, any>;
}

export interface ChatChannel {
  channel_id: string;
  channel_type: ChatChannelType;
  name: string;
  owner_id?: number;
  server_ids: string[];
  is_active: boolean;
  max_participants?: number;
  created_at: Date;
}

// =====================================================
// CROSS-SERVER COMMUNICATION
// =====================================================

export interface CrossServerEvent {
  id: number;
  event_type: string;
  source_server_id: string;
  target_server_ids: string[];
  payload: Record<string, any>;
  priority: MessagePriority;
  requires_ack: boolean;
  created_at: Date;
  processed_at?: Date;
}

export interface ServerMessageEnvelope {
  message_id: string;
  source_server: string;
  target_servers: string[];
  message_type: string;
  payload: any;
  timestamp: Date;
  priority: MessagePriority;
  ttl?: number;
}

export interface MessageAcknowledgement {
  message_id: string;
  server_id: string;
  status: 'received' | 'processed' | 'failed';
  timestamp: Date;
  error?: string;
}

// =====================================================
// RESOURCE MARKET TYPES
// =====================================================

export interface CrossServerMarket {
  id: number;
  resource_type: string;
  quantity: number;
  price_per_unit: number;
  seller_user_id: number;
  seller_server_id: string;
  buyer_user_id?: number;
  buyer_server_id?: string;
  status: 'listed' | 'pending' | 'completed' | 'cancelled';
  expires_at: Date;
  created_at: Date;
}

export interface MarketTradeRequest {
  resource_type: string;
  quantity: number;
  price_per_unit: number;
  buyer_user_id: number;
  buyer_server_id: string;
  seller_user_id: number;
  seller_server_id: string;
}

export interface MarketPriceData {
  resource_type: string;
  average_price: number;
  min_price: number;
  max_price: number;
  total_volume: number;
  price_trend: 'rising' | 'falling' | 'stable';
  last_updated: Date;
}

// =====================================================
// MONITORING TYPES
// =====================================================

export interface HealthCheckResult {
  server_id: string;
  status: ServerStatus;
  health_score: number;
  checks: {
    api_responsive: boolean;
    database_connected: boolean;
    redis_connected: boolean;
    websocket_active: boolean;
    disk_space_available: boolean;
  };
  metrics: {
    cpu_usage: number;
    memory_usage: number;
    response_time: number;
    active_connections: number;
  };
  timestamp: Date;
}

export interface SystemHealthOverview {
  total_servers: number;
  online_servers: number;
  offline_servers: number;
  maintenance_servers: number;
  degraded_servers: number;
  total_capacity: number;
  total_load: number;
  average_health_score: number;
  average_cpu_usage: number;
  average_memory_usage: number;
  timestamp: Date;
}

export interface AlertRule {
  id: number;
  rule_name: string;
  metric: string;
  threshold: number;
  comparison: 'greater_than' | 'less_than' | 'equals';
  severity: 'low' | 'medium' | 'high' | 'critical';
  is_active: boolean;
  notification_channels: string[];
}

export interface PerformanceAlert {
  id: number;
  alert_rule_id: number;
  server_id: string;
  metric: string;
  current_value: number;
  threshold_value: number;
  severity: string;
  message: string;
  created_at: Date;
  resolved_at?: Date;
}

// =====================================================
// RESPONSE TYPES
// =====================================================

export interface ShardingApiResponse<T = any> {
  success: boolean;
  data?: T;
  error?: string;
  metadata?: {
    server_id?: string;
    timestamp: Date;
    processing_time_ms?: number;
  };
}

export interface PaginatedResponse<T> {
  data: T[];
  total: number;
  page: number;
  per_page: number;
  total_pages: number;
}
