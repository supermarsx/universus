-- =====================================================
-- PHASE 6: REAL-TIME COMMUNICATION SYSTEMS
-- =====================================================
-- Database schema for comprehensive real-time features
-- including chat, notifications, player status, and events
-- =====================================================

-- =====================================================
-- 1. CHAT SYSTEM TABLES
-- =====================================================

-- Chat Channels (General, Alliance, Sector, Trade, Help)
CREATE TABLE IF NOT EXISTS chat_channels (
    id SERIAL PRIMARY KEY,
    channel_name VARCHAR(50) UNIQUE NOT NULL,
    channel_type VARCHAR(20) NOT NULL CHECK (channel_type IN ('global', 'sector', 'alliance', 'private', 'trade', 'help', 'combat')),
    description TEXT,
    is_active BOOLEAN DEFAULT TRUE,
    max_message_length INTEGER DEFAULT 500,
    rate_limit_seconds INTEGER DEFAULT 3,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT chat_channels_name_check CHECK (char_length(channel_name) >= 2)
);

-- Chat Messages with rate limiting and moderation
CREATE TABLE IF NOT EXISTS chat_messages (
    id SERIAL PRIMARY KEY,
    channel_id INTEGER NOT NULL REFERENCES chat_channels(id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    message TEXT NOT NULL,
    message_type VARCHAR(20) DEFAULT 'text' CHECK (message_type IN ('text', 'system', 'combat', 'trade', 'fleet')),
    
    -- Metadata
    is_edited BOOLEAN DEFAULT FALSE,
    edited_at TIMESTAMP,
    is_deleted BOOLEAN DEFAULT FALSE,
    deleted_at TIMESTAMP,
    
    -- Moderation
    is_flagged BOOLEAN DEFAULT FALSE,
    flag_reason TEXT,
    flagged_by INTEGER REFERENCES users(id),
    flagged_at TIMESTAMP,
    
    -- References (for combat/trade/fleet messages)
    reference_type VARCHAR(20),
    reference_id INTEGER,
    
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT chat_messages_length CHECK (char_length(message) >= 1 AND char_length(message) <= 2000)
);

-- Private Chat Conversations
CREATE TABLE IF NOT EXISTS private_conversations (
    id SERIAL PRIMARY KEY,
    user1_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    user2_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    last_message_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    user1_unread_count INTEGER DEFAULT 0,
    user2_unread_count INTEGER DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(user1_id, user2_id),
    CONSTRAINT private_conversations_different_users CHECK (user1_id < user2_id)
);

-- Private Messages
CREATE TABLE IF NOT EXISTS private_messages (
    id SERIAL PRIMARY KEY,
    conversation_id INTEGER NOT NULL REFERENCES private_conversations(id) ON DELETE CASCADE,
    sender_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    message TEXT NOT NULL,
    is_read BOOLEAN DEFAULT FALSE,
    read_at TIMESTAMP,
    is_deleted_by_sender BOOLEAN DEFAULT FALSE,
    is_deleted_by_receiver BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT private_messages_length CHECK (char_length(message) >= 1 AND char_length(message) <= 2000)
);

-- Chat Bans/Mutes
CREATE TABLE IF NOT EXISTS chat_restrictions (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    channel_id INTEGER REFERENCES chat_channels(id) ON DELETE CASCADE,
    restriction_type VARCHAR(20) NOT NULL CHECK (restriction_type IN ('mute', 'ban', 'slowmode')),
    reason TEXT,
    restricted_by INTEGER NOT NULL REFERENCES users(id),
    expires_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT chat_restrictions_unique UNIQUE(user_id, channel_id, restriction_type)
);

-- =====================================================
-- 2. NOTIFICATION SYSTEM TABLES
-- =====================================================

-- Notification Types Configuration
CREATE TABLE IF NOT EXISTS notification_types (
    id SERIAL PRIMARY KEY,
    type_name VARCHAR(50) UNIQUE NOT NULL,
    category VARCHAR(30) NOT NULL CHECK (category IN ('combat', 'fleet', 'resource', 'alliance', 'trade', 'system', 'achievement')),
    description TEXT,
    default_priority INTEGER DEFAULT 1 CHECK (default_priority BETWEEN 1 AND 5),
    icon VARCHAR(50),
    sound_enabled BOOLEAN DEFAULT TRUE,
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- User Notifications
CREATE TABLE IF NOT EXISTS notifications (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    notification_type_id INTEGER NOT NULL REFERENCES notification_types(id) ON DELETE CASCADE,
    
    -- Notification Content
    title VARCHAR(200) NOT NULL,
    message TEXT NOT NULL,
    priority INTEGER DEFAULT 1 CHECK (priority BETWEEN 1 AND 5),
    
    -- Status
    is_read BOOLEAN DEFAULT FALSE,
    read_at TIMESTAMP,
    is_archived BOOLEAN DEFAULT FALSE,
    archived_at TIMESTAMP,
    
    -- Action Links
    action_url VARCHAR(255),
    action_label VARCHAR(50),
    
    -- References (combat, fleet, trade, etc.)
    reference_type VARCHAR(30),
    reference_id INTEGER,
    
    -- Metadata
    metadata JSONB,
    
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMP
);

-- User Notification Preferences
CREATE TABLE IF NOT EXISTS notification_preferences (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    notification_type_id INTEGER NOT NULL REFERENCES notification_types(id) ON DELETE CASCADE,
    
    -- Delivery Preferences
    enabled BOOLEAN DEFAULT TRUE,
    sound_enabled BOOLEAN DEFAULT TRUE,
    desktop_enabled BOOLEAN DEFAULT TRUE,
    
    -- Filter by priority
    min_priority INTEGER DEFAULT 1,
    
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(user_id, notification_type_id)
);

-- =====================================================
-- 3. REAL-TIME PLAYER STATUS
-- =====================================================

-- Player Online Status and Presence
CREATE TABLE IF NOT EXISTS player_status (
    user_id INTEGER PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    
    -- Status
    status VARCHAR(20) DEFAULT 'offline' CHECK (status IN ('online', 'offline', 'away', 'busy', 'in_combat')),
    status_message VARCHAR(100),
    
    -- Activity
    last_activity TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    last_action VARCHAR(50),
    current_planet_id INTEGER REFERENCES planets(id) ON DELETE SET NULL,
    
    -- Session Info
    session_id VARCHAR(100),
    socket_id VARCHAR(100),
    
    -- Statistics
    session_count INTEGER DEFAULT 0,
    total_online_time INTEGER DEFAULT 0, -- in seconds
    
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Player Activity Log (for tracking real-time actions)
CREATE TABLE IF NOT EXISTS player_activity_log (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    activity_type VARCHAR(50) NOT NULL CHECK (activity_type IN (
        'login', 'logout', 'building_upgrade', 'research_start', 'fleet_dispatch',
        'fleet_return', 'combat', 'trade', 'chat_message', 'resource_collect',
        'alliance_join', 'alliance_leave', 'planet_view', 'galaxy_scan'
    )),
    activity_data JSONB,
    planet_id INTEGER REFERENCES planets(id) ON DELETE SET NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =====================================================
-- 4. REAL-TIME FLEET TRACKING
-- =====================================================

-- Fleet Movement Events (for real-time tracking)
CREATE TABLE IF NOT EXISTS fleet_events (
    id SERIAL PRIMARY KEY,
    fleet_id INTEGER NOT NULL REFERENCES fleets(id) ON DELETE CASCADE,
    event_type VARCHAR(30) NOT NULL CHECK (event_type IN (
        'dispatched', 'moving', 'checkpoint', 'arrived', 'returned',
        'combat_started', 'combat_ended', 'recalled', 'destroyed'
    )),
    event_data JSONB,
    
    -- Location at event time
    current_galaxy INTEGER,
    current_system INTEGER,
    current_position INTEGER,
    
    -- Progress
    progress_percent DECIMAL(5, 2),
    estimated_arrival TIMESTAMP,
    
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Fleet Watchers (users watching specific fleets)
CREATE TABLE IF NOT EXISTS fleet_watchers (
    id SERIAL PRIMARY KEY,
    fleet_id INTEGER NOT NULL REFERENCES fleets(id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    watch_type VARCHAR(20) CHECK (watch_type IN ('owner', 'target', 'alliance', 'spy')),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(fleet_id, user_id)
);

-- =====================================================
-- 5. REAL-TIME COMBAT ALERTS
-- =====================================================

-- Combat Alerts (instant notifications during battles)
CREATE TABLE IF NOT EXISTS combat_alerts (
    id SERIAL PRIMARY KEY,
    combat_id INTEGER NOT NULL,
    alert_type VARCHAR(30) NOT NULL CHECK (alert_type IN (
        'combat_started', 'round_complete', 'combat_ended', 
        'fleet_destroyed', 'defense_destroyed', 'resources_plundered'
    )),
    
    -- Participants
    attacker_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    defender_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    
    -- Alert Data
    alert_data JSONB NOT NULL,
    severity INTEGER DEFAULT 1 CHECK (severity BETWEEN 1 AND 5),
    
    -- Status
    attacker_read BOOLEAN DEFAULT FALSE,
    defender_read BOOLEAN DEFAULT FALSE,
    
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =====================================================
-- 6. REAL-TIME TRADING & COMMERCE
-- =====================================================

-- Trade Offers (for resource trading)
CREATE TABLE IF NOT EXISTS trade_offers (
    id SERIAL PRIMARY KEY,
    seller_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    
    -- Offer Details
    offer_type VARCHAR(20) CHECK (offer_type IN ('sell', 'buy', 'exchange')),
    resource_offered VARCHAR(20) NOT NULL CHECK (resource_offered IN ('metal', 'crystal', 'deuterium')),
    amount_offered BIGINT NOT NULL,
    resource_wanted VARCHAR(20) NOT NULL CHECK (resource_wanted IN ('metal', 'crystal', 'deuterium', 'dark_matter')),
    amount_wanted BIGINT NOT NULL,
    
    -- Exchange Rate
    exchange_rate DECIMAL(10, 4),
    
    -- Status
    status VARCHAR(20) DEFAULT 'active' CHECK (status IN ('active', 'completed', 'cancelled', 'expired')),
    
    -- Restrictions
    min_reputation INTEGER DEFAULT 0,
    alliance_only BOOLEAN DEFAULT FALSE,
    target_alliance_id INTEGER REFERENCES alliances(id),
    
    -- Completion
    buyer_id INTEGER REFERENCES users(id),
    completed_at TIMESTAMP,
    
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMP DEFAULT (CURRENT_TIMESTAMP + INTERVAL '7 days')
);

-- Trade Transactions (completed trades)
CREATE TABLE IF NOT EXISTS trade_transactions (
    id SERIAL PRIMARY KEY,
    trade_offer_id INTEGER REFERENCES trade_offers(id) ON DELETE SET NULL,
    seller_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    buyer_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    
    -- Transaction Details
    resource_given VARCHAR(20) NOT NULL,
    amount_given BIGINT NOT NULL,
    resource_received VARCHAR(20) NOT NULL,
    amount_received BIGINT NOT NULL,
    
    -- Metadata
    transaction_fee BIGINT DEFAULT 0,
    exchange_rate DECIMAL(10, 4),
    
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =====================================================
-- 7. INDEXES FOR PERFORMANCE
-- =====================================================

-- Chat System Indexes
CREATE INDEX IF NOT EXISTS idx_chat_messages_channel ON chat_messages(channel_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_chat_messages_user ON chat_messages(user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_chat_messages_deleted ON chat_messages(is_deleted, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_private_conversations_users ON private_conversations(user1_id, user2_id);
CREATE INDEX IF NOT EXISTS idx_private_messages_conversation ON private_messages(conversation_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_private_messages_unread ON private_messages(conversation_id, is_read);
CREATE INDEX IF NOT EXISTS idx_chat_restrictions_user ON chat_restrictions(user_id, expires_at);

-- Notification Indexes
CREATE INDEX IF NOT EXISTS idx_notifications_user ON notifications(user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_notifications_unread ON notifications(user_id, is_read, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_notifications_type ON notifications(notification_type_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_notifications_reference ON notifications(reference_type, reference_id);
CREATE INDEX IF NOT EXISTS idx_notifications_expires ON notifications(expires_at) WHERE expires_at IS NOT NULL;

-- Player Status Indexes
CREATE INDEX IF NOT EXISTS idx_player_status_online ON player_status(status, last_activity);
CREATE INDEX IF NOT EXISTS idx_player_activity_log_user ON player_activity_log(user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_player_activity_log_type ON player_activity_log(activity_type, created_at DESC);

-- Fleet Tracking Indexes
CREATE INDEX IF NOT EXISTS idx_fleet_events_fleet ON fleet_events(fleet_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_fleet_events_type ON fleet_events(event_type, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_fleet_watchers_fleet ON fleet_watchers(fleet_id);
CREATE INDEX IF NOT EXISTS idx_fleet_watchers_user ON fleet_watchers(user_id);

-- Combat Alert Indexes
CREATE INDEX IF NOT EXISTS idx_combat_alerts_attacker ON combat_alerts(attacker_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_combat_alerts_defender ON combat_alerts(defender_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_combat_alerts_combat ON combat_alerts(combat_id);
CREATE INDEX IF NOT EXISTS idx_combat_alerts_unread ON combat_alerts(attacker_read, defender_read);

-- Trading Indexes
CREATE INDEX IF NOT EXISTS idx_trade_offers_seller ON trade_offers(seller_id, status);
CREATE INDEX IF NOT EXISTS idx_trade_offers_active ON trade_offers(status, created_at DESC) WHERE status = 'active';
CREATE INDEX IF NOT EXISTS idx_trade_offers_expires ON trade_offers(expires_at) WHERE status = 'active';
CREATE INDEX IF NOT EXISTS idx_trade_transactions_seller ON trade_transactions(seller_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_trade_transactions_buyer ON trade_transactions(buyer_id, created_at DESC);

-- =====================================================
-- 8. VIEWS FOR ANALYTICS
-- =====================================================

-- Active Players View
CREATE OR REPLACE VIEW v_active_players AS
SELECT 
    u.id,
    u.username,
    ps.status,
    ps.last_activity,
    ps.status_message,
    ps.current_planet_id,
    EXTRACT(EPOCH FROM (CURRENT_TIMESTAMP - ps.last_activity)) AS seconds_since_activity,
    CASE 
        WHEN ps.status = 'online' AND ps.last_activity > CURRENT_TIMESTAMP - INTERVAL '5 minutes' THEN 'active'
        WHEN ps.status = 'online' AND ps.last_activity > CURRENT_TIMESTAMP - INTERVAL '15 minutes' THEN 'idle'
        ELSE 'offline'
    END AS actual_status
FROM users u
LEFT JOIN player_status ps ON u.id = ps.user_id
WHERE u.is_banned = FALSE;

-- Chat Activity View
CREATE OR REPLACE VIEW v_chat_activity AS
SELECT 
    cc.channel_name,
    cc.channel_type,
    COUNT(cm.id) AS message_count,
    COUNT(DISTINCT cm.user_id) AS unique_users,
    MAX(cm.created_at) AS last_message_at,
    COUNT(cm.id) FILTER (WHERE cm.created_at > CURRENT_TIMESTAMP - INTERVAL '1 hour') AS messages_last_hour
FROM chat_channels cc
LEFT JOIN chat_messages cm ON cc.id = cm.channel_id AND cm.is_deleted = FALSE
GROUP BY cc.id, cc.channel_name, cc.channel_type;

-- Unread Notifications View
CREATE OR REPLACE VIEW v_user_unread_notifications AS
SELECT 
    u.id AS user_id,
    u.username,
    COUNT(n.id) AS unread_count,
    COUNT(n.id) FILTER (WHERE n.priority >= 4) AS urgent_count,
    MAX(n.created_at) AS latest_notification_at,
    COUNT(n.id) FILTER (WHERE nt.category = 'combat') AS unread_combat,
    COUNT(n.id) FILTER (WHERE nt.category = 'fleet') AS unread_fleet,
    COUNT(n.id) FILTER (WHERE nt.category = 'trade') AS unread_trade
FROM users u
LEFT JOIN notifications n ON u.id = n.user_id AND n.is_read = FALSE AND n.is_archived = FALSE
LEFT JOIN notification_types nt ON n.notification_type_id = nt.id
WHERE u.is_banned = FALSE
GROUP BY u.id, u.username;

-- Active Trade Offers View
CREATE OR REPLACE VIEW v_active_trades AS
SELECT 
    t.id,
    u.username AS seller_username,
    t.resource_offered,
    t.amount_offered,
    t.resource_wanted,
    t.amount_wanted,
    t.exchange_rate,
    t.created_at,
    t.expires_at,
    EXTRACT(EPOCH FROM (t.expires_at - CURRENT_TIMESTAMP)) AS seconds_until_expiry
FROM trade_offers t
JOIN users u ON t.seller_id = u.id
WHERE t.status = 'active' AND t.expires_at > CURRENT_TIMESTAMP;

-- =====================================================
-- 9. FUNCTIONS AND TRIGGERS
-- =====================================================

-- Function: Update private conversation on new message
CREATE OR REPLACE FUNCTION update_conversation_on_message()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE private_conversations
    SET 
        last_message_at = NEW.created_at,
        user1_unread_count = CASE 
            WHEN NEW.sender_id = user2_id THEN user1_unread_count + 1 
            ELSE user1_unread_count 
        END,
        user2_unread_count = CASE 
            WHEN NEW.sender_id = user1_id THEN user2_unread_count + 1 
            ELSE user2_unread_count 
        END
    WHERE id = NEW.conversation_id;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_update_conversation_on_message
AFTER INSERT ON private_messages
FOR EACH ROW
EXECUTE FUNCTION update_conversation_on_message();

-- Function: Auto-expire trade offers
CREATE OR REPLACE FUNCTION auto_expire_trades()
RETURNS void AS $$
BEGIN
    UPDATE trade_offers
    SET status = 'expired'
    WHERE status = 'active' AND expires_at < CURRENT_TIMESTAMP;
END;
$$ LANGUAGE plpgsql;

-- Function: Clean old notifications
CREATE OR REPLACE FUNCTION clean_old_notifications(days_to_keep INTEGER DEFAULT 30)
RETURNS INTEGER AS $$
DECLARE
    deleted_count INTEGER;
BEGIN
    DELETE FROM notifications
    WHERE is_archived = TRUE 
      AND archived_at < CURRENT_TIMESTAMP - (days_to_keep || ' days')::INTERVAL;
    
    GET DIAGNOSTICS deleted_count = ROW_COUNT;
    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;

-- Function: Mark all notifications as read
CREATE OR REPLACE FUNCTION mark_all_notifications_read(p_user_id INTEGER)
RETURNS INTEGER AS $$
DECLARE
    updated_count INTEGER;
BEGIN
    UPDATE notifications
    SET is_read = TRUE, read_at = CURRENT_TIMESTAMP
    WHERE user_id = p_user_id AND is_read = FALSE;
    
    GET DIAGNOSTICS updated_count = ROW_COUNT;
    RETURN updated_count;
END;
$$ LANGUAGE plpgsql;

-- Function: Get user online status
CREATE OR REPLACE FUNCTION get_user_online_status(p_user_id INTEGER)
RETURNS VARCHAR AS $$
DECLARE
    user_status VARCHAR;
    last_active TIMESTAMP;
BEGIN
    SELECT status, last_activity INTO user_status, last_active
    FROM player_status
    WHERE user_id = p_user_id;
    
    IF user_status IS NULL THEN
        RETURN 'offline';
    END IF;
    
    IF user_status = 'online' AND last_active > CURRENT_TIMESTAMP - INTERVAL '5 minutes' THEN
        RETURN 'online';
    ELSIF user_status IN ('away', 'busy', 'in_combat') THEN
        RETURN user_status;
    ELSE
        RETURN 'offline';
    END IF;
END;
$$ LANGUAGE plpgsql;

-- =====================================================
-- 10. INITIAL DATA SEEDING
-- =====================================================

-- Insert default chat channels
INSERT INTO chat_channels (channel_name, channel_type, description, max_message_length, rate_limit_seconds) VALUES
('Global Chat', 'global', 'Main chat channel for all players', 500, 3),
('Trade Channel', 'trade', 'Buy and sell resources', 500, 5),
('Alliance Coordination', 'alliance', 'Alliance-wide strategic communication', 1000, 2),
('Combat Reports', 'combat', 'Live battle notifications and reports', 1000, 1),
('Help & Support', 'help', 'Get help from other players', 500, 5)
ON CONFLICT (channel_name) DO NOTHING;

-- Insert notification types
INSERT INTO notification_types (type_name, category, description, default_priority, icon) VALUES
('fleet_arrived', 'fleet', 'Your fleet has arrived at destination', 2, 'fleet'),
('fleet_returned', 'fleet', 'Your fleet has returned home', 2, 'fleet'),
('under_attack', 'combat', 'Your planet is under attack', 5, 'alert'),
('combat_report', 'combat', 'Combat report available', 3, 'combat'),
('building_complete', 'resource', 'Building construction completed', 1, 'building'),
('research_complete', 'resource', 'Research completed', 2, 'research'),
('alliance_invite', 'alliance', 'Alliance invitation received', 3, 'alliance'),
('alliance_message', 'alliance', 'New alliance message', 2, 'message'),
('trade_offer', 'trade', 'New trade offer available', 2, 'trade'),
('trade_complete', 'trade', 'Trade completed successfully', 2, 'trade'),
('achievement_unlocked', 'achievement', 'New achievement unlocked', 3, 'trophy'),
('system_announcement', 'system', 'System-wide announcement', 4, 'announcement')
ON CONFLICT (type_name) DO NOTHING;

-- =====================================================
-- END OF PHASE 6 SCHEMA
-- =====================================================
