-- Migration: Update messages table for enhanced messaging service

-- Drop old constraint if exists
ALTER TABLE messages DROP CONSTRAINT IF EXISTS messages_type_check;

-- Rename columns to match service interface
ALTER TABLE messages RENAME COLUMN sender_id TO from_user_id;
ALTER TABLE messages RENAME COLUMN recipient_id TO to_user_id;
ALTER TABLE messages RENAME COLUMN sent_at TO created_at;

-- Add metadata column for combat reports, espionage reports, etc.
ALTER TABLE messages ADD COLUMN IF NOT EXISTS metadata JSONB;

-- Update message type constraint to include new types
ALTER TABLE messages 
  DROP CONSTRAINT IF EXISTS messages_message_type_check,
  ADD CONSTRAINT messages_message_type_check 
    CHECK (message_type IN (
      'player_message',
      'combat_report',
      'espionage_report',
      'system_notification',
      'alliance_message',
      'alliance_circular'
    ));

-- Update existing message types to new format
UPDATE messages SET message_type = 'player_message' WHERE message_type = 'player';
UPDATE messages SET message_type = 'system_notification' WHERE message_type = 'system';
UPDATE messages SET message_type = 'combat_report' WHERE message_type = 'combat';
UPDATE messages SET message_type = 'alliance_message' WHERE message_type = 'alliance';

-- Update indexes
DROP INDEX IF EXISTS idx_messages_recipient_id;
CREATE INDEX IF NOT EXISTS idx_messages_to_user_id ON messages(to_user_id);
CREATE INDEX IF NOT EXISTS idx_messages_from_user_id ON messages(from_user_id);
CREATE INDEX IF NOT EXISTS idx_messages_type ON messages(message_type);
CREATE INDEX IF NOT EXISTS idx_messages_created_at ON messages(created_at DESC);
