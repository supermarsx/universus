-- Migration 28: Player blocking + shadow bans

BEGIN;

CREATE TABLE IF NOT EXISTS player_blocks (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    blocked_user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    block_scope VARCHAR(20) NOT NULL DEFAULT 'all' CHECK (block_scope IN ('all', 'chat', 'messages')),
    reason TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMP,
    UNIQUE(user_id, blocked_user_id, block_scope)
);

CREATE INDEX IF NOT EXISTS idx_player_blocks_user ON player_blocks(user_id);
CREATE INDEX IF NOT EXISTS idx_player_blocks_blocked ON player_blocks(blocked_user_id);

ALTER TABLE chat_restrictions
    DROP CONSTRAINT IF EXISTS chat_restrictions_restriction_type_check;

ALTER TABLE chat_restrictions
    ADD CONSTRAINT chat_restrictions_restriction_type_check
    CHECK (restriction_type IN ('mute', 'ban', 'slowmode', 'shadow'));

COMMIT;
