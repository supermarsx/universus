-- Add phalanx cooldown and daily cap tracking to moons table

ALTER TABLE moons ADD COLUMN IF NOT EXISTS last_scan_time TIMESTAMP;
ALTER TABLE moons ADD COLUMN IF NOT EXISTS daily_scan_count INTEGER DEFAULT 0;
ALTER TABLE moons ADD COLUMN IF NOT EXISTS last_reset_day DATE;