-- ========================================
-- UNIVERSUS COMBAT DEBRIS & LOOT SYSTEM
-- Phase 3: Realistic Space Combat Debris
-- ========================================

-- Combat Debris Fields (Main debris field locations)
CREATE TABLE IF NOT EXISTS combat_debris (
  id SERIAL PRIMARY KEY,
  galaxy INTEGER NOT NULL,
  system INTEGER NOT NULL,
  position INTEGER NOT NULL,
  debris_type VARCHAR(50) NOT NULL CHECK (debris_type IN ('light', 'heavy', 'wreckage', 'components', 'rare', 'radiation')),
  total_metal BIGINT DEFAULT 0,
  total_crystal BIGINT DEFAULT 0,
  total_deuterium BIGINT DEFAULT 0,
  total_rare_materials BIGINT DEFAULT 0,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  created_by_combat_id INTEGER,
  decay_start TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  decay_rate NUMERIC DEFAULT 0.1,
  expires_at TIMESTAMP,
  is_active BOOLEAN DEFAULT TRUE,
  is_claimed BOOLEAN DEFAULT FALSE,
  claimed_by INTEGER REFERENCES users(id),
  claimed_at TIMESTAMP,
  hazard_level INTEGER CHECK (hazard_level BETWEEN 0 AND 10) DEFAULT 0,
  radiation_level INTEGER CHECK (radiation_level BETWEEN 0 AND 100) DEFAULT 0,
  spread_radius INTEGER DEFAULT 100,
  metadata JSONB
);

CREATE INDEX idx_debris_location ON combat_debris(galaxy, system, position);
CREATE INDEX idx_debris_active ON combat_debris(is_active);
CREATE INDEX idx_debris_expires ON combat_debris(expires_at);
CREATE INDEX idx_debris_claimed ON combat_debris(is_claimed);

-- Debris Resources (Individual resource items within debris fields)
CREATE TABLE IF NOT EXISTS debris_resources (
  id SERIAL PRIMARY KEY,
  debris_id INTEGER REFERENCES combat_debris(id) ON DELETE CASCADE,
  resource_type VARCHAR(50) NOT NULL,
  resource_subtype VARCHAR(50),
  quantity BIGINT NOT NULL,
  quality_grade VARCHAR(20) CHECK (quality_grade IN ('poor', 'common', 'uncommon', 'rare', 'legendary')) DEFAULT 'common',
  recyclable BOOLEAN DEFAULT TRUE,
  recycle_efficiency NUMERIC DEFAULT 0.8,
  position_x INTEGER,
  position_y INTEGER,
  position_z INTEGER,
  is_collected BOOLEAN DEFAULT FALSE,
  collected_by INTEGER REFERENCES users(id),
  collected_at TIMESTAMP,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_debris_res_field ON debris_resources(debris_id);
CREATE INDEX idx_debris_res_type ON debris_resources(resource_type);
CREATE INDEX idx_debris_res_collected ON debris_resources(is_collected);

-- Debris Salvage Operations (Player salvage missions)
CREATE TABLE IF NOT EXISTS debris_salvage (
  id SERIAL PRIMARY KEY,
  user_id INTEGER REFERENCES users(id) ON DELETE CASCADE,
  debris_id INTEGER REFERENCES combat_debris(id) ON DELETE CASCADE,
  salvage_type VARCHAR(50) NOT NULL CHECK (salvage_type IN ('automated', 'manual', 'alliance', 'emergency', 'deep_space', 'commercial')),
  fleet_id INTEGER REFERENCES fleets(id),
  ship_types JSONB,
  cargo_capacity BIGINT,
  salvage_efficiency NUMERIC DEFAULT 1.0,
  status VARCHAR(30) CHECK (status IN ('planned', 'en_route', 'salvaging', 'returning', 'completed', 'failed')) DEFAULT 'planned',
  start_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  arrival_time TIMESTAMP,
  completion_time TIMESTAMP,
  return_time TIMESTAMP,
  resources_collected JSONB,
  components_collected JSONB,
  total_value BIGINT DEFAULT 0,
  experience_gained INTEGER DEFAULT 0,
  success_rate NUMERIC,
  hazards_encountered JSONB,
  alliance_id INTEGER,
  is_competitive BOOLEAN DEFAULT FALSE,
  ranking INTEGER,
  notes TEXT
);

CREATE INDEX idx_salvage_user ON debris_salvage(user_id);
CREATE INDEX idx_salvage_debris ON debris_salvage(debris_id);
CREATE INDEX idx_salvage_status ON debris_salvage(status);
CREATE INDEX idx_salvage_fleet ON debris_salvage(fleet_id);

-- Debris Claims (Temporary claims on debris fields)
CREATE TABLE IF NOT EXISTS debris_claims (
  id SERIAL PRIMARY KEY,
  debris_id INTEGER REFERENCES combat_debris(id) ON DELETE CASCADE,
  user_id INTEGER REFERENCES users(id) ON DELETE CASCADE,
  alliance_id INTEGER,
  claim_type VARCHAR(30) CHECK (claim_type IN ('exclusive', 'shared', 'contested', 'alliance')) DEFAULT 'exclusive',
  claim_start TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  claim_duration INTEGER DEFAULT 3600,
  claim_expires TIMESTAMP,
  is_active BOOLEAN DEFAULT TRUE,
  priority_level INTEGER DEFAULT 1,
  claim_reason VARCHAR(100),
  UNIQUE(debris_id, user_id)
);

CREATE INDEX idx_claims_debris ON debris_claims(debris_id);
CREATE INDEX idx_claims_user ON debris_claims(user_id);
CREATE INDEX idx_claims_active ON debris_claims(is_active);
CREATE INDEX idx_claims_expires ON debris_claims(claim_expires);

-- Ship Components (Recyclable components from destroyed ships)
CREATE TABLE IF NOT EXISTS ship_components (
  id SERIAL PRIMARY KEY,
  component_type VARCHAR(50) NOT NULL CHECK (component_type IN ('engine', 'weapon', 'armor', 'electronics', 'advanced_material', 'research_data')),
  component_name VARCHAR(100) NOT NULL,
  component_subtype VARCHAR(50),
  quality_grade VARCHAR(20) CHECK (quality_grade IN ('poor', 'common', 'uncommon', 'rare', 'legendary')) DEFAULT 'common',
  condition_percent INTEGER CHECK (condition_percent BETWEEN 0 AND 100) DEFAULT 100,
  source_ship_type VARCHAR(50),
  tech_level INTEGER DEFAULT 1,
  recycle_value_metal BIGINT DEFAULT 0,
  recycle_value_crystal BIGINT DEFAULT 0,
  recycle_value_deuterium BIGINT DEFAULT 0,
  recycle_efficiency NUMERIC DEFAULT 0.8,
  market_value BIGINT DEFAULT 0,
  is_tradeable BOOLEAN DEFAULT TRUE,
  is_unique BOOLEAN DEFAULT FALSE,
  required_tech JSONB,
  bonus_stats JSONB,
  description TEXT,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_components_type ON ship_components(component_type);
CREATE INDEX idx_components_quality ON ship_components(quality_grade);
CREATE INDEX idx_components_source ON ship_components(source_ship_type);

-- Player Component Inventory
CREATE TABLE IF NOT EXISTS player_component_inventory (
  id SERIAL PRIMARY KEY,
  user_id INTEGER REFERENCES users(id) ON DELETE CASCADE,
  component_id INTEGER REFERENCES ship_components(id),
  quantity INTEGER DEFAULT 1,
  acquired_from VARCHAR(50),
  acquired_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  is_equipped BOOLEAN DEFAULT FALSE,
  equipped_to_ship VARCHAR(50),
  UNIQUE(user_id, component_id)
);

CREATE INDEX idx_inventory_user ON player_component_inventory(user_id);
CREATE INDEX idx_inventory_component ON player_component_inventory(component_id);

-- Debris Events (Combat events that generated debris)
CREATE TABLE IF NOT EXISTS debris_events (
  id SERIAL PRIMARY KEY,
  event_type VARCHAR(50) NOT NULL CHECK (event_type IN ('combat', 'asteroid_mining', 'ship_destruction', 'station_destruction', 'natural_disaster')),
  debris_id INTEGER REFERENCES combat_debris(id),
  galaxy INTEGER NOT NULL,
  system INTEGER NOT NULL,
  position INTEGER NOT NULL,
  attacker_id INTEGER REFERENCES users(id),
  defender_id INTEGER REFERENCES users(id),
  attacker_alliance INTEGER,
  defender_alliance INTEGER,
  ships_destroyed JSONB,
  total_destroyed_value BIGINT DEFAULT 0,
  debris_generated_metal BIGINT DEFAULT 0,
  debris_generated_crystal BIGINT DEFAULT 0,
  debris_generated_deuterium BIGINT DEFAULT 0,
  debris_generation_rate NUMERIC DEFAULT 0.3,
  rare_components_generated INTEGER DEFAULT 0,
  combat_result VARCHAR(20) CHECK (combat_result IN ('attacker_victory', 'defender_victory', 'draw', 'mutual_destruction')),
  timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  battle_duration INTEGER,
  metadata JSONB
);

CREATE INDEX idx_debris_events_location ON debris_events(galaxy, system, position);
CREATE INDEX idx_debris_events_attacker ON debris_events(attacker_id);
CREATE INDEX idx_debris_events_defender ON debris_events(defender_id);
CREATE INDEX idx_debris_events_timestamp ON debris_events(timestamp DESC);

-- Debris Cleanup Schedule
CREATE TABLE IF NOT EXISTS debris_cleanup (
  id SERIAL PRIMARY KEY,
  debris_id INTEGER REFERENCES combat_debris(id) ON DELETE CASCADE,
  cleanup_type VARCHAR(30) CHECK (cleanup_type IN ('automatic', 'manual', 'forced', 'maintenance')) DEFAULT 'automatic',
  scheduled_at TIMESTAMP NOT NULL,
  executed_at TIMESTAMP,
  status VARCHAR(20) CHECK (status IN ('pending', 'in_progress', 'completed', 'failed')) DEFAULT 'pending',
  resources_recovered JSONB,
  cleanup_crew INTEGER REFERENCES users(id),
  cleanup_reason TEXT,
  performance_impact_before NUMERIC,
  performance_impact_after NUMERIC
);

CREATE INDEX idx_cleanup_debris ON debris_cleanup(debris_id);
CREATE INDEX idx_cleanup_scheduled ON debris_cleanup(scheduled_at);
CREATE INDEX idx_cleanup_status ON debris_cleanup(status);

-- Salvage Leaderboard & Statistics
CREATE TABLE IF NOT EXISTS salvage_statistics (
  id SERIAL PRIMARY KEY,
  user_id INTEGER REFERENCES users(id) ON DELETE CASCADE,
  total_salvage_missions INTEGER DEFAULT 0,
  successful_missions INTEGER DEFAULT 0,
  failed_missions INTEGER DEFAULT 0,
  total_metal_collected BIGINT DEFAULT 0,
  total_crystal_collected BIGINT DEFAULT 0,
  total_deuterium_collected BIGINT DEFAULT 0,
  total_rare_materials BIGINT DEFAULT 0,
  total_components_found INTEGER DEFAULT 0,
  legendary_components INTEGER DEFAULT 0,
  total_salvage_value BIGINT DEFAULT 0,
  fastest_salvage_time INTEGER,
  largest_single_haul BIGINT DEFAULT 0,
  salvage_efficiency_avg NUMERIC DEFAULT 0,
  competitive_wins INTEGER DEFAULT 0,
  alliance_contributions BIGINT DEFAULT 0,
  salvage_experience_points INTEGER DEFAULT 0,
  salvage_level INTEGER DEFAULT 1,
  salvage_rank VARCHAR(50),
  last_salvage_at TIMESTAMP,
  updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(user_id)
);

CREATE INDEX idx_salvage_stats_user ON salvage_statistics(user_id);
CREATE INDEX idx_salvage_stats_value ON salvage_statistics(total_salvage_value DESC);
CREATE INDEX idx_salvage_stats_level ON salvage_statistics(salvage_level DESC);

-- ========================================
-- ENHANCED EXISTING TABLES
-- ========================================

-- Add debris-related fields to users table
ALTER TABLE users ADD COLUMN IF NOT EXISTS salvage_tech_level INTEGER DEFAULT 1;
ALTER TABLE users ADD COLUMN IF NOT EXISTS salvage_experience INTEGER DEFAULT 0;
ALTER TABLE users ADD COLUMN IF NOT EXISTS total_debris_collected BIGINT DEFAULT 0;
ALTER TABLE users ADD COLUMN IF NOT EXISTS rare_finds INTEGER DEFAULT 0;

-- Add debris fields to planets (if debris lands near planets)
ALTER TABLE planets ADD COLUMN IF NOT EXISTS nearby_debris_count INTEGER DEFAULT 0;
ALTER TABLE planets ADD COLUMN IF NOT EXISTS debris_collection_bonus NUMERIC DEFAULT 1.0;

-- ========================================
-- VIEWS FOR ANALYTICS
-- ========================================

-- Active Debris Fields View
CREATE OR REPLACE VIEW v_active_debris_fields AS
SELECT 
  cd.*,
  COUNT(dr.id) as resource_count,
  SUM(dr.quantity) as total_resources,
  dc.user_id as claimed_by_user,
  u.username as claimant_username,
  (cd.total_metal + cd.total_crystal + cd.total_deuterium) as total_value,
  EXTRACT(epoch FROM (cd.expires_at - NOW())) / 3600 as hours_remaining
FROM combat_debris cd
LEFT JOIN debris_resources dr ON cd.id = dr.debris_id
LEFT JOIN debris_claims dc ON cd.id = dc.debris_id AND dc.is_active = TRUE
LEFT JOIN users u ON dc.user_id = u.id
WHERE cd.is_active = TRUE
  AND cd.expires_at > NOW()
GROUP BY cd.id, dc.user_id, u.username;

-- Top Salvagers View
CREATE OR REPLACE VIEW v_top_salvagers AS
SELECT 
  ss.*,
  u.username,
  RANK() OVER (ORDER BY ss.total_salvage_value DESC) as global_rank
FROM salvage_statistics ss
JOIN users u ON ss.user_id = u.id
ORDER BY ss.total_salvage_value DESC
LIMIT 100;

-- Debris Field Economy View
CREATE OR REPLACE VIEW v_debris_economy AS
SELECT 
  DATE_TRUNC('day', created_at) as date,
  COUNT(*) as fields_created,
  SUM(total_metal) as total_metal_generated,
  SUM(total_crystal) as total_crystal_generated,
  SUM(total_deuterium) as total_deuterium_generated,
  AVG(total_metal + total_crystal + total_deuterium) as avg_field_value,
  SUM(CASE WHEN is_claimed THEN 1 ELSE 0 END) as fields_claimed,
  SUM(CASE WHEN is_active THEN 1 ELSE 0 END) as fields_active
FROM combat_debris
WHERE created_at > NOW() - INTERVAL '30 days'
GROUP BY DATE_TRUNC('day', created_at)
ORDER BY date DESC;

-- ========================================
-- HELPER FUNCTIONS
-- ========================================

-- Function to generate debris from combat
CREATE OR REPLACE FUNCTION generate_combat_debris(
  p_galaxy INTEGER,
  p_system INTEGER,
  p_position INTEGER,
  p_destroyed_ships JSONB,
  p_total_value BIGINT,
  p_debris_rate NUMERIC DEFAULT 0.3
) RETURNS INTEGER AS $$
DECLARE
  v_debris_id INTEGER;
  v_metal BIGINT;
  v_crystal BIGINT;
  v_deuterium BIGINT;
  v_decay_hours INTEGER DEFAULT 72;
BEGIN
  -- Calculate debris amounts
  v_metal := FLOOR(p_total_value * p_debris_rate * 0.5);
  v_crystal := FLOOR(p_total_value * p_debris_rate * 0.3);
  v_deuterium := FLOOR(p_total_value * p_debris_rate * 0.2);
  
  -- Create debris field
  INSERT INTO combat_debris (
    galaxy, system, position,
    debris_type,
    total_metal, total_crystal, total_deuterium,
    expires_at,
    hazard_level,
    spread_radius
  ) VALUES (
    p_galaxy, p_system, p_position,
    CASE 
      WHEN p_total_value > 10000000 THEN 'wreckage'
      WHEN p_total_value > 1000000 THEN 'heavy'
      ELSE 'light'
    END,
    v_metal, v_crystal, v_deuterium,
    NOW() + INTERVAL '1 hour' * v_decay_hours,
    CASE 
      WHEN p_total_value > 5000000 THEN 5
      WHEN p_total_value > 1000000 THEN 3
      ELSE 1
    END,
    CASE 
      WHEN p_total_value > 10000000 THEN 500
      WHEN p_total_value > 1000000 THEN 200
      ELSE 100
    END
  ) RETURNING id INTO v_debris_id;
  
  RETURN v_debris_id;
END;
$$ LANGUAGE plpgsql;

-- Function to calculate salvage efficiency
CREATE OR REPLACE FUNCTION calculate_salvage_efficiency(
  p_user_id INTEGER,
  p_debris_id INTEGER
) RETURNS NUMERIC AS $$
DECLARE
  v_base_efficiency NUMERIC DEFAULT 0.7;
  v_tech_level INTEGER;
  v_tech_bonus NUMERIC;
  v_hazard_penalty NUMERIC;
  v_competition_penalty NUMERIC;
  v_final_efficiency NUMERIC;
BEGIN
  -- Get user tech level
  SELECT salvage_tech_level INTO v_tech_level
  FROM users WHERE id = p_user_id;
  
  -- Calculate tech bonus (1% per level, max 30%)
  v_tech_bonus := LEAST(v_tech_level * 0.01, 0.30);
  
  -- Get hazard penalty from debris field
  SELECT 
    CASE 
      WHEN hazard_level > 5 THEN 0.2
      WHEN hazard_level > 3 THEN 0.1
      ELSE 0
    END INTO v_hazard_penalty
  FROM combat_debris WHERE id = p_debris_id;
  
  -- Calculate competition penalty
  SELECT 
    CASE 
      WHEN COUNT(*) > 3 THEN 0.15
      WHEN COUNT(*) > 1 THEN 0.05
      ELSE 0
    END INTO v_competition_penalty
  FROM debris_salvage
  WHERE debris_id = p_debris_id AND status IN ('en_route', 'salvaging');
  
  -- Calculate final efficiency
  v_final_efficiency := v_base_efficiency + v_tech_bonus - v_hazard_penalty - v_competition_penalty;
  
  RETURN GREATEST(0.3, LEAST(1.0, v_final_efficiency));
END;
$$ LANGUAGE plpgsql;

-- Function to auto-decay debris fields
CREATE OR REPLACE FUNCTION auto_decay_debris() RETURNS INTEGER AS $$
DECLARE
  v_decayed_count INTEGER;
BEGIN
  -- Decay debris fields based on time
  UPDATE combat_debris
  SET 
    total_metal = GREATEST(0, FLOOR(total_metal * (1 - decay_rate))),
    total_crystal = GREATEST(0, FLOOR(total_crystal * (1 - decay_rate))),
    total_deuterium = GREATEST(0, FLOOR(total_deuterium * (1 - decay_rate)))
  WHERE is_active = TRUE
    AND decay_start < NOW() - INTERVAL '1 hour';
  
  -- Mark expired debris as inactive
  UPDATE combat_debris
  SET is_active = FALSE
  WHERE expires_at < NOW() OR (total_metal + total_crystal + total_deuterium) < 100;
  
  GET DIAGNOSTICS v_decayed_count = ROW_COUNT;
  RETURN v_decayed_count;
END;
$$ LANGUAGE plpgsql;

-- ========================================
-- TRIGGERS
-- ========================================

-- Update salvage statistics on salvage completion
CREATE OR REPLACE FUNCTION update_salvage_statistics() RETURNS TRIGGER AS $$
BEGIN
  IF NEW.status = 'completed' AND OLD.status != 'completed' THEN
    INSERT INTO salvage_statistics (user_id, total_salvage_missions, successful_missions)
    VALUES (NEW.user_id, 1, 1)
    ON CONFLICT (user_id) DO UPDATE SET
      total_salvage_missions = salvage_statistics.total_salvage_missions + 1,
      successful_missions = salvage_statistics.successful_missions + 1,
      total_metal_collected = salvage_statistics.total_metal_collected + 
        COALESCE((NEW.resources_collected->>'metal')::BIGINT, 0),
      total_crystal_collected = salvage_statistics.total_crystal_collected + 
        COALESCE((NEW.resources_collected->>'crystal')::BIGINT, 0),
      total_deuterium_collected = salvage_statistics.total_deuterium_collected + 
        COALESCE((NEW.resources_collected->>'deuterium')::BIGINT, 0),
      total_salvage_value = salvage_statistics.total_salvage_value + NEW.total_value,
      last_salvage_at = NOW(),
      updated_at = NOW();
  ELSIF NEW.status = 'failed' AND OLD.status != 'failed' THEN
    INSERT INTO salvage_statistics (user_id, total_salvage_missions, failed_missions)
    VALUES (NEW.user_id, 1, 1)
    ON CONFLICT (user_id) DO UPDATE SET
      total_salvage_missions = salvage_statistics.total_salvage_missions + 1,
      failed_missions = salvage_statistics.failed_missions + 1,
      updated_at = NOW();
  END IF;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_salvage_statistics
AFTER UPDATE ON debris_salvage
FOR EACH ROW
EXECUTE FUNCTION update_salvage_statistics();

-- ========================================
-- INITIAL DATA
-- ========================================

-- Insert sample ship components
INSERT INTO ship_components (component_type, component_name, quality_grade, recycle_value_metal, recycle_value_crystal, market_value, description) VALUES
('engine', 'Basic Combustion Drive', 'common', 1000, 500, 2000, 'Standard propulsion system'),
('engine', 'Advanced Impulse Drive', 'uncommon', 5000, 3000, 12000, 'High-efficiency propulsion'),
('weapon', 'Laser Cannon Array', 'common', 2000, 1000, 4000, 'Standard weapon system'),
('weapon', 'Plasma Turret', 'rare', 10000, 8000, 25000, 'Advanced energy weapon'),
('armor', 'Titanium Plating', 'common', 3000, 500, 5000, 'Basic hull reinforcement'),
('armor', 'Quantum Armor', 'legendary', 50000, 40000, 150000, 'Cutting-edge defensive system'),
('electronics', 'Sensor Array', 'uncommon', 1000, 2000, 5000, 'Advanced detection systems'),
('advanced_material', 'Rare Crystal Core', 'rare', 5000, 20000, 35000, 'Valuable energy source'),
('research_data', 'Blueprint Fragment', 'legendary', 0, 0, 100000, 'Partial ship design data')
ON CONFLICT DO NOTHING;

-- ========================================
-- DEBRIS SYSTEM COMPLETE
-- ========================================
-- Total tables created: 10 new tables
-- Total views: 3 analytical views
-- Total functions: 3 helper functions
-- Total triggers: 1 automatic trigger
-- ========================================
