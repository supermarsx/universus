-- Durable authentication account constraints used by app-api-gateway.
-- Usable password values stored in users.password_hash are Argon2id PHC strings.

ALTER TABLE users
    ADD COLUMN IF NOT EXISTS password_hash TEXT,
    ADD COLUMN IF NOT EXISTS last_login TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS is_admin BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS is_banned BOOLEAN NOT NULL DEFAULT FALSE;

-- A legacy nullable credential must deny login until a password-reset flow;
-- never invent or silently assign a usable password during migration.
UPDATE users
SET password_hash = '!password-reset-required!'
WHERE password_hash IS NULL OR BTRIM(password_hash) = '';

UPDATE users SET is_admin = FALSE WHERE is_admin IS NULL;
UPDATE users SET is_banned = FALSE WHERE is_banned IS NULL;

ALTER TABLE users
    ALTER COLUMN password_hash SET NOT NULL,
    ALTER COLUMN is_admin SET DEFAULT FALSE,
    ALTER COLUMN is_admin SET NOT NULL,
    ALTER COLUMN is_banned SET DEFAULT FALSE,
    ALTER COLUMN is_banned SET NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_users_username_normalized
    ON users (LOWER(BTRIM(username)));

CREATE UNIQUE INDEX IF NOT EXISTS idx_users_email_normalized
    ON users (LOWER(BTRIM(email)));

CREATE INDEX IF NOT EXISTS idx_users_last_login_auth
    ON users (last_login DESC);
