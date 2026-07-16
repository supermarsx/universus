-- Universus-Inspired Browser RPG Database Schema

-- Users table
CREATE TABLE IF NOT EXISTS users (
    id SERIAL PRIMARY KEY,
    username VARCHAR(50) UNIQUE NOT NULL,
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    dark_matter INTEGER DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    last_login TIMESTAMP,
    is_admin BOOLEAN DEFAULT FALSE,
    is_banned BOOLEAN DEFAULT FALSE,
    alliance_id INTEGER,
    CONSTRAINT users_username_length CHECK (char_length(username) >= 3)
);

-- Alliances table
CREATE TABLE IF NOT EXISTS alliances (
    id SERIAL PRIMARY KEY,
    name VARCHAR(100) UNIQUE NOT NULL,
    tag VARCHAR(10) UNIQUE NOT NULL,
    description TEXT,
    founder_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    total_score BIGINT DEFAULT 0
);

-- Add foreign key for users alliance_id. Legacy installs may already contain
-- the untracked core schema, so guard the named constraint explicitly.
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'users'::regclass AND conname = 'fk_users_alliance'
    ) THEN
        ALTER TABLE users ADD CONSTRAINT fk_users_alliance
            FOREIGN KEY (alliance_id) REFERENCES alliances(id) ON DELETE SET NULL;
    END IF;
END
$$;

-- Planets table
CREATE TABLE IF NOT EXISTS planets (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    galaxy INTEGER NOT NULL,
    system INTEGER NOT NULL,
    position INTEGER NOT NULL,
    planet_type VARCHAR(20) DEFAULT 'planet',
    temperature INTEGER DEFAULT 20,
    diameter INTEGER DEFAULT 12800,
    
    -- Resources
    metal BIGINT DEFAULT 500,
    crystal BIGINT DEFAULT 300,
    deuterium BIGINT DEFAULT 100,
    energy INTEGER DEFAULT 0,
    
    -- Last resource update timestamp for lazy calculation
    last_resource_update TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    -- Buildings levels (0 means not built)
    metal_mine INTEGER DEFAULT 0,
    crystal_mine INTEGER DEFAULT 0,
    deuterium_synthesizer INTEGER DEFAULT 0,
    solar_plant INTEGER DEFAULT 0,
    fusion_reactor INTEGER DEFAULT 0,
    robotics_factory INTEGER DEFAULT 0,
    nanite_factory INTEGER DEFAULT 0,
    shipyard INTEGER DEFAULT 0,
    metal_storage INTEGER DEFAULT 0,
    crystal_storage INTEGER DEFAULT 0,
    deuterium_tank INTEGER DEFAULT 0,
    research_lab INTEGER DEFAULT 0,
    alliance_depot INTEGER DEFAULT 0,
    missile_silo INTEGER DEFAULT 0,
    
    -- Defense counts
    rocket_launcher INTEGER DEFAULT 0,
    light_laser INTEGER DEFAULT 0,
    heavy_laser INTEGER DEFAULT 0,
    gauss_cannon INTEGER DEFAULT 0,
    ion_cannon INTEGER DEFAULT 0,
    plasma_turret INTEGER DEFAULT 0,
    small_shield_dome INTEGER DEFAULT 0,
    large_shield_dome INTEGER DEFAULT 0,
    
    -- Ships counts
    small_cargo INTEGER DEFAULT 0,
    large_cargo INTEGER DEFAULT 0,
    light_fighter INTEGER DEFAULT 0,
    heavy_fighter INTEGER DEFAULT 0,
    cruiser INTEGER DEFAULT 0,
    battleship INTEGER DEFAULT 0,
    colony_ship INTEGER DEFAULT 0,
    recycler INTEGER DEFAULT 0,
    espionage_probe INTEGER DEFAULT 0,
    bomber INTEGER DEFAULT 0,
    destroyer INTEGER DEFAULT 0,
    deathstar INTEGER DEFAULT 0,
    
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(galaxy, system, position),
    CONSTRAINT planets_galaxy_range CHECK (galaxy >= 1 AND galaxy <= 9),
    CONSTRAINT planets_system_range CHECK (system >= 1 AND system <= 499),
    CONSTRAINT planets_position_range CHECK (position >= 1 AND position <= 15)
);

-- Research table (per user, not per planet)
CREATE TABLE IF NOT EXISTS research (
    user_id INTEGER PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    
    -- Research levels
    energy_technology INTEGER DEFAULT 0,
    laser_technology INTEGER DEFAULT 0,
    ion_technology INTEGER DEFAULT 0,
    hyperspace_technology INTEGER DEFAULT 0,
    plasma_technology INTEGER DEFAULT 0,
    combustion_drive INTEGER DEFAULT 0,
    impulse_drive INTEGER DEFAULT 0,
    hyperspace_drive INTEGER DEFAULT 0,
    espionage_technology INTEGER DEFAULT 0,
    computer_technology INTEGER DEFAULT 0,
    astrophysics INTEGER DEFAULT 0,
    intergalactic_research_network INTEGER DEFAULT 0,
    graviton_technology INTEGER DEFAULT 0,
    weapons_technology INTEGER DEFAULT 0,
    shielding_technology INTEGER DEFAULT 0,
    armor_technology INTEGER DEFAULT 0,
    
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Construction queue (buildings being built)
CREATE TABLE IF NOT EXISTS construction_queue (
    id SERIAL PRIMARY KEY,
    planet_id INTEGER NOT NULL REFERENCES planets(id) ON DELETE CASCADE,
    building_type VARCHAR(50) NOT NULL,
    level INTEGER NOT NULL,
    start_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    end_time TIMESTAMP NOT NULL,
    metal_cost BIGINT NOT NULL,
    crystal_cost BIGINT NOT NULL,
    deuterium_cost BIGINT NOT NULL
);

-- Research queue (research being conducted)
CREATE TABLE IF NOT EXISTS research_queue (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    planet_id INTEGER NOT NULL REFERENCES planets(id) ON DELETE CASCADE,
    research_type VARCHAR(50) NOT NULL,
    level INTEGER NOT NULL,
    start_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    end_time TIMESTAMP NOT NULL,
    metal_cost BIGINT NOT NULL,
    crystal_cost BIGINT NOT NULL,
    deuterium_cost BIGINT NOT NULL
);

-- Shipyard queue (ships/defenses being built)
CREATE TABLE IF NOT EXISTS shipyard_queue (
    id SERIAL PRIMARY KEY,
    planet_id INTEGER NOT NULL REFERENCES planets(id) ON DELETE CASCADE,
    unit_type VARCHAR(50) NOT NULL,
    quantity INTEGER NOT NULL,
    start_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    end_time TIMESTAMP NOT NULL,
    metal_cost BIGINT NOT NULL,
    crystal_cost BIGINT NOT NULL,
    deuterium_cost BIGINT NOT NULL
);

-- Fleets (missions in progress)
CREATE TABLE IF NOT EXISTS fleets (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    mission_type VARCHAR(20) NOT NULL,
    
    -- Origin and destination
    origin_planet_id INTEGER NOT NULL REFERENCES planets(id),
    target_galaxy INTEGER NOT NULL,
    target_system INTEGER NOT NULL,
    target_position INTEGER NOT NULL,
    
    -- Timing
    departure_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    arrival_time TIMESTAMP NOT NULL,
    return_time TIMESTAMP,
    
    -- Fleet composition (stored as JSON)
    ships JSONB NOT NULL,
    
    -- Cargo
    cargo_metal BIGINT DEFAULT 0,
    cargo_crystal BIGINT DEFAULT 0,
    cargo_deuterium BIGINT DEFAULT 0,
    
    -- Status
    status VARCHAR(20) DEFAULT 'outbound',
    
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT fleets_mission_type_check CHECK (mission_type IN ('attack', 'transport', 'deploy', 'espionage', 'colonize', 'harvest', 'acs_attack', 'acs_defend'))
);

-- Combat reports
CREATE TABLE IF NOT EXISTS combat_reports (
    id SERIAL PRIMARY KEY,
    attacker_id INTEGER NOT NULL REFERENCES users(id),
    defender_id INTEGER REFERENCES users(id),
    planet_galaxy INTEGER NOT NULL,
    planet_system INTEGER NOT NULL,
    planet_position INTEGER NOT NULL,
    
    -- Battle data (stored as JSON)
    rounds JSONB NOT NULL,
    winner VARCHAR(20) NOT NULL,
    
    -- Losses
    attacker_losses JSONB NOT NULL,
    defender_losses JSONB NOT NULL,
    
    -- Loot
    loot_metal BIGINT DEFAULT 0,
    loot_crystal BIGINT DEFAULT 0,
    loot_deuterium BIGINT DEFAULT 0,
    
    -- Debris
    debris_metal BIGINT DEFAULT 0,
    debris_crystal BIGINT DEFAULT 0,
    
    battle_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT combat_reports_winner_check CHECK (winner IN ('attacker', 'defender', 'draw'))
);

-- Messages
CREATE TABLE IF NOT EXISTS messages (
    id SERIAL PRIMARY KEY,
    sender_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
    recipient_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    subject VARCHAR(255) NOT NULL,
    content TEXT NOT NULL,
    message_type VARCHAR(20) DEFAULT 'player',
    is_read BOOLEAN DEFAULT FALSE,
    combat_report_id INTEGER REFERENCES combat_reports(id) ON DELETE CASCADE,
    sent_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT messages_type_check CHECK (message_type IN ('player', 'system', 'combat', 'alliance'))
);

-- Alliance members (for tracking roles)
CREATE TABLE IF NOT EXISTS alliance_members (
    alliance_id INTEGER NOT NULL REFERENCES alliances(id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role VARCHAR(20) DEFAULT 'member',
    joined_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (alliance_id, user_id),
    CONSTRAINT alliance_members_role_check CHECK (role IN ('founder', 'leader', 'officer', 'member'))
);

-- Alliance chat
CREATE TABLE IF NOT EXISTS alliance_chat (
    id SERIAL PRIMARY KEY,
    alliance_id INTEGER NOT NULL REFERENCES alliances(id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    message TEXT NOT NULL,
    sent_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Debris fields
CREATE TABLE IF NOT EXISTS debris_fields (
    id SERIAL PRIMARY KEY,
    galaxy INTEGER NOT NULL,
    system INTEGER NOT NULL,
    position INTEGER NOT NULL,
    metal BIGINT DEFAULT 0,
    crystal BIGINT DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(galaxy, system, position)
);

-- Player scores (for leaderboard)
CREATE TABLE IF NOT EXISTS player_scores (
    user_id INTEGER PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    total_score BIGINT DEFAULT 0,
    economy_score BIGINT DEFAULT 0,
    research_score BIGINT DEFAULT 0,
    military_score BIGINT DEFAULT 0,
    last_updated TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_planets_user_id ON planets(user_id);
CREATE INDEX IF NOT EXISTS idx_planets_coordinates ON planets(galaxy, system, position);
CREATE INDEX IF NOT EXISTS idx_fleets_user_id ON fleets(user_id);
CREATE INDEX IF NOT EXISTS idx_fleets_arrival_time ON fleets(arrival_time);
CREATE INDEX IF NOT EXISTS idx_fleets_status ON fleets(status);
CREATE INDEX IF NOT EXISTS idx_construction_queue_planet_id ON construction_queue(planet_id);
CREATE INDEX IF NOT EXISTS idx_construction_queue_end_time ON construction_queue(end_time);
CREATE INDEX IF NOT EXISTS idx_research_queue_user_id ON research_queue(user_id);
CREATE INDEX IF NOT EXISTS idx_messages_recipient_id ON messages(recipient_id);
CREATE INDEX IF NOT EXISTS idx_messages_is_read ON messages(is_read);
CREATE INDEX IF NOT EXISTS idx_combat_reports_attacker_id ON combat_reports(attacker_id);
CREATE INDEX IF NOT EXISTS idx_combat_reports_defender_id ON combat_reports(defender_id);
CREATE INDEX IF NOT EXISTS idx_alliance_members_user_id ON alliance_members(user_id);
CREATE INDEX IF NOT EXISTS idx_player_scores_total_score ON player_scores(total_score DESC);
