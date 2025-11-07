-- Phase 11: Enhanced Alliance Management System
-- Comprehensive alliance system with wars, diplomacy, hierarchies, and territory control
-- Created: 2025-11-06

-- ============================================================================
-- CORE ALLIANCE TABLES
-- ============================================================================

-- Alliance Ranks
CREATE TYPE alliance_rank AS ENUM (
    'founder',
    'leader',
    'officer',
    'member',
    'recruit'
);

-- Alliance Permissions
CREATE TYPE alliance_permission AS ENUM (
    'manage_members',
    'manage_ranks',
    'declare_war',
    'manage_diplomacy',
    'manage_resources',
    'send_announcements',
    'view_treasury',
    'withdraw_resources',
    'manage_territory',
    'kick_members'
);

-- Alliances table
CREATE TABLE alliances (
    id SERIAL PRIMARY KEY,
    tag VARCHAR(6) UNIQUE NOT NULL,
    name VARCHAR(100) UNIQUE NOT NULL,
    description TEXT,
    founder_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
    logo_url TEXT,
    banner_url TEXT,
    color_primary VARCHAR(7) DEFAULT '#00ff41',
    color_secondary VARCHAR(7) DEFAULT '#008f11',
    
    -- Settings
    is_open BOOLEAN DEFAULT false,
    is_recruiting BOOLEAN DEFAULT true,
    min_score_requirement INTEGER DEFAULT 0,
    
    -- Statistics
    total_members INTEGER DEFAULT 1,
    total_score BIGINT DEFAULT 0,
    total_planets INTEGER DEFAULT 0,
    total_fleets INTEGER DEFAULT 0,
    
    -- Treasury
    metal_treasury BIGINT DEFAULT 0,
    crystal_treasury BIGINT DEFAULT 0,
    deuterium_treasury BIGINT DEFAULT 0,
    
    -- Metadata
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    disbanded_at TIMESTAMP
);

-- Alliance Members
CREATE TABLE alliance_members (
    id SERIAL PRIMARY KEY,
    alliance_id INTEGER NOT NULL REFERENCES alliances(id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    rank alliance_rank NOT NULL DEFAULT 'recruit',
    
    -- Contributions
    metal_contributed BIGINT DEFAULT 0,
    crystal_contributed BIGINT DEFAULT 0,
    deuterium_contributed BIGINT DEFAULT 0,
    wars_participated INTEGER DEFAULT 0,
    battles_won INTEGER DEFAULT 0,
    
    -- Metadata
    joined_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    promoted_at TIMESTAMP,
    last_contribution_at TIMESTAMP,
    
    UNIQUE(alliance_id, user_id)
);

-- Alliance Rank Permissions
CREATE TABLE alliance_rank_permissions (
    id SERIAL PRIMARY KEY,
    alliance_id INTEGER NOT NULL REFERENCES alliances(id) ON DELETE CASCADE,
    rank alliance_rank NOT NULL,
    permission alliance_permission NOT NULL,
    granted BOOLEAN DEFAULT true,
    
    UNIQUE(alliance_id, rank, permission)
);

-- Alliance Applications
CREATE TABLE alliance_applications (
    id SERIAL PRIMARY KEY,
    alliance_id INTEGER NOT NULL REFERENCES alliances(id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    message TEXT,
    status VARCHAR(20) DEFAULT 'pending', -- pending, accepted, rejected
    
    reviewed_by INTEGER REFERENCES users(id),
    reviewed_at TIMESTAMP,
    
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    UNIQUE(alliance_id, user_id, status)
);

-- ============================================================================
-- ALLIANCE WARS SYSTEM
-- ============================================================================

CREATE TYPE war_status AS ENUM (
    'declared',
    'active',
    'ceasefire',
    'ended'
);

-- Alliance Wars
CREATE TABLE alliance_wars (
    id SERIAL PRIMARY KEY,
    attacker_alliance_id INTEGER NOT NULL REFERENCES alliances(id) ON DELETE CASCADE,
    defender_alliance_id INTEGER NOT NULL REFERENCES alliances(id) ON DELETE CASCADE,
    
    declaration_message TEXT,
    status war_status DEFAULT 'declared',
    
    -- Victory conditions
    victory_condition VARCHAR(50) DEFAULT 'points', -- points, planets, time_limit
    victory_threshold INTEGER DEFAULT 1000,
    
    -- Statistics
    attacker_score INTEGER DEFAULT 0,
    defender_score INTEGER DEFAULT 0,
    total_battles INTEGER DEFAULT 0,
    
    -- Timestamps
    declared_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    started_at TIMESTAMP,
    ended_at TIMESTAMP,
    
    -- Result
    winner_alliance_id INTEGER REFERENCES alliances(id),
    end_reason VARCHAR(100)
);

-- War Battles
CREATE TABLE war_battles (
    id SERIAL PRIMARY KEY,
    war_id INTEGER NOT NULL REFERENCES alliance_wars(id) ON DELETE CASCADE,
    
    attacker_user_id INTEGER NOT NULL REFERENCES users(id),
    defender_user_id INTEGER NOT NULL REFERENCES users(id),
    
    attacker_alliance_id INTEGER NOT NULL REFERENCES alliances(id),
    defender_alliance_id INTEGER NOT NULL REFERENCES alliances(id),
    
    -- Battle details
    combat_id INTEGER,
    winner_user_id INTEGER REFERENCES users(id),
    winner_alliance_id INTEGER REFERENCES alliances(id),
    
    -- Points awarded
    attacker_points INTEGER DEFAULT 0,
    defender_points INTEGER DEFAULT 0,
    
    -- Loot and losses
    attacker_losses BIGINT DEFAULT 0,
    defender_losses BIGINT DEFAULT 0,
    loot_metal BIGINT DEFAULT 0,
    loot_crystal BIGINT DEFAULT 0,
    loot_deuterium BIGINT DEFAULT 0,
    
    battle_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- War Participants
CREATE TABLE war_participants (
    id SERIAL PRIMARY KEY,
    war_id INTEGER NOT NULL REFERENCES alliance_wars(id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    alliance_id INTEGER NOT NULL REFERENCES alliances(id) ON DELETE CASCADE,
    
    battles_fought INTEGER DEFAULT 0,
    battles_won INTEGER DEFAULT 0,
    total_points INTEGER DEFAULT 0,
    total_damage BIGINT DEFAULT 0,
    
    UNIQUE(war_id, user_id)
);

-- ============================================================================
-- DIPLOMATIC RELATIONS
-- ============================================================================

CREATE TYPE diplomatic_status AS ENUM (
    'neutral',
    'nap', -- non-aggression pact
    'alliance', -- allied
    'trade',
    'defense_pact',
    'war',
    'hostile'
);

-- Diplomatic Relations
CREATE TABLE diplomatic_relations (
    id SERIAL PRIMARY KEY,
    alliance_id INTEGER NOT NULL REFERENCES alliances(id) ON DELETE CASCADE,
    target_alliance_id INTEGER NOT NULL REFERENCES alliances(id) ON DELETE CASCADE,
    
    status diplomatic_status DEFAULT 'neutral',
    
    -- Treaty details
    treaty_terms TEXT,
    treaty_duration_days INTEGER,
    
    -- Metadata
    proposed_by INTEGER REFERENCES users(id),
    approved_by INTEGER REFERENCES users(id),
    
    established_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMP,
    terminated_at TIMESTAMP,
    
    UNIQUE(alliance_id, target_alliance_id)
);

-- Diplomatic Proposals
CREATE TABLE diplomatic_proposals (
    id SERIAL PRIMARY KEY,
    from_alliance_id INTEGER NOT NULL REFERENCES alliances(id) ON DELETE CASCADE,
    to_alliance_id INTEGER NOT NULL REFERENCES alliances(id) ON DELETE CASCADE,
    
    proposed_status diplomatic_status NOT NULL,
    terms TEXT,
    duration_days INTEGER,
    
    status VARCHAR(20) DEFAULT 'pending', -- pending, accepted, rejected, expired
    
    proposed_by INTEGER NOT NULL REFERENCES users(id),
    reviewed_by INTEGER REFERENCES users(id),
    
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    reviewed_at TIMESTAMP,
    expires_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP + INTERVAL '7 days'
);

-- ============================================================================
-- ALLIANCE RESOURCES & CONTRIBUTIONS
-- ============================================================================

-- Alliance Resource Contributions
CREATE TABLE alliance_contributions (
    id SERIAL PRIMARY KEY,
    alliance_id INTEGER NOT NULL REFERENCES alliances(id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    
    contribution_type VARCHAR(50) NOT NULL, -- metal, crystal, deuterium, research
    amount BIGINT NOT NULL,
    
    contributed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Alliance Research
CREATE TABLE alliance_research (
    id SERIAL PRIMARY KEY,
    alliance_id INTEGER NOT NULL REFERENCES alliances(id) ON DELETE CASCADE,
    
    research_name VARCHAR(100) NOT NULL,
    level INTEGER DEFAULT 0,
    
    -- Cost and benefits
    total_cost_metal BIGINT,
    total_cost_crystal BIGINT,
    total_cost_deuterium BIGINT,
    
    bonus_description TEXT,
    
    started_at TIMESTAMP,
    completed_at TIMESTAMP,
    
    UNIQUE(alliance_id, research_name)
);

-- ============================================================================
-- ALLIANCE TERRITORIES
-- ============================================================================

-- Alliance Territories
CREATE TABLE alliance_territories (
    id SERIAL PRIMARY KEY,
    alliance_id INTEGER NOT NULL REFERENCES alliances(id) ON DELETE CASCADE,
    
    galaxy INTEGER NOT NULL,
    system INTEGER NOT NULL,
    
    control_percentage DECIMAL(5,2) DEFAULT 0.00, -- 0.00 to 100.00
    planets_controlled INTEGER DEFAULT 0,
    total_planets INTEGER DEFAULT 0,
    
    claimed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    UNIQUE(alliance_id, galaxy, system)
);

-- Territory Control Log
CREATE TABLE territory_control_log (
    id SERIAL PRIMARY KEY,
    alliance_id INTEGER REFERENCES alliances(id) ON DELETE SET NULL,
    
    galaxy INTEGER NOT NULL,
    system INTEGER NOT NULL,
    
    action VARCHAR(50) NOT NULL, -- claimed, lost, contested
    control_change DECIMAL(5,2),
    
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- ============================================================================
-- ALLIANCE COMMUNICATIONS
-- ============================================================================

-- Alliance Messages
CREATE TABLE alliance_messages (
    id SERIAL PRIMARY KEY,
    alliance_id INTEGER NOT NULL REFERENCES alliances(id) ON DELETE CASCADE,
    sender_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    
    message_type VARCHAR(50) DEFAULT 'general', -- general, announcement, war_update, diplomacy
    subject VARCHAR(200),
    content TEXT NOT NULL,
    
    is_pinned BOOLEAN DEFAULT false,
    min_rank alliance_rank, -- minimum rank to view (NULL = all members)
    
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Alliance Message Reactions
CREATE TABLE alliance_message_reactions (
    id SERIAL PRIMARY KEY,
    message_id INTEGER NOT NULL REFERENCES alliance_messages(id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    
    reaction_type VARCHAR(20) NOT NULL, -- like, important, noted
    
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    UNIQUE(message_id, user_id)
);

-- ============================================================================
-- ALLIANCE EVENTS & COMPETITIONS
-- ============================================================================

-- Alliance Events
CREATE TABLE alliance_events (
    id SERIAL PRIMARY KEY,
    
    event_type VARCHAR(50) NOT NULL, -- competition, raid, defense, tournament
    event_name VARCHAR(200) NOT NULL,
    description TEXT,
    
    -- Participants
    participating_alliance_ids INTEGER[], -- array of alliance IDs
    
    -- Objectives
    objective_type VARCHAR(50), -- score, resources, planets, battles
    objective_target BIGINT,
    
    -- Rewards
    reward_metal BIGINT DEFAULT 0,
    reward_crystal BIGINT DEFAULT 0,
    reward_deuterium BIGINT DEFAULT 0,
    reward_description TEXT,
    
    -- Status
    status VARCHAR(20) DEFAULT 'upcoming', -- upcoming, active, completed, cancelled
    
    start_at TIMESTAMP NOT NULL,
    end_at TIMESTAMP NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    winner_alliance_id INTEGER REFERENCES alliances(id)
);

-- Alliance Event Participation
CREATE TABLE alliance_event_participation (
    id SERIAL PRIMARY KEY,
    event_id INTEGER NOT NULL REFERENCES alliance_events(id) ON DELETE CASCADE,
    alliance_id INTEGER NOT NULL REFERENCES alliances(id) ON DELETE CASCADE,
    
    score BIGINT DEFAULT 0,
    rank INTEGER,
    
    rewards_claimed BOOLEAN DEFAULT false,
    
    UNIQUE(event_id, alliance_id)
);

-- ============================================================================
-- ALLIANCE ACHIEVEMENTS & HISTORY
-- ============================================================================

-- Alliance Achievements
CREATE TABLE alliance_achievements (
    id SERIAL PRIMARY KEY,
    alliance_id INTEGER NOT NULL REFERENCES alliances(id) ON DELETE CASCADE,
    
    achievement_type VARCHAR(100) NOT NULL,
    achievement_name VARCHAR(200) NOT NULL,
    description TEXT,
    
    achieved_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Alliance History
CREATE TABLE alliance_history (
    id SERIAL PRIMARY KEY,
    alliance_id INTEGER NOT NULL REFERENCES alliances(id) ON DELETE CASCADE,
    
    event_type VARCHAR(100) NOT NULL, -- founded, war_won, war_lost, member_joined, rank_change, etc.
    description TEXT NOT NULL,
    
    related_user_id INTEGER REFERENCES users(id),
    related_alliance_id INTEGER REFERENCES alliances(id),
    
    metadata JSONB,
    
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- ============================================================================
-- INDEXES FOR PERFORMANCE
-- ============================================================================

-- Alliance indexes
CREATE INDEX idx_alliances_tag ON alliances(tag);
CREATE INDEX idx_alliances_founder ON alliances(founder_id);
CREATE INDEX idx_alliances_recruiting ON alliances(is_recruiting);

-- Alliance members indexes
CREATE INDEX idx_alliance_members_alliance ON alliance_members(alliance_id);
CREATE INDEX idx_alliance_members_user ON alliance_members(user_id);
CREATE INDEX idx_alliance_members_rank ON alliance_members(rank);

-- Alliance wars indexes
CREATE INDEX idx_alliance_wars_attacker ON alliance_wars(attacker_alliance_id);
CREATE INDEX idx_alliance_wars_defender ON alliance_wars(defender_alliance_id);
CREATE INDEX idx_alliance_wars_status ON alliance_wars(status);

-- War battles indexes
CREATE INDEX idx_war_battles_war ON war_battles(war_id);
CREATE INDEX idx_war_battles_attacker ON war_battles(attacker_user_id);
CREATE INDEX idx_war_battles_defender ON war_battles(defender_user_id);

-- Diplomatic relations indexes
CREATE INDEX idx_diplomatic_relations_alliance ON diplomatic_relations(alliance_id);
CREATE INDEX idx_diplomatic_relations_target ON diplomatic_relations(target_alliance_id);
CREATE INDEX idx_diplomatic_relations_status ON diplomatic_relations(status);

-- Territory indexes
CREATE INDEX idx_alliance_territories_alliance ON alliance_territories(alliance_id);
CREATE INDEX idx_alliance_territories_location ON alliance_territories(galaxy, system);

-- Messages indexes
CREATE INDEX idx_alliance_messages_alliance ON alliance_messages(alliance_id);
CREATE INDEX idx_alliance_messages_sender ON alliance_messages(sender_id);
CREATE INDEX idx_alliance_messages_type ON alliance_messages(message_type);

-- ============================================================================
-- VIEWS FOR ANALYTICS
-- ============================================================================

-- Alliance Leaderboard View
CREATE OR REPLACE VIEW v_alliance_leaderboard AS
SELECT 
    a.id,
    a.tag,
    a.name,
    a.total_members,
    a.total_score,
    a.total_planets,
    a.total_fleets,
    COUNT(DISTINCT aw1.id) FILTER (WHERE aw1.winner_alliance_id = a.id) as wars_won,
    COUNT(DISTINCT aw2.id) FILTER (WHERE aw2.status = 'active' AND (aw2.attacker_alliance_id = a.id OR aw2.defender_alliance_id = a.id)) as active_wars,
    RANK() OVER (ORDER BY a.total_score DESC) as rank
FROM alliances a
LEFT JOIN alliance_wars aw1 ON aw1.winner_alliance_id = a.id
LEFT JOIN alliance_wars aw2 ON (aw2.attacker_alliance_id = a.id OR aw2.defender_alliance_id = a.id)
WHERE a.disbanded_at IS NULL
GROUP BY a.id
ORDER BY a.total_score DESC;

-- Alliance Member Activity View
CREATE OR REPLACE VIEW v_alliance_member_activity AS
SELECT 
    am.alliance_id,
    am.user_id,
    u.username,
    am.rank,
    am.metal_contributed + am.crystal_contributed + am.deuterium_contributed as total_contributed,
    am.wars_participated,
    am.battles_won,
    am.joined_at,
    EXTRACT(days FROM CURRENT_TIMESTAMP - am.joined_at) as days_in_alliance
FROM alliance_members am
JOIN users u ON u.id = am.user_id;

-- Active Wars Summary View
CREATE OR REPLACE VIEW v_active_wars_summary AS
SELECT 
    aw.id,
    aw.status,
    a1.tag as attacker_tag,
    a1.name as attacker_name,
    a2.tag as defender_tag,
    a2.name as defender_name,
    aw.attacker_score,
    aw.defender_score,
    aw.total_battles,
    aw.declared_at,
    aw.started_at,
    CASE 
        WHEN aw.attacker_score > aw.defender_score THEN a1.name
        WHEN aw.defender_score > aw.attacker_score THEN a2.name
        ELSE 'Tied'
    END as current_leader
FROM alliance_wars aw
JOIN alliances a1 ON a1.id = aw.attacker_alliance_id
JOIN alliances a2 ON a2.id = aw.defender_alliance_id
WHERE aw.status IN ('declared', 'active');

-- ============================================================================
-- FUNCTIONS
-- ============================================================================

-- Update alliance statistics
CREATE OR REPLACE FUNCTION update_alliance_stats(p_alliance_id INTEGER)
RETURNS VOID AS $$
BEGIN
    UPDATE alliances
    SET 
        total_members = (
            SELECT COUNT(*) FROM alliance_members 
            WHERE alliance_id = p_alliance_id
        ),
        total_score = (
            SELECT COALESCE(SUM(u.score), 0) 
            FROM alliance_members am
            JOIN users u ON u.id = am.user_id
            WHERE am.alliance_id = p_alliance_id
        ),
        total_planets = (
            SELECT COUNT(*) 
            FROM planets p
            JOIN alliance_members am ON am.user_id = p.user_id
            WHERE am.alliance_id = p_alliance_id
        ),
        updated_at = CURRENT_TIMESTAMP
    WHERE id = p_alliance_id;
END;
$$ LANGUAGE plpgsql;

-- Check alliance permission
CREATE OR REPLACE FUNCTION check_alliance_permission(
    p_alliance_id INTEGER,
    p_user_id INTEGER,
    p_permission alliance_permission
) RETURNS BOOLEAN AS $$
DECLARE
    v_rank alliance_rank;
    v_has_permission BOOLEAN;
BEGIN
    -- Get user's rank
    SELECT rank INTO v_rank
    FROM alliance_members
    WHERE alliance_id = p_alliance_id AND user_id = p_user_id;
    
    IF v_rank IS NULL THEN
        RETURN FALSE;
    END IF;
    
    -- Founder and leaders have all permissions
    IF v_rank IN ('founder', 'leader') THEN
        RETURN TRUE;
    END IF;
    
    -- Check specific permission
    SELECT COALESCE(granted, FALSE) INTO v_has_permission
    FROM alliance_rank_permissions
    WHERE alliance_id = p_alliance_id 
    AND rank = v_rank 
    AND permission = p_permission;
    
    RETURN COALESCE(v_has_permission, FALSE);
END;
$$ LANGUAGE plpgsql;

-- Calculate war score for battle
CREATE OR REPLACE FUNCTION calculate_war_points(
    p_winner_losses BIGINT,
    p_loser_losses BIGINT
) RETURNS INTEGER AS $$
DECLARE
    v_points INTEGER;
BEGIN
    -- Points based on total losses (100 points per 1M resources destroyed)
    v_points := FLOOR((p_winner_losses + p_loser_losses) / 10000);
    
    -- Bonus points for decisive victory (loser lost 3x more)
    IF p_loser_losses > p_winner_losses * 3 THEN
        v_points := v_points + 50;
    END IF;
    
    RETURN GREATEST(v_points, 10); -- Minimum 10 points
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- TRIGGERS
-- ============================================================================

-- Update alliance stats when member joins/leaves
CREATE OR REPLACE FUNCTION trigger_update_alliance_stats()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' OR TG_OP = 'UPDATE' THEN
        PERFORM update_alliance_stats(NEW.alliance_id);
    END IF;
    IF TG_OP = 'DELETE' THEN
        PERFORM update_alliance_stats(OLD.alliance_id);
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER alliance_member_stats_update
AFTER INSERT OR UPDATE OR DELETE ON alliance_members
FOR EACH ROW
EXECUTE FUNCTION trigger_update_alliance_stats();

-- Log alliance history
CREATE OR REPLACE FUNCTION trigger_log_alliance_history()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        INSERT INTO alliance_history (alliance_id, event_type, description, related_user_id)
        VALUES (NEW.alliance_id, 'member_joined', 
                'New member joined the alliance', NEW.user_id);
    ELSIF TG_OP = 'UPDATE' AND NEW.rank != OLD.rank THEN
        INSERT INTO alliance_history (alliance_id, event_type, description, related_user_id)
        VALUES (NEW.alliance_id, 'rank_changed', 
                'Member rank changed from ' || OLD.rank || ' to ' || NEW.rank, NEW.user_id);
    ELSIF TG_OP = 'DELETE' THEN
        INSERT INTO alliance_history (alliance_id, event_type, description, related_user_id)
        VALUES (OLD.alliance_id, 'member_left', 
                'Member left the alliance', OLD.user_id);
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER alliance_history_log
AFTER INSERT OR UPDATE OR DELETE ON alliance_members
FOR EACH ROW
EXECUTE FUNCTION trigger_log_alliance_history();

-- ============================================================================
-- SEED DATA
-- ============================================================================

-- Insert default permissions for each rank
INSERT INTO alliance_rank_permissions (alliance_id, rank, permission, granted)
SELECT a.id, 'officer', perm.permission, true
FROM alliances a
CROSS JOIN (
    VALUES 
    ('manage_members'::alliance_permission),
    ('send_announcements'::alliance_permission),
    ('view_treasury'::alliance_permission),
    ('manage_territory'::alliance_permission)
) perm(permission)
ON CONFLICT DO NOTHING;

-- Note: Founder and Leader permissions are handled in the function check_alliance_permission()
-- They automatically have all permissions

COMMENT ON TABLE alliances IS 'Core alliance information and statistics';
COMMENT ON TABLE alliance_wars IS 'Alliance vs alliance war declarations and tracking';
COMMENT ON TABLE diplomatic_relations IS 'Diplomatic relations between alliances';
COMMENT ON TABLE alliance_territories IS 'Alliance-controlled sectors and territories';
COMMENT ON TABLE alliance_messages IS 'Internal alliance communication system';
