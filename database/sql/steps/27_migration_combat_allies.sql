-- Migration 27: Track attacker allies in combat reports

BEGIN;

ALTER TABLE combat_reports
    ADD COLUMN IF NOT EXISTS attacker_allies JSONB DEFAULT '[]'::jsonb;

COMMIT;
