-- Migration: Add comprehensive bot system
-- Description: Enables AI-controlled players with configurable personalities and advanced decision-making

-- Bot profiles table - stores bot personality and configuration
CREATE TABLE IF NOT EXISTS bot_profiles (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
    personality_type VARCHAR(50) NOT NULL,
    is_active BOOLEAN DEFAULT true,
    difficulty_level INTEGER DEFAULT 5,
    
    -- Behavior configuration (0-100 scale for each parameter)
    aggression_level INTEGER DEFAULT 50,
    expansion_priority INTEGER DEFAULT 50,
    military_focus INTEGER DEFAULT 50,
    economy_focus INTEGER DEFAULT 50,
    research_focus INTEGER DEFAULT 50,
    diplomacy_focus INTEGER DEFAULT 50,
    risk_tolerance INTEGER DEFAULT 50,
    
    -- Strategy configuration
    preferred_ship_type VARCHAR(50),
    attack_frequency_hours DECIMAL(4,2) DEFAULT 24.0,
    resource_threshold_attack INTEGER DEFAULT 100000,
    fleet_size_preference VARCHAR(20) DEFAULT 'medium',
    alliance_behavior VARCHAR(20) DEFAULT 'neutral',
    
    -- Performance metrics
    total_attacks_launched INTEGER DEFAULT 0,
    total_resources_plundered BIGINT DEFAULT 0,
    total_ships_built INTEGER DEFAULT 0,
    total_research_completed INTEGER DEFAULT 0,
    win_rate DECIMAL(5,2) DEFAULT 0.00,
    
    -- AI state
    last_action_at TIMESTAMP WITH TIME ZONE,
    next_think_at TIMESTAMP WITH TIME ZONE,
    think_interval_minutes INTEGER DEFAULT 15,
    current_strategy JSONB,
    
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    
    CONSTRAINT valid_personality CHECK (personality_type IN (
        'aggressive_conqueror',
        'strategic_builder',
        'diplomatic_negotiator',
        'resource_hoarder',
        'speed_rusher',
        'tech_enthusiast',
        'alliance_focused',
        'solo_survivor'
    )),
    CONSTRAINT valid_difficulty CHECK (difficulty_level BETWEEN 1 AND 10),
    CONSTRAINT valid_behavior_params CHECK (
        aggression_level BETWEEN 0 AND 100 AND
        expansion_priority BETWEEN 0 AND 100 AND
        military_focus BETWEEN 0 AND 100 AND
        economy_focus BETWEEN 0 AND 100 AND
        research_focus BETWEEN 0 AND 100 AND
        diplomacy_focus BETWEEN 0 AND 100 AND
        risk_tolerance BETWEEN 0 AND 100
    )
);

-- Bot actions log - tracks all bot decisions and actions
CREATE TABLE IF NOT EXISTS bot_actions_log (
    id SERIAL PRIMARY KEY,
    bot_id INTEGER NOT NULL REFERENCES bot_profiles(id) ON DELETE CASCADE,
    action_type VARCHAR(50) NOT NULL,
    action_details JSONB NOT NULL,
    decision_factors JSONB,
    success BOOLEAN,
    resources_spent JSONB,
    resources_gained JSONB,
    execution_time_ms INTEGER,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    
    CONSTRAINT valid_action_type CHECK (action_type IN (
        'build_structure',
        'build_ships',
        'research_technology',
        'launch_attack',
        'launch_transport',
        'launch_colonization',
        'launch_espionage',
        'join_alliance',
        'leave_alliance',
        'send_message',
        'claim_planet',
        'upgrade_building',
        'cancel_build'
    ))
);

-- Bot performance stats - aggregated metrics for analytics
CREATE TABLE IF NOT EXISTS bot_stats (
    id SERIAL PRIMARY KEY,
    bot_id INTEGER NOT NULL REFERENCES bot_profiles(id) ON DELETE CASCADE,
    stat_date DATE NOT NULL,
    
    -- Activity metrics
    actions_taken INTEGER DEFAULT 0,
    decisions_made INTEGER DEFAULT 0,
    think_cycles_completed INTEGER DEFAULT 0,
    
    -- Economic metrics
    metal_produced BIGINT DEFAULT 0,
    crystal_produced BIGINT DEFAULT 0,
    deuterium_produced BIGINT DEFAULT 0,
    resources_spent BIGINT DEFAULT 0,
    
    -- Military metrics
    ships_built INTEGER DEFAULT 0,
    fleets_sent INTEGER DEFAULT 0,
    attacks_won INTEGER DEFAULT 0,
    attacks_lost INTEGER DEFAULT 0,
    defenses_won INTEGER DEFAULT 0,
    defenses_lost INTEGER DEFAULT 0,
    
    -- Development metrics
    buildings_upgraded INTEGER DEFAULT 0,
    research_completed INTEGER DEFAULT 0,
    planets_claimed INTEGER DEFAULT 0,
    
    -- Alliance metrics
    alliance_interactions INTEGER DEFAULT 0,
    messages_sent INTEGER DEFAULT 0,
    
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    
    UNIQUE(bot_id, stat_date)
);

-- Bot decision queue - stores pending bot decisions for async processing
CREATE TABLE IF NOT EXISTS bot_decision_queue (
    id SERIAL PRIMARY KEY,
    bot_id INTEGER NOT NULL REFERENCES bot_profiles(id) ON DELETE CASCADE,
    decision_type VARCHAR(50) NOT NULL,
    priority INTEGER DEFAULT 5,
    context_data JSONB,
    scheduled_for TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    processed_at TIMESTAMP WITH TIME ZONE,
    result JSONB,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    
    CONSTRAINT valid_priority CHECK (priority BETWEEN 1 AND 10)
);

-- Bot targets - stores information about potential attack targets
CREATE TABLE IF NOT EXISTS bot_targets (
    id SERIAL PRIMARY KEY,
    bot_id INTEGER NOT NULL REFERENCES bot_profiles(id) ON DELETE CASCADE,
    target_user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    target_planet_id INTEGER NOT NULL REFERENCES planets(id) ON DELETE CASCADE,
    
    -- Target evaluation
    threat_level INTEGER DEFAULT 5,
    resource_potential BIGINT DEFAULT 0,
    defense_strength INTEGER DEFAULT 0,
    last_espionage_at TIMESTAMP WITH TIME ZONE,
    espionage_data JSONB,
    
    -- Attack planning
    attack_priority INTEGER DEFAULT 5,
    last_attack_at TIMESTAMP WITH TIME ZONE,
    total_attacks INTEGER DEFAULT 0,
    successful_attacks INTEGER DEFAULT 0,
    
    -- Cooldown management
    next_attack_available_at TIMESTAMP WITH TIME ZONE,
    
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    
    UNIQUE(bot_id, target_planet_id),
    CONSTRAINT valid_threat CHECK (threat_level BETWEEN 1 AND 10),
    CONSTRAINT valid_attack_priority CHECK (attack_priority BETWEEN 1 AND 10)
);

-- Indexes for bot system
CREATE INDEX IF NOT EXISTS idx_bot_profiles_active ON bot_profiles(is_active, next_think_at) WHERE is_active = true;
CREATE INDEX IF NOT EXISTS idx_bot_profiles_personality ON bot_profiles(personality_type);
CREATE INDEX IF NOT EXISTS idx_bot_actions_log_bot ON bot_actions_log(bot_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_bot_actions_log_type ON bot_actions_log(action_type, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_bot_stats_bot_date ON bot_stats(bot_id, stat_date DESC);
CREATE INDEX IF NOT EXISTS idx_bot_decision_queue_pending ON bot_decision_queue(scheduled_for) WHERE processed_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_bot_decision_queue_bot ON bot_decision_queue(bot_id, scheduled_for);
CREATE INDEX IF NOT EXISTS idx_bot_targets_bot ON bot_targets(bot_id, attack_priority DESC);
CREATE INDEX IF NOT EXISTS idx_bot_targets_next_attack ON bot_targets(next_attack_available_at) WHERE next_attack_available_at IS NOT NULL;

-- Function to update bot_profiles updated_at timestamp
CREATE OR REPLACE FUNCTION update_bot_profiles_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger for bot_profiles
DROP TRIGGER IF EXISTS update_bot_profiles_updated_at_trigger ON bot_profiles;
CREATE TRIGGER update_bot_profiles_updated_at_trigger
    BEFORE UPDATE ON bot_profiles
    FOR EACH ROW
    EXECUTE FUNCTION update_bot_profiles_updated_at();

-- Function to update bot_targets updated_at timestamp
CREATE OR REPLACE FUNCTION update_bot_targets_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger for bot_targets
DROP TRIGGER IF NOT EXISTS update_bot_targets_updated_at_trigger ON bot_targets;
CREATE TRIGGER update_bot_targets_updated_at_trigger
    BEFORE UPDATE ON bot_targets
    FOR EACH ROW
    EXECUTE FUNCTION update_bot_targets_updated_at();

-- View for bot leaderboard
CREATE OR REPLACE VIEW bot_leaderboard AS
SELECT 
    u.id as user_id,
    u.username,
    bp.personality_type,
    bp.difficulty_level,
    bp.total_attacks_launched,
    bp.total_resources_plundered,
    bp.win_rate,
    COUNT(DISTINCT p.id) as total_planets,
    COALESCE(SUM(p.metal + p.crystal + p.deuterium), 0) as total_resources,
    (SELECT COUNT(*) FROM bot_actions_log WHERE bot_id = bp.id AND success = true) as successful_actions,
    bp.last_action_at,
    bp.is_active
FROM bot_profiles bp
JOIN users u ON bp.user_id = u.id
LEFT JOIN planets p ON u.id = p.user_id
GROUP BY u.id, u.username, bp.id, bp.personality_type, bp.difficulty_level, 
         bp.total_attacks_launched, bp.total_resources_plundered, bp.win_rate,
         bp.last_action_at, bp.is_active;

-- Comments on tables
COMMENT ON TABLE bot_profiles IS 'AI-controlled player profiles with personality and behavior configuration';
COMMENT ON TABLE bot_actions_log IS 'Complete audit trail of all bot decisions and actions';
COMMENT ON TABLE bot_stats IS 'Daily aggregated performance metrics for bot analytics';
COMMENT ON TABLE bot_decision_queue IS 'Async processing queue for bot decision-making';
COMMENT ON TABLE bot_targets IS 'Bot target tracking and attack planning data';

COMMENT ON COLUMN bot_profiles.personality_type IS 'Bot personality: aggressive_conqueror, strategic_builder, diplomatic_negotiator, resource_hoarder, speed_rusher, tech_enthusiast, alliance_focused, solo_survivor';
COMMENT ON COLUMN bot_profiles.difficulty_level IS 'Bot skill level from 1 (easy) to 10 (expert)';
COMMENT ON COLUMN bot_profiles.think_interval_minutes IS 'How often the bot AI makes decisions (in minutes)';
COMMENT ON COLUMN bot_profiles.current_strategy IS 'JSON object containing current strategic priorities and goals';
