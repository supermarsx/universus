-- =====================================================
-- PHASE 4.1: UNIVERSE SEEDING SYSTEM EXTENSIONS (MULTI-UNIVERSE MGMT)
-- Schema changes for advanced universe lifecycle, registration, speed, merging, and announcements
-- Added: 2025-11-11
-- =====================================================

-- Add registration and lifecycle management fields
ALTER TABLE universe_seeds
    ADD COLUMN IF NOT EXISTS registration_status VARCHAR(20) DEFAULT 'open' CHECK (registration_status IN ('open', 'closed', 'scheduled', 'paused')),
    ADD COLUMN IF NOT EXISTS registration_open_at TIMESTAMP WITH TIME ZONE,
    ADD COLUMN IF NOT EXISTS registration_close_at TIMESTAMP WITH TIME ZONE,
    ADD COLUMN IF NOT EXISTS universe_open_at TIMESTAMP WITH TIME ZONE,
    ADD COLUMN IF NOT EXISTS universe_close_at TIMESTAMP WITH TIME ZONE,
    ADD COLUMN IF NOT EXISTS is_active BOOLEAN DEFAULT TRUE,
    ADD COLUMN IF NOT EXISTS closure_reason TEXT;

-- Add speed/progression fields
ALTER TABLE universe_seeds
    ADD COLUMN IF NOT EXISTS speed_multiplier DECIMAL(5,2) DEFAULT 1.00 CHECK (speed_multiplier > 0),
    ADD COLUMN IF NOT EXISTS speed_progression_type VARCHAR(20) DEFAULT 'static' CHECK (speed_progression_type IN ('static', 'scheduled', 'dynamic', 'decreasing')),
    ADD COLUMN IF NOT EXISTS speed_schedule JSONB,
    -- Detached building/research speed
    ADD COLUMN IF NOT EXISTS building_speed_multiplier DECIMAL(5,2) DEFAULT 1.00 CHECK (building_speed_multiplier > 0),
    ADD COLUMN IF NOT EXISTS research_speed_multiplier DECIMAL(5,2) DEFAULT 1.00 CHECK (research_speed_multiplier > 0),
    ADD COLUMN IF NOT EXISTS building_speed_schedule JSONB,
    ADD COLUMN IF NOT EXISTS research_speed_schedule JSONB;

-- Add base rations for storage and production
ALTER TABLE universe_seeds
    ADD COLUMN IF NOT EXISTS base_storage_ration JSONB,
    ADD COLUMN IF NOT EXISTS base_production_ration JSONB;

-- Add merging/end-of-universe fields
ALTER TABLE universe_seeds
    ADD COLUMN IF NOT EXISTS is_merging BOOLEAN DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS merge_target_universe_id INTEGER REFERENCES universe_seeds(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS merge_scheduled_at TIMESTAMP WITH TIME ZONE,
    ADD COLUMN IF NOT EXISTS end_of_universe_event_at TIMESTAMP WITH TIME ZONE,
    ADD COLUMN IF NOT EXISTS end_of_universe_type VARCHAR(20) CHECK (end_of_universe_type IN ('shutdown', 'merge', 'archive', 'other')),
    ADD COLUMN IF NOT EXISTS end_of_universe_announcement TEXT;

-- Add announcement/event fields
ALTER TABLE universe_seeds
    ADD COLUMN IF NOT EXISTS announcement TEXT,
    ADD COLUMN IF NOT EXISTS announcement_type VARCHAR(20) CHECK (announcement_type IN ('info', 'warning', 'event', 'closure')),
    ADD COLUMN IF NOT EXISTS announcement_expires_at TIMESTAMP WITH TIME ZONE;

-- Indexes for new fields
CREATE INDEX IF NOT EXISTS idx_universe_registration_status ON universe_seeds(registration_status);
CREATE INDEX IF NOT EXISTS idx_universe_active ON universe_seeds(is_active);
CREATE INDEX IF NOT EXISTS idx_universe_merging ON universe_seeds(is_merging);
CREATE INDEX IF NOT EXISTS idx_universe_merge_target ON universe_seeds(merge_target_universe_id);
CREATE INDEX IF NOT EXISTS idx_universe_end_event ON universe_seeds(end_of_universe_event_at);
CREATE INDEX IF NOT EXISTS idx_universe_announcement_type ON universe_seeds(announcement_type);

-- =====================================================
-- END PHASE 4.1 SCHEMA EXTENSIONS
-- =====================================================
