/**
 * PHASE 6: REAL-TIME COMMUNICATION SYSTEMS
 * TypeScript type definitions for chat, notifications, player status, and real-time events
 */

// =====================================================
// 1. CHAT SYSTEM TYPES
// =====================================================

export enum ChatChannelType {
  GLOBAL = 'global',
  SECTOR = 'sector',
  ALLIANCE = 'alliance',
  PRIVATE = 'private',
  TRADE = 'trade',
  HELP = 'help',
  COMBAT = 'combat',
}

export enum ChatMessageType {
  TEXT = 'text',
  SYSTEM = 'system',
  COMBAT = 'combat',
  TRADE = 'trade',
  FLEET = 'fleet',
}

export enum ChatReactionType {
  THUMBS_UP = 'thumbs_up',
  THUMBS_DOWN = 'thumbs_down',
  ROFL = 'rofl',
  CLAP = 'clap',
  ANGRY = 'angry',
  CRY = 'cry',
}

export interface ChatChannel {
  id: number;
  channel_name: string;
  channel_type: ChatChannelType;
  description?: string;
  is_active: boolean;
  max_message_length: number;
  rate_limit_seconds: number;
  created_at: Date;
}

export interface ChatMessage {
  id: number;
  channel_id: number;
  user_id: number;
  message: string;
  message_type: ChatMessageType;
  is_edited: boolean;
  edited_at?: Date;
  is_deleted: boolean;
  deleted_at?: Date;
  is_flagged: boolean;
  flag_reason?: string;
  flagged_by?: number;
  flagged_at?: Date;
  reference_type?: string;
  reference_id?: number;
  created_at: Date;
  is_announcement?: boolean;
  announcement_expires_at?: Date;
  is_pinned?: boolean;
  pinned_by?: number;
  pinned_at?: Date;
  reactions?: Partial<Record<ChatReactionType, number>>;
  viewerReactions?: ChatReactionType[];
  
  // Joined fields
  username?: string;
  alliance_tag?: string;
}

export interface PrivateConversation {
  id: number;
  user1_id: number;
  user2_id: number;
  last_message_at: Date;
  user1_unread_count: number;
  user2_unread_count: number;
  created_at: Date;
  
  // Joined fields
  other_user_id?: number;
  other_username?: string;
  last_message?: string;
}

export interface PrivateMessage {
  id: number;
  conversation_id: number;
  sender_id: number;
  message: string;
  is_read: boolean;
  read_at?: Date;
  is_deleted_by_sender: boolean;
  is_deleted_by_receiver: boolean;
  created_at: Date;
  
  // Joined fields
  sender_username?: string;
}

export enum ChatRestrictionType {
  MUTE = 'mute',
  BAN = 'ban',
  SLOWMODE = 'slowmode',
}

export interface ChatRestriction {
  id: number;
  user_id: number;
  channel_id?: number;
  restriction_type: ChatRestrictionType;
  reason?: string;
  restricted_by: number;
  expires_at?: Date;
  created_at: Date;
}

// =====================================================
// 2. NOTIFICATION SYSTEM TYPES
// =====================================================

export enum NotificationCategory {
  COMBAT = 'combat',
  FLEET = 'fleet',
  RESOURCE = 'resource',
  ALLIANCE = 'alliance',
  TRADE = 'trade',
  SYSTEM = 'system',
  ACHIEVEMENT = 'achievement',
}

export interface NotificationType {
  id: number;
  type_name: string;
  category: NotificationCategory;
  description?: string;
  default_priority: number;
  icon?: string;
  sound_enabled: boolean;
  is_active: boolean;
  created_at: Date;
}

export interface Notification {
  id: number;
  user_id: number;
  notification_type_id: number;
  title: string;
  message: string;
  priority: number;
  is_read: boolean;
  read_at?: Date;
  is_archived: boolean;
  archived_at?: Date;
  action_url?: string;
  action_label?: string;
  reference_type?: string;
  reference_id?: number;
  metadata?: any;
  created_at: Date;
  expires_at?: Date;
  
  // Joined fields
  type_name?: string;
  category?: NotificationCategory;
  icon?: string;
}

export interface NotificationPreferences {
  id: number;
  user_id: number;
  notification_type_id: number;
  enabled: boolean;
  sound_enabled: boolean;
  desktop_enabled: boolean;
  min_priority: number;
  created_at: Date;
  updated_at: Date;
}

// =====================================================
// 3. PLAYER STATUS TYPES
// =====================================================

export enum PlayerStatus {
  ONLINE = 'online',
  OFFLINE = 'offline',
  AWAY = 'away',
  BUSY = 'busy',
  IN_COMBAT = 'in_combat',
}

export interface PlayerStatusInfo {
  user_id: number;
  status: PlayerStatus;
  status_message?: string;
  last_activity: Date;
  last_action?: string;
  current_planet_id?: number;
  session_id?: string;
  socket_id?: string;
  session_count: number;
  total_online_time: number;
  updated_at: Date;
  
  // Computed fields
  actual_status?: 'active' | 'idle' | 'offline';
  seconds_since_activity?: number;
}

export enum PlayerActivityType {
  LOGIN = 'login',
  LOGOUT = 'logout',
  BUILDING_UPGRADE = 'building_upgrade',
  RESEARCH_START = 'research_start',
  FLEET_DISPATCH = 'fleet_dispatch',
  FLEET_RETURN = 'fleet_return',
  COMBAT = 'combat',
  TRADE = 'trade',
  CHAT_MESSAGE = 'chat_message',
  RESOURCE_COLLECT = 'resource_collect',
  ALLIANCE_JOIN = 'alliance_join',
  ALLIANCE_LEAVE = 'alliance_leave',
  PLANET_VIEW = 'planet_view',
  GALAXY_SCAN = 'galaxy_scan',
}

export interface PlayerActivityLog {
  id: number;
  user_id: number;
  activity_type: PlayerActivityType;
  activity_data?: any;
  planet_id?: number;
  created_at: Date;
}

// =====================================================
// 4. FLEET TRACKING TYPES
// =====================================================

export enum FleetEventType {
  DISPATCHED = 'dispatched',
  MOVING = 'moving',
  CHECKPOINT = 'checkpoint',
  ARRIVED = 'arrived',
  RETURNED = 'returned',
  COMBAT_STARTED = 'combat_started',
  COMBAT_ENDED = 'combat_ended',
  RECALLED = 'recalled',
  DESTROYED = 'destroyed',
}

export interface FleetEvent {
  id: number;
  fleet_id: number;
  event_type: FleetEventType;
  event_data?: any;
  current_galaxy?: number;
  current_system?: number;
  current_position?: number;
  progress_percent?: number;
  estimated_arrival?: Date;
  created_at: Date;
}

export enum FleetWatchType {
  OWNER = 'owner',
  TARGET = 'target',
  ALLIANCE = 'alliance',
  SPY = 'spy',
}

export interface FleetWatcher {
  id: number;
  fleet_id: number;
  user_id: number;
  watch_type: FleetWatchType;
  created_at: Date;
}

// =====================================================
// 5. COMBAT ALERT TYPES
// =====================================================

export enum CombatAlertType {
  COMBAT_STARTED = 'combat_started',
  ROUND_COMPLETE = 'round_complete',
  COMBAT_ENDED = 'combat_ended',
  FLEET_DESTROYED = 'fleet_destroyed',
  DEFENSE_DESTROYED = 'defense_destroyed',
  RESOURCES_PLUNDERED = 'resources_plundered',
}

export interface CombatAlert {
  id: number;
  combat_id: number;
  alert_type: CombatAlertType;
  attacker_id: number;
  defender_id: number;
  alert_data: any;
  severity: number;
  attacker_read: boolean;
  defender_read: boolean;
  created_at: Date;
}

// =====================================================
// 6. TRADING & COMMERCE TYPES
// =====================================================

export enum TradeOfferType {
  SELL = 'sell',
  BUY = 'buy',
  EXCHANGE = 'exchange',
}

export enum TradeOfferStatus {
  ACTIVE = 'active',
  COMPLETED = 'completed',
  CANCELLED = 'cancelled',
  EXPIRED = 'expired',
}

export type ResourceType = 'metal' | 'crystal' | 'deuterium' | 'dark_matter';

export interface TradeOffer {
  id: number;
  seller_id: number;
  offer_type: TradeOfferType;
  resource_offered: ResourceType;
  amount_offered: number;
  resource_wanted: ResourceType;
  amount_wanted: number;
  exchange_rate: number;
  status: TradeOfferStatus;
  min_reputation?: number;
  alliance_only: boolean;
  target_alliance_id?: number;
  buyer_id?: number;
  completed_at?: Date;
  created_at: Date;
  expires_at: Date;
  
  // Joined fields
  seller_username?: string;
  seller_alliance_tag?: string;
  seconds_until_expiry?: number;
}

export interface TradeTransaction {
  id: number;
  trade_offer_id?: number;
  seller_id: number;
  buyer_id: number;
  resource_given: ResourceType;
  amount_given: number;
  resource_received: ResourceType;
  amount_received: number;
  transaction_fee: number;
  exchange_rate: number;
  created_at: Date;
  
  // Joined fields
  seller_username?: string;
  buyer_username?: string;
}

// =====================================================
// 7. SOCKET.IO EVENT TYPES
// =====================================================

export interface SocketUser {
  userId: number;
  username: string;
  socket: any;
}

// Chat Events
export interface ChatMessageEvent {
  channelId: number;
  message: ChatMessage;
}

export interface PrivateMessageEvent {
  conversationId: number;
  senderId: number;
  senderUsername: string;
  receiverId: number;
  message: string;
  timestamp: Date;
  messageId?: number;
}

// Notification Events
export interface NotificationEvent {
  notificationId: number;
  userId: number;
  type: string;
  category: NotificationCategory;
  title: string;
  message: string;
  priority: number;
  actionUrl?: string;
  actionLabel?: string;
  icon?: string;
  timestamp: Date;
}

// Player Status Events
export interface PlayerStatusEvent {
  userId: number;
  username: string;
  status: PlayerStatus;
  statusMessage?: string;
  lastActivity: Date;
}

// Fleet Events
export interface FleetMovementEvent {
  fleetId: number;
  ownerId: number;
  eventType: FleetEventType;
  progressPercent: number;
  estimatedArrival: Date;
  currentLocation: {
    galaxy: number;
    system: number;
    position: number;
  };
}

// Combat Events
export interface CombatAlertEvent {
  combatId: number;
  alertType: CombatAlertType;
  attackerId: number;
  attackerUsername: string;
  defenderId: number;
  defenderUsername: string;
  severity: number;
  data: any;
  timestamp: Date;
}

// Trade Events
export interface TradeUpdateEvent {
  tradeId: number;
  sellerId: number;
  sellerUsername: string;
  resourceOffered: ResourceType;
  amountOffered: number;
  resourceWanted: ResourceType;
  amountWanted: number;
  exchangeRate: number;
  expiresAt: Date;
}

// =====================================================
// 8. REQUEST/RESPONSE TYPES
// =====================================================

// Chat Requests
export interface SendChatMessageRequest {
  channelId: number;
  message: string;
  messageType?: ChatMessageType;
  isAnnouncement?: boolean;
  announcementExpiresAt?: Date;
  pinMessage?: boolean;
}

export interface SendPrivateMessageRequest {
  receiverId: number;
  message: string;
}

export interface GetChatHistoryRequest {
  channelId: number;
  limit?: number;
  before?: Date;
  viewerUserId?: number;
}

export interface GetPrivateConversationsRequest {
  limit?: number;
  offset?: number;
}

export interface GetPrivateMessagesRequest {
  conversationId: number;
  limit?: number;
  before?: Date;
}

// Notification Requests
export interface CreateNotificationRequest {
  userId: number;
  notificationTypeId: number;
  title: string;
  message: string;
  priority?: number;
  actionUrl?: string;
  actionLabel?: string;
  referenceType?: string;
  referenceId?: number;
  metadata?: any;
}

export interface GetNotificationsRequest {
  userId: number;
  unreadOnly?: boolean;
  category?: NotificationCategory;
  limit?: number;
  offset?: number;
}

export interface UpdateNotificationPreferencesRequest {
  notificationTypeId: number;
  enabled?: boolean;
  soundEnabled?: boolean;
  desktopEnabled?: boolean;
  minPriority?: number;
}

// Player Status Requests
export interface UpdatePlayerStatusRequest {
  status: PlayerStatus;
  statusMessage?: string;
  currentPlanetId?: number;
}

export interface GetOnlinePlayersRequest {
  limit?: number;
  allianceId?: number;
}

// Trade Requests
export interface CreateTradeOfferRequest {
  offerType: TradeOfferType;
  resourceOffered: ResourceType;
  amountOffered: number;
  resourceWanted: ResourceType;
  amountWanted: number;
  minReputation?: number;
  allianceOnly?: boolean;
  targetAllianceId?: number;
  expiresInHours?: number;
}

export interface AcceptTradeOfferRequest {
  tradeOfferId: number;
}

export interface GetTradeOffersRequest {
  status?: TradeOfferStatus;
  resourceOffered?: ResourceType;
  resourceWanted?: ResourceType;
  allianceOnly?: boolean;
  limit?: number;
  offset?: number;
}

// =====================================================
// 9. RESPONSE TYPES
// =====================================================

export interface ChatHistoryResponse {
  messages: ChatMessage[];
  pinnedMessages: ChatMessage[];
  announcements: ChatMessage[];
  hasMore: boolean;
  total: number;
}

export interface PrivateConversationsResponse {
  conversations: PrivateConversation[];
  total: number;
}

export interface PrivateMessagesResponse {
  messages: PrivateMessage[];
  hasMore: boolean;
  total: number;
}

export interface NotificationsResponse {
  notifications: Notification[];
  total: number;
  unreadCount: number;
}

export interface OnlinePlayersResponse {
  players: PlayerStatusInfo[];
  total: number;
  onlineCount: number;
}

export interface TradeOffersResponse {
  offers: TradeOffer[];
  total: number;
}

export interface TradeTransactionsResponse {
  transactions: TradeTransaction[];
  total: number;
}

// =====================================================
// 10. ANALYTICS TYPES
// =====================================================

export interface ChatActivityStats {
  channel_name: string;
  channel_type: ChatChannelType;
  message_count: number;
  unique_users: number;
  last_message_at?: Date;
  messages_last_hour: number;
}

export interface UserUnreadStats {
  user_id: number;
  username: string;
  unread_count: number;
  urgent_count: number;
  latest_notification_at?: Date;
  unread_combat: number;
  unread_fleet: number;
  unread_trade: number;
}

export interface ActiveTradesStats {
  id: number;
  seller_username: string;
  resource_offered: ResourceType;
  amount_offered: number;
  resource_wanted: ResourceType;
  amount_wanted: number;
  exchange_rate: number;
  created_at: Date;
  expires_at: Date;
  seconds_until_expiry: number;
}

// =====================================================
// 11. UTILITY TYPES
// =====================================================

export interface RateLimitInfo {
  userId: number;
  channelId: number;
  lastMessageAt: Date;
  messageCount: number;
  isLimited: boolean;
}

export interface ChatModerationAction {
  userId: number;
  moderatorId: number;
  action: 'mute' | 'ban' | 'warn' | 'delete';
  reason: string;
  duration?: number; // in seconds
  channelId?: number;
}

export interface NotificationBatch {
  userIds: number[];
  notificationTypeId: number;
  title: string;
  message: string;
  priority?: number;
  metadata?: any;
}

export interface FleetTrackingUpdate {
  fleetId: number;
  progressPercent: number;
  estimatedArrival: Date;
  currentLocation: {
    galaxy: number;
    system: number;
    position: number;
  };
}

// =====================================================
// END OF REALTIME TYPES
// =====================================================
