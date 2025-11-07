-- =====================================================
-- PHASE 4: UNIVERSE SEEDING SYSTEM
-- Database Schema for Universus RPG
-- Created: 2025-11-06
-- =====================================================

-- =====================================================
-- TABLE: universe_seeds
-- Purpose: Universe configuration and parameters
-- =====================================================
CREATE TABLE IF NOT EXISTS universe_seeds (
    id SERIAL PRIMARY KEY,
    
    -- Universe Configuration
    universe_name VARCHAR(100) NOT NULL UNIQUE,
    universe_type VARCHAR(30) NOT NULL CHECK (universe_type IN (
        'balanced', 'resource_rich', 'combat_focused', 
        'research_heavy', 'mixed_economy', 'hardcore'
    )),
    
    -- Galaxy Configuration
    galaxy_count INTEGER NOT NULL DEFAULT 9 CHECK (galaxy_count BETWEEN 1 AND 20),
    systems_per_galaxy INTEGER NOT NULL DEFAULT 499 CHECK (systems_per_galaxy BETWEEN 100 AND 999),
    positions_per_system INTEGER NOT NULL DEFAULT 15 CHECK (positions_per_system BETWEEN 10 AND 20),
    
    -- Population Configuration
    max_players INTEGER NOT NULL DEFAULT 10000,
    current_players INTEGER DEFAULT 0,
    bot_percentage DECIMAL(5,2) DEFAULT 30.00 CHECK (bot_percentage BETWEEN 0 AND 80),
    target_bot_count INTEGER DEFAULT 0,
    
    -- Resource Configuration
    resource_multiplier DECIMAL(5,2) DEFAULT 1.00 CHECK (resource_multiplier > 0),
    starting_resources_metal BIGINT DEFAULT 500,
    starting_resources_crystal BIGINT DEFAULT 300,
    starting_resources_deuterium BIGINT DEFAULT 100,
    
    -- Difficulty Configuration
    difficulty_curve VARCHAR(20) DEFAULT 'progressive' CHECK (difficulty_curve IN ('flat', 'progressive', 'steep', 'custom')),
    beginner_protection_days INTEGER DEFAULT 7,
    
    -- Seeding Status
    is_seeded BOOLEAN DEFAULT FALSE,
    seed_version INTEGER DEFAULT 1,
    seeding_started_at TIMESTAMP WITH TIME ZONE,
    seeding_completed_at TIMESTAMP WITH TIME ZONE,
    last_maintained_at TIMESTAMP WITH TIME ZONE,
    
    -- Metadata
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    created_by INTEGER REFERENCES users(id) ON DELETE SET NULL,
    configuration JSONB
);

CREATE INDEX idx_universe_type ON universe_seeds(universe_type);
CREATE INDEX idx_universe_seeded ON universe_seeds(is_seeded);

-- =====================================================
-- TABLE: galaxy_seeds
-- Purpose: Individual galaxy configurations
-- =====================================================
CREATE TABLE IF NOT EXISTS galaxy_seeds (
    id SERIAL PRIMARY KEY,
    universe_id INTEGER NOT NULL REFERENCES universe_seeds(id) ON DELETE CASCADE,
    
    -- Galaxy Identification
    galaxy_number INTEGER NOT NULL CHECK (galaxy_number > 0),
    galaxy_name VARCHAR(100),
    
    -- Galaxy Type
    galaxy_type VARCHAR(30) NOT NULL CHECK (galaxy_type IN (
        'standard', 'resource_rich', 'military', 'research', 
        'wasteland', 'endgame', 'safe_zone', 'pvp_zone'
    )),
    
    -- Structure Configuration
    system_count INTEGER NOT NULL DEFAULT 499,
    sector_divisions INTEGER DEFAULT 10,
    
    -- Resource Configuration
    metal_abundance DECIMAL(5,2) DEFAULT 1.00,
    crystal_abundance DECIMAL(5,2) DEFAULT 1.00,
    deuterium_abundance DECIMAL(5,2) DEFAULT 1.00,
    rare_materials_chance DECIMAL(5,2) DEFAULT 5.00,
    
    -- Difficulty Configuration
    base_difficulty INTEGER DEFAULT 5 CHECK (base_difficulty BETWEEN 1 AND 10),
    npc_strength_multiplier DECIMAL(5,2) DEFAULT 1.00,
    
    -- Population Configuration
    max_players_per_galaxy INTEGER DEFAULT 1000,
    current_players INTEGER DEFAULT 0,
    bot_count INTEGER DEFAULT 0,
    
    -- Strategic Features
    has_safe_zones BOOLEAN DEFAULT TRUE,
    has_pvp_zones BOOLEAN DEFAULT TRUE,
    has_resource_zones BOOLEAN DEFAULT TRUE,
    has_event_zones BOOLEAN DEFAULT FALSE,
    
    -- Seeding Status
    is_generated BOOLEAN DEFAULT FALSE,
    generated_at TIMESTAMP WITH TIME ZONE,
    
    -- Metadata
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    
    CONSTRAINT unique_galaxy_per_universe UNIQUE (universe_id, galaxy_number)
);

CREATE INDEX idx_galaxy_universe ON galaxy_seeds(universe_id);
CREATE INDEX idx_galaxy_type ON galaxy_seeds(galaxy_type);
CREATE INDEX idx_galaxy_generated ON galaxy_seeds(is_generated);

-- =====================================================
-- TABLE: sector_configurations
-- Purpose: Sector-based difficulty and resource settings
-- =====================================================
CREATE TABLE IF NOT EXISTS sector_configurations (
    id SERIAL PRIMARY KEY,
    galaxy_id INTEGER NOT NULL REFERENCES galaxy_seeds(id) ON DELETE CASCADE,
    
    -- Sector Identification
    sector_number INTEGER NOT NULL CHECK (sector_number BETWEEN 1 AND 10),
    sector_name VARCHAR(50),
    
    -- Difficulty Settings
    difficulty_tier INTEGER NOT NULL CHECK (difficulty_tier BETWEEN 1 AND 10),
    recommended_level INTEGER DEFAULT 1,
    
    -- System Range (which systems belong to this sector)
    system_start INTEGER NOT NULL,
    system_end INTEGER NOT NULL,
    
    -- Resource Distribution
    metal_multiplier DECIMAL(5,2) DEFAULT 1.00,
    crystal_multiplier DECIMAL(5,2) DEFAULT 1.00,
    deuterium_multiplier DECIMAL(5,2) DEFAULT 1.00,
    
    -- Strategic Properties
    is_safe_zone BOOLEAN DEFAULT FALSE,
    is_pvp_zone BOOLEAN DEFAULT FALSE,
    is_beginner_zone BOOLEAN DEFAULT FALSE,
    is_endgame_zone BOOLEAN DEFAULT FALSE,
    
    -- NPC Configuration
    npc_density DECIMAL(5,2) DEFAULT 0.30,
    npc_strength_level INTEGER DEFAULT 5,
    
    -- Metadata
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    
    CONSTRAINT unique_sector_per_galaxy UNIQUE (galaxy_id, sector_number)
);

CREATE INDEX idx_sector_galaxy ON sector_configurations(galaxy_id);
CREATE INDEX idx_sector_tier ON sector_configurations(difficulty_tier);

-- =====================================================
-- TABLE: player_placement_rules
-- Purpose: Player starting position logic
-- =====================================================
CREATE TABLE IF NOT EXISTS player_placement_rules (
    id SERIAL PRIMARY KEY,
    universe_id INTEGER NOT NULL REFERENCES universe_seeds(id) ON DELETE CASCADE,
    
    -- Rule Configuration
    rule_name VARCHAR(100) NOT NULL,
    rule_priority INTEGER DEFAULT 100,
    is_active BOOLEAN DEFAULT TRUE,
    
    -- Placement Criteria
    player_level_min INTEGER DEFAULT 0,
    player_level_max INTEGER DEFAULT 999,
    preferred_galaxy_types TEXT[],
    preferred_sector_tiers INTEGER[],
    
    -- Placement Strategy
    strategy VARCHAR(30) NOT NULL CHECK (strategy IN (
        'random', 'balanced', 'clustered', 'dispersed', 
        'alliance_grouped', 'skill_based'
    )),
    
    -- Constraints
    min_distance_from_players INTEGER DEFAULT 5,
    max_distance_from_center INTEGER,
    avoid_high_activity_zones BOOLEAN DEFAULT FALSE,
    
    -- Resource Preferences
    prefer_metal_rich BOOLEAN DEFAULT FALSE,
    prefer_crystal_rich BOOLEAN DEFAULT FALSE,
    prefer_deuterium_rich BOOLEAN DEFAULT FALSE,
    
    -- Metadata
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    configuration JSONB
);

CREATE INDEX idx_placement_universe ON player_placement_rules(universe_id);
CREATE INDEX idx_placement_active ON player_placement_rules(is_active);

-- =====================================================
-- TABLE: player_placements
-- Purpose: Track actual player starting positions
-- =====================================================
CREATE TABLE IF NOT EXISTS player_placements (
    id SERIAL PRIMARY KEY,
    universe_id INTEGER NOT NULL REFERENCES universe_seeds(id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    
    -- Placement Location
    galaxy INTEGER NOT NULL,
    system INTEGER NOT NULL,
    position INTEGER NOT NULL,
    
    -- Placement Context
    placement_strategy VARCHAR(30),
    placement_rule_id INTEGER REFERENCES player_placement_rules(id),
    
    -- Player Context at Placement
    player_level_at_placement INTEGER DEFAULT 1,
    player_experience_at_placement BIGINT DEFAULT 0,
    preferred_playstyle VARCHAR(30),
    
    -- Alliance Context
    alliance_id INTEGER,
    was_grouped_placement BOOLEAN DEFAULT FALSE,
    
    -- Starting Resources
    starting_metal BIGINT,
    starting_crystal BIGINT,
    starting_deuterium BIGINT,
    
    -- Placement Quality
    placement_quality_score DECIMAL(5,2),
    resource_richness_score DECIMAL(5,2),
    strategic_value_score DECIMAL(5,2),
    
    -- Timestamps
    placed_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    
    CONSTRAINT unique_player_placement UNIQUE (universe_id, user_id)
);

CREATE INDEX idx_placement_user ON player_placements(user_id);
CREATE INDEX idx_placement_location ON player_placements(galaxy, system, position);
CREATE INDEX idx_placement_universe ON player_placements(universe_id);

-- =====================================================
-- TABLE: bot_generation_templates
-- Purpose: Bot player generation configurations
-- =====================================================
CREATE TABLE IF NOT EXISTS bot_generation_templates (
    id SERIAL PRIMARY KEY,
    universe_id INTEGER NOT NULL REFERENCES universe_seeds(id) ON DELETE CASCADE,
    
    -- Template Configuration
    template_name VARCHAR(100) NOT NULL,
    bot_personality VARCHAR(30) NOT NULL CHECK (bot_personality IN (
        'aggressive', 'defensive', 'economic', 'explorer', 
        'researcher', 'diplomatic', 'opportunist', 'balanced'
    )),
    
    -- Skill Configuration
    skill_level VARCHAR(20) NOT NULL CHECK (skill_level IN ('novice', 'intermediate', 'advanced', 'expert')),
    skill_randomness DECIMAL(5,2) DEFAULT 0.20,
    
    -- Behavior Configuration
    aggression_level INTEGER DEFAULT 5 CHECK (aggression_level BETWEEN 1 AND 10),
    expansion_rate DECIMAL(5,2) DEFAULT 1.00,
    trading_activity DECIMAL(5,2) DEFAULT 0.50,
    alliance_participation BOOLEAN DEFAULT TRUE,
    
    -- Resource Management
    resource_focus VARCHAR(20) DEFAULT 'balanced' CHECK (resource_focus IN ('metal', 'crystal', 'deuterium', 'balanced')),
    building_priority TEXT[],
    research_priority TEXT[],
    
    -- Fleet Management
    fleet_composition JSONB,
    preferred_ship_types TEXT[],
    combat_willingness DECIMAL(5,2) DEFAULT 0.50,
    
    -- Generation Parameters
    generation_weight INTEGER DEFAULT 100,
    max_bots_from_template INTEGER,
    current_bots_generated INTEGER DEFAULT 0,
    
    -- Metadata
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    configuration JSONB
);

CREATE INDEX idx_bot_template_universe ON bot_generation_templates(universe_id);
CREATE INDEX idx_bot_template_personality ON bot_generation_templates(bot_personality);

-- =====================================================
-- TABLE: generated_bots
-- Purpose: Track generated bot players
-- =====================================================
CREATE TABLE IF NOT EXISTS generated_bots (
    id SERIAL PRIMARY KEY,
    universe_id INTEGER NOT NULL REFERENCES universe_seeds(id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    template_id INTEGER REFERENCES bot_generation_templates(id),
    
    -- Bot Configuration
    bot_name VARCHAR(100) NOT NULL,
    bot_personality VARCHAR(30) NOT NULL,
    skill_level VARCHAR(20) NOT NULL,
    
    -- Placement
    galaxy INTEGER NOT NULL,
    system INTEGER NOT NULL,
    position INTEGER NOT NULL,
    
    -- Status
    is_active BOOLEAN DEFAULT TRUE,
    activation_date TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    deactivation_date TIMESTAMP WITH TIME ZONE,
    
    -- Performance Tracking
    total_attacks INTEGER DEFAULT 0,
    total_defenses INTEGER DEFAULT 0,
    total_trades INTEGER DEFAULT 0,
    total_resources_collected BIGINT DEFAULT 0,
    
    -- Alliance Membership
    alliance_id INTEGER,
    alliance_role VARCHAR(30),
    
    -- Metadata
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    
    CONSTRAINT unique_bot_user UNIQUE (universe_id, user_id)
);

CREATE INDEX idx_generated_bot_universe ON generated_bots(universe_id);
CREATE INDEX idx_generated_bot_user ON generated_bots(user_id);
CREATE INDEX idx_generated_bot_active ON generated_bots(is_active);
CREATE INDEX idx_generated_bot_location ON generated_bots(galaxy, system, position);

-- =====================================================
-- TABLE: resource_distribution_patterns
-- Purpose: Resource placement algorithms
-- =====================================================
CREATE TABLE IF NOT EXISTS resource_distribution_patterns (
    id SERIAL PRIMARY KEY,
    galaxy_id INTEGER NOT NULL REFERENCES galaxy_seeds(id) ON DELETE CASCADE,
    
    -- Pattern Configuration
    pattern_name VARCHAR(100) NOT NULL,
    pattern_type VARCHAR(30) NOT NULL CHECK (pattern_type IN (
        'uniform', 'clustered', 'radial', 'strategic', 'random'
    )),
    
    -- Resource Type
    resource_type VARCHAR(20) NOT NULL CHECK (resource_type IN (
        'metal', 'crystal', 'deuterium', 'rare_materials', 'mixed'
    )),
    
    -- Distribution Parameters
    base_abundance DECIMAL(5,2) DEFAULT 1.00,
    variation_percentage DECIMAL(5,2) DEFAULT 0.20,
    cluster_size INTEGER DEFAULT 5,
    cluster_density DECIMAL(5,2) DEFAULT 1.50,
    
    -- Strategic Positioning
    prefer_outer_systems BOOLEAN DEFAULT FALSE,
    prefer_center_systems BOOLEAN DEFAULT FALSE,
    strategic_chokepoints BOOLEAN DEFAULT FALSE,
    
    -- Application Status
    is_applied BOOLEAN DEFAULT FALSE,
    applied_at TIMESTAMP WITH TIME ZONE,
    
    -- Metadata
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    configuration JSONB
);

CREATE INDEX idx_resource_pattern_galaxy ON resource_distribution_patterns(galaxy_id);
CREATE INDEX idx_resource_pattern_type ON resource_distribution_patterns(pattern_type);

-- =====================================================
-- TABLE: planet_resources
-- Purpose: Track resource richness of planets
-- =====================================================
CREATE TABLE IF NOT EXISTS planet_resources (
    id SERIAL PRIMARY KEY,
    planet_id INTEGER REFERENCES planets(id) ON DELETE CASCADE,
    
    -- Location (for unoccupied positions)
    galaxy INTEGER,
    system INTEGER,
    position INTEGER,
    
    -- Resource Richness
    metal_richness DECIMAL(5,2) DEFAULT 1.00 CHECK (metal_richness >= 0),
    crystal_richness DECIMAL(5,2) DEFAULT 1.00 CHECK (crystal_richness >= 0),
    deuterium_richness DECIMAL(5,2) DEFAULT 1.00 CHECK (deuterium_richness >= 0),
    
    -- Special Resources
    has_rare_materials BOOLEAN DEFAULT FALSE,
    rare_material_type VARCHAR(50),
    rare_material_abundance DECIMAL(5,2) DEFAULT 0.00,
    
    -- Strategic Value
    strategic_value INTEGER DEFAULT 5 CHECK (strategic_value BETWEEN 1 AND 10),
    is_chokepoint BOOLEAN DEFAULT FALSE,
    is_hidden BOOLEAN DEFAULT FALSE,
    
    -- Discovery
    is_discovered BOOLEAN DEFAULT FALSE,
    discovered_by INTEGER REFERENCES users(id),
    discovered_at TIMESTAMP WITH TIME ZONE,
    
    -- Metadata
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    
    CONSTRAINT planet_or_location CHECK (
        (planet_id IS NOT NULL) OR 
        (galaxy IS NOT NULL AND system IS NOT NULL AND position IS NOT NULL)
    )
);

CREATE INDEX idx_planet_resources_planet ON planet_resources(planet_id);
CREATE INDEX idx_planet_resources_location ON planet_resources(galaxy, system, position);
CREATE INDEX idx_planet_resources_strategic ON planet_resources(strategic_value);

-- =====================================================
-- TABLE: alliance_seeds
-- Purpose: Alliance formation and placement
-- =====================================================
CREATE TABLE IF NOT EXISTS alliance_seeds (
    id SERIAL PRIMARY KEY,
    universe_id INTEGER NOT NULL REFERENCES universe_seeds(id) ON DELETE CASCADE,
    
    -- Alliance Configuration
    alliance_name VARCHAR(100) NOT NULL,
    alliance_tag VARCHAR(10) NOT NULL,
    alliance_type VARCHAR(30) CHECK (alliance_type IN (
        'military', 'economic', 'research', 'balanced', 'role_play'
    )),
    
    -- Formation Strategy
    formation_strategy VARCHAR(30) NOT NULL CHECK (formation_strategy IN (
        'pre_seeded', 'player_created', 'bot_alliance', 'mixed'
    )),
    
    -- Target Configuration
    target_member_count INTEGER DEFAULT 50,
    current_member_count INTEGER DEFAULT 0,
    bot_member_percentage DECIMAL(5,2) DEFAULT 50.00,
    
    -- Territory Configuration
    home_galaxy INTEGER,
    home_sector INTEGER,
    territory_systems TEXT[],
    
    -- Specialization
    specialization TEXT[],
    alliance_bonuses JSONB,
    
    -- Status
    is_formed BOOLEAN DEFAULT FALSE,
    formed_at TIMESTAMP WITH TIME ZONE,
    
    -- Metadata
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    configuration JSONB
);

CREATE INDEX idx_alliance_seed_universe ON alliance_seeds(universe_id);
CREATE INDEX idx_alliance_seed_formed ON alliance_seeds(is_formed);

-- =====================================================
-- TABLE: universe_maintenance_tasks
-- Purpose: Automated universe management
-- =====================================================
CREATE TABLE IF NOT EXISTS universe_maintenance_tasks (
    id SERIAL PRIMARY KEY,
    universe_id INTEGER NOT NULL REFERENCES universe_seeds(id) ON DELETE CASCADE,
    
    -- Task Configuration
    task_name VARCHAR(100) NOT NULL,
    task_type VARCHAR(50) NOT NULL CHECK (task_type IN (
        'population_balance', 'resource_balance', 'bot_management',
        'cleanup', 'analytics', 'performance', 'security'
    )),
    
    -- Schedule Configuration
    run_frequency_hours INTEGER NOT NULL DEFAULT 24,
    last_run_at TIMESTAMP WITH TIME ZONE,
    next_run_at TIMESTAMP WITH TIME ZONE,
    
    -- Execution Configuration
    is_active BOOLEAN DEFAULT TRUE,
    is_running BOOLEAN DEFAULT FALSE,
    auto_adjust BOOLEAN DEFAULT TRUE,
    
    -- Performance Tracking
    total_runs INTEGER DEFAULT 0,
    successful_runs INTEGER DEFAULT 0,
    failed_runs INTEGER DEFAULT 0,
    average_duration_seconds INTEGER,
    
    -- Task Results
    last_result JSONB,
    last_error TEXT,
    
    -- Thresholds and Triggers
    trigger_conditions JSONB,
    action_parameters JSONB,
    
    -- Metadata
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_maintenance_universe ON universe_maintenance_tasks(universe_id);
CREATE INDEX idx_maintenance_active ON universe_maintenance_tasks(is_active, next_run_at);
CREATE INDEX idx_maintenance_type ON universe_maintenance_tasks(task_type);

-- =====================================================
-- TABLE: universe_analytics
-- Purpose: Track universe health and balance metrics
-- =====================================================
CREATE TABLE IF NOT EXISTS universe_analytics (
    id SERIAL PRIMARY KEY,
    universe_id INTEGER NOT NULL REFERENCES universe_seeds(id) ON DELETE CASCADE,
    
    -- Snapshot Timestamp
    snapshot_date DATE NOT NULL,
    snapshot_hour INTEGER DEFAULT 0,
    
    -- Population Metrics
    total_active_players INTEGER DEFAULT 0,
    total_active_bots INTEGER DEFAULT 0,
    new_players_24h INTEGER DEFAULT 0,
    churned_players_24h INTEGER DEFAULT 0,
    
    -- Economic Metrics
    total_metal_economy BIGINT DEFAULT 0,
    total_crystal_economy BIGINT DEFAULT 0,
    total_deuterium_economy BIGINT DEFAULT 0,
    average_player_resources BIGINT DEFAULT 0,
    
    -- Military Metrics
    total_fleet_power BIGINT DEFAULT 0,
    total_combats_24h INTEGER DEFAULT 0,
    total_debris_generated_24h BIGINT DEFAULT 0,
    
    -- Balance Metrics
    gini_coefficient DECIMAL(5,4),
    resource_distribution_variance DECIMAL(10,2),
    power_concentration_top10 DECIMAL(5,2),
    
    -- Activity Metrics
    average_session_duration_minutes INTEGER,
    daily_active_users INTEGER,
    peak_concurrent_users INTEGER,
    
    -- Alliance Metrics
    total_alliances INTEGER DEFAULT 0,
    average_alliance_size DECIMAL(10,2),
    alliance_war_count INTEGER DEFAULT 0,
    
    -- Metadata
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    
    CONSTRAINT unique_universe_snapshot UNIQUE (universe_id, snapshot_date, snapshot_hour)
);

CREATE INDEX idx_analytics_universe ON universe_analytics(universe_id);
CREATE INDEX idx_analytics_date ON universe_analytics(snapshot_date DESC);

-- =====================================================
-- VIEWS
-- =====================================================

-- Active Universes Overview
CREATE OR REPLACE VIEW v_active_universes AS
SELECT 
    us.*,
    COUNT(DISTINCT pp.user_id) as actual_player_count,
    COUNT(DISTINCT gb.user_id) as actual_bot_count,
    (SELECT COUNT(*) FROM galaxy_seeds WHERE universe_id = us.id AND is_generated = TRUE) as generated_galaxies
FROM universe_seeds us
LEFT JOIN player_placements pp ON us.id = pp.universe_id
LEFT JOIN generated_bots gb ON us.id = gb.universe_id AND gb.is_active = TRUE
WHERE us.is_seeded = TRUE
GROUP BY us.id;

-- Galaxy Statistics
CREATE OR REPLACE VIEW v_galaxy_statistics AS
SELECT 
    gs.*,
    COUNT(DISTINCT pp.user_id) as player_count,
    COUNT(DISTINCT gb.user_id) as bot_count,
    AVG(pr.metal_richness) as avg_metal_richness,
    AVG(pr.crystal_richness) as avg_crystal_richness,
    AVG(pr.deuterium_richness) as avg_deuterium_richness
FROM galaxy_seeds gs
LEFT JOIN player_placements pp ON gs.universe_id = pp.universe_id AND gs.galaxy_number = pp.galaxy
LEFT JOIN generated_bots gb ON gs.universe_id = gb.universe_id AND gs.galaxy_number = gb.galaxy AND gb.is_active = TRUE
LEFT JOIN planet_resources pr ON gs.galaxy_number = pr.galaxy
GROUP BY gs.id;

-- Bot Performance Leaderboard
CREATE OR REPLACE VIEW v_bot_performance AS
SELECT 
    gb.*,
    u.username,
    (gb.total_attacks + gb.total_defenses + gb.total_trades) as total_activity,
    RANK() OVER (PARTITION BY gb.universe_id ORDER BY gb.total_resources_collected DESC) as resource_rank
FROM generated_bots gb
JOIN users u ON gb.user_id = u.id
WHERE gb.is_active = TRUE
ORDER BY gb.total_resources_collected DESC;

-- =====================================================
-- FUNCTIONS
-- =====================================================

-- Function: Calculate placement quality score
CREATE OR REPLACE FUNCTION calculate_placement_quality(
    p_galaxy INTEGER,
    p_system INTEGER,
    p_position INTEGER,
    p_universe_id INTEGER
) RETURNS DECIMAL AS $$
DECLARE
    v_resource_score DECIMAL := 0;
    v_distance_score DECIMAL := 0;
    v_competition_score DECIMAL := 0;
    v_final_score DECIMAL;
BEGIN
    -- Resource richness score (0-40 points)
    SELECT 
        COALESCE(AVG(metal_richness + crystal_richness + deuterium_richness) * 10, 0)
    INTO v_resource_score
    FROM planet_resources
    WHERE galaxy = p_galaxy AND system = p_system
    LIMIT 5;
    
    -- Distance from center score (0-30 points)
    v_distance_score := 30 - (ABS(p_system - 250) / 250.0 * 30);
    
    -- Competition score (0-30 points) - less competition = higher score
    SELECT 
        30 - (COUNT(*) * 3)
    INTO v_competition_score
    FROM player_placements
    WHERE universe_id = p_universe_id
      AND galaxy = p_galaxy
      AND ABS(system - p_system) < 50;
    
    v_competition_score := GREATEST(0, v_competition_score);
    
    -- Calculate final score
    v_final_score := v_resource_score + v_distance_score + v_competition_score;
    
    RETURN LEAST(100, v_final_score);
END;
$$ LANGUAGE plpgsql;

-- Function: Get next bot name
CREATE OR REPLACE FUNCTION get_next_bot_name(p_personality VARCHAR) RETURNS VARCHAR AS $$
DECLARE
    v_name_prefix VARCHAR;
    v_name_suffix INTEGER;
    v_full_name VARCHAR;
BEGIN
    v_name_prefix := CASE p_personality
        WHEN 'aggressive' THEN 'Warmaster'
        WHEN 'defensive' THEN 'Guardian'
        WHEN 'economic' THEN 'Trader'
        WHEN 'explorer' THEN 'Navigator'
        WHEN 'researcher' THEN 'Scientist'
        WHEN 'diplomatic' THEN 'Ambassador'
        WHEN 'opportunist' THEN 'Opportunist'
        ELSE 'Commander'
    END;
    
    SELECT COUNT(*) + 1 INTO v_name_suffix
    FROM generated_bots
    WHERE bot_personality = p_personality;
    
    v_full_name := v_name_prefix || '-' || v_name_suffix;
    
    RETURN v_full_name;
END;
$$ LANGUAGE plpgsql;

-- =====================================================
-- TRIGGERS
-- =====================================================

-- Auto-update timestamps
CREATE OR REPLACE FUNCTION update_universe_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER universe_seeds_updated_at
    BEFORE UPDATE ON universe_seeds
    FOR EACH ROW EXECUTE FUNCTION update_universe_timestamp();

CREATE TRIGGER galaxy_seeds_updated_at
    BEFORE UPDATE ON galaxy_seeds
    FOR EACH ROW EXECUTE FUNCTION update_universe_timestamp();

CREATE TRIGGER bot_templates_updated_at
    BEFORE UPDATE ON bot_generation_templates
    FOR EACH ROW EXECUTE FUNCTION update_universe_timestamp();

CREATE TRIGGER alliance_seeds_updated_at
    BEFORE UPDATE ON alliance_seeds
    FOR EACH ROW EXECUTE FUNCTION update_universe_timestamp();

-- =====================================================
-- INITIAL DATA
-- =====================================================

-- Insert default universe seed
INSERT INTO universe_seeds (
    universe_name,
    universe_type,
    galaxy_count,
    systems_per_galaxy,
    positions_per_system,
    max_players,
    bot_percentage,
    resource_multiplier,
    starting_resources_metal,
    starting_resources_crystal,
    starting_resources_deuterium,
    difficulty_curve,
    beginner_protection_days
) VALUES (
    'Universus Alpha',
    'balanced',
    9,
    499,
    15,
    10000,
    30.00,
    1.00,
    500,
    300,
    100,
    'progressive',
    7
) ON CONFLICT (universe_name) DO NOTHING;

-- =====================================================
-- COMPLETION NOTES
-- =====================================================
-- This schema provides:
-- ✓ Complete universe seeding configuration
-- ✓ Galaxy and sector-based organization
-- ✓ Player placement with strategic algorithms
-- ✓ Bot generation with personality templates
-- ✓ Resource distribution patterns
-- ✓ Alliance seeding system
-- ✓ Automated maintenance tasks
-- ✓ Comprehensive analytics tracking
-- ✓ Views for quick insights
-- ✓ Helper functions for quality scoring
-- =====================================================
