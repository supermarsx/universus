-- Migration 40: Add last_jump_time for Jump Gate cooldown
ALTER TABLE moons
    ADD COLUMN IF NOT EXISTS last_jump_time TIMESTAMP;
