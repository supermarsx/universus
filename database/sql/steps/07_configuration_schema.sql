-- Phase 7: Comprehensive Configuration System
-- Database schema for dynamic game configuration management

-- Configuration categories
CREATE TABLE IF NOT EXISTS config_categories (
    category_id SERIAL PRIMARY KEY,
    category_name VARCHAR(100) UNIQUE NOT NULL,
    display_name VARCHAR(200) NOT NULL,
    description TEXT,
    sort_order INTEGER DEFAULT 0,
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Configuration parameters
CREATE TABLE IF NOT EXISTS config_parameters (
    parameter_id SERIAL PRIMARY KEY,
    category_id INTEGER REFERENCES config_categories(category_id),
    parameter_key VARCHAR(200) UNIQUE NOT NULL,
    parameter_name VARCHAR(200) NOT NULL,
    description TEXT,
    data_type VARCHAR(50) NOT NULL, -- 'number', 'string', 'boolean', 'json', 'formula'
    current_value TEXT NOT NULL,
    default_value TEXT NOT NULL,
    min_value NUMERIC,
    max_value NUMERIC,
    validation_rules JSONB, -- Additional validation rules
    requires_restart BOOLEAN DEFAULT FALSE,
    is_editable BOOLEAN DEFAULT TRUE,
    sort_order INTEGER DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Configuration change history
CREATE TABLE IF NOT EXISTS config_change_history (
    change_id SERIAL PRIMARY KEY,
    parameter_id INTEGER REFERENCES config_parameters(parameter_id),
    old_value TEXT,
    new_value TEXT,
    changed_by INTEGER REFERENCES users(id),
    change_reason TEXT,
    applied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    is_rolled_back BOOLEAN DEFAULT FALSE,
    rolled_back_at TIMESTAMP,
    rolled_back_by INTEGER REFERENCES users(id)
);

-- Configuration templates (presets)
CREATE TABLE IF NOT EXISTS config_templates (
    template_id SERIAL PRIMARY KEY,
    template_name VARCHAR(200) UNIQUE NOT NULL,
    description TEXT,
    template_data JSONB NOT NULL, -- Complete configuration snapshot
    created_by INTEGER REFERENCES users(id),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    is_public BOOLEAN DEFAULT FALSE,
    usage_count INTEGER DEFAULT 0
);

-- Active configuration cache (for fast access)
CREATE TABLE IF NOT EXISTS config_cache (
    cache_key VARCHAR(200) PRIMARY KEY,
    cache_value JSONB NOT NULL,
    last_updated TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMP
);

-- Configuration locks (for atomic updates)
CREATE TABLE IF NOT EXISTS config_locks (
    lock_id SERIAL PRIMARY KEY,
    category_id INTEGER REFERENCES config_categories(category_id),
    locked_by INTEGER REFERENCES users(id),
    locked_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    lock_reason TEXT,
    UNIQUE(category_id)
);

-- ============================================
-- INDEXES for performance
-- ============================================

CREATE INDEX idx_config_parameters_category ON config_parameters(category_id);
CREATE INDEX idx_config_parameters_key ON config_parameters(parameter_key);
CREATE INDEX idx_config_parameters_active ON config_parameters(is_editable);
CREATE INDEX idx_config_change_history_parameter ON config_change_history(parameter_id);
CREATE INDEX idx_config_change_history_user ON config_change_history(changed_by);
CREATE INDEX idx_config_change_history_date ON config_change_history(applied_at DESC);
CREATE INDEX idx_config_templates_public ON config_templates(is_public);
CREATE INDEX idx_config_cache_expires ON config_cache(expires_at);

-- ============================================
-- FUNCTIONS
-- ============================================

-- Function to get current configuration value
CREATE OR REPLACE FUNCTION get_config_value(p_key VARCHAR)
RETURNS TEXT AS $$
DECLARE
    v_value TEXT;
BEGIN
    SELECT current_value INTO v_value
    FROM config_parameters
    WHERE parameter_key = p_key;
    
    RETURN v_value;
END;
$$ LANGUAGE plpgsql;

-- Function to update configuration value
CREATE OR REPLACE FUNCTION update_config_value(
    p_key VARCHAR,
    p_new_value TEXT,
    p_user_id INTEGER,
    p_reason TEXT DEFAULT NULL
)
RETURNS BOOLEAN AS $$
DECLARE
    v_parameter_id INTEGER;
    v_old_value TEXT;
BEGIN
    -- Get current value and parameter_id
    SELECT parameter_id, current_value 
    INTO v_parameter_id, v_old_value
    FROM config_parameters
    WHERE parameter_key = p_key;
    
    IF v_parameter_id IS NULL THEN
        RETURN FALSE;
    END IF;
    
    -- Insert change history
    INSERT INTO config_change_history (parameter_id, old_value, new_value, changed_by, change_reason)
    VALUES (v_parameter_id, v_old_value, p_new_value, p_user_id, p_reason);
    
    -- Update parameter
    UPDATE config_parameters
    SET current_value = p_new_value,
        updated_at = CURRENT_TIMESTAMP
    WHERE parameter_id = v_parameter_id;
    
    -- Invalidate cache
    DELETE FROM config_cache WHERE cache_key LIKE p_key || '%';
    
    RETURN TRUE;
END;
$$ LANGUAGE plpgsql;

-- Function to rollback configuration change
CREATE OR REPLACE FUNCTION rollback_config_change(
    p_change_id INTEGER,
    p_user_id INTEGER
)
RETURNS BOOLEAN AS $$
DECLARE
    v_parameter_id INTEGER;
    v_old_value TEXT;
BEGIN
    -- Get change details
    SELECT parameter_id, old_value
    INTO v_parameter_id, v_old_value
    FROM config_change_history
    WHERE change_id = p_change_id AND is_rolled_back = FALSE;
    
    IF v_parameter_id IS NULL THEN
        RETURN FALSE;
    END IF;
    
    -- Mark change as rolled back
    UPDATE config_change_history
    SET is_rolled_back = TRUE,
        rolled_back_at = CURRENT_TIMESTAMP,
        rolled_back_by = p_user_id
    WHERE change_id = p_change_id;
    
    -- Revert to old value
    UPDATE config_parameters
    SET current_value = v_old_value,
        updated_at = CURRENT_TIMESTAMP
    WHERE parameter_id = v_parameter_id;
    
    -- Invalidate cache
    DELETE FROM config_cache;
    
    RETURN TRUE;
END;
$$ LANGUAGE plpgsql;

-- Function to export configuration as JSON
CREATE OR REPLACE FUNCTION export_config_snapshot()
RETURNS JSONB AS $$
DECLARE
    v_snapshot JSONB;
BEGIN
    SELECT jsonb_object_agg(
        cp.parameter_key,
        jsonb_build_object(
            'value', cp.current_value,
            'type', cp.data_type,
            'category', cc.category_name
        )
    ) INTO v_snapshot
    FROM config_parameters cp
    JOIN config_categories cc ON cp.category_id = cc.category_id
    WHERE cp.is_editable = TRUE;
    
    RETURN v_snapshot;
END;
$$ LANGUAGE plpgsql;

-- ============================================
-- TRIGGERS
-- ============================================

-- Trigger to update config_cache timestamp
CREATE OR REPLACE FUNCTION update_config_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER config_parameters_update_trigger
BEFORE UPDATE ON config_parameters
FOR EACH ROW
EXECUTE FUNCTION update_config_timestamp();

-- ============================================
-- SEED DATA: Configuration Categories
-- ============================================

INSERT INTO config_categories (category_name, display_name, description, sort_order) VALUES
('combat', 'Combat System', 'Combat formulas, damage calculations, and battle mechanics', 1),
('resources', 'Resource Management', 'Production rates, consumption rates, and storage limits', 2),
('buildings', 'Buildings', 'Construction costs, build times, and production multipliers', 3),
('research', 'Research', 'Research costs, research times, and technology requirements', 4),
('ships', 'Ships and Fleet', 'Ship stats, speeds, cargo capacities, and fuel consumption', 5),
('defense', 'Defense Systems', 'Defense structures stats and costs', 6),
('universe', 'Universe Settings', 'Galaxy sizes, planet distribution, and universe generation', 7),
('economy', 'Economic Settings', 'Trade rates, market prices, and economic multipliers', 8),
('alliances', 'Alliance System', 'Alliance mechanics, member limits, and diplomacy rules', 9),
('events', 'Events and Festivals', 'Event schedules, rewards, and special event configurations', 10),
('leaderboards', 'Leaderboards', 'Ranking algorithms, time ranges, and score weighting', 11),
('moderation', 'Moderation and Limits', 'Rate limits, content filters, and user restrictions', 12),
('gameplay', 'General Gameplay', 'Game speed, starting resources, and general game rules', 13)
ON CONFLICT (category_name) DO NOTHING;

-- ============================================
-- SEED DATA: Core Configuration Parameters
-- ============================================

-- COMBAT PARAMETERS
INSERT INTO config_parameters (category_id, parameter_key, parameter_name, description, data_type, current_value, default_value, min_value, max_value) 
SELECT category_id, 'combat.damage_multiplier', 'Damage Multiplier', 'Global damage multiplier for all combat', 'number', '1.0', '1.0', 0.1, 10.0
FROM config_categories WHERE category_name = 'combat';

INSERT INTO config_parameters (category_id, parameter_key, parameter_name, description, data_type, current_value, default_value, min_value, max_value)
SELECT category_id, 'combat.shield_absorption', 'Shield Absorption Rate', 'Percentage of damage absorbed by shields', 'number', '1.0', '1.0', 0.0, 1.0
FROM config_categories WHERE category_name = 'combat';

INSERT INTO config_parameters (category_id, parameter_key, parameter_name, description, data_type, current_value, default_value, min_value, max_value)
SELECT category_id, 'combat.armor_reduction', 'Armor Damage Reduction', 'Percentage of damage reduced by armor', 'number', '1.0', '1.0', 0.0, 1.0
FROM config_categories WHERE category_name = 'combat';

INSERT INTO config_parameters (category_id, parameter_key, parameter_name, description, data_type, current_value, default_value, min_value, max_value)
SELECT category_id, 'combat.max_battle_rounds', 'Maximum Battle Rounds', 'Maximum number of combat rounds before draw', 'number', '6', '6', 1, 100
FROM config_categories WHERE category_name = 'combat';

INSERT INTO config_parameters (category_id, parameter_key, parameter_name, description, data_type, current_value, default_value, min_value, max_value)
SELECT category_id, 'combat.rapid_fire_enabled', 'Enable Rapid Fire', 'Enable rapid fire mechanics in combat', 'boolean', 'true', 'true', NULL, NULL
FROM config_categories WHERE category_name = 'combat';

INSERT INTO config_parameters (category_id, parameter_key, parameter_name, description, data_type, current_value, default_value, min_value, max_value)
SELECT category_id, 'combat.debris_field_percentage', 'Debris Field Percentage', 'Percentage of destroyed ships that become debris', 'number', '0.3', '0.3', 0.0, 1.0
FROM config_categories WHERE category_name = 'combat';

-- RESOURCE PARAMETERS
INSERT INTO config_parameters (category_id, parameter_key, parameter_name, description, data_type, current_value, default_value, min_value, max_value)
SELECT category_id, 'resources.production_multiplier', 'Production Multiplier', 'Global resource production multiplier', 'number', '1.0', '1.0', 0.1, 100.0
FROM config_categories WHERE category_name = 'resources';

INSERT INTO config_parameters (category_id, parameter_key, parameter_name, description, data_type, current_value, default_value, min_value, max_value)
SELECT category_id, 'resources.metal_multiplier', 'Metal Production Multiplier', 'Metal mine production multiplier', 'number', '1.0', '1.0', 0.1, 100.0
FROM config_categories WHERE category_name = 'resources';

INSERT INTO config_parameters (category_id, parameter_key, parameter_name, description, data_type, current_value, default_value, min_value, max_value)
SELECT category_id, 'resources.crystal_multiplier', 'Crystal Production Multiplier', 'Crystal mine production multiplier', 'number', '1.0', '1.0', 0.1, 100.0
FROM config_categories WHERE category_name = 'resources';

INSERT INTO config_parameters (category_id, parameter_key, parameter_name, description, data_type, current_value, default_value, min_value, max_value)
SELECT category_id, 'resources.deuterium_multiplier', 'Deuterium Production Multiplier', 'Deuterium synthesizer production multiplier', 'number', '1.0', '1.0', 0.1, 100.0
FROM config_categories WHERE category_name = 'resources';

INSERT INTO config_parameters (category_id, parameter_key, parameter_name, description, data_type, current_value, default_value, min_value, max_value)
SELECT category_id, 'resources.starting_metal', 'Starting Metal', 'Metal amount for new players', 'number', '500', '500', 0, 1000000
FROM config_categories WHERE category_name = 'resources';

INSERT INTO config_parameters (category_id, parameter_key, parameter_name, description, data_type, current_value, default_value, min_value, max_value)
SELECT category_id, 'resources.starting_crystal', 'Starting Crystal', 'Crystal amount for new players', 'number', '500', '500', 0, 1000000
FROM config_categories WHERE category_name = 'resources';

INSERT INTO config_parameters (category_id, parameter_key, parameter_name, description, data_type, current_value, default_value, min_value, max_value)
SELECT category_id, 'resources.starting_deuterium', 'Starting Deuterium', 'Deuterium amount for new players', 'number', '0', '0', 0, 1000000
FROM config_categories WHERE category_name = 'resources';

-- BUILDING PARAMETERS
INSERT INTO config_parameters (category_id, parameter_key, parameter_name, description, data_type, current_value, default_value, min_value, max_value)
SELECT category_id, 'buildings.cost_multiplier', 'Building Cost Multiplier', 'Global building cost multiplier', 'number', '1.0', '1.0', 0.1, 10.0
FROM config_categories WHERE category_name = 'buildings';

INSERT INTO config_parameters (category_id, parameter_key, parameter_name, description, data_type, current_value, default_value, min_value, max_value)
SELECT category_id, 'buildings.time_multiplier', 'Building Time Multiplier', 'Global construction time multiplier', 'number', '1.0', '1.0', 0.1, 10.0
FROM config_categories WHERE category_name = 'buildings';

INSERT INTO config_parameters (category_id, parameter_key, parameter_name, description, data_type, current_value, default_value, min_value, max_value)
SELECT category_id, 'buildings.max_queue_size', 'Maximum Build Queue', 'Maximum number of buildings in construction queue', 'number', '5', '5', 1, 100
FROM config_categories WHERE category_name = 'buildings';

-- RESEARCH PARAMETERS
INSERT INTO config_parameters (category_id, parameter_key, parameter_name, description, data_type, current_value, default_value, min_value, max_value)
SELECT category_id, 'research.cost_multiplier', 'Research Cost Multiplier', 'Global research cost multiplier', 'number', '1.0', '1.0', 0.1, 10.0
FROM config_categories WHERE category_name = 'research';

INSERT INTO config_parameters (category_id, parameter_key, parameter_name, description, data_type, current_value, default_value, min_value, max_value)
SELECT category_id, 'research.time_multiplier', 'Research Time Multiplier', 'Global research time multiplier', 'number', '1.0', '1.0', 0.1, 10.0
FROM config_categories WHERE category_name = 'research';

-- FLEET PARAMETERS
INSERT INTO config_parameters (category_id, parameter_key, parameter_name, description, data_type, current_value, default_value, min_value, max_value)
SELECT category_id, 'fleet.speed_multiplier', 'Fleet Speed Multiplier', 'Global fleet speed multiplier', 'number', '1.0', '1.0', 0.1, 100.0
FROM config_categories WHERE category_name = 'ships';

INSERT INTO config_parameters (category_id, parameter_key, parameter_name, description, data_type, current_value, default_value, min_value, max_value)
SELECT category_id, 'fleet.fuel_consumption_multiplier', 'Fuel Consumption Multiplier', 'Global fleet fuel consumption multiplier', 'number', '1.0', '1.0', 0.1, 10.0
FROM config_categories WHERE category_name = 'ships';

INSERT INTO config_parameters (category_id, parameter_key, parameter_name, description, data_type, current_value, default_value, min_value, max_value)
SELECT category_id, 'fleet.cargo_multiplier', 'Cargo Capacity Multiplier', 'Global cargo capacity multiplier', 'number', '1.0', '1.0', 0.1, 100.0
FROM config_categories WHERE category_name = 'ships';

-- UNIVERSE PARAMETERS
INSERT INTO config_parameters (category_id, parameter_key, parameter_name, description, data_type, current_value, default_value, min_value, max_value)
SELECT category_id, 'universe.max_galaxies', 'Maximum Galaxies', 'Number of galaxies in universe', 'number', '9', '9', 1, 100
FROM config_categories WHERE category_name = 'universe';

INSERT INTO config_parameters (category_id, parameter_key, parameter_name, description, data_type, current_value, default_value, min_value, max_value)
SELECT category_id, 'universe.max_systems', 'Maximum Systems', 'Number of systems per galaxy', 'number', '499', '499', 10, 9999
FROM config_categories WHERE category_name = 'universe';

INSERT INTO config_parameters (category_id, parameter_key, parameter_name, description, data_type, current_value, default_value, min_value, max_value)
SELECT category_id, 'universe.max_planets', 'Maximum Planets', 'Number of planet positions per system', 'number', '15', '15', 1, 50
FROM config_categories WHERE category_name = 'universe';

INSERT INTO config_parameters (category_id, parameter_key, parameter_name, description, data_type, current_value, default_value, min_value, max_value)
SELECT category_id, 'universe.player_starting_planets', 'Starting Planets', 'Number of planets new players start with', 'number', '1', '1', 1, 10
FROM config_categories WHERE category_name = 'universe';

-- ALLIANCE PARAMETERS
INSERT INTO config_parameters (category_id, parameter_key, parameter_name, description, data_type, current_value, default_value, min_value, max_value)
SELECT category_id, 'alliance.max_members', 'Maximum Alliance Members', 'Maximum number of members per alliance', 'number', '100', '100', 1, 10000
FROM config_categories WHERE category_name = 'alliances';

INSERT INTO config_parameters (category_id, parameter_key, parameter_name, description, data_type, current_value, default_value, min_value, max_value)
SELECT category_id, 'alliance.creation_cost', 'Alliance Creation Cost', 'Crystal cost to create an alliance', 'number', '50000', '50000', 0, 10000000
FROM config_categories WHERE category_name = 'alliances';

-- GAMEPLAY PARAMETERS
INSERT INTO config_parameters (category_id, parameter_key, parameter_name, description, data_type, current_value, default_value, min_value, max_value, requires_restart)
SELECT category_id, 'gameplay.speed', 'Game Speed', 'Overall game speed multiplier', 'number', '1', '1', 1, 100, TRUE
FROM config_categories WHERE category_name = 'gameplay';

INSERT INTO config_parameters (category_id, parameter_key, parameter_name, description, data_type, current_value, default_value)
SELECT category_id, 'gameplay.server_name', 'Server Name', 'Display name of the game server', 'string', 'Universus Space Empire', 'Universus Space Empire'
FROM config_categories WHERE category_name = 'gameplay';

INSERT INTO config_parameters (category_id, parameter_key, parameter_name, description, data_type, current_value, default_value)
SELECT category_id, 'gameplay.maintenance_mode', 'Maintenance Mode', 'Enable maintenance mode (players cannot login)', 'boolean', 'false', 'false'
FROM config_categories WHERE category_name = 'gameplay';

-- ============================================
-- VIEWS
-- ============================================

-- View for active configuration with category info
CREATE OR REPLACE VIEW v_active_config AS
SELECT 
    cp.parameter_id,
    cp.parameter_key,
    cp.parameter_name,
    cp.description,
    cp.data_type,
    cp.current_value,
    cp.default_value,
    cp.min_value,
    cp.max_value,
    cp.requires_restart,
    cc.category_name,
    cc.display_name as category_display_name,
    cp.updated_at
FROM config_parameters cp
JOIN config_categories cc ON cp.category_id = cc.category_id
WHERE cp.is_editable = TRUE AND cc.is_active = TRUE
ORDER BY cc.sort_order, cp.sort_order;

-- View for recent configuration changes
CREATE OR REPLACE VIEW v_recent_config_changes AS
SELECT 
    ch.change_id,
    cp.parameter_key,
    cp.parameter_name,
    ch.old_value,
    ch.new_value,
    u.username as changed_by_username,
    ch.change_reason,
    ch.applied_at,
    ch.is_rolled_back
FROM config_change_history ch
JOIN config_parameters cp ON ch.parameter_id = cp.parameter_id
LEFT JOIN users u ON ch.changed_by = u.id
ORDER BY ch.applied_at DESC
LIMIT 100;

-- View for configuration statistics
CREATE OR REPLACE VIEW v_config_statistics AS
SELECT
    cc.category_name,
    cc.display_name,
    COUNT(cp.parameter_id) as total_parameters,
    COUNT(CASE WHEN cp.current_value != cp.default_value THEN 1 END) as modified_parameters,
    COUNT(CASE WHEN cp.requires_restart = TRUE THEN 1 END) as restart_required_parameters
FROM config_categories cc
LEFT JOIN config_parameters cp ON cc.category_id = cp.category_id
WHERE cc.is_active = TRUE
GROUP BY cc.category_id, cc.category_name, cc.display_name
ORDER BY cc.sort_order;

-- Create composite index for better query performance
CREATE INDEX idx_config_parameters_composite ON config_parameters(category_id, is_editable, sort_order);
