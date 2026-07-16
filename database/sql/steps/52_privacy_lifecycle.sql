-- Tenant-scoped privacy lifecycle, consent, delivery, and worker contracts.
--
-- This migration deliberately upgrades the Phase-9 gdpr_requests table in
-- place. Existing request identifiers remain stable, while unsafe prototype
-- URL/note fields are retired in favor of encrypted request payloads and
-- digest-only delivery credentials.

ALTER TABLE users
    ADD COLUMN IF NOT EXISTS auth_epoch BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS privacy_restriction_active BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS privacy_restricted_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS privacy_erasure_pending BOOLEAN NOT NULL DEFAULT FALSE;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'users'::regclass
          AND conname = 'users_auth_epoch_nonnegative'
    ) THEN
        ALTER TABLE users
            ADD CONSTRAINT users_auth_epoch_nonnegative CHECK (auth_epoch >= 0);
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'users'::regclass
          AND conname = 'users_privacy_restriction_timestamp'
    ) THEN
        ALTER TABLE users
            ADD CONSTRAINT users_privacy_restriction_timestamp CHECK (
                privacy_restriction_active OR privacy_restricted_at IS NULL
            );
    END IF;
END $$;

CREATE UNIQUE INDEX IF NOT EXISTS idx_users_universe_identity
    ON users (universe_id, id);

-- Interpret all Phase-9 timezone-free values as UTC before introducing new
-- absolute lifecycle instants.
DROP VIEW IF EXISTS gdpr_compliance_status_view;

DO $$
DECLARE
    candidate TEXT;
    kind TEXT;
BEGIN
    FOREACH candidate IN ARRAY ARRAY[
        'requested_at', 'processed_at', 'completed_at', 'expires_at',
        'created_at', 'updated_at'
    ] LOOP
        SELECT data_type INTO kind
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'gdpr_requests'
          AND column_name = candidate;

        IF kind = 'timestamp without time zone' THEN
            EXECUTE format(
                'ALTER TABLE gdpr_requests ALTER COLUMN %I TYPE TIMESTAMPTZ USING %I AT TIME ZONE ''UTC''',
                candidate,
                candidate
            );
        END IF;
    END LOOP;
END $$;

ALTER TABLE gdpr_requests
    ADD COLUMN IF NOT EXISTS universe_id BIGINT,
    ADD COLUMN IF NOT EXISTS idempotency_key TEXT,
    ADD COLUMN IF NOT EXISTS request_source TEXT NOT NULL DEFAULT 'legacy',
    ADD COLUMN IF NOT EXISTS requester_ip_digest BYTEA,
    ADD COLUMN IF NOT EXISTS request_payload_ciphertext BYTEA,
    ADD COLUMN IF NOT EXISTS payload_key_id TEXT,
    ADD COLUMN IF NOT EXISTS payload_nonce BYTEA,
    ADD COLUMN IF NOT EXISTS payload_sha256 BYTEA,
    ADD COLUMN IF NOT EXISTS cooling_off_until TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS approved_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS cancelled_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS cancelled_by_user_id INTEGER,
    ADD COLUMN IF NOT EXISTS cancellation_reason_code TEXT,
    ADD COLUMN IF NOT EXISTS legal_hold_active BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS legal_hold_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS legal_hold_by_admin_id INTEGER,
    ADD COLUMN IF NOT EXISTS legal_hold_reason_code TEXT,
    ADD COLUMN IF NOT EXISTS legal_hold_released_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS legal_hold_released_by_admin_id INTEGER,
    ADD COLUMN IF NOT EXISTS status_before_legal_hold TEXT,
    ADD COLUMN IF NOT EXISTS retention_until TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS legacy_content_redacted_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS version BIGINT NOT NULL DEFAULT 1;

UPDATE gdpr_requests AS request
SET universe_id = users.universe_id
FROM users
WHERE users.id = request.user_id
  AND request.universe_id IS NULL;

UPDATE gdpr_requests
SET request_type = CASE LOWER(BTRIM(request_type))
        WHEN 'export_data' THEN 'export'
        WHEN 'data_export' THEN 'export'
        WHEN 'export' THEN 'export'
        WHEN 'correct_data' THEN 'correction'
        WHEN 'rectification' THEN 'correction'
        WHEN 'correction' THEN 'correction'
        WHEN 'restrict_processing' THEN 'restriction'
        WHEN 'restriction' THEN 'restriction'
        WHEN 'delete_data' THEN 'erasure'
        WHEN 'deletion' THEN 'erasure'
        WHEN 'erasure' THEN 'erasure'
        ELSE 'correction'
    END,
    status = CASE LOWER(BTRIM(status))
        WHEN 'pending' THEN 'pending'
        WHEN 'cooling_off' THEN 'cooling_off'
        WHEN 'in_review' THEN 'in_review'
        WHEN 'approved' THEN 'approved'
        WHEN 'queued' THEN 'queued'
        WHEN 'processing' THEN 'processing'
        WHEN 'completed' THEN 'completed'
        WHEN 'cancelled' THEN 'cancelled'
        WHEN 'canceled' THEN 'cancelled'
        WHEN 'rejected' THEN 'rejected'
        WHEN 'failed' THEN 'failed'
        WHEN 'blocked_legal_hold' THEN 'blocked_legal_hold'
        ELSE 'in_review'
    END,
    idempotency_key = COALESCE(NULLIF(BTRIM(idempotency_key), ''), 'legacy:' || id::TEXT),
    request_source = COALESCE(NULLIF(BTRIM(request_source), ''), 'legacy'),
    retention_until = COALESCE(
        retention_until,
        completed_at + INTERVAL '6 years',
        requested_at + INTERVAL '6 years'
    ),
    cooling_off_until = CASE
        WHEN request_type IN ('delete_data', 'deletion', 'erasure')
            THEN COALESCE(cooling_off_until, requested_at + INTERVAL '14 days')
        ELSE cooling_off_until
    END,
    completed_at = CASE
        WHEN LOWER(BTRIM(status)) = 'completed'
            THEN COALESCE(completed_at, processed_at, updated_at, requested_at)
        ELSE completed_at
    END
WHERE request_type NOT IN ('export', 'correction', 'restriction', 'erasure')
   OR status NOT IN (
        'pending', 'cooling_off', 'in_review', 'approved', 'queued',
        'processing', 'completed', 'cancelled', 'rejected', 'failed',
        'blocked_legal_hold'
   )
   OR idempotency_key IS NULL OR BTRIM(idempotency_key) = ''
   OR request_source IS NULL OR BTRIM(request_source) = ''
   OR retention_until IS NULL
   OR (request_type IN ('delete_data', 'deletion', 'erasure')
       AND cooling_off_until IS NULL)
   OR (LOWER(BTRIM(status)) = 'completed' AND completed_at IS NULL);

-- Prototype URLs could contain bearer credentials and notes could contain raw
-- PII. Record that legacy content was retired, then remove both unsafe fields.
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'gdpr_requests'
          AND column_name = 'data_url'
    ) THEN
        UPDATE gdpr_requests
        SET legacy_content_redacted_at = COALESCE(legacy_content_redacted_at, now())
        WHERE data_url IS NOT NULL;
        ALTER TABLE gdpr_requests DROP COLUMN data_url;
    END IF;
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'gdpr_requests'
          AND column_name = 'notes'
    ) THEN
        UPDATE gdpr_requests
        SET legacy_content_redacted_at = COALESCE(legacy_content_redacted_at, now())
        WHERE notes IS NOT NULL;
        ALTER TABLE gdpr_requests DROP COLUMN notes;
    END IF;
END $$;

ALTER TABLE gdpr_requests
    ALTER COLUMN universe_id SET NOT NULL,
    ALTER COLUMN idempotency_key SET NOT NULL,
    ALTER COLUMN request_source SET NOT NULL,
    ALTER COLUMN requested_at SET DEFAULT now(),
    ALTER COLUMN requested_at SET NOT NULL,
    ALTER COLUMN created_at SET DEFAULT now(),
    ALTER COLUMN created_at SET NOT NULL,
    ALTER COLUMN updated_at SET DEFAULT now(),
    ALTER COLUMN updated_at SET NOT NULL,
    ALTER COLUMN retention_until SET NOT NULL,
    ALTER COLUMN version SET DEFAULT 1,
    ALTER COLUMN version SET NOT NULL;

-- Replace the old user-only cascading FK with an immutable tenant/user pair.
DO $$
DECLARE
    constraint_name TEXT;
BEGIN
    FOR constraint_name IN
        SELECT conname
        FROM pg_constraint
        WHERE conrelid = 'gdpr_requests'::regclass
          AND contype = 'f'
          AND pg_get_constraintdef(oid) LIKE 'FOREIGN KEY (user_id)%'
    LOOP
        EXECUTE format('ALTER TABLE gdpr_requests DROP CONSTRAINT %I', constraint_name);
    END LOOP;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'gdpr_requests'::regclass
          AND conname = 'gdpr_requests_universe_user_fkey'
    ) THEN
        ALTER TABLE gdpr_requests
            ADD CONSTRAINT gdpr_requests_universe_user_fkey
            FOREIGN KEY (universe_id, user_id)
            REFERENCES users(universe_id, id) ON DELETE RESTRICT;
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'gdpr_requests'::regclass
          AND conname = 'gdpr_requests_cancelled_by_fkey'
    ) THEN
        ALTER TABLE gdpr_requests
            ADD CONSTRAINT gdpr_requests_cancelled_by_fkey
            FOREIGN KEY (universe_id, cancelled_by_user_id)
            REFERENCES users(universe_id, id) ON DELETE RESTRICT;
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'gdpr_requests'::regclass
          AND conname = 'gdpr_requests_legal_hold_admin_fkey'
    ) THEN
        ALTER TABLE gdpr_requests
            ADD CONSTRAINT gdpr_requests_legal_hold_admin_fkey
            FOREIGN KEY (universe_id, legal_hold_by_admin_id)
            REFERENCES users(universe_id, id) ON DELETE RESTRICT;
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'gdpr_requests'::regclass
          AND conname = 'gdpr_requests_legal_hold_release_admin_fkey'
    ) THEN
        ALTER TABLE gdpr_requests
            ADD CONSTRAINT gdpr_requests_legal_hold_release_admin_fkey
            FOREIGN KEY (universe_id, legal_hold_released_by_admin_id)
            REFERENCES users(universe_id, id) ON DELETE RESTRICT;
    END IF;
END $$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conrelid = 'gdpr_requests'::regclass
          AND conname = 'gdpr_requests_type_valid'
    ) THEN
        ALTER TABLE gdpr_requests ADD CONSTRAINT gdpr_requests_type_valid
            CHECK (request_type IN ('export', 'correction', 'restriction', 'erasure'));
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conrelid = 'gdpr_requests'::regclass
          AND conname = 'gdpr_requests_status_valid'
    ) THEN
        ALTER TABLE gdpr_requests ADD CONSTRAINT gdpr_requests_status_valid
            CHECK (status IN (
                'pending', 'cooling_off', 'in_review', 'approved', 'queued',
                'processing', 'completed', 'cancelled', 'rejected', 'failed',
                'blocked_legal_hold'
            ));
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conrelid = 'gdpr_requests'::regclass
          AND conname = 'gdpr_requests_idempotency_valid'
    ) THEN
        ALTER TABLE gdpr_requests ADD CONSTRAINT gdpr_requests_idempotency_valid
            CHECK (char_length(BTRIM(idempotency_key)) BETWEEN 1 AND 200);
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conrelid = 'gdpr_requests'::regclass
          AND conname = 'gdpr_requests_payload_encrypted'
    ) THEN
        ALTER TABLE gdpr_requests ADD CONSTRAINT gdpr_requests_payload_encrypted
            CHECK (
                (request_payload_ciphertext IS NULL
                    AND payload_key_id IS NULL
                    AND payload_nonce IS NULL
                    AND payload_sha256 IS NULL)
                OR
                (request_payload_ciphertext IS NOT NULL
                    AND NULLIF(BTRIM(payload_key_id), '') IS NOT NULL
                    AND octet_length(payload_nonce) = 12
                    AND octet_length(payload_sha256) = 32)
            );
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conrelid = 'gdpr_requests'::regclass
          AND conname = 'gdpr_requests_ip_digest_valid'
    ) THEN
        ALTER TABLE gdpr_requests ADD CONSTRAINT gdpr_requests_ip_digest_valid
            CHECK (requester_ip_digest IS NULL OR octet_length(requester_ip_digest) = 32);
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conrelid = 'gdpr_requests'::regclass
          AND conname = 'gdpr_requests_cancellation_consistent'
    ) THEN
        ALTER TABLE gdpr_requests ADD CONSTRAINT gdpr_requests_cancellation_consistent
            CHECK (
                (cancelled_at IS NULL AND cancelled_by_user_id IS NULL
                    AND cancellation_reason_code IS NULL)
                OR
                (cancelled_at IS NOT NULL AND cancelled_by_user_id IS NOT NULL
                    AND NULLIF(BTRIM(cancellation_reason_code), '') IS NOT NULL)
            );
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conrelid = 'gdpr_requests'::regclass
          AND conname = 'gdpr_requests_legal_hold_consistent'
    ) THEN
        ALTER TABLE gdpr_requests ADD CONSTRAINT gdpr_requests_legal_hold_consistent
            CHECK (
                (
                    legal_hold_at IS NULL
                    AND legal_hold_by_admin_id IS NULL
                    AND legal_hold_reason_code IS NULL
                    AND legal_hold_released_at IS NULL
                    AND legal_hold_released_by_admin_id IS NULL
                    AND legal_hold_active = FALSE
                )
                OR
                (
                    legal_hold_at IS NOT NULL
                    AND legal_hold_by_admin_id IS NOT NULL
                    AND NULLIF(BTRIM(legal_hold_reason_code), '') IS NOT NULL
                    AND (
                        (legal_hold_active = TRUE
                            AND legal_hold_released_at IS NULL
                            AND legal_hold_released_by_admin_id IS NULL)
                        OR
                        (legal_hold_active = FALSE
                            AND legal_hold_released_at IS NOT NULL
                            AND legal_hold_released_by_admin_id IS NOT NULL)
                    )
                )
            );
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conrelid = 'gdpr_requests'::regclass
          AND conname = 'gdpr_requests_version_positive'
    ) THEN
        ALTER TABLE gdpr_requests ADD CONSTRAINT gdpr_requests_version_positive
            CHECK (version > 0);
    END IF;
END $$;

CREATE UNIQUE INDEX IF NOT EXISTS idx_gdpr_requests_idempotency
    ON gdpr_requests (universe_id, user_id, idempotency_key);
CREATE UNIQUE INDEX IF NOT EXISTS idx_gdpr_requests_tenant_request_identity
    ON gdpr_requests (id, universe_id, user_id);
CREATE INDEX IF NOT EXISTS idx_gdpr_requests_tenant_owner
    ON gdpr_requests (universe_id, user_id, requested_at DESC);
CREATE INDEX IF NOT EXISTS idx_gdpr_requests_lifecycle
    ON gdpr_requests (universe_id, status, requested_at);
CREATE INDEX IF NOT EXISTS idx_gdpr_requests_retention
    ON gdpr_requests (retention_until)
    WHERE legal_hold_active = FALSE;

CREATE TABLE IF NOT EXISTS privacy_request_events (
    id BIGINT PRIMARY KEY GENERATED BY DEFAULT AS IDENTITY,
    request_id INTEGER NOT NULL,
    universe_id BIGINT NOT NULL,
    user_id INTEGER NOT NULL,
    event_type TEXT NOT NULL CHECK (event_type IN (
        'requested', 'status_changed', 'legal_hold_applied',
        'legal_hold_released', 'payload_redacted'
    )),
    from_status TEXT,
    to_status TEXT NOT NULL,
    actor_type TEXT NOT NULL CHECK (actor_type IN ('user', 'admin', 'worker', 'system')),
    actor_user_id INTEGER,
    reason_code TEXT,
    request_version BIGINT NOT NULL CHECK (request_version > 0),
    dedupe_key TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (request_id, universe_id, user_id)
        REFERENCES gdpr_requests(id, universe_id, user_id) ON DELETE RESTRICT,
    FOREIGN KEY (universe_id, user_id)
        REFERENCES users(universe_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (universe_id, actor_user_id)
        REFERENCES users(universe_id, id) ON DELETE RESTRICT
);

CREATE INDEX IF NOT EXISTS idx_privacy_request_events_request
    ON privacy_request_events (request_id, id);
CREATE INDEX IF NOT EXISTS idx_privacy_request_events_tenant
    ON privacy_request_events (universe_id, created_at DESC);

CREATE TABLE IF NOT EXISTS privacy_admin_decisions (
    id BIGINT PRIMARY KEY GENERATED BY DEFAULT AS IDENTITY,
    request_id INTEGER NOT NULL,
    universe_id BIGINT NOT NULL,
    user_id INTEGER NOT NULL,
    admin_user_id INTEGER NOT NULL,
    decision TEXT NOT NULL CHECK (decision IN (
        'approve', 'reject', 'apply_legal_hold', 'release_legal_hold',
        'complete_correction', 'complete_erasure'
    )),
    reason_code TEXT NOT NULL CHECK (char_length(BTRIM(reason_code)) BETWEEN 1 AND 100),
    decided_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (request_id, admin_user_id, decision),
    CHECK (admin_user_id <> user_id),
    FOREIGN KEY (request_id, universe_id, user_id)
        REFERENCES gdpr_requests(id, universe_id, user_id) ON DELETE RESTRICT,
    FOREIGN KEY (universe_id, user_id)
        REFERENCES users(universe_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (universe_id, admin_user_id)
        REFERENCES users(universe_id, id) ON DELETE RESTRICT
);

CREATE INDEX IF NOT EXISTS idx_privacy_admin_decisions_request
    ON privacy_admin_decisions (request_id, decision, decided_at);

CREATE TABLE IF NOT EXISTS privacy_consents (
    universe_id BIGINT NOT NULL,
    user_id INTEGER NOT NULL,
    purpose TEXT NOT NULL CHECK (char_length(BTRIM(purpose)) BETWEEN 1 AND 100),
    channel TEXT NOT NULL CHECK (channel IN ('all', 'email', 'in_app', 'push', 'sms')),
    status TEXT NOT NULL CHECK (status IN ('granted', 'denied', 'withdrawn')),
    lawful_basis TEXT NOT NULL CHECK (lawful_basis IN (
        'consent', 'contract', 'legal_obligation', 'vital_interests',
        'public_task', 'legitimate_interests'
    )),
    policy_version TEXT NOT NULL CHECK (char_length(BTRIM(policy_version)) BETWEEN 1 AND 100),
    proof_digest BYTEA,
    collected_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    PRIMARY KEY (universe_id, user_id, purpose, channel),
    CHECK (proof_digest IS NULL OR octet_length(proof_digest) = 32),
    CHECK (lawful_basis <> 'consent' OR status <> 'granted' OR proof_digest IS NOT NULL),
    FOREIGN KEY (universe_id, user_id)
        REFERENCES users(universe_id, id) ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS privacy_consent_events (
    id BIGINT PRIMARY KEY GENERATED BY DEFAULT AS IDENTITY,
    universe_id BIGINT NOT NULL,
    user_id INTEGER NOT NULL,
    purpose TEXT NOT NULL,
    channel TEXT NOT NULL,
    status TEXT NOT NULL,
    lawful_basis TEXT NOT NULL,
    policy_version TEXT NOT NULL,
    proof_digest BYTEA,
    changed_by_user_id INTEGER,
    actor_type TEXT NOT NULL CHECK (actor_type IN ('user', 'admin', 'system')),
    consent_version BIGINT NOT NULL CHECK (consent_version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (universe_id, user_id, purpose, channel, consent_version),
    FOREIGN KEY (universe_id, user_id)
        REFERENCES users(universe_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (universe_id, changed_by_user_id)
        REFERENCES users(universe_id, id) ON DELETE RESTRICT
);

CREATE INDEX IF NOT EXISTS idx_privacy_consent_events_owner
    ON privacy_consent_events (universe_id, user_id, created_at DESC);

CREATE TABLE IF NOT EXISTS privacy_communication_preferences (
    universe_id BIGINT NOT NULL,
    user_id INTEGER NOT NULL,
    channel TEXT NOT NULL CHECK (channel IN ('email', 'in_app', 'push', 'sms')),
    category TEXT NOT NULL CHECK (category IN (
        'marketing', 'product_updates', 'gameplay_digest', 'security', 'transactional'
    )),
    enabled BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    PRIMARY KEY (universe_id, user_id, channel, category),
    FOREIGN KEY (universe_id, user_id)
        REFERENCES users(universe_id, id) ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS privacy_communication_preference_events (
    id BIGINT PRIMARY KEY GENERATED BY DEFAULT AS IDENTITY,
    universe_id BIGINT NOT NULL,
    user_id INTEGER NOT NULL,
    channel TEXT NOT NULL,
    category TEXT NOT NULL,
    enabled BOOLEAN NOT NULL,
    changed_by_user_id INTEGER,
    actor_type TEXT NOT NULL CHECK (actor_type IN ('user', 'admin', 'system')),
    preference_version BIGINT NOT NULL CHECK (preference_version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (universe_id, user_id, channel, category, preference_version),
    FOREIGN KEY (universe_id, user_id)
        REFERENCES users(universe_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (universe_id, changed_by_user_id)
        REFERENCES users(universe_id, id) ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS privacy_outbox (
    id BIGINT PRIMARY KEY GENERATED BY DEFAULT AS IDENTITY,
    request_id INTEGER NOT NULL,
    universe_id BIGINT NOT NULL,
    user_id INTEGER NOT NULL,
    event_type TEXT NOT NULL CHECK (event_type IN (
        'privacy.export.prepare', 'privacy.restriction.apply',
        'privacy.erasure.invalidate_access'
    )),
    dedupe_key TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN (
        'pending', 'processing', 'retry', 'delivered', 'cancelled', 'dead'
    )),
    available_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    lease_owner TEXT,
    lease_expires_at TIMESTAMPTZ,
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    max_attempts INTEGER NOT NULL DEFAULT 10 CHECK (max_attempts > 0),
    last_error_code TEXT,
    delivered_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (
        (status = 'processing' AND NULLIF(BTRIM(lease_owner), '') IS NOT NULL
            AND lease_expires_at IS NOT NULL)
        OR
        (status <> 'processing' AND lease_owner IS NULL AND lease_expires_at IS NULL)
    ),
    CHECK (last_error_code IS NULL OR char_length(last_error_code) BETWEEN 1 AND 100),
    FOREIGN KEY (request_id, universe_id, user_id)
        REFERENCES gdpr_requests(id, universe_id, user_id) ON DELETE RESTRICT,
    FOREIGN KEY (universe_id, user_id)
        REFERENCES users(universe_id, id) ON DELETE RESTRICT
);

CREATE INDEX IF NOT EXISTS idx_privacy_outbox_claim
    ON privacy_outbox (status, available_at, lease_expires_at, id)
    WHERE status IN ('pending', 'retry', 'processing');
CREATE INDEX IF NOT EXISTS idx_privacy_outbox_tenant
    ON privacy_outbox (universe_id, status, id);

CREATE TABLE IF NOT EXISTS privacy_export_artifacts (
    id BIGINT PRIMARY KEY GENERATED BY DEFAULT AS IDENTITY,
    request_id INTEGER NOT NULL UNIQUE,
    universe_id BIGINT NOT NULL,
    user_id INTEGER NOT NULL,
    ciphertext BYTEA,
    encryption_key_id TEXT,
    encryption_nonce BYTEA,
    plaintext_sha256 BYTEA,
    plaintext_size BIGINT NOT NULL CHECK (plaintext_size >= 0),
    download_token_digest BYTEA,
    token_issued_at TIMESTAMPTZ,
    token_expires_at TIMESTAMPTZ,
    downloaded_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,
    purged_at TIMESTAMPTZ,
    CHECK (
        (ciphertext IS NULL AND encryption_key_id IS NULL
            AND encryption_nonce IS NULL AND plaintext_sha256 IS NULL)
        OR
        (ciphertext IS NOT NULL AND NULLIF(BTRIM(encryption_key_id), '') IS NOT NULL
            AND octet_length(encryption_nonce) = 12
            AND octet_length(plaintext_sha256) = 32)
    ),
    CHECK (
        (download_token_digest IS NULL AND token_issued_at IS NULL AND token_expires_at IS NULL)
        OR
        (octet_length(download_token_digest) = 32
            AND token_issued_at IS NOT NULL AND token_expires_at IS NOT NULL)
    ),
    CHECK (purged_at IS NULL OR ciphertext IS NULL),
    FOREIGN KEY (request_id, universe_id, user_id)
        REFERENCES gdpr_requests(id, universe_id, user_id) ON DELETE RESTRICT,
    FOREIGN KEY (universe_id, user_id)
        REFERENCES users(universe_id, id) ON DELETE RESTRICT
);

CREATE INDEX IF NOT EXISTS idx_privacy_export_artifacts_owner
    ON privacy_export_artifacts (universe_id, user_id, request_id);
CREATE INDEX IF NOT EXISTS idx_privacy_export_artifacts_retention
    ON privacy_export_artifacts (expires_at)
    WHERE purged_at IS NULL;

-- Append-only regulatory evidence must never be rewritten in place.
CREATE OR REPLACE FUNCTION privacy_reject_evidence_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION '% is append-only', TG_TABLE_NAME
        USING ERRCODE = '55000';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS privacy_request_events_immutable ON privacy_request_events;
CREATE TRIGGER privacy_request_events_immutable
    BEFORE UPDATE OR DELETE ON privacy_request_events
    FOR EACH ROW EXECUTE FUNCTION privacy_reject_evidence_mutation();

CREATE OR REPLACE FUNCTION privacy_validate_request_event_actor()
RETURNS TRIGGER AS $$
DECLARE
    admin_valid BOOLEAN;
BEGIN
    IF NEW.actor_type = 'user' THEN
        IF NEW.actor_user_id IS NULL OR NEW.actor_user_id <> NEW.user_id THEN
            RAISE EXCEPTION 'privacy user event actor must be the request subject'
                USING ERRCODE = '42501';
        END IF;
    ELSIF NEW.actor_type = 'admin' THEN
        SELECT EXISTS (
            SELECT 1 FROM users
            WHERE id = NEW.actor_user_id
              AND universe_id = NEW.universe_id
              AND is_admin = TRUE
              AND is_banned = FALSE
        ) INTO admin_valid;
        IF NOT COALESCE(admin_valid, FALSE) THEN
            RAISE EXCEPTION 'privacy event requires an active tenant administrator'
                USING ERRCODE = '42501';
        END IF;
    ELSIF NEW.actor_user_id IS NOT NULL THEN
        RAISE EXCEPTION 'privacy worker/system events cannot name a user actor'
            USING ERRCODE = '42501';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS privacy_request_events_actor_validate ON privacy_request_events;
CREATE TRIGGER privacy_request_events_actor_validate
    BEFORE INSERT ON privacy_request_events
    FOR EACH ROW EXECUTE FUNCTION privacy_validate_request_event_actor();

DROP TRIGGER IF EXISTS privacy_admin_decisions_immutable ON privacy_admin_decisions;
CREATE TRIGGER privacy_admin_decisions_immutable
    BEFORE UPDATE OR DELETE ON privacy_admin_decisions
    FOR EACH ROW EXECUTE FUNCTION privacy_reject_evidence_mutation();

DROP TRIGGER IF EXISTS privacy_consent_events_immutable ON privacy_consent_events;
CREATE TRIGGER privacy_consent_events_immutable
    BEFORE UPDATE OR DELETE ON privacy_consent_events
    FOR EACH ROW EXECUTE FUNCTION privacy_reject_evidence_mutation();

DROP TRIGGER IF EXISTS privacy_communication_events_immutable
    ON privacy_communication_preference_events;
CREATE TRIGGER privacy_communication_events_immutable
    BEFORE UPDATE OR DELETE ON privacy_communication_preference_events
    FOR EACH ROW EXECUTE FUNCTION privacy_reject_evidence_mutation();

CREATE OR REPLACE FUNCTION privacy_validate_admin_decision()
RETURNS TRIGGER AS $$
DECLARE
    request_row gdpr_requests%ROWTYPE;
    admin_valid BOOLEAN;
BEGIN
    SELECT * INTO request_row FROM gdpr_requests WHERE id = NEW.request_id;
    IF NOT FOUND
       OR request_row.universe_id <> NEW.universe_id
       OR request_row.user_id <> NEW.user_id THEN
        RAISE EXCEPTION 'privacy decision tenant/request mismatch'
            USING ERRCODE = '23514';
    END IF;

    SELECT EXISTS (
        SELECT 1 FROM users
        WHERE id = NEW.admin_user_id
          AND universe_id = NEW.universe_id
          AND is_admin = TRUE
          AND is_banned = FALSE
    ) INTO admin_valid;
    IF NOT admin_valid THEN
        RAISE EXCEPTION 'privacy decision requires an active tenant administrator'
            USING ERRCODE = '42501';
    END IF;
    IF NEW.admin_user_id = NEW.user_id THEN
        RAISE EXCEPTION 'administrator cannot decide their own privacy request'
            USING ERRCODE = '42501';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS privacy_admin_decisions_validate ON privacy_admin_decisions;
CREATE TRIGGER privacy_admin_decisions_validate
    BEFORE INSERT ON privacy_admin_decisions
    FOR EACH ROW EXECUTE FUNCTION privacy_validate_admin_decision();

CREATE OR REPLACE FUNCTION privacy_validate_request_transition()
RETURNS TRIGGER AS $$
DECLARE
    approvals INTEGER;
BEGIN
    IF NEW.id <> OLD.id
       OR NEW.universe_id <> OLD.universe_id
       OR NEW.user_id <> OLD.user_id
       OR NEW.request_type <> OLD.request_type
       OR NEW.idempotency_key <> OLD.idempotency_key THEN
        RAISE EXCEPTION 'privacy request identity is immutable'
            USING ERRCODE = '55000';
    END IF;

    IF NEW.status <> OLD.status AND NOT (
        (OLD.status = 'pending' AND NEW.status IN (
            'cooling_off', 'in_review', 'queued', 'cancelled', 'blocked_legal_hold'
        )) OR
        (OLD.status = 'cooling_off' AND NEW.status IN (
            'in_review', 'cancelled', 'blocked_legal_hold'
        )) OR
        (OLD.status = 'in_review' AND NEW.status IN (
            'approved', 'queued', 'rejected', 'cancelled', 'blocked_legal_hold', 'completed'
        )) OR
        (OLD.status = 'approved' AND NEW.status IN (
            'queued', 'processing', 'cancelled', 'blocked_legal_hold', 'completed'
        )) OR
        (OLD.status = 'queued' AND NEW.status IN (
            'processing', 'approved', 'cancelled', 'blocked_legal_hold', 'failed'
        )) OR
        (OLD.status = 'processing' AND NEW.status IN (
            'completed', 'failed', 'queued', 'approved', 'blocked_legal_hold'
        )) OR
        (OLD.status = 'failed' AND NEW.status IN (
            'queued', 'cancelled', 'blocked_legal_hold'
        )) OR
        (OLD.status = 'blocked_legal_hold' AND NEW.status IN (
            'cooling_off', 'in_review', 'approved', 'queued', 'cancelled'
        ))
    ) THEN
        RAISE EXCEPTION 'invalid privacy request transition: % -> %', OLD.status, NEW.status
            USING ERRCODE = '23514';
    END IF;

    IF NEW.request_type = 'erasure'
       AND NEW.status IN ('approved', 'queued', 'processing', 'completed')
       AND OLD.status IS DISTINCT FROM NEW.status THEN
        IF NEW.legal_hold_active THEN
            RAISE EXCEPTION 'erasure request is under legal hold'
                USING ERRCODE = '55000';
        END IF;
        IF NEW.cooling_off_until IS NULL OR NEW.cooling_off_until > now() THEN
            RAISE EXCEPTION 'erasure cooling-off period has not elapsed'
                USING ERRCODE = '55000';
        END IF;
        SELECT COUNT(DISTINCT admin_user_id)::INTEGER INTO approvals
        FROM privacy_admin_decisions
        WHERE request_id = NEW.id AND decision = 'approve';
        IF approvals < 2 THEN
            RAISE EXCEPTION 'erasure requires approval by two distinct administrators'
                USING ERRCODE = '42501';
        END IF;
    END IF;

    IF NEW.status = 'completed' AND NEW.completed_at IS NULL THEN
        NEW.completed_at := now();
    END IF;
    IF NEW.status = 'approved' AND NEW.approved_at IS NULL THEN
        NEW.approved_at := now();
    END IF;
    NEW.updated_at := now();
    NEW.version := OLD.version + 1;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS privacy_request_validate_transition ON gdpr_requests;
CREATE TRIGGER privacy_request_validate_transition
    BEFORE UPDATE ON gdpr_requests
    FOR EACH ROW EXECUTE FUNCTION privacy_validate_request_transition();

CREATE OR REPLACE FUNCTION privacy_record_request_event()
RETURNS TRIGGER AS $$
DECLARE
    actor_kind TEXT;
    actor_id INTEGER;
    reason TEXT;
    event_name TEXT;
BEGIN
    actor_kind := COALESCE(NULLIF(current_setting('app.privacy_actor_type', TRUE), ''), 'system');
    IF actor_kind NOT IN ('user', 'admin', 'worker', 'system') THEN
        actor_kind := 'system';
    END IF;
    actor_id := NULLIF(current_setting('app.privacy_actor_user_id', TRUE), '')::INTEGER;
    reason := NULLIF(current_setting('app.privacy_reason_code', TRUE), '');

    IF TG_OP = 'INSERT' THEN
        event_name := 'requested';
        INSERT INTO privacy_request_events (
            request_id, universe_id, user_id, event_type, from_status, to_status,
            actor_type, actor_user_id, reason_code, request_version, dedupe_key
        ) VALUES (
            NEW.id, NEW.universe_id, NEW.user_id, event_name, NULL, NEW.status,
            actor_kind, actor_id, reason, NEW.version,
            NEW.id || ':' || NEW.version || ':requested'
        ) ON CONFLICT (dedupe_key) DO NOTHING;
    ELSIF NEW.legal_hold_active IS DISTINCT FROM OLD.legal_hold_active THEN
        event_name := CASE WHEN NEW.legal_hold_active
            THEN 'legal_hold_applied' ELSE 'legal_hold_released' END;
        INSERT INTO privacy_request_events (
            request_id, universe_id, user_id, event_type, from_status, to_status,
            actor_type, actor_user_id, reason_code, request_version, dedupe_key
        ) VALUES (
            NEW.id, NEW.universe_id, NEW.user_id, event_name, OLD.status, NEW.status,
            actor_kind, actor_id, reason, NEW.version,
            NEW.id || ':' || NEW.version || ':' || event_name
        ) ON CONFLICT (dedupe_key) DO NOTHING;
    ELSIF NEW.status IS DISTINCT FROM OLD.status THEN
        INSERT INTO privacy_request_events (
            request_id, universe_id, user_id, event_type, from_status, to_status,
            actor_type, actor_user_id, reason_code, request_version, dedupe_key
        ) VALUES (
            NEW.id, NEW.universe_id, NEW.user_id, 'status_changed', OLD.status, NEW.status,
            actor_kind, actor_id, reason, NEW.version,
            NEW.id || ':' || NEW.version || ':status_changed'
        ) ON CONFLICT (dedupe_key) DO NOTHING;
    ELSIF OLD.request_payload_ciphertext IS NOT NULL
          AND NEW.request_payload_ciphertext IS NULL THEN
        INSERT INTO privacy_request_events (
            request_id, universe_id, user_id, event_type, from_status, to_status,
            actor_type, actor_user_id, reason_code, request_version, dedupe_key
        ) VALUES (
            NEW.id, NEW.universe_id, NEW.user_id, 'payload_redacted', OLD.status, NEW.status,
            actor_kind, actor_id, reason, NEW.version,
            NEW.id || ':' || NEW.version || ':payload_redacted'
        ) ON CONFLICT (dedupe_key) DO NOTHING;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS privacy_request_audit ON gdpr_requests;
CREATE TRIGGER privacy_request_audit
    AFTER INSERT OR UPDATE ON gdpr_requests
    FOR EACH ROW EXECUTE FUNCTION privacy_record_request_event();

-- Backfill one immutable evidence row for each request that predates this
-- trigger. No raw notes, URLs, payloads, or IP addresses enter the audit.
INSERT INTO privacy_request_events (
    request_id, universe_id, user_id, event_type, from_status, to_status,
    actor_type, reason_code, request_version, dedupe_key, created_at
)
SELECT id, universe_id, user_id, 'requested', NULL, status,
       'system', 'legacy_request_imported', version,
       id || ':' || version || ':requested', created_at
FROM gdpr_requests
ON CONFLICT (dedupe_key) DO NOTHING;

CREATE OR REPLACE FUNCTION privacy_prepare_consent_change()
RETURNS TRIGGER AS $$
DECLARE
    actor_kind TEXT;
    actor_id INTEGER;
BEGIN
    IF TG_OP = 'UPDATE' THEN
        IF NEW.universe_id <> OLD.universe_id OR NEW.user_id <> OLD.user_id
           OR NEW.purpose <> OLD.purpose OR NEW.channel <> OLD.channel THEN
            RAISE EXCEPTION 'consent identity is immutable' USING ERRCODE = '55000';
        END IF;
        NEW.version := OLD.version + 1;
        NEW.updated_at := now();
    END IF;
    actor_kind := COALESCE(NULLIF(current_setting('app.privacy_actor_type', TRUE), ''), 'system');
    IF actor_kind NOT IN ('user', 'admin', 'system') THEN actor_kind := 'system'; END IF;
    actor_id := NULLIF(current_setting('app.privacy_actor_user_id', TRUE), '')::INTEGER;
    IF actor_kind = 'user' AND actor_id IS DISTINCT FROM NEW.user_id THEN
        RAISE EXCEPTION 'a user may only change their own consent'
            USING ERRCODE = '42501';
    ELSIF actor_kind = 'admin' AND NOT EXISTS (
        SELECT 1 FROM users
        WHERE universe_id = NEW.universe_id
          AND id = actor_id
          AND is_admin = TRUE
          AND is_banned = FALSE
    ) THEN
        RAISE EXCEPTION 'consent change requires an active tenant administrator'
            USING ERRCODE = '42501';
    ELSIF actor_kind = 'system' AND actor_id IS NOT NULL THEN
        RAISE EXCEPTION 'system consent changes cannot impersonate a user'
            USING ERRCODE = '42501';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION privacy_record_consent_event()
RETURNS TRIGGER AS $$
DECLARE
    actor_kind TEXT;
    actor_id INTEGER;
BEGIN
    actor_kind := COALESCE(NULLIF(current_setting('app.privacy_actor_type', TRUE), ''), 'system');
    IF actor_kind NOT IN ('user', 'admin', 'system') THEN actor_kind := 'system'; END IF;
    actor_id := NULLIF(current_setting('app.privacy_actor_user_id', TRUE), '')::INTEGER;
    INSERT INTO privacy_consent_events (
        universe_id, user_id, purpose, channel, status, lawful_basis,
        policy_version, proof_digest, changed_by_user_id, actor_type, consent_version
    ) VALUES (
        NEW.universe_id, NEW.user_id, NEW.purpose, NEW.channel, NEW.status,
        NEW.lawful_basis, NEW.policy_version, NEW.proof_digest, actor_id,
        actor_kind, NEW.version
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS privacy_consent_validate ON privacy_consents;
CREATE TRIGGER privacy_consent_validate
    BEFORE INSERT OR UPDATE ON privacy_consents
    FOR EACH ROW EXECUTE FUNCTION privacy_prepare_consent_change();

DROP TRIGGER IF EXISTS privacy_consent_audit ON privacy_consents;
CREATE TRIGGER privacy_consent_audit
    AFTER INSERT OR UPDATE ON privacy_consents
    FOR EACH ROW EXECUTE FUNCTION privacy_record_consent_event();

CREATE OR REPLACE FUNCTION privacy_prepare_communication_change()
RETURNS TRIGGER AS $$
DECLARE
    actor_kind TEXT;
    actor_id INTEGER;
BEGIN
    IF TG_OP = 'UPDATE' THEN
        IF NEW.universe_id <> OLD.universe_id OR NEW.user_id <> OLD.user_id
           OR NEW.channel <> OLD.channel OR NEW.category <> OLD.category THEN
            RAISE EXCEPTION 'communication preference identity is immutable'
                USING ERRCODE = '55000';
        END IF;
        NEW.version := OLD.version + 1;
        NEW.updated_at := now();
    END IF;
    actor_kind := COALESCE(NULLIF(current_setting('app.privacy_actor_type', TRUE), ''), 'system');
    IF actor_kind NOT IN ('user', 'admin', 'system') THEN actor_kind := 'system'; END IF;
    actor_id := NULLIF(current_setting('app.privacy_actor_user_id', TRUE), '')::INTEGER;
    IF actor_kind = 'user' AND actor_id IS DISTINCT FROM NEW.user_id THEN
        RAISE EXCEPTION 'a user may only change their own communication preferences'
            USING ERRCODE = '42501';
    ELSIF actor_kind = 'admin' AND NOT EXISTS (
        SELECT 1 FROM users
        WHERE universe_id = NEW.universe_id
          AND id = actor_id
          AND is_admin = TRUE
          AND is_banned = FALSE
    ) THEN
        RAISE EXCEPTION 'preference change requires an active tenant administrator'
            USING ERRCODE = '42501';
    ELSIF actor_kind = 'system' AND actor_id IS NOT NULL THEN
        RAISE EXCEPTION 'system preference changes cannot impersonate a user'
            USING ERRCODE = '42501';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION privacy_record_communication_event()
RETURNS TRIGGER AS $$
DECLARE
    actor_kind TEXT;
    actor_id INTEGER;
BEGIN
    actor_kind := COALESCE(NULLIF(current_setting('app.privacy_actor_type', TRUE), ''), 'system');
    IF actor_kind NOT IN ('user', 'admin', 'system') THEN actor_kind := 'system'; END IF;
    actor_id := NULLIF(current_setting('app.privacy_actor_user_id', TRUE), '')::INTEGER;
    INSERT INTO privacy_communication_preference_events (
        universe_id, user_id, channel, category, enabled,
        changed_by_user_id, actor_type, preference_version
    ) VALUES (
        NEW.universe_id, NEW.user_id, NEW.channel, NEW.category, NEW.enabled,
        actor_id, actor_kind, NEW.version
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS privacy_communication_validate
    ON privacy_communication_preferences;
CREATE TRIGGER privacy_communication_validate
    BEFORE INSERT OR UPDATE ON privacy_communication_preferences
    FOR EACH ROW EXECUTE FUNCTION privacy_prepare_communication_change();

DROP TRIGGER IF EXISTS privacy_communication_audit
    ON privacy_communication_preferences;
CREATE TRIGGER privacy_communication_audit
    AFTER INSERT OR UPDATE ON privacy_communication_preferences
    FOR EACH ROW EXECUTE FUNCTION privacy_record_communication_event();

CREATE OR REPLACE VIEW gdpr_compliance_status_view AS
SELECT
    u.universe_id,
    u.id AS user_id,
    u.username,
    u.email,
    COUNT(*) FILTER (WHERE gr.request_type = 'export') AS data_export_requests,
    COUNT(*) FILTER (WHERE gr.request_type = 'erasure') AS deletion_requests,
    MAX(gr.requested_at) AS last_request_date,
    u.created_at AS account_created_at,
    u.privacy_restriction_active,
    u.privacy_erasure_pending,
    u.auth_epoch
FROM users AS u
LEFT JOIN gdpr_requests AS gr
  ON gr.universe_id = u.universe_id AND gr.user_id = u.id
GROUP BY u.universe_id, u.id, u.username, u.email, u.created_at,
         u.privacy_restriction_active, u.privacy_erasure_pending, u.auth_epoch;

COMMENT ON TABLE gdpr_requests IS
    'Canonical tenant-scoped privacy request lifecycle; supersedes the Phase-9 prototype.';
COMMENT ON TABLE privacy_request_events IS
    'Append-only privacy lifecycle evidence containing identifiers and reason codes, never raw PII payloads.';
COMMENT ON COLUMN privacy_export_artifacts.download_token_digest IS
    'SHA-256 digest of a one-time delivery token; the raw token is never persisted.';
