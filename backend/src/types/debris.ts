// ========================================
// COMBAT DEBRIS & LOOT SYSTEM TYPES
// ========================================

export type DebrisType = 'light' | 'heavy' | 'wreckage' | 'components' | 'rare' | 'radiation';
export type QualityGrade = 'poor' | 'common' | 'uncommon' | 'rare' | 'legendary';
export type SalvageType = 'automated' | 'manual' | 'alliance' | 'emergency' | 'deep_space' | 'commercial';
export type SalvageStatus = 'planned' | 'en_route' | 'salvaging' | 'returning' | 'completed' | 'failed';
export type ComponentType = 'engine' | 'weapon' | 'armor' | 'electronics' | 'advanced_material' | 'research_data';
export type ClaimType = 'exclusive' | 'shared' | 'contested' | 'alliance';
export type EventType = 'combat' | 'asteroid_mining' | 'ship_destruction' | 'station_destruction' | 'natural_disaster';
export type CombatResult = 'attacker_victory' | 'defender_victory' | 'draw' | 'mutual_destruction';

// Constants for DebrisType values
export const DebrisTypeValues = {
  LIGHT: 'light' as DebrisType,
  HEAVY: 'heavy' as DebrisType,
  WRECKAGE: 'wreckage' as DebrisType,
  COMPONENTS: 'components' as DebrisType,
  RARE: 'rare' as DebrisType,
  RADIATION: 'radiation' as DebrisType,
} as const;

// Constants for SalvageType values
export const SalvageTypeValues = {
  AUTOMATED: 'automated' as SalvageType,
  MANUAL: 'manual' as SalvageType,
  ALLIANCE: 'alliance' as SalvageType,
  EMERGENCY: 'emergency' as SalvageType,
  DEEP_SPACE: 'deep_space' as SalvageType,
  COMMERCIAL: 'commercial' as SalvageType,
} as const;

// ========================================
// DEBRIS FIELD TYPES
// ========================================

export interface CombatDebris {
  id: number;
  galaxy: number;
  system: number;
  position: number;
  debris_type: DebrisType;
  total_metal: number;
  total_crystal: number;
  total_deuterium: number;
  total_rare_materials: number;
  created_at: Date;
  created_by_combat_id?: number;
  decay_start: Date;
  decay_rate: number;
  expires_at?: Date;
  is_active: boolean;
  is_claimed: boolean;
  claimed_by?: number;
  claimed_at?: Date;
  hazard_level: number;
  radiation_level: number;
  spread_radius: number;
  metadata?: Record<string, any>;
}

export interface DebrisResource {
  id: number;
  debris_id: number;
  resource_type: string;
  resource_subtype?: string;
  quantity: number;
  quality_grade: QualityGrade;
  recyclable: boolean;
  recycle_efficiency: number;
  position_x?: number;
  position_y?: number;
  position_z?: number;
  is_collected: boolean;
  collected_by?: number;
  collected_at?: Date;
  created_at: Date;
}

// ========================================
// SALVAGE OPERATION TYPES
// ========================================

export interface DebrisSalvage {
  id: number;
  user_id: number;
  debris_id: number;
  salvage_type: SalvageType;
  fleet_id?: number;
  ship_types?: Record<string, number>;
  cargo_capacity: number;
  salvage_efficiency: number;
  status: SalvageStatus;
  start_time: Date;
  arrival_time?: Date;
  completion_time?: Date;
  return_time?: Date;
  resources_collected?: ResourceCollection;
  components_collected?: ComponentCollection;
  total_value: number;
  experience_gained: number;
  success_rate?: number;
  hazards_encountered?: Hazard[];
  alliance_id?: number;
  is_competitive: boolean;
  ranking?: number;
  notes?: string;
}

export interface ResourceCollection {
  metal: number;
  crystal: number;
  deuterium: number;
  rare_materials?: number;
}

export interface ComponentCollection {
  [componentId: number]: {
    quantity: number;
    quality: QualityGrade;
  };
}

export interface Hazard {
  type: string;
  severity: number;
  damage?: number;
  description: string;
}

// ========================================
// CLAIMS AND OWNERSHIP
// ========================================

export interface DebrisClaim {
  id: number;
  debris_id: number;
  user_id: number;
  alliance_id?: number;
  claim_type: ClaimType;
  claim_start: Date;
  claim_duration: number;
  claim_expires?: Date;
  is_active: boolean;
  priority_level: number;
  claim_reason?: string;
}

// ========================================
// SHIP COMPONENTS
// ========================================

export interface ShipComponent {
  id: number;
  component_type: ComponentType;
  component_name: string;
  component_subtype?: string;
  quality_grade: QualityGrade;
  condition_percent: number;
  source_ship_type?: string;
  tech_level: number;
  recycle_value_metal: number;
  recycle_value_crystal: number;
  recycle_value_deuterium: number;
  recycle_efficiency: number;
  market_value: number;
  is_tradeable: boolean;
  is_unique: boolean;
  required_tech?: Record<string, number>;
  bonus_stats?: Record<string, any>;
  description?: string;
  created_at: Date;
}

export interface PlayerComponentInventory {
  id: number;
  user_id: number;
  component_id: number;
  quantity: number;
  acquired_from?: string;
  acquired_at: Date;
  is_equipped: boolean;
  equipped_to_ship?: string;
}

// ========================================
// DEBRIS EVENTS
// ========================================

export interface DebrisEvent {
  id: number;
  event_type: EventType;
  debris_id?: number;
  galaxy: number;
  system: number;
  position: number;
  attacker_id?: number;
  defender_id?: number;
  attacker_alliance?: number;
  defender_alliance?: number;
  ships_destroyed?: Record<string, number>;
  total_destroyed_value: number;
  debris_generated_metal: number;
  debris_generated_crystal: number;
  debris_generated_deuterium: number;
  debris_generation_rate: number;
  rare_components_generated: number;
  combat_result?: CombatResult;
  timestamp: Date;
  battle_duration?: number;
  metadata?: Record<string, any>;
}

// ========================================
// CLEANUP AND MAINTENANCE
// ========================================

export interface DebrisCleanup {
  id: number;
  debris_id: number;
  cleanup_type: 'automatic' | 'manual' | 'forced' | 'maintenance';
  scheduled_at: Date;
  executed_at?: Date;
  status: 'pending' | 'in_progress' | 'completed' | 'failed';
  resources_recovered?: ResourceCollection;
  cleanup_crew?: number;
  cleanup_reason?: string;
  performance_impact_before?: number;
  performance_impact_after?: number;
}

// ========================================
// SALVAGE STATISTICS
// ========================================

export interface SalvageStatistics {
  id: number;
  user_id: number;
  total_salvage_missions: number;
  successful_missions: number;
  failed_missions: number;
  total_metal_collected: number;
  total_crystal_collected: number;
  total_deuterium_collected: number;
  total_rare_materials: number;
  total_components_found: number;
  legendary_components: number;
  total_salvage_value: number;
  fastest_salvage_time?: number;
  largest_single_haul: number;
  salvage_efficiency_avg: number;
  competitive_wins: number;
  alliance_contributions: number;
  salvage_experience_points: number;
  salvage_level: number;
  salvage_rank?: string;
  last_salvage_at?: Date;
  updated_at: Date;
}

// ========================================
// ACTION TYPES
// ========================================

export interface CreateDebrisAction {
  galaxy: number;
  system: number;
  position: number;
  destroyed_ships: Record<string, number>;
  total_value: number;
  debris_rate?: number;
  attacker_id?: number;
  defender_id?: number;
  combat_result?: CombatResult;
}

export interface StartSalvageAction {
  user_id: number;
  debris_id: number;
  salvage_type: SalvageType;
  fleet_id?: number;
  ship_types: Record<string, number>;
  is_competitive?: boolean;
}

export interface ClaimDebrisAction {
  debris_id: number;
  user_id: number;
  claim_type: ClaimType;
  claim_duration?: number;
  alliance_id?: number;
}

export interface RecycleComponentAction {
  user_id: number;
  component_id: number;
  quantity: number;
}

// ========================================
// RESPONSE TYPES
// ========================================

export interface DebrisFieldInfo extends CombatDebris {
  debris: CombatDebris;
  resource_count: number;
  total_resources: number;
  claimed_by_user?: number;
  claimant_username?: string;
  total_value: number;
  hours_remaining: number;
  nearby_salvagers: number;
}

export interface SalvageResult {
  success: boolean;
  resources_collected: ResourceCollection;
  components_found: ShipComponent[];
  total_value: number;
  experience_gained: number;
  salvage_efficiency: number;
  hazards_encountered: Hazard[];
  ranking?: number;
}

export interface DebrisAnalytics {
  total_fields_active: number;
  total_value_in_space: number;
  fields_created_today: number;
  fields_claimed_today: number;
  top_debris_locations: Array<{
    galaxy: number;
    system: number;
    count: number;
    total_value: number;
  }>;
  salvage_competition_level: 'low' | 'medium' | 'high';
}

export interface SalvageLeaderboard {
  rank: number;
  user_id: number;
  username: string;
  total_salvage_value: number;
  total_missions: number;
  success_rate: number;
  salvage_level: number;
  legendary_finds: number;
}

// ========================================
// FILTER AND SEARCH TYPES
// ========================================

export interface DebrisFilter {
  galaxy?: number;
  system?: number;
  position?: number;
  debris_type?: DebrisType;
  min_value?: number;
  max_value?: number;
  is_claimed?: boolean;
  is_active?: boolean;
  hazard_level_max?: number;
  sort_by?: 'value' | 'distance' | 'expiry' | 'created';
  sort_order?: 'ASC' | 'DESC';
  page?: number;
  limit?: number;
}

export interface SalvageFilter {
  user_id?: number;
  status?: SalvageStatus;
  salvage_type?: SalvageType;
  date_from?: Date;
  date_to?: Date;
  min_value?: number;
  is_competitive?: boolean;
  page?: number;
  limit?: number;
}

// ========================================
// PAGINATION RESPONSE
// ========================================

export interface PaginatedDebrisResponse {
  data: DebrisFieldInfo[];
  total: number;
  page: number;
  limit: number;
  totalPages: number;
}

export interface PaginatedSalvageResponse {
  data: DebrisSalvage[];
  total: number;
  page: number;
  limit: number;
  totalPages: number;
}

// ========================================
// REQUEST/RESPONSE TYPES
// ========================================

export interface CreateDebrisRequest {
  galaxy: number;
  system: number;
  position: number;
  destroyed_ships?: Record<string, number>;
  total_value: number;
  debris_rate?: number;
  combat_id?: number;
  attacker_id?: number;
  defender_id?: number;
}

export interface DebrisGenerationResult {
  debris_id: number;
  metal_amount: number;
  crystal_amount: number;
  deuterium_amount: number;
  debris_type: DebrisType;
  total_value: number;
  error?: string;
}

export interface DebrisGenerationConfig {
  base_debris_percentage: number;
  component_drop_rate: number;
  rare_material_chance: number;
  hazard_multiplier: number;
}

export interface DebrisSystemStats {
  totalDebrisFields: number;
  activeFields: number;
  expiredFields: number;
  totalValueAvailable: number;
  avgFieldValue: number;
  totalSalvageOperations: number;
  activeSalvageOps: number;
  totalComponentsGenerated: number;
  legendaryComponentsFound: number;
}

export interface StartSalvageRequest {
  userId: number;
  debrisId: number;
  salvageType: SalvageType;
  fleetId?: number;
  shipTypes?: Record<string, number>;
  cargoCapacity: number;
}

export interface DebrisSalvageOperation {
  id: number;
  user_id: number;
  debris_id: number;
  salvage_type: SalvageType;
  fleet_id?: number;
  ship_types?: Record<string, number>;
  cargo_capacity: number;
  salvage_efficiency: number;
  status: SalvageStatus;
  start_time: Date;
  arrival_time?: Date;
  completion_time?: Date;
  return_time?: Date;
  resources_collected?: any;
  components_collected?: any;
  total_value?: number;
  experience_gained?: number;
  success_rate?: number;
  hazards_encountered?: any;
  alliance_id?: number;
  is_competitive?: boolean;
  ranking?: number;
  notes?: string;
}

export interface SalvageOperationResult {
  operation_id: number;
  estimated_duration: number;
  estimated_resources: ResourceCollection;
  success_probability: number;
  message?: string;
  error?: string;
}

export interface SalvageCompletionResult {
  success: boolean;
  resources_collected: ResourceCollection;
  components_collected: ComponentCollection;
  experienceGained: number;
  efficiencyAchieved: number;
  conflicts: boolean;
  message: string;
}

export interface SalvageResources {
  metal: number;
  crystal: number;
  deuterium: number;
  rare_materials: number;
}

export interface SalvageEfficiencyCalculation {
  base_efficiency: number;
  tech_bonus: number;
  hazard_penalty: number;
  competition_penalty: number;
  weather_factor: number;
  final_efficiency: number;
}

export interface Coordinates {
  galaxy: number;
  system: number;
  position: number;
}

export interface DistanceCalculation {
  distance: number;
  travel_time: number;
  fuel_cost: number;
}

export interface UserSalvageProfile {
  user_id: number;
  username: string;
  stats: SalvageStatistics;
  recent_operations: DebrisSalvageOperation[];
  component_inventory: any[];
  active_claims: any[];
  rank: number;
  next_level_experience: number;
}

export interface RecycleComponentRequest {
  component_id: number;
  user_id: number;
  quantity: number;
  recycle_all?: boolean;
}

export interface ComponentRecycleResult {
  resources_gained: ResourceCollection;
  recycle_efficiency: number;
  message?: string;
  error?: string;
}

export interface ComponentBonus {
  speed?: number;
  attack?: number;
  defense?: number;
  cargo?: number;
  fuel?: number;
  research?: number;
  production?: number;
  [key: string]: number | undefined;
}

// ========================================
// STATISTICS TYPES
// ========================================

export interface SalvageStatistics {
  id: number;
  user_id: number;
  total_salvage_missions: number;
  successful_missions: number;
  failed_missions: number;
  total_metal_collected: number;
  total_crystal_collected: number;
  total_deuterium_collected: number;
  total_rare_materials: number;
  total_components_found: number;
  legendary_components: number;
  total_salvage_value: number;
  fastest_salvage_time?: number;
  largest_single_haul: number;
  salvage_efficiency_avg: number;
  competitive_wins: number;
  alliance_contributions: number;
  salvage_experience_points: number;
  salvage_level: number;
  salvage_rank?: string;
  last_salvage_at?: Date;
  updated_at: Date;
}

export interface SalvageLeaderboard {
  rank: number;
  user_id: number;
  username: string;
  total_value: number;
  operations_count: number;
  success_rate: number;
}

export interface SalvageProfile {
  user_id: number;
  username: string;
  salvage_level: number;
  total_operations: number;
  success_rate: number;
  total_resources_collected: ResourceCollection;
  favorite_salvage_type: SalvageType;
  highest_value_operation: number;
}
