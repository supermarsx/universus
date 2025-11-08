-- Migration 25: Add custom CSS support to theme preferences
-- Provides storage for user-supplied CSS snippets with audit metadata

BEGIN;

ALTER TABLE theme_preferences
    ADD COLUMN IF NOT EXISTS custom_css TEXT,
    ADD COLUMN IF NOT EXISTS custom_css_updated_at TIMESTAMP;

-- Ensure existing rows have a timestamp for consistency
UPDATE theme_preferences
SET custom_css_updated_at = COALESCE(custom_css_updated_at, updated_at)
WHERE custom_css IS NOT NULL;

COMMIT;
