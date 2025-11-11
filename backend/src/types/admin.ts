import { Request } from 'express';
import { User } from './index';

// ========================================
// ADMIN USER TYPES
// ========================================

export interface AdminUser {
  id: number;
  user_id: number;
  role_id: number;
  role_name: string;
  permissions: string[];
  created_at: Date;
  created_by?: number;
  two_factor_enabled: boolean;
  two_factor_secret?: string;
  ip_whitelist: string[];
  last_login?: Date;
  is_active: boolean;
  notes?: string;
}


// ========================================
// AUDIT LOG TYPES
// ========================================

export type ActionCategory = 'user_management' | 'game_config' | 'server_control' | 'data_modification' | 'security' | 'monitoring';
export type Severity = 'low' | 'medium' | 'high' | 'critical';

export interface AdminAuditLog {
  id: number;
  admin_id?: number;
  admin_username: string;
  action_type: string;
  action_category: ActionCategory;
  target_type?: string;
  target_id?: number;
  target_identifier?: string;
  action_details?: Record<string, any>;
  before_state?: Record<string, any>;
  after_state?: Record<string, any>;
  ip_address?: string;
  user_agent?: string;
  timestamp: Date;
  severity: Severity;
  success: boolean;
  error_message?: string;
}

// ========================================
// ADMIN SETTINGS TYPES
// ========================================

export type SettingCategory = 'game_mechanics' | 'economy' | 'combat' | 'limits' | 'server' | 'security' | 'features';
export type SettingDataType = 'string' | 'number' | 'boolean' | 'json' | 'array';

export interface AdminSetting {
  id: number;
  setting_key: string;
  setting_value: any;
  setting_category: SettingCategory;
  description?: string;
  data_type: SettingDataType;
  is_public: boolean;
  requires_restart: boolean;
  last_modified: Date;
  modified_by?: number;
  version: number;
  previous_value?: any;
}

// ========================================
// USER BLOCK TYPES
// ========================================

export type BlockType = 'ban' | 'mute' | 'restrict' | 'warning';
export type AppealStatus = 'pending' | 'approved' | 'rejected' | 'none';

export interface UserBlock {
  id: number;
  user_id: number;
  block_type: BlockType;
  reason: string;
  duration_minutes?: number;
  start_time: Date;
  end_time?: Date;
  is_permanent: boolean;
  is_active: boolean;
  blocked_by?: number;
  unblocked_by?: number;
  unblock_time?: Date;
  unblock_reason?: string;
  appeal_status: AppealStatus;
  notes?: string;
  severity_level?: number;
}

// ========================================
// PLAYER TAG TYPES
// ========================================

export type TagCategory = 'behavior' | 'payment' | 'skill' | 'special' | 'support' | 'custom';

export interface PlayerTag {
  id: number;
  user_id: number;
  tag_name: string;
  tag_category: TagCategory;
  tag_color?: string;
  description?: string;
  added_by?: number;
  added_at: Date;
  expires_at?: Date;
  is_active: boolean;
  metadata?: Record<string, any>;
}

// ========================================
// NOTIFICATION TYPES
// ========================================

export type NotificationPriority = 'low' | 'medium' | 'high' | 'critical';

export interface AdminNotification {
  id: number;
  notification_type: string;
  priority: NotificationPriority;
  title: string;
  message: string;
  data?: Record<string, any>;
  target_admin_role?: string;
  target_admin_level?: string;
  target_admin_ids?: number[];
  created_at: Date;
  expires_at?: Date;
  is_read: boolean;
  read_by?: number[];
  action_url?: string;
  requires_acknowledgment: boolean;
  acknowledged_by?: number[];
  auto_dismiss: boolean;
}

// ========================================
// SERVER MONITORING TYPES
// ========================================

export interface ServerMetric {
  id: number;
  metric_type: string;
  metric_name: string;
  metric_value: number;
  metric_unit?: string;
  timestamp: Date;
  server_instance?: string;
  metadata?: Record<string, any>;
  threshold_exceeded: boolean;
  alert_sent: boolean;
}

// ========================================
// GAME EVENT TYPES
// ========================================

export type EventType = 'announcement' | 'maintenance' | 'tournament' | 'bonus' | 'special_event' | 'emergency';
export type TargetScope = 'all' | 'alliance' | 'user' | 'galaxy' | 'custom';
export type EventVisibility = 'public' | 'hidden' | 'admin_only';

export interface GameEvent {
  id: number;
  event_type: EventType;
  event_name: string;
  event_description?: string;
  event_data?: Record<string, any>;
  start_time: Date;
  end_time?: Date;
  is_active: boolean;
  is_recurring: boolean;
  recurrence_pattern?: string;
  target_scope: TargetScope;
  target_ids?: number[];
  created_by?: number;
  created_at: Date;
  modified_at?: Date;
  priority: number;
  visibility: EventVisibility;
  requires_participation: boolean;
  participation_count: number;
  rewards?: Record<string, any>;
  conditions?: Record<string, any>;
}

// ========================================
// REQUEST EXTENSIONS
// ========================================

export interface AdminAuthRequest extends Request {
  user?: User;
  admin?: AdminUser;
  adminRole?: string;
  adminPermissions?: string[];
  twoFactorVerified?: boolean;
  adminLevel?: number;
}

// ========================================
// ANALYTICS & REPORTING TYPES
// ========================================

export interface UserAnalytics {
  total_users: number;
  active_users_today: number;
  active_users_week: number;
  active_users_month: number;
  new_users_today: number;
  new_users_week: number;
  new_users_month: number;
  banned_users: number;
  suspended_users: number;
  retention_rate_7day: number;
  retention_rate_30day: number;
  avg_session_duration: number;
  churn_rate: number;
}

export interface ResourceAnalytics {
  total_metal: number;
  total_crystal: number;
  total_deuterium: number;
  metal_production_rate: number;
  crystal_production_rate: number;
  deuterium_production_rate: number;
  avg_resources_per_user: {
    metal: number;
    crystal: number;
    deuterium: number;
  };
}

export interface CombatAnalytics {
  total_battles: number;
  battles_today: number;
  total_ships_destroyed: number;
  most_used_ships: Array<{ ship_type: string; count: number }>;
  top_attackers: Array<{ user_id: number; username: string; wins: number }>;
  combat_balance_score: number;
}

export interface ServerHealth {
  cpu_usage: number;
  memory_usage: number;
  database_connections: number;
  active_players: number;
  api_response_time: number;
  error_rate: number;
  uptime: number;
  status: 'healthy' | 'warning' | 'critical';
}

// ========================================
// ADMIN ACTION TYPES
// ========================================

export interface BlockUserAction {
  user_id: number;
  block_type: BlockType;
  reason: string;
  duration_minutes?: number;
  is_permanent?: boolean;
  severity_level?: number;
}

export interface TagUserAction {
  user_id: number;
  tag_name: string;
  tag_category: TagCategory;
  tag_color?: string;
  description?: string;
  expires_at?: Date;
}

export interface AdjustResourcesAction {
  user_id: number;
  planet_id?: number;
  metal?: number;
  crystal?: number;
  deuterium?: number;
  dark_matter?: number;
  reason: string;
}

export interface TriggerEventAction {
  event_type: EventType;
  event_name: string;
  event_description?: string;
  event_data?: Record<string, any>;
  priority?: number;
  start_time: Date;
  end_time?: Date;
  target_scope: TargetScope;
  target_ids?: number[];
  rewards?: Record<string, any>;
}

// ========================================
// ADMIN DASHBOARD TYPES
// ========================================

export interface AdminDashboard {
  server_health: ServerHealth;
  user_analytics: UserAnalytics;
  resource_analytics: ResourceAnalytics;
  combat_analytics: CombatAnalytics;
  recent_audit_logs: AdminAuditLog[];
  active_events: GameEvent[];
  pending_reports: number;
  critical_alerts: AdminNotification[];
  online_admins: number;
}



// ========================================
// UTILITY TYPES
// ========================================

export interface PaginatedResponse<T> {
  data: T[];
  total: number;
  page: number;
  limit: number;
  totalPages: number;
}

export interface AdminFilter {
  search?: string;
  status?: string;
  dateFrom?: Date;
  dateTo?: Date;
  category?: string;
  sortBy?: string;
  sortOrder?: 'ASC' | 'DESC';
  page?: number;
  limit?: number;
}

export interface BulkAction {
  action: string;
  target_ids: number[];
  params?: Record<string, any>;
  reason?: string;
}
