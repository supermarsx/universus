-- Durable, privacy-enforced outbound communications.
--
-- Message bodies and raw destinations are deliberately absent. Jobs reference
-- server-owned templates and authoritative event identifiers. Raw contact data
-- is resolved from the account immediately before dispatch; only keyed HMAC
-- evidence and a masked display value may be retained by this subsystem.

ALTER TABLE users
    ADD COLUMN IF NOT EXISTS email_verified BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS phone_number VARCHAR(32),
    ADD COLUMN IF NOT EXISTS phone_verified BOOLEAN NOT NULL DEFAULT FALSE;

UPDATE users SET email_verified = FALSE WHERE email_verified IS NULL;
UPDATE users SET phone_verified = FALSE WHERE phone_verified IS NULL;
UPDATE users SET phone_verified = FALSE WHERE phone_number IS NULL OR BTRIM(phone_number) = '';

ALTER TABLE users
    ALTER COLUMN email_verified SET DEFAULT FALSE,
    ALTER COLUMN email_verified SET NOT NULL,
    ALTER COLUMN phone_verified SET DEFAULT FALSE,
    ALTER COLUMN phone_verified SET NOT NULL;

-- Canonical cross-domain legal-hold predicate. Migration 52 owns the source
-- lifecycle rows; defining the predicate here keeps communication retention
-- independently safe before later privacy extensions are deployed.
CREATE OR REPLACE FUNCTION privacy_subject_has_active_legal_hold(
    target_universe_id BIGINT,
    target_user_id INTEGER
)
RETURNS BOOLEAN
LANGUAGE SQL
STABLE
AS $$
    SELECT CASE
        WHEN target_universe_id IS NULL OR target_universe_id <= 0
          OR target_user_id IS NULL OR target_user_id <= 0
            THEN TRUE
        ELSE EXISTS (
            SELECT 1
            FROM gdpr_requests
            WHERE universe_id = target_universe_id
              AND user_id = target_user_id
              AND legal_hold_active = TRUE
        )
    END
$$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'users'::regclass
          AND conname = 'users_verified_email_requires_contact'
    ) THEN
        ALTER TABLE users ADD CONSTRAINT users_verified_email_requires_contact
            CHECK (NOT email_verified OR NULLIF(BTRIM(email), '') IS NOT NULL);
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'users'::regclass
          AND conname = 'users_verified_phone_requires_e164_contact'
    ) THEN
        ALTER TABLE users ADD CONSTRAINT users_verified_phone_requires_e164_contact
            CHECK (
                NOT phone_verified
                OR phone_number ~ '^\+[1-9][0-9]{6,14}$'
            );
    END IF;
END $$;

CREATE TABLE IF NOT EXISTS communication_verified_contacts (
    universe_id BIGINT NOT NULL,
    user_id INTEGER NOT NULL,
    channel TEXT NOT NULL CHECK (channel IN ('email', 'sms')),
    destination_hmac BYTEA NOT NULL CHECK (octet_length(destination_hmac) = 32),
    destination_masked TEXT NOT NULL CHECK (
        char_length(destination_masked) BETWEEN 3 AND 96
        AND destination_masked = BTRIM(destination_masked)
    ),
    verification_method TEXT NOT NULL CHECK (
        verification_method ~ '^[a-z0-9][a-z0-9_.:-]{1,63}$'
    ),
    verified_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    retention_until TIMESTAMPTZ NOT NULL DEFAULT (now() + INTERVAL '90 days'),
    PRIMARY KEY (universe_id, user_id, channel),
    CONSTRAINT communication_verified_contact_user_fk
        FOREIGN KEY (universe_id, user_id)
        REFERENCES users (universe_id, id) ON DELETE CASCADE,
    CONSTRAINT communication_verified_contact_expiry CHECK (expires_at > verified_at),
    CONSTRAINT communication_verified_contact_retention CHECK (retention_until >= verified_at)
);

CREATE INDEX IF NOT EXISTS idx_communication_verified_contacts_active
    ON communication_verified_contacts (universe_id, user_id, channel, expires_at)
    WHERE revoked_at IS NULL;

CREATE TABLE IF NOT EXISTS communication_contact_versions (
    universe_id BIGINT NOT NULL,
    user_id INTEGER NOT NULL,
    channel TEXT NOT NULL CHECK (channel IN ('email', 'sms')),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (universe_id, user_id, channel),
    CONSTRAINT communication_contact_version_user_fk
        FOREIGN KEY (universe_id, user_id)
        REFERENCES users (universe_id, id) ON DELETE CASCADE
);

CREATE OR REPLACE FUNCTION communication_invalidate_contact_change()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE
    actor_hmac_hex TEXT;
    change_reason TEXT;
    next_version BIGINT;
BEGIN
    actor_hmac_hex := current_setting('app.communication_actor_subject_hmac', TRUE);
    change_reason := current_setting('app.communication_change_reason', TRUE);
    IF actor_hmac_hex IS NULL OR actor_hmac_hex !~ '^[0-9a-f]{64}$'
       OR change_reason IS NULL
       OR change_reason !~ '^[a-z][a-z0-9_.:-]{1,63}$' THEN
        RAISE EXCEPTION 'communication contact changes require audited actor context'
            USING ERRCODE = '42501';
    END IF;

    IF NEW.email IS DISTINCT FROM OLD.email THEN
        NEW.email_verified := FALSE;
        INSERT INTO communication_contact_versions (universe_id, user_id, channel)
        VALUES (OLD.universe_id, OLD.id, 'email')
        ON CONFLICT (universe_id, user_id, channel) DO UPDATE
        SET version = communication_contact_versions.version + 1, updated_at = now()
        RETURNING version INTO next_version;
        UPDATE communication_verified_contacts
        SET revoked_at = COALESCE(revoked_at, now()), version = next_version
        WHERE universe_id = OLD.universe_id AND user_id = OLD.id
          AND channel = 'email' AND revoked_at IS NULL;
        INSERT INTO communication_control_events (
            universe_id, user_id, control_type, channel, action, reason_code,
            control_version, actor_subject_hmac
        ) VALUES (
            OLD.universe_id, OLD.id, 'verified_contact', 'email', 'revoked',
            change_reason, next_version, decode(actor_hmac_hex, 'hex')
        );
    END IF;
    IF NEW.phone_number IS DISTINCT FROM OLD.phone_number THEN
        NEW.phone_verified := FALSE;
        INSERT INTO communication_contact_versions (universe_id, user_id, channel)
        VALUES (OLD.universe_id, OLD.id, 'sms')
        ON CONFLICT (universe_id, user_id, channel) DO UPDATE
        SET version = communication_contact_versions.version + 1, updated_at = now()
        RETURNING version INTO next_version;
        UPDATE communication_verified_contacts
        SET revoked_at = COALESCE(revoked_at, now()), version = next_version
        WHERE universe_id = OLD.universe_id AND user_id = OLD.id
          AND channel = 'sms' AND revoked_at IS NULL;
        INSERT INTO communication_control_events (
            universe_id, user_id, control_type, channel, action, reason_code,
            control_version, actor_subject_hmac
        ) VALUES (
            OLD.universe_id, OLD.id, 'verified_contact', 'sms', 'revoked',
            change_reason, next_version, decode(actor_hmac_hex, 'hex')
        );
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS communication_users_contact_change ON users;
CREATE TRIGGER communication_users_contact_change
    BEFORE UPDATE OF email, phone_number ON users
    FOR EACH ROW EXECUTE FUNCTION communication_invalidate_contact_change();

CREATE TABLE IF NOT EXISTS communication_templates (
    channel TEXT NOT NULL CHECK (channel IN ('email', 'sms')),
    template_key TEXT NOT NULL CHECK (
        template_key ~ '^[a-z][a-z0-9_.-]{1,63}$'
    ),
    category TEXT NOT NULL CHECK (category IN (
        'marketing', 'product_updates', 'gameplay_digest', 'security', 'transactional'
    )),
    provider_template_key TEXT NOT NULL CHECK (
        provider_template_key ~ '^[A-Za-z0-9][A-Za-z0-9_.:-]{1,127}$'
    ),
    active BOOLEAN NOT NULL DEFAULT TRUE,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (channel, template_key)
);

INSERT INTO communication_templates (channel, template_key, category, provider_template_key)
VALUES
    ('email', 'welcome', 'transactional', 'universus.email.welcome.v1'),
    ('email', 'password_reset', 'security', 'universus.email.password_reset.v1'),
    ('email', 'account_verification', 'security', 'universus.email.account_verification.v1'),
    ('email', 'fleet_arrival', 'gameplay_digest', 'universus.email.fleet_arrival.v1'),
    ('email', 'attack_incoming', 'gameplay_digest', 'universus.email.attack_incoming.v1'),
    ('email', 'alliance_invite', 'gameplay_digest', 'universus.email.alliance_invite.v1'),
    ('sms', 'password_reset', 'security', 'universus.sms.password_reset.v1'),
    ('sms', 'account_verification', 'security', 'universus.sms.account_verification.v1'),
    ('sms', 'attack_incoming', 'gameplay_digest', 'universus.sms.attack_incoming.v1')
ON CONFLICT (channel, template_key) DO UPDATE
SET category = EXCLUDED.category,
    provider_template_key = EXCLUDED.provider_template_key,
    updated_at = now();

CREATE TABLE IF NOT EXISTS communication_channel_policies (
    universe_id BIGINT NOT NULL REFERENCES universes(id) ON DELETE CASCADE,
    channel TEXT NOT NULL CHECK (channel IN ('email', 'sms')),
    category TEXT NOT NULL CHECK (category IN (
        'marketing', 'product_updates', 'gameplay_digest', 'security', 'transactional'
    )),
    provider_key TEXT NOT NULL CHECK (
        provider_key ~ '^[a-z][a-z0-9_.-]{1,63}$'
    ),
    enabled BOOLEAN NOT NULL DEFAULT FALSE,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (universe_id, channel, category)
);

CREATE TABLE IF NOT EXISTS communication_outbox (
    id BIGSERIAL PRIMARY KEY,
    universe_id BIGINT NOT NULL,
    user_id INTEGER NOT NULL,
    channel TEXT NOT NULL CHECK (channel IN ('email', 'sms')),
    category TEXT NOT NULL CHECK (category IN (
        'marketing', 'product_updates', 'gameplay_digest', 'security', 'transactional'
    )),
    template_key TEXT NOT NULL,
    payload_identity TEXT NOT NULL CHECK (
        payload_identity ~ '^(account_event|game_event|security_event|transaction):[0-9a-fA-F-]{1,64}$'
    ),
    idempotency_key TEXT NOT NULL CHECK (
        idempotency_key ~ '^[A-Za-z0-9][A-Za-z0-9_.:-]{7,127}$'
    ),
    state TEXT NOT NULL DEFAULT 'pending' CHECK (state IN (
        'pending', 'leased', 'retry', 'sent', 'dead', 'suppressed'
    )),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    max_attempts INTEGER NOT NULL DEFAULT 5 CHECK (max_attempts BETWEEN 1 AND 20),
    available_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    lease_owner TEXT,
    lease_until TIMESTAMPTZ,
    provider_key TEXT,
    provider_message_hmac BYTEA CHECK (
        provider_message_hmac IS NULL OR octet_length(provider_message_hmac) = 32
    ),
    destination_hmac BYTEA CHECK (
        destination_hmac IS NULL OR octet_length(destination_hmac) = 32
    ),
    destination_masked TEXT CHECK (
        destination_masked IS NULL OR char_length(destination_masked) BETWEEN 3 AND 96
    ),
    last_reason_code TEXT CHECK (
        last_reason_code IS NULL
        OR last_reason_code ~ '^[a-z][a-z0-9_.:-]{1,63}$'
    ),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    sent_at TIMESTAMPTZ,
    terminal_at TIMESTAMPTZ,
    retention_until TIMESTAMPTZ NOT NULL DEFAULT (now() + INTERVAL '90 days'),
    CONSTRAINT communication_outbox_user_fk
        FOREIGN KEY (universe_id, user_id)
        REFERENCES users (universe_id, id) ON DELETE RESTRICT,
    CONSTRAINT communication_outbox_template_fk
        FOREIGN KEY (channel, template_key)
        REFERENCES communication_templates (channel, template_key) ON DELETE RESTRICT,
    CONSTRAINT communication_outbox_lease_shape CHECK (
        (state = 'leased' AND NULLIF(BTRIM(lease_owner), '') IS NOT NULL AND lease_until IS NOT NULL)
        OR (state <> 'leased' AND lease_owner IS NULL AND lease_until IS NULL)
    ),
    CONSTRAINT communication_outbox_sent_shape CHECK (
        (state = 'sent' AND sent_at IS NOT NULL AND terminal_at IS NOT NULL)
        OR (state <> 'sent' AND sent_at IS NULL)
    ),
    CONSTRAINT communication_outbox_terminal_shape CHECK (
        (state IN ('sent', 'dead', 'suppressed') AND terminal_at IS NOT NULL)
        OR (state NOT IN ('sent', 'dead', 'suppressed') AND terminal_at IS NULL)
    ),
    UNIQUE (id, universe_id),
    UNIQUE (universe_id, user_id, channel, idempotency_key)
);

CREATE INDEX IF NOT EXISTS idx_communication_outbox_claim
    ON communication_outbox (channel, available_at, id)
    WHERE state IN ('pending', 'retry');
CREATE INDEX IF NOT EXISTS idx_communication_outbox_reclaim
    ON communication_outbox (channel, lease_until, id)
    WHERE state = 'leased';
CREATE INDEX IF NOT EXISTS idx_communication_outbox_owner
    ON communication_outbox (universe_id, user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_communication_outbox_retention
    ON communication_outbox (retention_until)
    WHERE destination_hmac IS NOT NULL OR destination_masked IS NOT NULL;

CREATE TABLE IF NOT EXISTS communication_outbox_events (
    id BIGSERIAL PRIMARY KEY,
    outbox_id BIGINT NOT NULL,
    universe_id BIGINT NOT NULL,
    channel TEXT NOT NULL CHECK (channel IN ('email', 'sms')),
    category TEXT NOT NULL CHECK (category IN (
        'marketing', 'product_updates', 'gameplay_digest', 'security', 'transactional'
    )),
    event_type TEXT NOT NULL CHECK (event_type IN (
        'enqueued', 'leased', 'lease_reclaimed', 'retry_scheduled',
        'sent', 'dead', 'suppressed', 'contact_evidence_redacted'
    )),
    state TEXT NOT NULL CHECK (state IN (
        'pending', 'leased', 'retry', 'sent', 'dead', 'suppressed'
    )),
    reason_code TEXT CHECK (
        reason_code IS NULL OR reason_code ~ '^[a-z][a-z0-9_.:-]{1,63}$'
    ),
    attempt INTEGER NOT NULL CHECK (attempt >= 0),
    actor_subject_hmac BYTEA NOT NULL CHECK (octet_length(actor_subject_hmac) = 32),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    retention_until TIMESTAMPTZ NOT NULL DEFAULT (now() + INTERVAL '400 days'),
    CONSTRAINT communication_outbox_event_tenant_fk
        FOREIGN KEY (outbox_id, universe_id)
        REFERENCES communication_outbox (id, universe_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS communication_control_events (
    id BIGSERIAL PRIMARY KEY,
    universe_id BIGINT NOT NULL,
    user_id INTEGER,
    control_type TEXT NOT NULL CHECK (control_type IN ('channel_policy', 'verified_contact')),
    channel TEXT NOT NULL CHECK (channel IN ('email', 'sms')),
    category TEXT CHECK (category IS NULL OR category IN (
        'marketing', 'product_updates', 'gameplay_digest', 'security', 'transactional'
    )),
    action TEXT NOT NULL CHECK (action IN ('enabled', 'disabled', 'verified', 'revoked')),
    reason_code TEXT NOT NULL CHECK (reason_code ~ '^[a-z][a-z0-9_.:-]{1,63}$'),
    control_version BIGINT NOT NULL CHECK (control_version > 0),
    actor_subject_hmac BYTEA NOT NULL CHECK (octet_length(actor_subject_hmac) = 32),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    retention_until TIMESTAMPTZ NOT NULL DEFAULT (now() + INTERVAL '400 days'),
    CONSTRAINT communication_control_event_shape CHECK (
        (control_type = 'channel_policy'
            AND user_id IS NULL AND category IS NOT NULL
            AND action IN ('enabled', 'disabled'))
        OR
        (control_type = 'verified_contact'
            AND user_id IS NOT NULL AND category IS NULL
            AND action IN ('verified', 'revoked'))
    )
);

CREATE INDEX IF NOT EXISTS idx_communication_control_events_tenant
    ON communication_control_events (universe_id, control_type, channel, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_communication_control_events_retention
    ON communication_control_events (retention_until);

CREATE INDEX IF NOT EXISTS idx_communication_events_outbox
    ON communication_outbox_events (outbox_id, id);
CREATE INDEX IF NOT EXISTS idx_communication_events_aggregate
    ON communication_outbox_events (universe_id, channel, category, state, created_at);
CREATE INDEX IF NOT EXISTS idx_communication_events_retention
    ON communication_outbox_events (retention_until);

CREATE OR REPLACE FUNCTION communication_reject_event_mutation()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'DELETE'
       AND COALESCE(current_setting('app.communication_retention_cleanup', TRUE), '') = 'enabled'
       AND OLD.retention_until <= now() THEN
        RETURN OLD;
    END IF;
    RAISE EXCEPTION 'communication outbox events are append-only'
        USING ERRCODE = '55000';
END;
$$;

DROP TRIGGER IF EXISTS communication_outbox_events_immutable
    ON communication_outbox_events;
CREATE TRIGGER communication_outbox_events_immutable
    BEFORE UPDATE OR DELETE ON communication_outbox_events
    FOR EACH ROW EXECUTE FUNCTION communication_reject_event_mutation();

DROP TRIGGER IF EXISTS communication_control_events_immutable
    ON communication_control_events;
CREATE TRIGGER communication_control_events_immutable
    BEFORE UPDATE OR DELETE ON communication_control_events
    FOR EACH ROW EXECUTE FUNCTION communication_reject_event_mutation();

CREATE OR REPLACE VIEW communication_delivery_status_aggregate AS
SELECT universe_id, channel, category, state, COUNT(*)::BIGINT AS job_count,
       MIN(created_at) AS oldest_created_at,
       MAX(updated_at) AS newest_updated_at
FROM communication_outbox
GROUP BY universe_id, channel, category, state;

COMMENT ON TABLE communication_outbox IS
    'Durable communication jobs: no raw destination, subject, body, or arbitrary template variables.';
COMMENT ON TABLE communication_outbox_events IS
    'Append-only aggregate-safe delivery evidence; service identities and destinations are keyed-HMAC only.';
COMMENT ON TABLE communication_verified_contacts IS
    'Bounded contact verification evidence. Raw contacts remain in the authoritative users record only.';
