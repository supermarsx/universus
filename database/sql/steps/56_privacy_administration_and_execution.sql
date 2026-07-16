-- Production GDPR administration and restart-safe execution.
--
-- Subject identifiers remain tenant-scoped. Erasure retains only a locked,
-- pseudonymous user tombstone so that regulatory evidence and game-world
-- referential integrity remain valid without retaining contact credentials.

ALTER TABLE users
    ADD COLUMN IF NOT EXISTS privacy_erased_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS privacy_subject_hmac BYTEA,
    ADD COLUMN IF NOT EXISTS privacy_erasure_request_id INTEGER;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'users'::regclass
          AND conname = 'users_privacy_subject_hmac_shape'
    ) THEN
        ALTER TABLE users ADD CONSTRAINT users_privacy_subject_hmac_shape
            CHECK (privacy_subject_hmac IS NULL OR octet_length(privacy_subject_hmac) = 32);
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'users'::regclass
          AND conname = 'users_privacy_erasure_consistent'
    ) THEN
        ALTER TABLE users ADD CONSTRAINT users_privacy_erasure_consistent
            CHECK (
                (privacy_erased_at IS NULL
                    AND privacy_subject_hmac IS NULL
                    AND privacy_erasure_request_id IS NULL)
                OR
                (privacy_erased_at IS NOT NULL
                    AND privacy_subject_hmac IS NOT NULL
                    AND privacy_erasure_request_id IS NOT NULL
                    AND account_status = 'deleted'
                    AND is_banned = TRUE
                    AND privacy_restriction_active = TRUE
                    AND privacy_erasure_pending = FALSE
                    AND email_verified = FALSE
                    AND phone_verified = FALSE
                    AND phone_number IS NULL)
            );
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'users'::regclass
          AND conname = 'users_privacy_erasure_request_fkey'
    ) THEN
        ALTER TABLE users ADD CONSTRAINT users_privacy_erasure_request_fkey
            FOREIGN KEY (privacy_erasure_request_id, universe_id, id)
            REFERENCES gdpr_requests(id, universe_id, user_id) ON DELETE RESTRICT;
    END IF;
END $$;

CREATE UNIQUE INDEX IF NOT EXISTS idx_users_privacy_subject_hmac
    ON users (privacy_subject_hmac)
    WHERE privacy_subject_hmac IS NOT NULL;

ALTER TABLE gdpr_requests
    ADD COLUMN IF NOT EXISTS correction_applied_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS erasure_executed_at TIMESTAMPTZ;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'gdpr_requests'::regclass
          AND conname = 'gdpr_requests_payload_ciphertext_bound'
    ) THEN
        ALTER TABLE gdpr_requests ADD CONSTRAINT gdpr_requests_payload_ciphertext_bound
            CHECK (
                request_payload_ciphertext IS NULL
                OR octet_length(request_payload_ciphertext) BETWEEN 17 AND 16384
            );
    END IF;
END $$;

ALTER TABLE privacy_export_artifacts
    ADD COLUMN IF NOT EXISTS format_version INTEGER NOT NULL DEFAULT 1;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'privacy_export_artifacts'::regclass
          AND conname = 'privacy_export_artifacts_format_version_bound'
    ) THEN
        ALTER TABLE privacy_export_artifacts
            ADD CONSTRAINT privacy_export_artifacts_format_version_bound
            CHECK (format_version BETWEEN 1 AND 100);
    END IF;
END $$;

-- Extend the durable job vocabulary while preserving legacy authorization
-- jobs so an in-flight deployment can be upgraded without losing work.
ALTER TABLE privacy_outbox
    DROP CONSTRAINT IF EXISTS privacy_outbox_event_type_check;
ALTER TABLE privacy_outbox
    ADD CONSTRAINT privacy_outbox_event_type_check CHECK (event_type IN (
        'privacy.export.prepare',
        'privacy.restriction.apply',
        'privacy.erasure.invalidate_access',
        'privacy.erasure.execute',
        'privacy.correction.apply'
    ));

ALTER TABLE privacy_request_events
    DROP CONSTRAINT IF EXISTS privacy_request_events_event_type_check;
ALTER TABLE privacy_request_events
    ADD CONSTRAINT privacy_request_events_event_type_check CHECK (event_type IN (
        'requested', 'status_changed', 'legal_hold_applied',
        'legal_hold_released', 'payload_redacted',
        'correction_applied', 'erasure_completed',
        'export_delivery_issued', 'export_consumed', 'export_purged'
    ));

CREATE TABLE IF NOT EXISTS privacy_correction_executions (
    request_id INTEGER PRIMARY KEY,
    universe_id BIGINT NOT NULL,
    user_id INTEGER NOT NULL,
    applied_fields TEXT[] NOT NULL,
    completed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (cardinality(applied_fields) BETWEEN 1 AND 3),
    CHECK (applied_fields <@ ARRAY['username', 'email', 'phone_number']::TEXT[]),
    FOREIGN KEY (request_id, universe_id, user_id)
        REFERENCES gdpr_requests(id, universe_id, user_id) ON DELETE RESTRICT,
    FOREIGN KEY (universe_id, user_id)
        REFERENCES users(universe_id, id) ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS privacy_erasure_executions (
    request_id INTEGER PRIMARY KEY,
    universe_id BIGINT NOT NULL,
    user_id INTEGER NOT NULL,
    subject_hmac BYTEA NOT NULL CHECK (octet_length(subject_hmac) = 32),
    credentials_deleted BIGINT NOT NULL CHECK (credentials_deleted >= 0),
    sessions_deleted BIGINT NOT NULL CHECK (sessions_deleted >= 0),
    personal_content_deleted BIGINT NOT NULL CHECK (personal_content_deleted >= 0),
    contact_evidence_redacted BIGINT NOT NULL CHECK (contact_evidence_redacted >= 0),
    completed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (request_id, universe_id, user_id)
        REFERENCES gdpr_requests(id, universe_id, user_id) ON DELETE RESTRICT,
    FOREIGN KEY (universe_id, user_id)
        REFERENCES users(universe_id, id) ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS privacy_execution_events (
    id BIGINT PRIMARY KEY GENERATED BY DEFAULT AS IDENTITY,
    request_id INTEGER NOT NULL,
    universe_id BIGINT NOT NULL,
    user_id INTEGER NOT NULL,
    action TEXT NOT NULL CHECK (action IN (
        'correction_applied', 'erasure_completed',
        'export_delivery_issued', 'export_consumed', 'export_purged'
    )),
    actor_type TEXT NOT NULL CHECK (actor_type IN ('user', 'admin', 'worker', 'system')),
    actor_user_id INTEGER,
    reason_code TEXT NOT NULL CHECK (
        reason_code ~ '^[a-z][a-z0-9_.:-]{1,99}$'
    ),
    field_names TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    subject_hmac BYTEA CHECK (subject_hmac IS NULL OR octet_length(subject_hmac) = 32),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    dedupe_key TEXT NOT NULL UNIQUE CHECK (char_length(dedupe_key) BETWEEN 8 AND 200),
    CHECK (field_names <@ ARRAY['username', 'email', 'phone_number']::TEXT[]),
    CONSTRAINT privacy_execution_events_action_shape CHECK (
        (action = 'correction_applied'
            AND cardinality(field_names) BETWEEN 1 AND 3
            AND subject_hmac IS NULL)
        OR
        (action = 'erasure_completed'
            AND cardinality(field_names) = 0
            AND subject_hmac IS NOT NULL)
        OR
        (action IN ('export_delivery_issued', 'export_consumed', 'export_purged')
            AND cardinality(field_names) = 0
            AND subject_hmac IS NULL)
    ),
    CHECK (
        (actor_type IN ('user', 'admin') AND actor_user_id IS NOT NULL)
        OR (actor_type IN ('worker', 'system') AND actor_user_id IS NULL)
    ),
    FOREIGN KEY (request_id, universe_id, user_id)
        REFERENCES gdpr_requests(id, universe_id, user_id) ON DELETE RESTRICT,
    FOREIGN KEY (universe_id, user_id)
        REFERENCES users(universe_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (universe_id, actor_user_id)
        REFERENCES users(universe_id, id) ON DELETE RESTRICT
);

ALTER TABLE privacy_execution_events
    DROP CONSTRAINT IF EXISTS privacy_execution_events_actor_type_check,
    DROP CONSTRAINT IF EXISTS privacy_execution_events_check;
ALTER TABLE privacy_execution_events
    ADD CONSTRAINT privacy_execution_events_actor_type_check
        CHECK (actor_type IN ('user', 'admin', 'worker', 'system')),
    ADD CONSTRAINT privacy_execution_events_actor_shape CHECK (
        (actor_type IN ('user', 'admin') AND actor_user_id IS NOT NULL)
        OR (actor_type IN ('worker', 'system') AND actor_user_id IS NULL)
    );

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'privacy_execution_events'::regclass
          AND conname = 'privacy_execution_events_action_shape'
    ) THEN
        ALTER TABLE privacy_execution_events
            ADD CONSTRAINT privacy_execution_events_action_shape CHECK (
                (action = 'correction_applied'
                    AND cardinality(field_names) BETWEEN 1 AND 3
                    AND subject_hmac IS NULL)
                OR
                (action = 'erasure_completed'
                    AND cardinality(field_names) = 0
                    AND subject_hmac IS NOT NULL)
                OR
                (action IN ('export_delivery_issued', 'export_consumed', 'export_purged')
                    AND cardinality(field_names) = 0
                    AND subject_hmac IS NULL)
            );
    END IF;
END $$;

CREATE INDEX IF NOT EXISTS idx_privacy_execution_events_request
    ON privacy_execution_events (request_id, id);
CREATE INDEX IF NOT EXISTS idx_privacy_execution_events_tenant
    ON privacy_execution_events (universe_id, created_at DESC);

CREATE TABLE IF NOT EXISTS privacy_retention_runs (
    id BIGINT PRIMARY KEY GENERATED BY DEFAULT AS IDENTITY,
    universe_id BIGINT,
    actor_type TEXT NOT NULL CHECK (actor_type IN ('admin', 'system')),
    actor_user_id INTEGER,
    reason_code TEXT NOT NULL CHECK (
        reason_code ~ '^[a-z][a-z0-9_.:-]{1,99}$'
    ),
    artifacts_purged BIGINT NOT NULL CHECK (artifacts_purged >= 0),
    request_payloads_redacted BIGINT NOT NULL CHECK (request_payloads_redacted >= 0),
    privacy_outbox_rows_deleted BIGINT NOT NULL CHECK (privacy_outbox_rows_deleted >= 0),
    communication_evidence_redacted BIGINT NOT NULL CHECK (communication_evidence_redacted >= 0),
    communication_events_deleted BIGINT NOT NULL CHECK (communication_events_deleted >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (
        (actor_type = 'admin' AND actor_user_id IS NOT NULL AND universe_id IS NOT NULL)
        OR (actor_type = 'system' AND actor_user_id IS NULL)
    ),
    FOREIGN KEY (universe_id) REFERENCES universes(id) ON DELETE RESTRICT,
    CONSTRAINT privacy_retention_runs_actor_fkey
        FOREIGN KEY (universe_id, actor_user_id)
        REFERENCES users(universe_id, id) ON DELETE RESTRICT
);

-- Repair an early development form of the retention actor FK if migration 56
-- is re-applied to a disposable database.
ALTER TABLE privacy_retention_runs
    DROP CONSTRAINT IF EXISTS privacy_retention_runs_actor_user_id_fkey;
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'privacy_retention_runs'::regclass
          AND conname = 'privacy_retention_runs_actor_fkey'
    ) THEN
        ALTER TABLE privacy_retention_runs
            ADD CONSTRAINT privacy_retention_runs_actor_fkey
            FOREIGN KEY (universe_id, actor_user_id)
            REFERENCES users(universe_id, id) ON DELETE RESTRICT;
    END IF;
END $$;

-- Regulatory execution evidence is insert-only. Retention run summaries are
-- aggregate-only and also immutable.
DROP TRIGGER IF EXISTS privacy_correction_executions_immutable
    ON privacy_correction_executions;
CREATE TRIGGER privacy_correction_executions_immutable
    BEFORE UPDATE OR DELETE ON privacy_correction_executions
    FOR EACH ROW EXECUTE FUNCTION privacy_reject_evidence_mutation();

DROP TRIGGER IF EXISTS privacy_erasure_executions_immutable
    ON privacy_erasure_executions;
CREATE TRIGGER privacy_erasure_executions_immutable
    BEFORE UPDATE OR DELETE ON privacy_erasure_executions
    FOR EACH ROW EXECUTE FUNCTION privacy_reject_evidence_mutation();

DROP TRIGGER IF EXISTS privacy_execution_events_immutable
    ON privacy_execution_events;
CREATE TRIGGER privacy_execution_events_immutable
    BEFORE UPDATE OR DELETE ON privacy_execution_events
    FOR EACH ROW EXECUTE FUNCTION privacy_reject_evidence_mutation();

DROP TRIGGER IF EXISTS privacy_retention_runs_immutable
    ON privacy_retention_runs;
CREATE TRIGGER privacy_retention_runs_immutable
    BEFORE UPDATE OR DELETE ON privacy_retention_runs
    FOR EACH ROW EXECUTE FUNCTION privacy_reject_evidence_mutation();

-- Shared fail-closed predicate for every retention subsystem. Callers must
-- skip destructive or redaction work whenever this returns TRUE.
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

COMMENT ON FUNCTION privacy_subject_has_active_legal_hold(BIGINT, INTEGER) IS
    'Authoritative tenant/user legal-hold predicate; retention and erasure must fail closed when true.';
COMMENT ON TABLE privacy_erasure_executions IS
    'Exactly-once aggregate evidence that a subject erasure transaction completed; contains no raw PII.';
COMMENT ON TABLE privacy_correction_executions IS
    'Exactly-once evidence of the bounded profile fields changed by an approved correction.';
COMMENT ON TABLE privacy_execution_events IS
    'Append-only privacy execution and delivery evidence; no raw token, contact, or correction value is stored.';
