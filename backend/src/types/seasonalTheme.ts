// =====================================================
// Phase 8: Seasonal Theme System - TypeScript Types
// =====================================================

/**
 * Theme category enumeration
 */
export enum ThemeCategory {
    SEASONAL = 'seasonal',
    EVENT = 'event',
    SPECIAL = 'special',
    CUSTOM = 'custom'
}

/**
 * Activation type enumeration
 */
export enum ThemeActivationType {
    SCHEDULED = 'scheduled',
    MANUAL = 'manual',
    PREVIEW = 'preview',
    TEST = 'test'
}

/**
 * Asset type enumeration
 */
export enum ThemeAssetType {
    IMAGE = 'image',
    SOUND = 'sound',
    VIDEO = 'video',
    FONT = 'font',
    CSS = 'css',
    ANIMATION = 'animation'
}

/**
 * Loading strategy enumeration
 */
export enum AssetLoadStrategy {
    EAGER = 'eager',
    LAZY = 'lazy',
    ON_DEMAND = 'on_demand'
}

/**
 * Transition type enumeration
 */
export enum TransitionType {
    FADE = 'fade',
    SLIDE = 'slide',
    DISSOLVE = 'dissolve',
    ZOOM = 'zoom',
    NONE = 'none'
}

/**
 * Visual effects configuration
 */
export interface VisualEffects {
    // Snow effects (Christmas)
    snow?: {
        enabled: boolean;
        intensity: 'low' | 'medium' | 'high';
        flakeCount: number;
        speed?: number;
    };

    // Fireworks (New Year)
    fireworks?: {
        enabled: boolean;
        frequency: 'low' | 'medium' | 'high';
        colors: string[];
        duration?: number;
    };

    // Confetti
    confetti?: {
        enabled: boolean;
        intensity: 'low' | 'medium' | 'high';
        colors: string[];
    };

    // Fog/Mist (Halloween)
    fog?: {
        enabled: boolean;
        intensity: 'low' | 'medium' | 'high';
        color: string;
        opacity?: number;
    };

    // Butterflies (Easter)
    butterflies?: {
        enabled: boolean;
        count: number;
        colors: string[];
        speed?: 'slow' | 'medium' | 'fast';
    };

    // Floating elements
    bats?: {
        enabled: boolean;
        count: number;
        speed: 'slow' | 'medium' | 'fast';
    };

    // Lights/Sparkles
    lights?: {
        enabled: boolean;
        colors: string[];
        twinkle: boolean;
    };

    sparkles?: {
        enabled: boolean;
        color: string;
    };

    // Additional effects
    lightning?: {
        enabled: boolean;
        frequency: 'rare' | 'occasional' | 'frequent';
    };

    cobwebs?: {
        enabled: boolean;
        opacity: number;
    };

    flowers?: {
        enabled: boolean;
        bloom: boolean;
    };

    sunshine?: {
        enabled: boolean;
        rays: boolean;
    };

    petals?: {
        enabled: boolean;
        fallSpeed: 'slow' | 'medium' | 'fast';
    };

    countdown?: {
        enabled: boolean;
        size: 'small' | 'medium' | 'large';
    };

    sparklers?: {
        enabled: boolean;
        color: string;
    };
}

/**
 * Sound effects configuration
 */
export interface SoundEffects {
    music?: {
        file: string;
        volume: number;
        loop: boolean;
        fadeIn?: number;
        fadeOut?: number;
    };

    ui?: {
        buttonClick?: string;
        success?: string;
        error?: string;
        notification?: string;
        ambient?: string;
    };

    countdown?: {
        tick?: string;
        celebration?: string;
    };
}

/**
 * Animation configuration
 */
export interface AnimationConfig {
    entrance?: {
        type: string;
        duration: number;
        easing?: string;
    };

    idle?: {
        type: string;
        duration: number;
        loop?: boolean;
    };

    exit?: {
        type: string;
        duration: number;
        easing?: string;
    };
}

/**
 * Decorations configuration
 */
export interface DecorationsConfig {
    header?: {
        type: string;
        position: string;
        opacity?: number;
    };

    footer?: {
        type: string;
        position: string;
        opacity?: number;
    };

    corners?: {
        type: string;
        positions: string[];
    };

    floating?: {
        type: string;
        count: number;
        speed?: string;
    };

    sides?: {
        type: string;
        positions: string[];
    };

    screen?: {
        type: string;
        opacity: number;
    };
}

/**
 * CSS variables override
 */
export interface CSSVariables {
    [key: string]: string;
}

/**
 * Core theme interface
 */
export interface Theme {
    id: number;
    theme_key: string;
    name: string;
    description?: string;
    category: ThemeCategory;

    // Visual settings
    primary_color: string;
    secondary_color: string;
    accent_color: string;
    background_color?: string;
    text_color?: string;

    // Effects
    visual_effects: VisualEffects;
    sound_effects: SoundEffects;
    animations: AnimationConfig;
    decorations: DecorationsConfig;

    // CSS
    css_variables: CSSVariables;
    custom_css?: string;

    // Status
    is_active: boolean;
    is_available: boolean;
    preview_mode: boolean;

    // Performance
    load_priority: number;
    cache_duration: number;

    // Metadata
    created_at: Date;
    updated_at: Date;
    created_by?: number;
    updated_by?: number;
}

/**
 * Theme schedule interface
 */
export interface ThemeSchedule {
    id: number;
    theme_id: number;

    // Scheduling
    schedule_name: string;
    start_date: Date;
    end_date: Date;
    start_time: string;
    end_time: string;

    // Recurrence
    is_recurring: boolean;
    recurrence_pattern?: string;
    recurrence_data?: any;

    // Priority
    priority: number;

    // Conditions
    enabled: boolean;
    require_admin_approval: boolean;
    min_server_version?: string;

    // Transitions
    transition_duration: number;
    transition_type: TransitionType;

    // Status
    is_active: boolean;
    activation_count: number;
    last_activated_at?: Date;

    // Metadata
    created_at: Date;
    updated_at: Date;
    created_by?: number;
}

/**
 * Theme asset interface
 */
export interface ThemeAsset {
    id: number;
    theme_id: number;

    // Asset information
    asset_key: string;
    asset_type: ThemeAssetType;
    file_path: string;
    file_url?: string;

    // Properties
    file_size?: number;
    mime_type?: string;
    dimensions?: string;
    duration?: number;

    // Usage
    usage_context: string;
    display_position?: string;
    z_index: number;

    // Loading
    load_strategy: AssetLoadStrategy;
    preload: boolean;

    // Optimization
    is_compressed: boolean;
    compression_quality?: number;
    has_fallback: boolean;
    fallback_asset_id?: number;

    // Status
    is_active: boolean;
    is_cdn_cached: boolean;

    // Metadata
    created_at: Date;
    updated_at: Date;
}

/**
 * Theme configuration interface
 */
export interface ThemeConfiguration {
    id: number;
    theme_id: number;

    // Configuration
    config_key: string;
    config_value: any;
    config_type: 'string' | 'number' | 'boolean' | 'object' | 'array';

    // Description
    display_name?: string;
    description?: string;
    category?: string;

    // Validation
    is_required: boolean;
    default_value?: any;
    validation_rules?: any;

    // Status
    is_active: boolean;
    is_user_configurable: boolean;

    // Metadata
    created_at: Date;
    updated_at: Date;
}

/**
 * Theme activation interface
 */
export interface ThemeActivation {
    id: number;
    theme_id: number;
    schedule_id?: number;

    // Activation details
    activation_type: ThemeActivationType;
    activated_by?: number;

    // Timing
    activated_at: Date;
    deactivated_at?: Date;
    duration_seconds?: number;

    // Context
    activation_reason?: string;
    ip_address?: string;
    user_agent?: string;

    // Analytics
    unique_viewers: number;
    total_page_views: number;
    avg_session_duration?: number;
    interaction_count: number;

    // Performance
    avg_load_time_ms?: number;
    error_count: number;
    error_logs: any[];

    // Status
    was_successful: boolean;

    // Metadata
    created_at: Date;
}

/**
 * User theme preferences interface
 */
export interface ThemePreferences {
    id: number;
    user_id: number;

    // Theme preferences
    enabled: boolean;
    preferred_theme_id?: number;

    // Feature toggles
    enable_visual_effects: boolean;
    enable_sound_effects: boolean;
    enable_animations: boolean;
    enable_decorations: boolean;

    // Performance
    reduce_motion: boolean;
    reduce_transparency: boolean;

    // Intensity controls (0-100)
    effect_intensity: number;
    sound_volume: number;
    animation_speed: number;

    // Custom CSS
    custom_css?: string | null;
    custom_css_updated_at?: Date | null;

    // Metadata
    created_at: Date;
    updated_at: Date;
}

/**
 * Request/Response Types
 */

export interface CreateThemeRequest {
    theme_key: string;
    name: string;
    description?: string;
    category: ThemeCategory;
    primary_color: string;
    secondary_color: string;
    accent_color: string;
    background_color?: string;
    text_color?: string;
    visual_effects?: VisualEffects;
    sound_effects?: SoundEffects;
    animations?: AnimationConfig;
    decorations?: DecorationsConfig;
    css_variables?: CSSVariables;
    custom_css?: string;
}

export interface UpdateThemeRequest extends Partial<CreateThemeRequest> {
    is_active?: boolean;
    is_available?: boolean;
    preview_mode?: boolean;
}

export interface CreateThemeScheduleRequest {
    theme_id: number;
    schedule_name: string;
    start_date: string;
    end_date: string;
    start_time?: string;
    end_time?: string;
    is_recurring?: boolean;
    priority?: number;
    transition_duration?: number;
    transition_type?: TransitionType;
}

export interface UpdateThemeScheduleRequest extends Partial<CreateThemeScheduleRequest> {
    enabled?: boolean;
}

export interface CreateThemeAssetRequest {
    theme_id: number;
    asset_key: string;
    asset_type: ThemeAssetType;
    file_path: string;
    file_url?: string;
    usage_context: string;
    display_position?: string;
    z_index?: number;
    load_strategy?: AssetLoadStrategy;
}

export interface UpdateThemeAssetRequest extends Partial<CreateThemeAssetRequest> {
    is_active?: boolean;
}

export interface ThemePreviewRequest {
    theme_id: number;
    user_id?: number;
}

export interface ThemeActivationRequest {
    theme_id: number;
    activation_type?: ThemeActivationType;
    activation_reason?: string;
}

export interface UpdateThemePreferencesRequest {
    enabled?: boolean;
    preferred_theme_id?: number;
    enable_visual_effects?: boolean;
    enable_sound_effects?: boolean;
    enable_animations?: boolean;
    enable_decorations?: boolean;
    reduce_motion?: boolean;
    reduce_transparency?: boolean;
    effect_intensity?: number;
    sound_volume?: number;
    animation_speed?: number;
}

/**
 * Response types
 */

export interface ThemeResponse {
    success: boolean;
    theme?: Theme;
    message?: string;
}

export interface ThemesListResponse {
    success: boolean;
    themes: Theme[];
    total: number;
    page?: number;
    pageSize?: number;
}

export interface CurrentThemeResponse {
    success: boolean;
    theme?: Theme;
    schedule?: ThemeSchedule;
    assets?: ThemeAsset[];
    configurations?: ThemeConfiguration[];
}

export interface ThemeScheduleResponse {
    success: boolean;
    schedule?: ThemeSchedule;
    message?: string;
}

export interface ThemeSchedulesListResponse {
    success: boolean;
    schedules: ThemeSchedule[];
    total: number;
}

export interface ThemeAssetResponse {
    success: boolean;
    asset?: ThemeAsset;
    message?: string;
}

export interface ThemeAssetsListResponse {
    success: boolean;
    assets: ThemeAsset[];
    total: number;
}

export interface ThemeActivationResponse {
    success: boolean;
    activation?: ThemeActivation;
    message?: string;
}

export interface ThemeAnalyticsResponse {
    success: boolean;
    analytics: {
        theme_id: number;
        theme_key: string;
        activation_count: number;
        total_unique_viewers: number;
        total_page_views: number;
        avg_session_duration: number;
        avg_load_time: number;
        total_errors: number;
        last_activated?: Date;
    };
}

export interface ThemePreferencesResponse {
    success: boolean;
    preferences?: ThemePreferences;
    message?: string;
}

/**
 * Utility types
 */

export interface ThemeWithDetails extends Theme {
    schedules?: ThemeSchedule[];
    assets?: ThemeAsset[];
    configurations?: ThemeConfiguration[];
    statistics?: {
        activation_count: number;
        total_viewers: number;
        avg_duration_hours: number;
        success_rate: number;
        avg_load_time_ms: number;
    };
}

export interface ActiveThemeData {
    theme: Theme;
    schedule?: ThemeSchedule;
    assets: ThemeAsset[];
    cssVariables: CSSVariables;
    customCSS?: string;
}
