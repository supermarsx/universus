-- Achievements, Badges, Rewards, Ladders, and Hall of Fame Schema

-- Badges table
CREATE TABLE IF NOT EXISTS badges (
    id SERIAL PRIMARY KEY,
    code VARCHAR(64) UNIQUE NOT NULL, -- e.g. 'VETERAN'
    name VARCHAR(128) NOT NULL,
    description TEXT NOT NULL,
    icon_url VARCHAR(255),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Rewards table
CREATE TABLE IF NOT EXISTS rewards (
    id SERIAL PRIMARY KEY,
    code VARCHAR(64) UNIQUE NOT NULL, -- e.g. '1000_DARK_MATTER'
    name VARCHAR(128) NOT NULL,
    description TEXT NOT NULL,
    reward_type VARCHAR(32) NOT NULL, -- e.g. 'currency', 'item', 'title'
    value INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Achievements reference badges and rewards, so their dependencies must exist
-- first. Migrations never drop these durable player-progression tables.
CREATE TABLE IF NOT EXISTS achievements (
    id SERIAL PRIMARY KEY,
    code VARCHAR(64) UNIQUE NOT NULL, -- e.g. 'FIRST_BLOOD'
    name VARCHAR(128) NOT NULL,
    description TEXT NOT NULL,
    points INTEGER DEFAULT 0,
    badge_id INTEGER REFERENCES badges(id),
    reward_id INTEGER REFERENCES rewards(id),
    is_secret BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- User Achievements table
CREATE TABLE IF NOT EXISTS user_achievements (
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    achievement_id INTEGER NOT NULL REFERENCES achievements(id) ON DELETE CASCADE,
    progress INTEGER DEFAULT 0,
    unlocked_at TIMESTAMP,
    PRIMARY KEY (user_id, achievement_id)
);

-- User Badges table
CREATE TABLE IF NOT EXISTS user_badges (
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    badge_id INTEGER NOT NULL REFERENCES badges(id) ON DELETE CASCADE,
    earned_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (user_id, badge_id)
);

-- User Rewards table
CREATE TABLE IF NOT EXISTS user_rewards (
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    reward_id INTEGER NOT NULL REFERENCES rewards(id) ON DELETE CASCADE,
    granted_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (user_id, reward_id)
);

-- Ladders table
CREATE TABLE IF NOT EXISTS ladders (
    id SERIAL PRIMARY KEY,
    code VARCHAR(64) UNIQUE NOT NULL, -- e.g. 'PVP_SEASON_1'
    name VARCHAR(128) NOT NULL,
    description TEXT NOT NULL,
    start_time TIMESTAMP NOT NULL,
    end_time TIMESTAMP NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Ladder Entries table
CREATE TABLE IF NOT EXISTS ladder_entries (
    ladder_id INTEGER NOT NULL REFERENCES ladders(id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    score BIGINT DEFAULT 0,
    rank INTEGER,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (ladder_id, user_id)
);

-- Hall of Fame table
CREATE TABLE IF NOT EXISTS hall_of_fame (
    id SERIAL PRIMARY KEY,
    ladder_id INTEGER REFERENCES ladders(id) ON DELETE SET NULL,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    achievement_id INTEGER REFERENCES achievements(id) ON DELETE SET NULL,
    badge_id INTEGER REFERENCES badges(id) ON DELETE SET NULL,
    reward_id INTEGER REFERENCES rewards(id) ON DELETE SET NULL,
    score BIGINT,
    rank INTEGER,
    season VARCHAR(32),
    inducted_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_user_achievements_user_id ON user_achievements(user_id);
CREATE INDEX IF NOT EXISTS idx_user_badges_user_id ON user_badges(user_id);
CREATE INDEX IF NOT EXISTS idx_user_rewards_user_id ON user_rewards(user_id);
CREATE INDEX IF NOT EXISTS idx_ladder_entries_ladder_id ON ladder_entries(ladder_id);
CREATE INDEX IF NOT EXISTS idx_hall_of_fame_user_id ON hall_of_fame(user_id);
