-- Phase 5: Server Sharding Architecture - Database Schema
-- Universus Enterprise Multi-Server System
-- Version: 1.0.0
-- Date: 2025-11-06

-- ============================================================================
-- SHARD SERVER REGISTRY
-- ============================================================================

-- Server registry and health status tracking
CREATE TABLE IF NOT EXISTS shard_servers (
    id SERIAL PRIMARY KEY,
    server_id VARCHAR(100) UNIQUE NOT NULL,
    server_name VARCHAR(255) NOT NULL,
    server_type VARCHAR(50) NOT NULL, -- 'game', 'chat', 'leaderboard', 'market', 'analytics'
    region VARCHAR(50) NOT NULL, -- 'us-east', 'us-west', 'eu-west', 'asia-east'
    host_address VARCHAR(255) NOT NULL,
    port INTEGER NOT NULL,
    websocket_port INTEGER,
    capacity INTEGER DEFAULT 1000, -- Max players
    current_load INTEGER DEFAULT 0, -- Current players
    status VARCHAR(50) DEFAULT 'online', -- 'online', 'offline', 'maintenance', 'degraded'
    health_score INTEGER DEFAULT 100, -- 0-100 health metric
    cpu_usage DECIMAL(5,2) DEFAULT 0.0,
    memory_usage DECIMAL(5,2) DEFAULT 0.0,
    response_time_ms INTEGER DEFAULT 0,
    last_heartbeat TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    metadata JSONB DEFAULT '{}',
    CONSTRAINT check_capacity CHECK (capacity > 0),
    CONSTRAINT check_health_score CHECK (health_score >= 0 AND health_score <= 100),
    CONSTRAINT check_cpu_usage CHECK (cpu_usage >= 0 AND cpu_usage <= 100),
    CONSTRAINT check_memory_usage CHECK (memory_usage >= 0 AND memory_usage <= 100)
);

CREATE INDEX idx_shard_servers_type ON shard_servers(server_type);
CREATE INDEX idx_shard_servers_region ON shard_servers(region);
CREATE INDEX idx_shard_servers_status ON shard_servers(status);
CREATE INDEX idx_shard_servers_health ON shard_servers(health_score);
CREATE INDEX idx_shard_servers_load ON shard_servers(current_load);

-- ============================================================================
-- PLAYER-SERVER MAPPING
-- ============================================================================

-- Player routing and server assignment
CREATE TABLE IF NOT EXISTS shard_players (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    server_id VARCHAR(100) NOT NULL,
    session_id VARCHAR(255),
    assigned_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    last_active TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    connection_quality INTEGER DEFAULT 100, -- 0-100 connection quality
    preferred_region VARCHAR(50),
    is_active BOOLEAN DEFAULT true,
    metadata JSONB DEFAULT '{}',
    CONSTRAINT fk_shard_server FOREIGN KEY (server_id) REFERENCES shard_servers(server_id) ON DELETE CASCADE
);

CREATE INDEX idx_shard_players_user ON shard_players(user_id);
CREATE INDEX idx_shard_players_server ON shard_players(server_id);
CREATE INDEX idx_shard_players_session ON shard_players(session_id);
CREATE INDEX idx_shard_players_active ON shard_players(is_active);

-- ============================================================================
-- GLOBAL LEADERBOARDS
-- ============================================================================

-- Cross-server leaderboard aggregation
CREATE TABLE IF NOT EXISTS shard_leaderboards (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    server_id VARCHAR(100) NOT NULL,
    category VARCHAR(50) NOT NULL, -- 'total_points', 'fleet_power', 'research_level', 'resources'
    score BIGINT NOT NULL DEFAULT 0,
    rank INTEGER,
    previous_rank INTEGER,
    rank_change INTEGER DEFAULT 0,
    alliance_id INTEGER REFERENCES alliances(id) ON DELETE SET NULL,
    last_updated TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    metadata JSONB DEFAULT '{}',
    CONSTRAINT fk_leaderboard_server FOREIGN KEY (server_id) REFERENCES shard_servers(server_id) ON DELETE CASCADE
);

CREATE INDEX idx_shard_leaderboards_user ON shard_leaderboards(user_id);
CREATE INDEX idx_shard_leaderboards_category ON shard_leaderboards(category);
CREATE INDEX idx_shard_leaderboards_rank ON shard_leaderboards(rank);
CREATE INDEX idx_shard_leaderboards_score ON shard_leaderboards(score DESC);
CREATE INDEX idx_shard_leaderboards_alliance ON shard_leaderboards(alliance_id);

-- Time-based leaderboards
CREATE TABLE IF NOT EXISTS shard_leaderboard_snapshots (
    id SERIAL PRIMARY KEY,
    snapshot_date DATE NOT NULL,
    period VARCHAR(20) NOT NULL, -- 'daily', 'weekly', 'monthly'
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    category VARCHAR(50) NOT NULL,
    score BIGINT NOT NULL,
    rank INTEGER NOT NULL,
    server_id VARCHAR(100),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_leaderboard_snapshots_date ON shard_leaderboard_snapshots(snapshot_date);
CREATE INDEX idx_leaderboard_snapshots_period ON shard_leaderboard_snapshots(period);
CREATE INDEX idx_leaderboard_snapshots_user ON shard_leaderboard_snapshots(user_id);

-- ============================================================================
-- CROSS-SERVER CHAT
-- ============================================================================

-- Global chat message routing
CREATE TABLE IF NOT EXISTS shard_chat_messages (
    id SERIAL PRIMARY KEY,
    message_id VARCHAR(255) UNIQUE NOT NULL,
    sender_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    sender_server_id VARCHAR(100) NOT NULL,
    channel VARCHAR(50) NOT NULL, -- 'world', 'alliance', 'sector', 'private', 'system'
    recipient_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
    content TEXT NOT NULL,
    is_system_message BOOLEAN DEFAULT false,
    priority INTEGER DEFAULT 0, -- 0=normal, 1=important, 2=emergency
    sent_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    delivered_servers TEXT[], -- Array of server IDs that received the message
    metadata JSONB DEFAULT '{}',
    CONSTRAINT fk_sender_server FOREIGN KEY (sender_server_id) REFERENCES shard_servers(server_id) ON DELETE CASCADE
);

CREATE INDEX idx_shard_chat_sender ON shard_chat_messages(sender_id);
CREATE INDEX idx_shard_chat_channel ON shard_chat_messages(channel);
CREATE INDEX idx_shard_chat_recipient ON shard_chat_messages(recipient_id);
CREATE INDEX idx_shard_chat_sent_at ON shard_chat_messages(sent_at DESC);

-- Chat channels and subscriptions
CREATE TABLE IF NOT EXISTS shard_chat_channels (
    id SERIAL PRIMARY KEY,
    channel_name VARCHAR(100) UNIQUE NOT NULL,
    channel_type VARCHAR(50) NOT NULL, -- 'public', 'alliance', 'sector', 'private'
    server_id VARCHAR(100),
    alliance_id INTEGER REFERENCES alliances(id) ON DELETE CASCADE,
    max_members INTEGER DEFAULT 1000,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    metadata JSONB DEFAULT '{}'
);

CREATE INDEX idx_chat_channels_type ON shard_chat_channels(channel_type);
CREATE INDEX idx_chat_channels_server ON shard_chat_channels(server_id);

-- ============================================================================
-- CROSS-SERVER EVENTS
-- ============================================================================

-- Event coordination across shards
CREATE TABLE IF NOT EXISTS shard_events (
    id SERIAL PRIMARY KEY,
    event_id VARCHAR(255) UNIQUE NOT NULL,
    event_type VARCHAR(50) NOT NULL, -- 'server_startup', 'maintenance', 'alliance_war', 'market_event'
    source_server_id VARCHAR(100) NOT NULL,
    target_servers TEXT[], -- Array of target server IDs
    event_data JSONB NOT NULL,
    status VARCHAR(50) DEFAULT 'pending', -- 'pending', 'processing', 'completed', 'failed'
    priority INTEGER DEFAULT 0,
    scheduled_at TIMESTAMP,
    started_at TIMESTAMP,
    completed_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    metadata JSONB DEFAULT '{}',
    CONSTRAINT fk_event_source_server FOREIGN KEY (source_server_id) REFERENCES shard_servers(server_id) ON DELETE CASCADE
);

CREATE INDEX idx_shard_events_type ON shard_events(event_type);
CREATE INDEX idx_shard_events_status ON shard_events(status);
CREATE INDEX idx_shard_events_scheduled ON shard_events(scheduled_at);
CREATE INDEX idx_shard_events_source ON shard_events(source_server_id);

-- ============================================================================
-- RESOURCE MARKET SHARDING
-- ============================================================================

-- Cross-server resource trading
CREATE TABLE IF NOT EXISTS shard_market_listings (
    id SERIAL PRIMARY KEY,
    listing_id VARCHAR(255) UNIQUE NOT NULL,
    seller_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    seller_server_id VARCHAR(100) NOT NULL,
    resource_type VARCHAR(50) NOT NULL, -- 'metal', 'crystal', 'deuterium'
    quantity BIGINT NOT NULL,
    price_per_unit INTEGER NOT NULL,
    total_price BIGINT NOT NULL,
    status VARCHAR(50) DEFAULT 'active', -- 'active', 'sold', 'cancelled', 'expired'
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMP,
    sold_at TIMESTAMP,
    buyer_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
    buyer_server_id VARCHAR(100),
    metadata JSONB DEFAULT '{}',
    CONSTRAINT fk_seller_server FOREIGN KEY (seller_server_id) REFERENCES shard_servers(server_id) ON DELETE CASCADE,
    CONSTRAINT check_quantity CHECK (quantity > 0),
    CONSTRAINT check_price CHECK (price_per_unit > 0)
);

CREATE INDEX idx_market_seller ON shard_market_listings(seller_id);
CREATE INDEX idx_market_resource ON shard_market_listings(resource_type);
CREATE INDEX idx_market_status ON shard_market_listings(status);
CREATE INDEX idx_market_price ON shard_market_listings(price_per_unit);
CREATE INDEX idx_market_created ON shard_market_listings(created_at DESC);

-- Market price history
CREATE TABLE IF NOT EXISTS shard_market_prices (
    id SERIAL PRIMARY KEY,
    resource_type VARCHAR(50) NOT NULL,
    server_id VARCHAR(100),
    avg_price DECIMAL(10,2) NOT NULL,
    min_price DECIMAL(10,2) NOT NULL,
    max_price DECIMAL(10,2) NOT NULL,
    volume BIGINT NOT NULL,
    transactions INTEGER NOT NULL,
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    period VARCHAR(20) NOT NULL -- 'hourly', 'daily', 'weekly'
);

CREATE INDEX idx_market_prices_resource ON shard_market_prices(resource_type);
CREATE INDEX idx_market_prices_timestamp ON shard_market_prices(timestamp DESC);

-- ============================================================================
-- ALLIANCE SHARDING
-- ============================================================================

-- Cross-server alliance data
CREATE TABLE IF NOT EXISTS shard_alliances (
    id SERIAL PRIMARY KEY,
    alliance_id INTEGER NOT NULL REFERENCES alliances(id) ON DELETE CASCADE,
    home_server_id VARCHAR(100) NOT NULL,
    member_servers TEXT[], -- Servers where alliance members are located
    total_members INTEGER DEFAULT 0,
    total_power BIGINT DEFAULT 0,
    global_rank INTEGER,
    is_cross_server BOOLEAN DEFAULT false,
    last_sync TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    metadata JSONB DEFAULT '{}',
    CONSTRAINT fk_alliance_home_server FOREIGN KEY (home_server_id) REFERENCES shard_servers(server_id) ON DELETE CASCADE
);

CREATE INDEX idx_shard_alliances_alliance ON shard_alliances(alliance_id);
CREATE INDEX idx_shard_alliances_server ON shard_alliances(home_server_id);
CREATE INDEX idx_shard_alliances_rank ON shard_alliances(global_rank);

-- ============================================================================
-- SERVER MONITORING
-- ============================================================================

-- Performance and health metrics
CREATE TABLE IF NOT EXISTS shard_monitoring (
    id SERIAL PRIMARY KEY,
    server_id VARCHAR(100) NOT NULL,
    metric_type VARCHAR(50) NOT NULL, -- 'cpu', 'memory', 'disk', 'network', 'response_time', 'errors'
    metric_value DECIMAL(10,2) NOT NULL,
    threshold_warning DECIMAL(10,2),
    threshold_critical DECIMAL(10,2),
    status VARCHAR(50) DEFAULT 'normal', -- 'normal', 'warning', 'critical'
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    metadata JSONB DEFAULT '{}',
    CONSTRAINT fk_monitoring_server FOREIGN KEY (server_id) REFERENCES shard_servers(server_id) ON DELETE CASCADE
);

CREATE INDEX idx_monitoring_server ON shard_monitoring(server_id);
CREATE INDEX idx_monitoring_type ON shard_monitoring(metric_type);
CREATE INDEX idx_monitoring_timestamp ON shard_monitoring(timestamp DESC);
CREATE INDEX idx_monitoring_status ON shard_monitoring(status);

-- Server alerts and notifications
CREATE TABLE IF NOT EXISTS shard_alerts (
    id SERIAL PRIMARY KEY,
    alert_id VARCHAR(255) UNIQUE NOT NULL,
    server_id VARCHAR(100) NOT NULL,
    alert_type VARCHAR(50) NOT NULL, -- 'high_load', 'low_health', 'error_rate', 'offline'
    severity VARCHAR(20) NOT NULL, -- 'info', 'warning', 'critical'
    message TEXT NOT NULL,
    is_resolved BOOLEAN DEFAULT false,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    resolved_at TIMESTAMP,
    metadata JSONB DEFAULT '{}',
    CONSTRAINT fk_alert_server FOREIGN KEY (server_id) REFERENCES shard_servers(server_id) ON DELETE CASCADE
);

CREATE INDEX idx_alerts_server ON shard_alerts(server_id);
CREATE INDEX idx_alerts_severity ON shard_alerts(severity);
CREATE INDEX idx_alerts_resolved ON shard_alerts(is_resolved);
CREATE INDEX idx_alerts_created ON shard_alerts(created_at DESC);

-- ============================================================================
-- LOAD BALANCER CONFIGURATION
-- ============================================================================

-- Load balancing rules and routing
CREATE TABLE IF NOT EXISTS shard_routing_rules (
    id SERIAL PRIMARY KEY,
    rule_name VARCHAR(255) UNIQUE NOT NULL,
    rule_type VARCHAR(50) NOT NULL, -- 'geographic', 'load_based', 'affinity', 'weighted'
    priority INTEGER DEFAULT 0,
    conditions JSONB NOT NULL,
    target_servers TEXT[],
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    metadata JSONB DEFAULT '{}'
);

CREATE INDEX idx_routing_rules_type ON shard_routing_rules(rule_type);
CREATE INDEX idx_routing_rules_priority ON shard_routing_rules(priority);
CREATE INDEX idx_routing_rules_active ON shard_routing_rules(is_active);

-- ============================================================================
-- SCALING CONFIGURATION
-- ============================================================================

-- Automatic scaling rules
CREATE TABLE IF NOT EXISTS shard_scaling_config (
    id SERIAL PRIMARY KEY,
    config_name VARCHAR(255) UNIQUE NOT NULL,
    server_type VARCHAR(50) NOT NULL,
    min_servers INTEGER DEFAULT 1,
    max_servers INTEGER DEFAULT 10,
    scale_up_threshold DECIMAL(5,2) DEFAULT 80.0, -- CPU/Memory %
    scale_down_threshold DECIMAL(5,2) DEFAULT 20.0,
    cooldown_period INTEGER DEFAULT 300, -- Seconds
    is_enabled BOOLEAN DEFAULT true,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    metadata JSONB DEFAULT '{}'
);

-- Scaling events history
CREATE TABLE IF NOT EXISTS shard_scaling_events (
    id SERIAL PRIMARY KEY,
    event_type VARCHAR(50) NOT NULL, -- 'scale_up', 'scale_down', 'migration'
    server_type VARCHAR(50) NOT NULL,
    trigger_reason TEXT NOT NULL,
    servers_before INTEGER NOT NULL,
    servers_after INTEGER NOT NULL,
    affected_players INTEGER DEFAULT 0,
    started_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    completed_at TIMESTAMP,
    status VARCHAR(50) DEFAULT 'in_progress', -- 'in_progress', 'completed', 'failed'
    metadata JSONB DEFAULT '{}'
);

CREATE INDEX idx_scaling_events_type ON shard_scaling_events(event_type);
CREATE INDEX idx_scaling_events_started ON shard_scaling_events(started_at DESC);

-- ============================================================================
-- VIEWS FOR MONITORING
-- ============================================================================

-- Server health overview
CREATE OR REPLACE VIEW shard_server_health_view AS
SELECT 
    s.server_id,
    s.server_name,
    s.server_type,
    s.region,
    s.status,
    s.health_score,
    s.current_load,
    s.capacity,
    ROUND((s.current_load::DECIMAL / s.capacity) * 100, 2) as load_percentage,
    s.cpu_usage,
    s.memory_usage,
    s.response_time_ms,
    s.last_heartbeat,
    EXTRACT(EPOCH FROM (NOW() - s.last_heartbeat)) as seconds_since_heartbeat
FROM shard_servers s;

-- Global leaderboard view
CREATE OR REPLACE VIEW shard_global_leaderboard_view AS
SELECT 
    u.id as user_id,
    u.username,
    l.category,
    l.score,
    l.rank,
    l.rank_change,
    l.server_id,
    a.name as alliance_name,
    l.last_updated
FROM shard_leaderboards l
JOIN users u ON l.user_id = u.id
LEFT JOIN alliances a ON l.alliance_id = a.id
ORDER BY l.category, l.rank;

-- Market analytics view
CREATE OR REPLACE VIEW shard_market_analytics_view AS
SELECT 
    resource_type,
    COUNT(*) as active_listings,
    SUM(quantity) as total_quantity,
    AVG(price_per_unit) as avg_price,
    MIN(price_per_unit) as min_price,
    MAX(price_per_unit) as max_price
FROM shard_market_listings
WHERE status = 'active'
GROUP BY resource_type;

-- ============================================================================
-- FUNCTIONS
-- ============================================================================

-- Function to register a new server
CREATE OR REPLACE FUNCTION register_shard_server(
    p_server_id VARCHAR(100),
    p_server_name VARCHAR(255),
    p_server_type VARCHAR(50),
    p_region VARCHAR(50),
    p_host VARCHAR(255),
    p_port INTEGER,
    p_capacity INTEGER DEFAULT 1000
) RETURNS INTEGER AS $$
DECLARE
    v_id INTEGER;
BEGIN
    INSERT INTO shard_servers (
        server_id, server_name, server_type, region, 
        host_address, port, capacity
    ) VALUES (
        p_server_id, p_server_name, p_server_type, p_region,
        p_host, p_port, p_capacity
    )
    ON CONFLICT (server_id) 
    DO UPDATE SET
        server_name = EXCLUDED.server_name,
        host_address = EXCLUDED.host_address,
        port = EXCLUDED.port,
        capacity = EXCLUDED.capacity,
        updated_at = CURRENT_TIMESTAMP
    RETURNING id INTO v_id;
    
    RETURN v_id;
END;
$$ LANGUAGE plpgsql;

-- Function to update server health
CREATE OR REPLACE FUNCTION update_server_health(
    p_server_id VARCHAR(100),
    p_cpu_usage DECIMAL(5,2),
    p_memory_usage DECIMAL(5,2),
    p_response_time_ms INTEGER,
    p_current_load INTEGER
) RETURNS VOID AS $$
DECLARE
    v_health_score INTEGER;
BEGIN
    -- Calculate health score (0-100)
    v_health_score := 100 
        - (p_cpu_usage / 2)::INTEGER
        - (p_memory_usage / 2)::INTEGER
        - LEAST(p_response_time_ms / 10, 20)::INTEGER;
    
    v_health_score := GREATEST(0, LEAST(100, v_health_score));
    
    UPDATE shard_servers
    SET 
        cpu_usage = p_cpu_usage,
        memory_usage = p_memory_usage,
        response_time_ms = p_response_time_ms,
        current_load = p_current_load,
        health_score = v_health_score,
        last_heartbeat = CURRENT_TIMESTAMP,
        updated_at = CURRENT_TIMESTAMP
    WHERE server_id = p_server_id;
END;
$$ LANGUAGE plpgsql;

-- Function to assign player to server
CREATE OR REPLACE FUNCTION assign_player_to_server(
    p_user_id INTEGER,
    p_preferred_region VARCHAR(50) DEFAULT NULL
) RETURNS VARCHAR(100) AS $$
DECLARE
    v_server_id VARCHAR(100);
    v_session_id VARCHAR(255);
BEGIN
    -- Generate session ID
    v_session_id := md5(random()::text || clock_timestamp()::text);
    
    -- Find best available server
    SELECT server_id INTO v_server_id
    FROM shard_servers
    WHERE server_type = 'game'
        AND status = 'online'
        AND current_load < capacity
        AND (p_preferred_region IS NULL OR region = p_preferred_region)
    ORDER BY 
        CASE WHEN region = p_preferred_region THEN 0 ELSE 1 END,
        (current_load::DECIMAL / capacity),
        health_score DESC
    LIMIT 1;
    
    IF v_server_id IS NULL THEN
        RAISE EXCEPTION 'No available servers found';
    END IF;
    
    -- Deactivate previous assignments
    UPDATE shard_players
    SET is_active = false
    WHERE user_id = p_user_id;
    
    -- Create new assignment
    INSERT INTO shard_players (
        user_id, server_id, session_id, preferred_region
    ) VALUES (
        p_user_id, v_server_id, v_session_id, p_preferred_region
    );
    
    -- Increment server load
    UPDATE shard_servers
    SET current_load = current_load + 1
    WHERE server_id = v_server_id;
    
    RETURN v_server_id;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- TRIGGERS
-- ============================================================================

-- Trigger to update server load on player disconnect
CREATE OR REPLACE FUNCTION update_server_load_on_disconnect()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.is_active = true AND NEW.is_active = false THEN
        UPDATE shard_servers
        SET current_load = GREATEST(0, current_load - 1)
        WHERE server_id = OLD.server_id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_player_disconnect
AFTER UPDATE ON shard_players
FOR EACH ROW
WHEN (OLD.is_active IS DISTINCT FROM NEW.is_active)
EXECUTE FUNCTION update_server_load_on_disconnect();

-- Trigger to create alerts on health issues
CREATE OR REPLACE FUNCTION create_health_alert()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.health_score < 50 AND (OLD.health_score IS NULL OR OLD.health_score >= 50) THEN
        INSERT INTO shard_alerts (
            alert_id, server_id, alert_type, severity, message
        ) VALUES (
            md5(random()::text || clock_timestamp()::text),
            NEW.server_id,
            'low_health',
            CASE 
                WHEN NEW.health_score < 25 THEN 'critical'
                ELSE 'warning'
            END,
            format('Server %s health dropped to %s', NEW.server_name, NEW.health_score)
        );
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_health_alert
AFTER UPDATE ON shard_servers
FOR EACH ROW
WHEN (OLD.health_score IS DISTINCT FROM NEW.health_score)
EXECUTE FUNCTION create_health_alert();

-- ============================================================================
-- INDEXES FOR PERFORMANCE
-- ============================================================================

-- Additional composite indexes for common queries
CREATE INDEX idx_shard_players_user_server ON shard_players(user_id, server_id);
CREATE INDEX idx_shard_leaderboards_category_rank ON shard_leaderboards(category, rank);
CREATE INDEX idx_market_listings_resource_status ON shard_market_listings(resource_type, status);
CREATE INDEX idx_chat_messages_channel_sent ON shard_chat_messages(channel, sent_at DESC);

-- ============================================================================
-- COMMENTS
-- ============================================================================

COMMENT ON TABLE shard_servers IS 'Registry of all game servers in the sharded system';
COMMENT ON TABLE shard_players IS 'Player-to-server assignment and routing information';
COMMENT ON TABLE shard_leaderboards IS 'Cross-server leaderboard data aggregation';
COMMENT ON TABLE shard_chat_messages IS 'Global chat messages routed across servers';
COMMENT ON TABLE shard_events IS 'Cross-server event coordination and synchronization';
COMMENT ON TABLE shard_market_listings IS 'Global resource market across all servers';
COMMENT ON TABLE shard_alliances IS 'Cross-server alliance information and coordination';
COMMENT ON TABLE shard_monitoring IS 'Server performance metrics and health monitoring';
COMMENT ON TABLE shard_alerts IS 'Automated alerts for server health issues';
COMMENT ON TABLE shard_routing_rules IS 'Load balancer routing configuration';
COMMENT ON TABLE shard_scaling_config IS 'Automatic scaling configuration and thresholds';
