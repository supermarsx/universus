-- =====================================================
-- Phase 8: Seasonal Theme System Database Schema
-- =====================================================
-- Purpose: Complete theming system with scheduling, assets, and effects
-- Features: Christmas, Halloween, Easter, New Year themes with automatic activation
-- Created: 2025-11-06
-- =====================================================

-- =====================================================
-- 1. THEMES TABLE
-- =====================================================
-- Core table storing all theme definitions
CREATE TABLE IF NOT EXISTS themes (
    id SERIAL PRIMARY KEY,
    theme_key VARCHAR(50) UNIQUE NOT NULL, -- 'christmas', 'halloween', 'easter', 'new_year'
    name VARCHAR(100) NOT NULL,
    description TEXT,
    category VARCHAR(50) NOT NULL DEFAULT 'seasonal', -- seasonal, event, special
    
    -- Visual Settings
    primary_color VARCHAR(20) NOT NULL,
    secondary_color VARCHAR(20) NOT NULL,
    accent_color VARCHAR(20) NOT NULL,
    background_color VARCHAR(20),
    text_color VARCHAR(20),
    
    -- Effects Configuration (JSON)
    visual_effects JSONB DEFAULT '{}', -- snow, fireworks, falling_leaves, particles
    sound_effects JSONB DEFAULT '{}', -- background_music, ui_sounds, ambient
    animations JSONB DEFAULT '{}', -- entry, exit, idle animations
    decorations JSONB DEFAULT '{}', -- header_decor, footer_decor, floating_elements
    
    -- CSS Overrides
    css_variables JSONB DEFAULT '{}', -- Custom CSS variable overrides
    custom_css TEXT, -- Additional custom CSS
    
    -- Status
    is_active BOOLEAN DEFAULT false,
    is_available BOOLEAN DEFAULT true, -- Can be activated
    preview_mode BOOLEAN DEFAULT false, -- Available for preview
    
    -- Metadata
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    created_by INTEGER REFERENCES users(id),
    updated_by INTEGER REFERENCES users(id),
    
    -- Performance
    load_priority INTEGER DEFAULT 0, -- Higher = loads first
    cache_duration INTEGER DEFAULT 3600 -- Seconds to cache theme assets
);

-- =====================================================
-- 2. THEME SCHEDULES TABLE
-- =====================================================
-- Automatic theme activation based on dates
CREATE TABLE IF NOT EXISTS theme_schedules (
    id SERIAL PRIMARY KEY,
    theme_id INTEGER NOT NULL REFERENCES themes(id) ON DELETE CASCADE,
    
    -- Scheduling
    schedule_name VARCHAR(100) NOT NULL,
    start_date DATE NOT NULL,
    end_date DATE NOT NULL,
    start_time TIME DEFAULT '00:00:00',
    end_time TIME DEFAULT '23:59:59',
    
    -- Recurrence (for annual events)
    is_recurring BOOLEAN DEFAULT true,
    recurrence_pattern VARCHAR(50), -- 'yearly', 'custom'
    recurrence_data JSONB, -- Additional recurrence configuration
    
    -- Priority (higher priority wins if schedules overlap)
    priority INTEGER DEFAULT 0,
    
    -- Conditions
    enabled BOOLEAN DEFAULT true,
    require_admin_approval BOOLEAN DEFAULT false,
    min_server_version VARCHAR(20), -- Minimum server version required
    
    -- Transition Settings
    transition_duration INTEGER DEFAULT 1000, -- milliseconds
    transition_type VARCHAR(50) DEFAULT 'fade', -- fade, slide, dissolve
    
    -- Status
    is_active BOOLEAN DEFAULT true,
    activation_count INTEGER DEFAULT 0, -- Times this schedule has activated
    last_activated_at TIMESTAMP,
    
    -- Metadata
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    created_by INTEGER REFERENCES users(id),
    
    -- Constraints
    CONSTRAINT valid_date_range CHECK (end_date >= start_date),
    CONSTRAINT valid_priority CHECK (priority >= 0 AND priority <= 100)
);

-- =====================================================
-- 3. THEME ASSETS TABLE
-- =====================================================
-- Asset management for theme-specific resources
CREATE TABLE IF NOT EXISTS theme_assets (
    id SERIAL PRIMARY KEY,
    theme_id INTEGER NOT NULL REFERENCES themes(id) ON DELETE CASCADE,
    
    -- Asset Information
    asset_key VARCHAR(100) NOT NULL, -- Unique key within theme
    asset_type VARCHAR(50) NOT NULL, -- 'image', 'sound', 'video', 'font', 'css', 'animation'
    file_path VARCHAR(500) NOT NULL, -- Path to asset file
    file_url VARCHAR(500), -- CDN URL if applicable
    
    -- Asset Properties
    file_size INTEGER, -- Bytes
    mime_type VARCHAR(100),
    dimensions VARCHAR(50), -- e.g., '1920x1080' for images
    duration INTEGER, -- For audio/video (milliseconds)
    
    -- Usage
    usage_context VARCHAR(100), -- 'background', 'decoration', 'icon', 'effect'
    display_position VARCHAR(50), -- 'header', 'footer', 'sidebar', 'overlay', 'fullscreen'
    z_index INTEGER DEFAULT 1,
    
    -- Loading Strategy
    load_strategy VARCHAR(50) DEFAULT 'lazy', -- 'eager', 'lazy', 'on_demand'
    preload BOOLEAN DEFAULT false,
    
    -- Optimization
    is_compressed BOOLEAN DEFAULT false,
    compression_quality INTEGER, -- 1-100
    has_fallback BOOLEAN DEFAULT false,
    fallback_asset_id INTEGER REFERENCES theme_assets(id),
    
    -- Status
    is_active BOOLEAN DEFAULT true,
    is_cdn_cached BOOLEAN DEFAULT false,
    
    -- Metadata
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    -- Constraints
    UNIQUE(theme_id, asset_key),
    CONSTRAINT valid_file_size CHECK (file_size IS NULL OR file_size > 0),
    CONSTRAINT valid_z_index CHECK (z_index >= 0 AND z_index <= 9999)
);

-- =====================================================
-- 4. THEME CONFIGURATIONS TABLE
-- =====================================================
-- Theme-specific configuration and feature flags
CREATE TABLE IF NOT EXISTS theme_configurations (
    id SERIAL PRIMARY KEY,
    theme_id INTEGER NOT NULL REFERENCES themes(id) ON DELETE CASCADE,
    
    -- Configuration
    config_key VARCHAR(100) NOT NULL,
    config_value JSONB NOT NULL,
    config_type VARCHAR(50) NOT NULL, -- 'string', 'number', 'boolean', 'object', 'array'
    
    -- Description
    display_name VARCHAR(100),
    description TEXT,
    category VARCHAR(50), -- 'visual', 'audio', 'interaction', 'performance'
    
    -- Validation
    is_required BOOLEAN DEFAULT false,
    default_value JSONB,
    validation_rules JSONB, -- min, max, pattern, enum values
    
    -- Status
    is_active BOOLEAN DEFAULT true,
    is_user_configurable BOOLEAN DEFAULT false, -- Can users override?
    
    -- Metadata
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    -- Constraints
    UNIQUE(theme_id, config_key)
);

-- =====================================================
-- 5. THEME ACTIVATIONS TABLE
-- =====================================================
-- Track theme activation history and analytics
CREATE TABLE IF NOT EXISTS theme_activations (
    id SERIAL PRIMARY KEY,
    theme_id INTEGER NOT NULL REFERENCES themes(id) ON DELETE CASCADE,
    schedule_id INTEGER REFERENCES theme_schedules(id) ON DELETE SET NULL,
    
    -- Activation Details
    activation_type VARCHAR(50) NOT NULL, -- 'scheduled', 'manual', 'preview', 'test'
    activated_by INTEGER REFERENCES users(id), -- NULL for automatic activations
    
    -- Timing
    activated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deactivated_at TIMESTAMP,
    duration_seconds INTEGER, -- Calculated on deactivation
    
    -- Context
    activation_reason TEXT,
    ip_address INET,
    user_agent TEXT,
    
    -- Analytics
    unique_viewers INTEGER DEFAULT 0,
    total_page_views INTEGER DEFAULT 0,
    avg_session_duration INTEGER, -- Seconds
    interaction_count INTEGER DEFAULT 0,
    
    -- Performance Metrics
    avg_load_time_ms INTEGER,
    error_count INTEGER DEFAULT 0,
    error_logs JSONB DEFAULT '[]',
    
    -- Status
    was_successful BOOLEAN DEFAULT true,
    
    -- Metadata
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =====================================================
-- 6. THEME PREFERENCES TABLE (User Overrides)
-- =====================================================
-- Allow users to customize or disable themes
CREATE TABLE IF NOT EXISTS theme_preferences (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    
    -- Theme Preferences
    enabled BOOLEAN DEFAULT true, -- User can disable all themes
    preferred_theme_id INTEGER REFERENCES themes(id), -- Override current theme
    
    -- Feature Toggles
    enable_visual_effects BOOLEAN DEFAULT true,
    enable_sound_effects BOOLEAN DEFAULT true,
    enable_animations BOOLEAN DEFAULT true,
    enable_decorations BOOLEAN DEFAULT true,
    
    -- Performance Settings
    reduce_motion BOOLEAN DEFAULT false,
    reduce_transparency BOOLEAN DEFAULT false,
    
    -- Intensity Controls (0-100)
    effect_intensity INTEGER DEFAULT 100,
    sound_volume INTEGER DEFAULT 50,
    animation_speed INTEGER DEFAULT 100,
    
    -- Metadata
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    -- Constraints
    UNIQUE(user_id),
    CONSTRAINT valid_intensity CHECK (effect_intensity >= 0 AND effect_intensity <= 100),
    CONSTRAINT valid_volume CHECK (sound_volume >= 0 AND sound_volume <= 100),
    CONSTRAINT valid_animation_speed CHECK (animation_speed >= 0 AND animation_speed <= 200)
);

-- =====================================================
-- INDEXES FOR PERFORMANCE
-- =====================================================

-- Theme lookups
CREATE INDEX idx_themes_theme_key ON themes(theme_key);
CREATE INDEX idx_themes_is_active ON themes(is_active);
CREATE INDEX idx_themes_category ON themes(category);

-- Schedule queries
CREATE INDEX idx_theme_schedules_theme_id ON theme_schedules(theme_id);
CREATE INDEX idx_theme_schedules_dates ON theme_schedules(start_date, end_date);
CREATE INDEX idx_theme_schedules_active ON theme_schedules(is_active, enabled);
CREATE INDEX idx_theme_schedules_priority ON theme_schedules(priority DESC);

-- Asset lookups
CREATE INDEX idx_theme_assets_theme_id ON theme_assets(theme_id);
CREATE INDEX idx_theme_assets_type ON theme_assets(asset_type);
CREATE INDEX idx_theme_assets_usage ON theme_assets(usage_context);
CREATE INDEX idx_theme_assets_active ON theme_assets(is_active);

-- Configuration queries
CREATE INDEX idx_theme_configurations_theme_id ON theme_configurations(theme_id);
CREATE INDEX idx_theme_configurations_key ON theme_configurations(config_key);
CREATE INDEX idx_theme_configurations_category ON theme_configurations(category);

-- Activation history
CREATE INDEX idx_theme_activations_theme_id ON theme_activations(theme_id);
CREATE INDEX idx_theme_activations_dates ON theme_activations(activated_at, deactivated_at);
CREATE INDEX idx_theme_activations_type ON theme_activations(activation_type);

-- User preferences
CREATE INDEX idx_theme_preferences_user_id ON theme_preferences(user_id);

-- =====================================================
-- VIEWS FOR COMMON QUERIES
-- =====================================================

-- Active schedules view
CREATE OR REPLACE VIEW v_active_theme_schedules AS
SELECT 
    s.*,
    t.name as theme_name,
    t.theme_key,
    t.is_active as theme_active
FROM theme_schedules s
JOIN themes t ON t.id = s.theme_id
WHERE s.enabled = true
    AND s.is_active = true
    AND t.is_available = true
    AND (
        -- Check if current date/time is within schedule
        (NOT s.is_recurring AND CURRENT_DATE BETWEEN s.start_date AND s.end_date)
        OR 
        -- For recurring schedules, check day/month match
        (s.is_recurring AND 
         EXTRACT(MONTH FROM CURRENT_DATE) = EXTRACT(MONTH FROM s.start_date) AND
         EXTRACT(DAY FROM CURRENT_DATE) BETWEEN EXTRACT(DAY FROM s.start_date) AND EXTRACT(DAY FROM s.end_date))
    )
ORDER BY s.priority DESC;

-- Theme analytics view
CREATE OR REPLACE VIEW v_theme_analytics AS
SELECT 
    t.id,
    t.theme_key,
    t.name,
    COUNT(DISTINCT ta.id) as activation_count,
    SUM(ta.unique_viewers) as total_unique_viewers,
    SUM(ta.total_page_views) as total_page_views,
    AVG(ta.avg_session_duration) as avg_session_duration,
    AVG(ta.avg_load_time_ms) as avg_load_time,
    SUM(ta.error_count) as total_errors,
    MAX(ta.activated_at) as last_activated
FROM themes t
LEFT JOIN theme_activations ta ON ta.theme_id = t.id
GROUP BY t.id, t.theme_key, t.name;

-- Current active theme view
CREATE OR REPLACE VIEW v_current_theme AS
SELECT 
    t.*,
    s.id as schedule_id,
    s.schedule_name,
    s.priority as schedule_priority
FROM themes t
LEFT JOIN v_active_theme_schedules s ON s.theme_id = t.id
WHERE t.is_active = true
ORDER BY COALESCE(s.priority, 0) DESC
LIMIT 1;

-- =====================================================
-- FUNCTIONS
-- =====================================================

-- Function to automatically activate theme based on schedule
CREATE OR REPLACE FUNCTION activate_scheduled_theme()
RETURNS INTEGER AS $$
DECLARE
    v_theme_id INTEGER;
    v_schedule_id INTEGER;
    v_current_active_id INTEGER;
BEGIN
    -- Get currently active theme
    SELECT id INTO v_current_active_id FROM themes WHERE is_active = true;
    
    -- Find highest priority active schedule
    SELECT theme_id, id INTO v_theme_id, v_schedule_id
    FROM v_active_theme_schedules
    ORDER BY priority DESC
    LIMIT 1;
    
    -- If no active schedule, deactivate all themes
    IF v_theme_id IS NULL THEN
        UPDATE themes SET is_active = false WHERE is_active = true;
        RETURN NULL;
    END IF;
    
    -- If different from current, switch themes
    IF v_theme_id IS DISTINCT FROM v_current_active_id THEN
        -- Deactivate old theme
        UPDATE themes SET is_active = false WHERE id = v_current_active_id;
        
        -- Activate new theme
        UPDATE themes SET is_active = true WHERE id = v_theme_id;
        
        -- Update schedule activation count
        UPDATE theme_schedules 
        SET activation_count = activation_count + 1,
            last_activated_at = CURRENT_TIMESTAMP
        WHERE id = v_schedule_id;
        
        -- Log activation
        INSERT INTO theme_activations (theme_id, schedule_id, activation_type)
        VALUES (v_theme_id, v_schedule_id, 'scheduled');
    END IF;
    
    RETURN v_theme_id;
END;
$$ LANGUAGE plpgsql;

-- Function to get theme assets by context
CREATE OR REPLACE FUNCTION get_theme_assets(
    p_theme_id INTEGER,
    p_usage_context VARCHAR DEFAULT NULL
)
RETURNS TABLE (
    asset_key VARCHAR,
    asset_type VARCHAR,
    file_path VARCHAR,
    file_url VARCHAR,
    display_position VARCHAR,
    z_index INTEGER
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        ta.asset_key,
        ta.asset_type,
        ta.file_path,
        ta.file_url,
        ta.display_position,
        ta.z_index
    FROM theme_assets ta
    WHERE ta.theme_id = p_theme_id
        AND ta.is_active = true
        AND (p_usage_context IS NULL OR ta.usage_context = p_usage_context)
    ORDER BY ta.z_index ASC, ta.id ASC;
END;
$$ LANGUAGE plpgsql;

-- Function to calculate theme activation statistics
CREATE OR REPLACE FUNCTION calculate_theme_stats(p_theme_id INTEGER)
RETURNS TABLE (
    total_activations BIGINT,
    total_viewers BIGINT,
    avg_duration_hours NUMERIC,
    success_rate NUMERIC,
    avg_load_time_ms NUMERIC
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        COUNT(*)::BIGINT as total_activations,
        SUM(unique_viewers)::BIGINT as total_viewers,
        ROUND(AVG(duration_seconds) / 3600.0, 2) as avg_duration_hours,
        ROUND(AVG(CASE WHEN was_successful THEN 1.0 ELSE 0.0 END) * 100, 2) as success_rate,
        ROUND(AVG(avg_load_time_ms), 0) as avg_load_time_ms
    FROM theme_activations
    WHERE theme_id = p_theme_id;
END;
$$ LANGUAGE plpgsql;

-- =====================================================
-- TRIGGERS
-- =====================================================

-- Update timestamps
CREATE OR REPLACE FUNCTION update_theme_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_themes_updated
    BEFORE UPDATE ON themes
    FOR EACH ROW
    EXECUTE FUNCTION update_theme_timestamp();

CREATE TRIGGER trg_theme_schedules_updated
    BEFORE UPDATE ON theme_schedules
    FOR EACH ROW
    EXECUTE FUNCTION update_theme_timestamp();

CREATE TRIGGER trg_theme_assets_updated
    BEFORE UPDATE ON theme_assets
    FOR EACH ROW
    EXECUTE FUNCTION update_theme_timestamp();

CREATE TRIGGER trg_theme_configurations_updated
    BEFORE UPDATE ON theme_configurations
    FOR EACH ROW
    EXECUTE FUNCTION update_theme_timestamp();

CREATE TRIGGER trg_theme_preferences_updated
    BEFORE UPDATE ON theme_preferences
    FOR EACH ROW
    EXECUTE FUNCTION update_theme_timestamp();

-- =====================================================
-- SEED DATA: DEFAULT SEASONAL THEMES
-- =====================================================

-- Christmas Theme
INSERT INTO themes (theme_key, name, description, category, primary_color, secondary_color, accent_color, background_color, text_color, visual_effects, sound_effects, animations, decorations, css_variables)
VALUES (
    'christmas',
    'Christmas',
    'Festive Christmas theme with snow effects, holiday decorations, and warm colors',
    'seasonal',
    '#c41e3a', -- Christmas red
    '#165b33', -- Christmas green
    '#ffd700', -- Gold accent
    '#1a1f2e', -- Dark blue background
    '#ffffff',
    '{"snow": {"enabled": true, "intensity": "medium", "flakeCount": 100}, "lights": {"enabled": true, "colors": ["red", "green", "gold", "white"], "twinkle": true}, "sparkles": {"enabled": true, "color": "gold"}}',
    '{"music": {"file": "jingle-bells.mp3", "volume": 0.3, "loop": true}, "ui": {"buttonClick": "bell-ring.mp3", "success": "ho-ho-ho.mp3"}}',
    '{"entrance": {"type": "snow_fall", "duration": 1000}, "idle": {"type": "gentle_sway", "duration": 3000}, "exit": {"type": "fade_out", "duration": 800}}',
    '{"header": {"type": "garland", "position": "top"}, "corners": {"type": "ornaments", "positions": ["top-left", "top-right"]}, "floating": {"type": "presents", "count": 5}}',
    '{"--theme-primary": "#c41e3a", "--theme-secondary": "#165b33", "--theme-accent": "#ffd700", "--theme-glow": "0 0 20px rgba(255, 215, 0, 0.5)"}'
) ON CONFLICT (theme_key) DO NOTHING;

-- Halloween Theme
INSERT INTO themes (theme_key, name, description, category, primary_color, secondary_color, accent_color, background_color, text_color, visual_effects, sound_effects, animations, decorations, css_variables)
VALUES (
    'halloween',
    'Halloween',
    'Spooky Halloween theme with dark atmosphere, fog effects, and eerie decorations',
    'seasonal',
    '#ff6600', -- Pumpkin orange
    '#1a0033', -- Dark purple
    '#00ff00', -- Eerie green
    '#0d0d0d', -- Very dark background
    '#ffffff',
    '{"fog": {"enabled": true, "intensity": "high", "color": "#666"}, "bats": {"enabled": true, "count": 15, "speed": "slow"}, "cobwebs": {"enabled": true, "opacity": 0.3}, "lightning": {"enabled": true, "frequency": "rare"}}',
    '{"music": {"file": "spooky-atmosphere.mp3", "volume": 0.2, "loop": true}, "ui": {"buttonClick": "ghost-whisper.mp3", "success": "witch-cackle.mp3", "ambient": "howling-wind.mp3"}}',
    '{"entrance": {"type": "fade_from_black", "duration": 1500}, "idle": {"type": "float", "duration": 4000}, "exit": {"type": "dissolve", "duration": 1000}}',
    '{"header": {"type": "spider_web", "position": "corners"}, "floating": {"type": "ghosts", "count": 3}, "sides": {"type": "pumpkins", "positions": ["left", "right"]}}',
    '{"--theme-primary": "#ff6600", "--theme-secondary": "#1a0033", "--theme-accent": "#00ff00", "--theme-glow": "0 0 30px rgba(255, 102, 0, 0.7)", "--theme-shadow": "0 5px 30px rgba(0, 0, 0, 0.9)"}'
) ON CONFLICT (theme_key) DO NOTHING;

-- Easter Theme
INSERT INTO themes (theme_key, name, description, category, primary_color, secondary_color, accent_color, background_color, text_color, visual_effects, sound_effects, animations, decorations, css_variables)
VALUES (
    'easter',
    'Easter',
    'Bright Easter theme with spring colors, butterflies, and cheerful decorations',
    'seasonal',
    '#ff69b4', -- Pink
    '#87ceeb', -- Sky blue
    '#ffff00', -- Yellow
    '#f0f8ff', -- Light blue background
    '#333333',
    '{"butterflies": {"enabled": true, "count": 10, "colors": ["pink", "blue", "yellow"]}, "flowers": {"enabled": true, "bloom": true}, "sunshine": {"enabled": true, "rays": true}, "petals": {"enabled": true, "fallSpeed": "slow"}}',
    '{"music": {"file": "spring-melody.mp3", "volume": 0.25, "loop": true}, "ui": {"buttonClick": "chick-chirp.mp3", "success": "bunny-hop.mp3"}}',
    '{"entrance": {"type": "bloom", "duration": 1200}, "idle": {"type": "bounce", "duration": 2000}, "exit": {"type": "fade_out", "duration": 800}}',
    '{"header": {"type": "flower_border", "position": "top"}, "floating": {"type": "easter_eggs", "count": 8}, "corners": {"type": "bunnies", "positions": ["bottom-left", "bottom-right"]}}',
    '{"--theme-primary": "#ff69b4", "--theme-secondary": "#87ceeb", "--theme-accent": "#ffff00", "--theme-glow": "0 0 15px rgba(255, 105, 180, 0.4)", "--theme-brightness": "1.1"}'
) ON CONFLICT (theme_key) DO NOTHING;

-- New Year Theme
INSERT INTO themes (theme_key, name, description, category, primary_color, secondary_color, accent_color, background_color, text_color, visual_effects, sound_effects, animations, decorations, css_variables)
VALUES (
    'new_year',
    'New Year',
    'Celebratory New Year theme with fireworks, confetti, and party atmosphere',
    'seasonal',
    '#ffd700', -- Gold
    '#c0c0c0', -- Silver
    '#4169e1', -- Royal blue
    '#0a0a1a', -- Dark background
    '#ffffff',
    '{"fireworks": {"enabled": true, "frequency": "high", "colors": ["gold", "silver", "red", "blue"]}, "confetti": {"enabled": true, "intensity": "high", "colors": ["gold", "silver", "red", "blue"]}, "countdown": {"enabled": true, "size": "large"}, "sparklers": {"enabled": true, "color": "gold"}}',
    '{"music": {"file": "celebration.mp3", "volume": 0.3, "loop": true}, "ui": {"buttonClick": "pop.mp3", "success": "cheers.mp3"}, "countdown": {"tick": "clock-tick.mp3", "celebration": "party-horn.mp3"}}',
    '{"entrance": {"type": "firework_burst", "duration": 1500}, "idle": {"type": "shimmer", "duration": 2000}, "exit": {"type": "confetti_fall", "duration": 1000}}',
    '{"header": {"type": "balloons", "position": "floating"}, "screen": {"type": "fireworks_overlay", "opacity": 0.3}, "sides": {"type": "streamers", "positions": ["left", "right", "top"]}}',
    '{"--theme-primary": "#ffd700", "--theme-secondary": "#c0c0c0", "--theme-accent": "#4169e1", "--theme-glow": "0 0 25px rgba(255, 215, 0, 0.8)", "--theme-shimmer": "shimmer 2s infinite"}'
) ON CONFLICT (theme_key) DO NOTHING;

-- =====================================================
-- DEFAULT SCHEDULES FOR SEASONAL THEMES
-- =====================================================

-- Christmas: December 1 - December 31
INSERT INTO theme_schedules (theme_id, schedule_name, start_date, end_date, is_recurring, priority, enabled)
SELECT id, 'Christmas Season', '2025-12-01', '2025-12-31', true, 90, true
FROM themes WHERE theme_key = 'christmas';

-- Halloween: October 20 - November 2
INSERT INTO theme_schedules (theme_id, schedule_name, start_date, end_date, is_recurring, priority, enabled)
SELECT id, 'Halloween Season', '2025-10-20', '2025-11-02', true, 85, true
FROM themes WHERE theme_key = 'halloween';

-- Easter: Variable (approximately April) - Using April 1-21 as placeholder
INSERT INTO theme_schedules (theme_id, schedule_name, start_date, end_date, is_recurring, priority, enabled)
SELECT id, 'Easter Season', '2025-04-01', '2025-04-21', true, 80, true
FROM themes WHERE theme_key = 'easter';

-- New Year: December 31 - January 2 (high priority for overlap)
INSERT INTO theme_schedules (theme_id, schedule_name, start_date, end_date, is_recurring, priority, enabled)
SELECT id, 'New Year Celebration', '2025-12-31', '2026-01-02', true, 100, true
FROM themes WHERE theme_key = 'new_year';

-- =====================================================
-- COMMENTS & DOCUMENTATION
-- =====================================================

COMMENT ON TABLE themes IS 'Core theme definitions with visual, audio, and animation settings';
COMMENT ON TABLE theme_schedules IS 'Automatic theme activation scheduling with date ranges and priorities';
COMMENT ON TABLE theme_assets IS 'Theme-specific assets (images, sounds, animations) with optimization settings';
COMMENT ON TABLE theme_configurations IS 'Detailed configuration options for each theme';
COMMENT ON TABLE theme_activations IS 'Historical tracking of theme activations with analytics';
COMMENT ON TABLE theme_preferences IS 'User-level theme preferences and overrides';

COMMENT ON FUNCTION activate_scheduled_theme() IS 'Automatically activates themes based on current date/time and schedules';
COMMENT ON FUNCTION get_theme_assets(INTEGER, VARCHAR) IS 'Retrieves active assets for a theme filtered by usage context';
COMMENT ON FUNCTION calculate_theme_stats(INTEGER) IS 'Calculates aggregated statistics for a theme';

-- =====================================================
-- END OF SCHEMA
-- =====================================================
