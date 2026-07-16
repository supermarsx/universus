-- Durable authentication sessions, one-time refresh lineage, and login throttling.
-- Raw refresh tokens, IP addresses, and user-agent values are never persisted.

ALTER TABLE users
    ADD COLUMN IF NOT EXISTS auth_epoch BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS privacy_restriction_active BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS privacy_erasure_pending BOOLEAN NOT NULL DEFAULT FALSE;

CREATE UNIQUE INDEX IF NOT EXISTS idx_users_universe_identity
    ON users (universe_id, id);

CREATE TABLE IF NOT EXISTS auth_sessions (
    session_id TEXT PRIMARY KEY,
    family_id TEXT NOT NULL,
    user_id INTEGER NOT NULL,
    universe_id BIGINT NOT NULL,
    auth_epoch_at_issue BIGINT NOT NULL CHECK (auth_epoch_at_issue >= 0),
    device_label TEXT,
    ip_digest BYTEA,
    user_agent_digest BYTEA,
    rotation_counter BIGINT NOT NULL DEFAULT 0 CHECK (rotation_counter >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_used_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    revoke_reason TEXT,
    CONSTRAINT auth_sessions_id_shape CHECK (
        session_id = BTRIM(session_id)
        AND char_length(session_id) BETWEEN 32 AND 128
        AND family_id = BTRIM(family_id)
        AND char_length(family_id) BETWEEN 32 AND 128
    ),
    CONSTRAINT auth_sessions_device_label_bound CHECK (
        device_label IS NULL OR char_length(device_label) BETWEEN 1 AND 128
    ),
    CONSTRAINT auth_sessions_ip_digest_shape CHECK (
        ip_digest IS NULL OR octet_length(ip_digest) = 32
    ),
    CONSTRAINT auth_sessions_user_agent_digest_shape CHECK (
        user_agent_digest IS NULL OR octet_length(user_agent_digest) = 32
    ),
    CONSTRAINT auth_sessions_revocation_shape CHECK (
        (revoked_at IS NULL AND revoke_reason IS NULL)
        OR (revoked_at IS NOT NULL AND NULLIF(BTRIM(revoke_reason), '') IS NOT NULL)
    ),
    CONSTRAINT auth_sessions_expiry_order CHECK (expires_at > created_at),
    CONSTRAINT auth_sessions_user_tenant_fk
        FOREIGN KEY (universe_id, user_id)
        REFERENCES users (universe_id, id)
        ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_auth_sessions_user_active
    ON auth_sessions (user_id, created_at, session_id)
    WHERE revoked_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_auth_sessions_family_active
    ON auth_sessions (family_id, session_id)
    WHERE revoked_at IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_auth_sessions_family_id
    ON auth_sessions (family_id);

CREATE INDEX IF NOT EXISTS idx_auth_sessions_expiry
    ON auth_sessions (expires_at)
    WHERE revoked_at IS NULL;

CREATE TABLE IF NOT EXISTS auth_refresh_tokens (
    token_digest BYTEA PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES auth_sessions(session_id) ON DELETE CASCADE,
    generation BIGINT NOT NULL CHECK (generation >= 0),
    parent_token_digest BYTEA REFERENCES auth_refresh_tokens(token_digest) ON DELETE SET NULL,
    issued_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ,
    replaced_by_digest BYTEA REFERENCES auth_refresh_tokens(token_digest) ON DELETE SET NULL,
    revoked_at TIMESTAMPTZ,
    CONSTRAINT auth_refresh_digest_shape CHECK (octet_length(token_digest) = 32),
    CONSTRAINT auth_refresh_parent_digest_shape CHECK (
        parent_token_digest IS NULL OR octet_length(parent_token_digest) = 32
    ),
    CONSTRAINT auth_refresh_replacement_digest_shape CHECK (
        replaced_by_digest IS NULL OR octet_length(replaced_by_digest) = 32
    ),
    CONSTRAINT auth_refresh_expiry_order CHECK (expires_at > issued_at),
    CONSTRAINT auth_refresh_consumption_shape CHECK (
        replaced_by_digest IS NULL OR consumed_at IS NOT NULL
    ),
    UNIQUE (session_id, generation)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_auth_refresh_one_active_per_session
    ON auth_refresh_tokens (session_id)
    WHERE consumed_at IS NULL AND revoked_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_auth_refresh_session_lineage
    ON auth_refresh_tokens (session_id, generation);

CREATE INDEX IF NOT EXISTS idx_auth_refresh_expiry
    ON auth_refresh_tokens (expires_at)
    WHERE consumed_at IS NULL AND revoked_at IS NULL;

CREATE TABLE IF NOT EXISTS auth_login_throttles (
    scope TEXT NOT NULL CHECK (scope IN ('account', 'ip', 'registration_ip')),
    subject_digest BYTEA NOT NULL CHECK (octet_length(subject_digest) = 32),
    window_started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    failure_count INTEGER NOT NULL DEFAULT 0 CHECK (failure_count >= 0),
    blocked_until TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (scope, subject_digest)
);

CREATE INDEX IF NOT EXISTS idx_auth_login_throttles_cleanup
    ON auth_login_throttles (updated_at, blocked_until);

CREATE OR REPLACE FUNCTION auth_bump_epoch_for_security_change()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF NEW.auth_epoch < OLD.auth_epoch THEN
        RAISE EXCEPTION 'users.auth_epoch cannot decrease';
    END IF;

    IF NEW.password_hash IS DISTINCT FROM OLD.password_hash
       OR NEW.email IS DISTINCT FROM OLD.email
       OR NEW.is_admin IS DISTINCT FROM OLD.is_admin
       OR NEW.is_banned IS DISTINCT FROM OLD.is_banned
       OR NEW.privacy_restriction_active IS DISTINCT FROM OLD.privacy_restriction_active
       OR NEW.privacy_erasure_pending IS DISTINCT FROM OLD.privacy_erasure_pending THEN
        IF NEW.auth_epoch <= OLD.auth_epoch THEN
            NEW.auth_epoch := OLD.auth_epoch + 1;
        END IF;
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS auth_users_security_epoch ON users;
CREATE TRIGGER auth_users_security_epoch
    BEFORE UPDATE OF password_hash, email, is_admin, is_banned,
        privacy_restriction_active, privacy_erasure_pending, auth_epoch
    ON users
    FOR EACH ROW EXECUTE FUNCTION auth_bump_epoch_for_security_change();

CREATE OR REPLACE FUNCTION auth_revoke_sessions_for_epoch_change()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF NEW.auth_epoch IS DISTINCT FROM OLD.auth_epoch THEN
        UPDATE auth_sessions
        SET revoked_at = COALESCE(revoked_at, now()),
            revoke_reason = COALESCE(revoke_reason, 'account_security_epoch_changed')
        WHERE user_id = NEW.id
          AND universe_id = NEW.universe_id
          AND revoked_at IS NULL;

        UPDATE auth_refresh_tokens AS token
        SET revoked_at = COALESCE(token.revoked_at, now())
        FROM auth_sessions AS session
        WHERE session.user_id = NEW.id
          AND session.universe_id = NEW.universe_id
          AND token.session_id = session.session_id
          AND token.revoked_at IS NULL;
    END IF;
    RETURN NULL;
END;
$$;

DROP TRIGGER IF EXISTS auth_users_revoke_sessions ON users;
CREATE TRIGGER auth_users_revoke_sessions
    AFTER UPDATE OF auth_epoch ON users
    FOR EACH ROW EXECUTE FUNCTION auth_revoke_sessions_for_epoch_change();

COMMENT ON TABLE auth_sessions IS
    'Server-authoritative login sessions. Network and agent metadata are digest-only.';
COMMENT ON TABLE auth_refresh_tokens IS
    'One-time refresh-token lineage. Only SHA-256 digests are stored.';
COMMENT ON TABLE auth_login_throttles IS
    'Bounded login failure windows keyed by normalized account and client-IP digests.';
