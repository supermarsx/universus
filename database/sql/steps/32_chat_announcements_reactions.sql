-- Migration 32: Chat announcements, pinned messages, and reactions
-- Adds metadata columns to chat_messages and introduces chat_message_reactions table

ALTER TABLE chat_messages
    ADD COLUMN IF NOT EXISTS is_announcement BOOLEAN DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS announcement_expires_at TIMESTAMP NULL,
    ADD COLUMN IF NOT EXISTS is_pinned BOOLEAN DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS pinned_by INTEGER REFERENCES users(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS pinned_at TIMESTAMP;

CREATE TABLE IF NOT EXISTS chat_message_reactions (
    id SERIAL PRIMARY KEY,
    message_id INTEGER NOT NULL REFERENCES chat_messages(id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    reaction_type VARCHAR(20) NOT NULL CHECK (reaction_type IN (
        'thumbs_up',
        'thumbs_down',
        'rofl',
        'clap',
        'angry',
        'cry'
    )),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (message_id, user_id, reaction_type)
);

CREATE INDEX IF NOT EXISTS idx_chat_reactions_message ON chat_message_reactions (message_id);
CREATE INDEX IF NOT EXISTS idx_chat_reactions_user ON chat_message_reactions (user_id);
