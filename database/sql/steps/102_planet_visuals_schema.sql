-- Migration 102: Durable planet visual metadata
ALTER TABLE planets ADD COLUMN IF NOT EXISTS visual_seed BIGINT;
ALTER TABLE planets ADD COLUMN IF NOT EXISTS visual_version VARCHAR(64);
ALTER TABLE planets ADD COLUMN IF NOT EXISTS icon_url TEXT;
ALTER TABLE planets ADD COLUMN IF NOT EXISTS banner_url TEXT;
