// Phase 7: Configuration System Types
// TypeScript type definitions for dynamic game configuration

export enum ConfigDataType {
    NUMBER = 'number',
    STRING = 'string',
    BOOLEAN = 'boolean',
    JSON = 'json',
    FORMULA = 'formula'
}

export enum ConfigCategory {
    COMBAT = 'combat',
    RESOURCES = 'resources',
    BUILDINGS = 'buildings',
    RESEARCH = 'research',
    SHIPS = 'ships',
    DEFENSE = 'defense',
    UNIVERSE = 'universe',
    ECONOMY = 'economy',
    ALLIANCES = 'alliances',
    EVENTS = 'events',
    LEADERBOARDS = 'leaderboards',
    MODERATION = 'moderation',
    GAMEPLAY = 'gameplay'
}

// Database models
export interface ConfigCategoryModel {
    category_id: number;
    category_name: string;
    display_name: string;
    description?: string;
    sort_order: number;
    is_active: boolean;
    created_at: Date;
}

export interface ConfigParameterModel {
    parameter_id: number;
    category_id: number;
    parameter_key: string;
    parameter_name: string;
    description?: string;
    data_type: ConfigDataType;
    current_value: string;
    default_value: string;
    min_value?: number;
    max_value?: number;
    validation_rules?: Record<string, any>;
    requires_restart: boolean;
    is_editable: boolean;
    sort_order: number;
    created_at: Date;
    updated_at: Date;
}

export interface ConfigChangeHistoryModel {
    change_id: number;
    parameter_id: number;
    old_value: string;
    new_value: string;
    changed_by: number;
    change_reason?: string;
    applied_at: Date;
    is_rolled_back: boolean;
    rolled_back_at?: Date;
    rolled_back_by?: number;
}

export interface ConfigTemplateModel {
    template_id: number;
    template_name: string;
    description?: string;
    template_data: Record<string, any>;
    created_by: number;
    created_at: Date;
    is_public: boolean;
    usage_count: number;
}

export interface ConfigLockModel {
    lock_id: number;
    category_id: number;
    locked_by: number;
    locked_at: Date;
    lock_reason?: string;
}

// API request/response types
export interface ConfigParameterCreateRequest {
    category_id: number;
    parameter_key: string;
    parameter_name: string;
    description?: string;
    data_type: ConfigDataType;
    current_value: string;
    default_value: string;
    min_value?: number;
    max_value?: number;
    validation_rules?: Record<string, any>;
    requires_restart?: boolean;
}

export interface ConfigParameterUpdateRequest {
    current_value: string;
    change_reason?: string;
}

export interface ConfigBulkUpdateRequest {
    updates: Array<{
        parameter_key: string;
        value: string;
    }>;
    change_reason?: string;
}

export interface ConfigTemplateCreateRequest {
    template_name: string;
    description?: string;
    include_categories?: string[];
}

export interface ConfigTemplateApplyRequest {
    template_id: number;
    confirm: boolean;
}

export interface ConfigRollbackRequest {
    change_id: number;
    confirm: boolean;
}

export interface ConfigExportOptions {
    categories?: string[];
    include_defaults?: boolean;
    format?: 'json' | 'yaml';
}

export interface ConfigImportRequest {
    data: Record<string, any>;
    merge_strategy?: 'replace' | 'merge' | 'update_only';
    validate_only?: boolean;
}

// Response types
export interface ConfigParameterResponse extends ConfigParameterModel {
    category_name: string;
    category_display_name: string;
    can_rollback: boolean;
    last_changed?: Date;
    last_changed_by?: string;
}

export interface ConfigChangeResponse extends ConfigChangeHistoryModel {
    parameter_key: string;
    parameter_name: string;
    changed_by_username: string;
}

export interface ConfigCategoryResponse extends ConfigCategoryModel {
    parameter_count: number;
    modified_count: number;
    restart_required_count: number;
}

export interface ConfigValidationResult {
    is_valid: boolean;
    errors: ConfigValidationError[];
    warnings: ConfigValidationWarning[];
}

export interface ConfigValidationError {
    parameter_key: string;
    error_type: string;
    message: string;
}

export interface ConfigValidationWarning {
    parameter_key: string;
    warning_type: string;
    message: string;
}

export interface ConfigDiffResult {
    added: string[];
    modified: Array<{
        key: string;
        old_value: string;
        new_value: string;
    }>;
    removed: string[];
}

// Service types
export interface ConfigSnapshot {
    timestamp: Date;
    parameters: Record<string, any>;
    metadata: {
        version: string;
        server_name: string;
        total_parameters: number;
    };
}

export interface ConfigCacheEntry {
    value: any;
    cached_at: Date;
    expires_at?: Date;
}

export interface HotReloadEvent {
    parameter_key: string;
    old_value: any;
    new_value: any;
    requires_restart: boolean;
    timestamp: Date;
}

// Typed configuration accessors
export interface CombatConfig {
    damage_multiplier: number;
    shield_absorption: number;
    armor_reduction: number;
    max_battle_rounds: number;
    rapid_fire_enabled: boolean;
    debris_field_percentage: number;
}

export interface ResourceConfig {
    production_multiplier: number;
    metal_multiplier: number;
    crystal_multiplier: number;
    deuterium_multiplier: number;
    starting_metal: number;
    starting_crystal: number;
    starting_deuterium: number;
}

export interface BuildingConfig {
    cost_multiplier: number;
    time_multiplier: number;
    max_queue_size: number;
}

export interface ResearchConfig {
    cost_multiplier: number;
    time_multiplier: number;
}

export interface FleetConfig {
    speed_multiplier: number;
    fuel_consumption_multiplier: number;
    cargo_multiplier: number;
}

export interface UniverseConfig {
    max_galaxies: number;
    max_systems: number;
    max_planets: number;
    player_starting_planets: number;
}

export interface AllianceConfig {
    max_members: number;
    creation_cost: number;
}

export interface GameplayConfig {
    speed: number;
    server_name: string;
    maintenance_mode: boolean;
}

// Complete typed configuration interface
export interface GameConfiguration {
    combat: CombatConfig;
    resources: ResourceConfig;
    buildings: BuildingConfig;
    research: ResearchConfig;
    fleet: FleetConfig;
    universe: UniverseConfig;
    alliance: AllianceConfig;
    gameplay: GameplayConfig;
}

// Event types for real-time updates
export interface ConfigChangeEvent {
    type: 'config:changed';
    category: string;
    parameter_key: string;
    old_value: any;
    new_value: any;
    changed_by: number;
    timestamp: Date;
}

export interface ConfigReloadEvent {
    type: 'config:reload';
    categories: string[];
    timestamp: Date;
}

export interface ConfigLockEvent {
    type: 'config:locked' | 'config:unlocked';
    category: string;
    locked_by?: number;
    timestamp: Date;
}

// Socket.io event types
export type ConfigSocketEvent = 
    | ConfigChangeEvent
    | ConfigReloadEvent
    | ConfigLockEvent;

// Utility types
export type ConfigValue = string | number | boolean | Record<string, any>;

export interface ConfigUpdateResult {
    success: boolean;
    parameter_key: string;
    old_value: ConfigValue;
    new_value: ConfigValue;
    requires_restart: boolean;
    message?: string;
}

export interface ConfigBulkUpdateResult {
    success: boolean;
    updated_count: number;
    failed_count: number;
    results: ConfigUpdateResult[];
    requires_restart: boolean;
}

// Admin UI types
export interface ConfigEditorState {
    category: string;
    parameters: ConfigParameterResponse[];
    is_locked: boolean;
    locked_by?: string;
    has_unsaved_changes: boolean;
    loading: boolean;
    error?: string;
}

export interface ConfigHistoryFilter {
    category?: string;
    parameter_key?: string;
    changed_by?: number;
    start_date?: Date;
    end_date?: Date;
    show_rolled_back?: boolean;
    limit?: number;
}

export interface ConfigTemplateFilter {
    is_public?: boolean;
    created_by?: number;
    search?: string;
}
