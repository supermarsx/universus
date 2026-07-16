use platform_db::{
    CommunicationPreferenceUpdate, ConsentStatus, ConsentUpdate, Database, EncryptedPrivacyPayload,
    PreparedExportArtifact, PrivacyAdminDecision, PrivacyAdminDecisionInput, PrivacyAuthGuard,
    PrivacyError, PrivacyRequestCreateInput, PrivacyRequestStatus, PrivacyRequestType,
    PRIVACY_EXPORT_DATA_INVENTORY,
};
use tokio::time::{sleep, Duration};
use tokio_postgres::{Client, NoTls};

const PRIVACY_SCHEMA: &str = include_str!("../../../database/sql/steps/52_privacy_lifecycle.sql");

#[derive(Debug, Clone, Copy)]
struct Actors {
    subject: i32,
    second_tenant_subject: i32,
    admin_one: i32,
    admin_two: i32,
    second_tenant_admin: i32,
}

fn request_input(
    universe_id: i64,
    user_id: i32,
    request_type: PrivacyRequestType,
    key: &str,
) -> PrivacyRequestCreateInput {
    PrivacyRequestCreateInput {
        universe_id,
        user_id,
        request_type,
        idempotency_key: key.to_string(),
        request_source: "integration_test".to_string(),
        requester_ip_digest: Some([3; 32]),
        encrypted_payload: None,
        erasure_cooling_off_seconds: Some(0),
    }
}

fn encrypted_request_input(user_id: i32, key: &str) -> PrivacyRequestCreateInput {
    let mut input = request_input(1, user_id, PrivacyRequestType::Correction, key);
    input.encrypted_payload = Some(EncryptedPrivacyPayload {
        ciphertext: vec![7, 8, 9, 10],
        key_id: "privacy-request-key-v1".to_string(),
        nonce: [4; 12],
        plaintext_sha256: [5; 32],
    });
    input
}

async fn seed_pre_privacy_schema(client: &Client) -> Actors {
    client
        .batch_execute("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
        .await
        .expect("reset disposable schema");
    apply_canonical_steps_before_privacy(client).await;
    client
        .execute(
            "INSERT INTO universes (id, name, speed, registration_open)
             VALUES (2, 'Second Tenant', 1, TRUE)
             ON CONFLICT (id) DO NOTHING",
            &[],
        )
        .await
        .unwrap();

    let rows = client
        .query(
            "INSERT INTO users (
                username, email, password_hash, universe_id, is_admin
             ) VALUES
                ('PrivacySubject', 'privacy-subject@example.test', '!test!', 1, FALSE),
                ('OtherTenantSubject', 'other-subject@example.test', '!test!', 2, FALSE),
                ('PrivacyAdminOne', 'privacy-admin-one@example.test', '!test!', 1, TRUE),
                ('PrivacyAdminTwo', 'privacy-admin-two@example.test', '!test!', 1, TRUE),
                ('OtherTenantAdmin', 'other-admin@example.test', '!test!', 2, TRUE)
             RETURNING id, username",
            &[],
        )
        .await
        .unwrap();
    let id = |name: &str| {
        rows.iter()
            .find(|row| row.get::<_, String>("username") == name)
            .unwrap()
            .get::<_, i32>("id")
    };
    let actors = Actors {
        subject: id("PrivacySubject"),
        second_tenant_subject: id("OtherTenantSubject"),
        admin_one: id("PrivacyAdminOne"),
        admin_two: id("PrivacyAdminTwo"),
        second_tenant_admin: id("OtherTenantAdmin"),
    };
    client
        .execute(
            "INSERT INTO gdpr_requests (
                user_id, request_type, status, data_url, notes
             ) VALUES ($1, 'export_data', 'pending',
                'https://legacy.invalid/download?bearer=legacy-secret-token',
                'legacy raw note containing subject data')",
            &[&actors.subject],
        )
        .await
        .unwrap();
    actors
}

async fn apply_canonical_steps_before_privacy(client: &Client) {
    let steps_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../database/sql/steps");
    let mut steps = std::fs::read_dir(&steps_dir)
        .expect("read canonical migration directory")
        .map(|entry| {
            let path = entry.expect("read migration entry").path();
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .expect("UTF-8 migration filename")
                .to_string();
            let version = file_name
                .split_once('_')
                .and_then(|(version, _)| version.parse::<u32>().ok());
            (version, file_name, path)
        })
        .filter_map(|(version, file_name, path)| {
            version
                .filter(|version| *version < 52)
                .map(|version| (version, file_name, path))
        })
        .collect::<Vec<_>>();
    steps.sort_by(|left, right| (left.0, &left.1).cmp(&(right.0, &right.1)));
    assert_eq!(steps.first().map(|step| step.0), Some(1));
    assert_eq!(steps.last().map(|step| step.0), Some(51));
    for duplicate in steps.windows(2) {
        assert_ne!(
            duplicate[0].0, duplicate[1].0,
            "duplicate migration version"
        );
    }
    for (version, file_name, path) in steps {
        let sql = std::fs::read_to_string(path).expect("read canonical migration SQL");
        client
            .batch_execute(&sql)
            .await
            .unwrap_or_else(|error| panic!("apply migration {version} {file_name}: {error:?}"));
    }
}

async fn seed_extended_export_inventory(client: &Client, actors: Actors) {
    let conversation_id: i32 = client
        .query_one(
            "INSERT INTO private_conversations (user1_id, user2_id)
             VALUES (LEAST($1::INTEGER, $2::INTEGER),
                     GREATEST($1::INTEGER, $2::INTEGER)) RETURNING id",
            &[&actors.subject, &actors.admin_one],
        )
        .await
        .unwrap()
        .get("id");
    client
        .execute(
            "INSERT INTO private_messages (conversation_id, sender_id, message)
             VALUES
                ($1, $2, 'subject-private-message'),
                ($1, $3, 'received-private-message')",
            &[&conversation_id, &actors.subject, &actors.admin_one],
        )
        .await
        .unwrap();
    let other_conversation_id: i32 = client
        .query_one(
            "INSERT INTO private_conversations (user1_id, user2_id)
             VALUES (LEAST($1::INTEGER, $2::INTEGER),
                     GREATEST($1::INTEGER, $2::INTEGER)) RETURNING id",
            &[&actors.second_tenant_subject, &actors.second_tenant_admin],
        )
        .await
        .unwrap()
        .get("id");
    client
        .execute(
            "INSERT INTO private_messages (conversation_id, sender_id, message)
             VALUES ($1, $2, 'other-tenant-private-message')",
            &[&other_conversation_id, &actors.second_tenant_subject],
        )
        .await
        .unwrap();

    let alliance_id: i32 = client
        .query_one(
            "INSERT INTO alliances (name, tag, founder_id, admin_notes, auto_application_notes)
             VALUES ('Privacy Alliance', 'PRIV', $1,
                     'alliance-admin-secret', 'alliance-application-secret')
             RETURNING id",
            &[&actors.admin_one],
        )
        .await
        .unwrap()
        .get("id");
    // Migration 11's alliance-member statistics trigger still reads the
    // removed users.score column. Bypass only that known legacy trigger while
    // creating this export fixture; a forward social/alliance migration owns
    // the repair, not the privacy lifecycle migration.
    client
        .batch_execute(&format!(
            "BEGIN;
             ALTER TABLE alliance_members DISABLE TRIGGER alliance_member_stats_update;
             INSERT INTO alliance_members (alliance_id, user_id, role)
             VALUES ({alliance_id}, {}, 'member');
             ALTER TABLE alliance_members ENABLE TRIGGER alliance_member_stats_update;
             COMMIT;",
            actors.subject
        ))
        .await
        .unwrap();
    client
        .execute(
            "INSERT INTO alliance_chat (alliance_id, user_id, message)
             VALUES ($1, $2, 'subject-alliance-chat')",
            &[&alliance_id, &actors.subject],
        )
        .await
        .unwrap();
    client
        .execute(
            "INSERT INTO alliance_messages (alliance_id, sender_id, content)
             VALUES ($1, $2, 'subject-alliance-message')",
            &[&alliance_id, &actors.subject],
        )
        .await
        .unwrap();

    let badge_id: i32 = client
        .query_one(
            "INSERT INTO badges (code, name, description)
             VALUES ('PRIVACY_BADGE', 'Privacy Badge', 'subject-badge-description')
             RETURNING id",
            &[],
        )
        .await
        .unwrap()
        .get("id");
    client
        .execute(
            "INSERT INTO user_badges (user_id, badge_id) VALUES ($1, $2)",
            &[&actors.subject, &badge_id],
        )
        .await
        .unwrap();
    let reward_id: i32 = client
        .query_one(
            "INSERT INTO rewards (code, name, description, reward_type, value)
             VALUES ('PRIVACY_REWARD', 'Privacy Reward',
                     'subject-reward-description', 'title', 1)
             RETURNING id",
            &[],
        )
        .await
        .unwrap()
        .get("id");
    client
        .execute(
            "INSERT INTO user_rewards (user_id, reward_id) VALUES ($1, $2)",
            &[&actors.subject, &reward_id],
        )
        .await
        .unwrap();

    client
        .execute(
            "INSERT INTO chat_restrictions (
                user_id, restriction_type, reason, restricted_by
             ) VALUES ($1, 'mute', 'subject-chat-restriction', $2)",
            &[&actors.subject, &actors.admin_one],
        )
        .await
        .unwrap();
    client
        .execute(
            "INSERT INTO user_blocks (
                user_id, block_type, reason, blocked_by, notes, severity_level
             ) VALUES ($1, 'warning', 'subject-account-warning', $2,
                       'account-block-admin-secret', 2)",
            &[&actors.subject, &actors.admin_one],
        )
        .await
        .unwrap();
    client
        .execute(
            "INSERT INTO shop_purchases_enhanced (
                user_id, item_type, item_id, price_usd, final_price,
                stripe_payment_intent_id, stripe_charge_id, device_type
             ) VALUES ($1, 'cosmetic', 'subject-enhanced-purchase', 499, 499,
                       'payment-intent-secret', 'payment-charge-secret', 'desktop')",
            &[&actors.subject],
        )
        .await
        .unwrap();
    client
        .execute(
            "INSERT INTO shop_purchases_enhanced (
                user_id, item_type, item_id, price_usd, final_price
             ) VALUES ($1, 'cosmetic', 'other-tenant-purchase', 299, 299)",
            &[&actors.second_tenant_subject],
        )
        .await
        .unwrap();

    client
        .execute(
            "INSERT INTO messages (
                from_user_id, to_user_id, subject, content, message_type, metadata
             ) VALUES ($1, $2, 'Export test', 'subject-standard-message',
                       'player_message',
                       '{\"password_hash\":\"nested-message-secret\",\"safe\":true}'::jsonb)",
            &[&actors.subject, &actors.admin_one],
        )
        .await
        .unwrap();
    client
        .execute(
            "INSERT INTO security_audit_logs (
                user_id, event_type, event_description, severity, metadata
             ) VALUES ($1, 'login', 'subject-security-event', 'low',
                       '{\"session_token\":\"nested-audit-secret\"}'::jsonb)",
            &[&actors.subject],
        )
        .await
        .unwrap();
    client
        .execute(
            "INSERT INTO admin_audit_logs (
                admin_id, admin_username, action_type, action_category,
                target_type, target_id, action_details
             ) VALUES
                ($1, 'PrivacyAdminOne', 'subject-user-audit', 'user_management',
                 'user-account', $2, '{\"session_token\":\"admin-audit-secret\"}'::jsonb),
                ($1, 'PrivacyAdminOne', 'numeric-resource-collision', 'data_modification',
                 'planet', $2, '{\"note\":\"must-not-export\"}'::jsonb)",
            &[&actors.admin_one, &actors.subject],
        )
        .await
        .unwrap();
}

fn assert_export_field_denylist(value: &serde_json::Value) {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                assert_export_field_denylist(value);
            }
        }
        serde_json::Value::Object(fields) => {
            for (name, value) in fields {
                let normalized = name.to_ascii_lowercase();
                assert!(
                    !normalized.contains("password"),
                    "exported forbidden key {name}"
                );
                assert!(
                    !normalized.contains("secret"),
                    "exported forbidden key {name}"
                );
                assert!(
                    !normalized.contains("token"),
                    "exported forbidden key {name}"
                );
                assert!(!matches!(
                    normalized.as_str(),
                    "backup_codes"
                        | "requester_ip_digest"
                        | "proof_digest"
                        | "request_payload_ciphertext"
                        | "payload_key_id"
                        | "payload_nonce"
                        | "payload_sha256"
                        | "stripe_payment_intent_id"
                        | "stripe_charge_id"
                        | "lease_owner"
                        | "lease_expires_at"
                        | "attempt_count"
                        | "max_attempts"
                        | "last_error_code"
                ));
                assert_export_field_denylist(value);
            }
        }
        _ => {}
    }
}

async fn expect_sql_rejected(client: &Client, sql: &str) {
    assert!(
        client.batch_execute(sql).await.is_err(),
        "database accepted adversarial SQL: {sql}"
    );
}

/// This test intentionally owns and resets the database named by
/// `UNIVERSUS_TEST_DATABASE_URL`; use only a disposable PostgreSQL database.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires disposable PostgreSQL in UNIVERSUS_TEST_DATABASE_URL"]
async fn privacy_lifecycle_repository_round_trip() {
    let database_url = std::env::var("UNIVERSUS_TEST_DATABASE_URL")
        .expect("UNIVERSUS_TEST_DATABASE_URL must name a disposable PostgreSQL database");
    let (client, connection) = tokio_postgres::connect(&database_url, NoTls)
        .await
        .expect("connect disposable PostgreSQL");
    tokio::spawn(async move {
        connection.await.expect("PostgreSQL test connection");
    });

    let actors = seed_pre_privacy_schema(&client).await;
    client
        .batch_execute(PRIVACY_SCHEMA)
        .await
        .expect("privacy migration first application");
    let legacy_before_repeat = client
        .query_one(
            "SELECT id, version, legacy_content_redacted_at IS NOT NULL AS redacted
             FROM gdpr_requests WHERE idempotency_key LIKE 'legacy:%'",
            &[],
        )
        .await
        .unwrap();
    let legacy_request_id: i32 = legacy_before_repeat.get("id");
    let legacy_version: i64 = legacy_before_repeat.get("version");
    assert!(legacy_before_repeat.get::<_, bool>("redacted"));
    assert_eq!(
        client
            .query_one(
                "SELECT COUNT(*)::BIGINT AS count
                 FROM information_schema.columns
                 WHERE table_schema = 'public' AND table_name = 'gdpr_requests'
                   AND column_name IN ('data_url', 'notes')",
                &[],
            )
            .await
            .unwrap()
            .get::<_, i64>("count"),
        0,
        "legacy raw URL/note columns must be retired"
    );
    let evidence_text: String = client
        .query_one(
            "SELECT COALESCE(string_agg(row_to_json(events)::TEXT, ''), '') AS evidence
             FROM privacy_request_events AS events",
            &[],
        )
        .await
        .unwrap()
        .get("evidence");
    assert!(!evidence_text.contains("legacy-secret-token"));
    assert!(!evidence_text.contains("legacy raw note"));

    client
        .batch_execute(PRIVACY_SCHEMA)
        .await
        .expect("privacy migration repeat application");
    let repeat = client
        .query_one(
            "SELECT version,
                    (SELECT COUNT(*) FROM privacy_request_events
                     WHERE request_id = gdpr_requests.id)::BIGINT AS event_count
             FROM gdpr_requests WHERE id = $1",
            &[&legacy_request_id],
        )
        .await
        .unwrap();
    assert_eq!(repeat.get::<_, i64>("version"), legacy_version);
    assert_eq!(repeat.get::<_, i64>("event_count"), 1);

    // Every request/evidence/job/artifact relationship rejects a request id
    // paired with another universe or subject, even over direct SQL.
    expect_sql_rejected(
        &client,
        &format!(
            "INSERT INTO privacy_request_events (
                request_id, universe_id, user_id, event_type, to_status,
                actor_type, request_version, dedupe_key
             ) VALUES ({legacy_request_id}, 2, {}, 'status_changed', 'pending',
                'system', 1, 'cross-tenant-event')",
            actors.second_tenant_subject
        ),
    )
    .await;
    expect_sql_rejected(
        &client,
        &format!(
            "INSERT INTO privacy_request_events (
                request_id, universe_id, user_id, event_type, to_status,
                actor_type, actor_user_id, request_version, dedupe_key
             ) VALUES ({legacy_request_id}, 1, {}, 'status_changed', 'pending',
                'user', {}, 1, 'forged-same-tenant-user-actor')",
            actors.subject, actors.admin_one
        ),
    )
    .await;
    expect_sql_rejected(
        &client,
        &format!(
            "INSERT INTO privacy_request_events (
                request_id, universe_id, user_id, event_type, to_status,
                actor_type, actor_user_id, request_version, dedupe_key
             ) VALUES ({legacy_request_id}, 1, {}, 'status_changed', 'pending',
                'admin', {}, 1, 'forged-non-admin-actor')",
            actors.subject, actors.subject
        ),
    )
    .await;
    expect_sql_rejected(
        &client,
        &format!(
            "INSERT INTO privacy_request_events (
                request_id, universe_id, user_id, event_type, to_status,
                actor_type, actor_user_id, request_version, dedupe_key
             ) VALUES ({legacy_request_id}, 1, {}, 'status_changed', 'pending',
                'worker', {}, 1, 'forged-worker-user-actor')",
            actors.subject, actors.admin_one
        ),
    )
    .await;
    expect_sql_rejected(
        &client,
        &format!(
            "INSERT INTO privacy_outbox (
                request_id, universe_id, user_id, event_type, dedupe_key
             ) VALUES ({legacy_request_id}, 2, {},
                'privacy.export.prepare', 'cross-tenant-outbox')",
            actors.second_tenant_subject
        ),
    )
    .await;
    expect_sql_rejected(
        &client,
        &format!(
            "INSERT INTO privacy_export_artifacts (
                request_id, universe_id, user_id, ciphertext, encryption_key_id,
                encryption_nonce, plaintext_sha256, plaintext_size, expires_at
             ) VALUES ({legacy_request_id}, 2, {}, decode('aa', 'hex'), 'key',
                decode(repeat('01', 12), 'hex'), decode(repeat('02', 32), 'hex'),
                1, now() + interval '1 day')",
            actors.second_tenant_subject
        ),
    )
    .await;
    expect_sql_rejected(
        &client,
        &format!(
            "INSERT INTO privacy_admin_decisions (
                request_id, universe_id, user_id, admin_user_id, decision, reason_code
             ) VALUES ({legacy_request_id}, 2, {}, {}, 'approve', 'cross_tenant')",
            actors.second_tenant_subject, actors.second_tenant_admin
        ),
    )
    .await;
    expect_sql_rejected(
        &client,
        &format!(
            "INSERT INTO privacy_request_events (
                request_id, universe_id, user_id, event_type, to_status,
                actor_type, actor_user_id, request_version, dedupe_key
             ) VALUES ({legacy_request_id}, 1, {}, 'status_changed', 'pending',
                'admin', {}, 1, 'cross-tenant-actor')",
            actors.subject, actors.second_tenant_admin
        ),
    )
    .await;
    expect_sql_rejected(
        &client,
        &format!(
            "INSERT INTO privacy_consent_events (
                universe_id, user_id, purpose, channel, status, lawful_basis,
                policy_version, changed_by_user_id, actor_type, consent_version
             ) VALUES (1, {}, 'marketing', 'email', 'denied', 'consent',
                'v1', {}, 'admin', 99)",
            actors.subject, actors.second_tenant_admin
        ),
    )
    .await;
    expect_sql_rejected(
        &client,
        &format!(
            "INSERT INTO privacy_communication_preference_events (
                universe_id, user_id, channel, category, enabled,
                changed_by_user_id, actor_type, preference_version
             ) VALUES (1, {}, 'email', 'marketing', FALSE,
                {}, 'admin', 99)",
            actors.subject, actors.second_tenant_admin
        ),
    )
    .await;

    let database = Database::from_database_url(&database_url).unwrap();
    database
        .privacy_repository_ready()
        .await
        .expect("privacy repository readiness");
    assert_eq!(
        database
            .privacy_auth_guard(1, actors.subject, 0)
            .await
            .unwrap(),
        PrivacyAuthGuard::Allowed
    );

    // Repository ownership is tenant + subject, and idempotency returns the
    // same request without duplicating its outbox delivery.
    let export_request = database
        .create_privacy_request(request_input(
            1,
            actors.subject,
            PrivacyRequestType::Export,
            "export-idempotency",
        ))
        .await
        .unwrap();
    let repeated_export = database
        .create_privacy_request(request_input(
            1,
            actors.subject,
            PrivacyRequestType::Export,
            "export-idempotency",
        ))
        .await
        .unwrap();
    assert_eq!(export_request.id, repeated_export.id);
    assert!(matches!(
        database
            .create_privacy_request(request_input(
                1,
                actors.subject,
                PrivacyRequestType::Correction,
                "export-idempotency",
            ))
            .await,
        Err(PrivacyError::Conflict(_))
    ));
    assert!(database
        .privacy_request_for_owner(2, actors.second_tenant_subject, export_request.id)
        .await
        .unwrap()
        .is_none());
    assert!(database
        .create_privacy_request(request_input(
            2,
            actors.subject,
            PrivacyRequestType::Export,
            "mismatched-tenant-owner",
        ))
        .await
        .is_err());
    assert_eq!(
        client
            .query_one(
                "SELECT COUNT(*)::BIGINT AS count FROM privacy_outbox
                 WHERE request_id = $1",
                &[&export_request.id],
            )
            .await
            .unwrap()
            .get::<_, i64>("count"),
        1
    );
    database
        .cancel_privacy_request(1, actors.subject, export_request.id, "user_cancelled")
        .await
        .unwrap();

    // A user cannot mutate another subject's consent/preferences. Marketing
    // requires both an enabled preference and current explicit consent.
    let impersonated_consent = ConsentUpdate {
        universe_id: 1,
        user_id: actors.subject,
        purpose: "marketing".to_string(),
        channel: "email".to_string(),
        status: ConsentStatus::Granted,
        lawful_basis: "consent".to_string(),
        policy_version: "privacy-v1".to_string(),
        proof_digest: Some([8; 32]),
        expires_at_unix: None,
        changed_by_user_id: actors.admin_one,
        actor_type: "user".to_string(),
    };
    assert!(database
        .set_privacy_consent(impersonated_consent)
        .await
        .is_err());
    assert!(database
        .set_communication_preference(CommunicationPreferenceUpdate {
            universe_id: 1,
            user_id: actors.subject,
            channel: "email".to_string(),
            category: "marketing".to_string(),
            enabled: true,
            changed_by_user_id: actors.admin_one,
            actor_type: "user".to_string(),
        })
        .await
        .is_err());
    database
        .set_communication_preference(CommunicationPreferenceUpdate {
            universe_id: 1,
            user_id: actors.subject,
            channel: "email".to_string(),
            category: "marketing".to_string(),
            enabled: true,
            changed_by_user_id: actors.subject,
            actor_type: "user".to_string(),
        })
        .await
        .unwrap();
    assert!(!database
        .communication_allowed(1, actors.subject, "email", "marketing")
        .await
        .unwrap());
    let consent = |status| ConsentUpdate {
        universe_id: 1,
        user_id: actors.subject,
        purpose: "marketing".to_string(),
        channel: "email".to_string(),
        status,
        lawful_basis: "consent".to_string(),
        policy_version: "privacy-v1".to_string(),
        proof_digest: Some([8; 32]),
        expires_at_unix: None,
        changed_by_user_id: actors.subject,
        actor_type: "user".to_string(),
    };
    database
        .set_privacy_consent(consent(ConsentStatus::Granted))
        .await
        .unwrap();
    assert!(database
        .communication_allowed(1, actors.subject, "email", "marketing")
        .await
        .unwrap());
    database
        .set_privacy_consent(consent(ConsentStatus::Withdrawn))
        .await
        .unwrap();
    assert!(!database
        .communication_allowed(1, actors.subject, "email", "marketing")
        .await
        .unwrap());
    expect_sql_rejected(
        &client,
        "UPDATE privacy_consent_events SET status = 'granted' WHERE id = (
            SELECT MIN(id) FROM privacy_consent_events
         )",
    )
    .await;

    // Only one concurrent worker claims the restriction job. An expired lease
    // is recovered after restart; the stale worker cannot commit side effects.
    let restriction = database
        .create_privacy_request(request_input(
            1,
            actors.subject,
            PrivacyRequestType::Restriction,
            "restriction-restart",
        ))
        .await
        .unwrap();
    client
        .execute(
            "INSERT INTO user_sessions (
                user_id, session_token, device_name, ip_address, status, expires_at
             ) VALUES
                ($1, 'subject-session-secret', 'subject-device', '192.0.2.10',
                 'active', now() + interval '1 day'),
                ($2, 'other-session-secret', 'other-tenant-device', '192.0.2.20',
                 'active', now() + interval '1 day')",
            &[&actors.subject, &actors.second_tenant_subject],
        )
        .await
        .unwrap();
    let database_b = database.clone();
    let (claim_a, claim_b) = tokio::join!(
        database.claim_privacy_jobs("worker-a", Some(1), 1, 1),
        database_b.claim_privacy_jobs("worker-b", Some(1), 1, 1)
    );
    let claims_a = claim_a.unwrap();
    let claims_b = claim_b.unwrap();
    assert_eq!(claims_a.len() + claims_b.len(), 1);
    let first_claim = claims_a
        .first()
        .or_else(|| claims_b.first())
        .unwrap()
        .clone();
    assert_eq!(first_claim.request_id, restriction.id);
    sleep(Duration::from_secs(2)).await;
    let recovered = database
        .claim_privacy_jobs("worker-restarted", Some(1), 1, 30)
        .await
        .unwrap();
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].id, first_claim.id);
    assert_eq!(recovered[0].attempt_count, 2);
    assert!(matches!(
        database
            .complete_privacy_restriction_job(first_claim.id, "worker-a")
            .await,
        Err(PrivacyError::LeaseLost)
    ));
    assert!(database
        .complete_privacy_restriction_job(first_claim.id, "worker-restarted")
        .await
        .unwrap());
    assert_eq!(
        database
            .privacy_auth_guard(1, actors.subject, 0)
            .await
            .unwrap(),
        PrivacyAuthGuard::StaleEpoch
    );
    assert_eq!(
        database
            .privacy_auth_guard(1, actors.subject, 1)
            .await
            .unwrap(),
        PrivacyAuthGuard::Restricted
    );
    assert_eq!(
        client
            .query_one(
                "SELECT status FROM user_sessions WHERE session_token = 'subject-session-secret'",
                &[],
            )
            .await
            .unwrap()
            .get::<_, String>("status"),
        "revoked"
    );

    // Legal hold blocks erasure. Repeating approval by one administrator is
    // idempotent and cannot satisfy dual control; a second distinct tenant
    // administrator is required.
    let erasure = database
        .create_privacy_request(request_input(
            1,
            actors.subject,
            PrivacyRequestType::Erasure,
            "dual-control-erasure",
        ))
        .await
        .unwrap();
    let decision = |admin_user_id, decision, reason: &str| PrivacyAdminDecisionInput {
        universe_id: 1,
        request_id: erasure.id,
        admin_user_id,
        decision,
        reason_code: reason.to_string(),
    };
    database
        .record_privacy_admin_decision(decision(
            actors.admin_one,
            PrivacyAdminDecision::ApplyLegalHold,
            "litigation_hold",
        ))
        .await
        .unwrap();
    assert!(matches!(
        database
            .record_privacy_admin_decision(decision(
                actors.admin_two,
                PrivacyAdminDecision::Approve,
                "approve_erasure"
            ))
            .await,
        Err(PrivacyError::LegalHold)
    ));
    database
        .record_privacy_admin_decision(decision(
            actors.admin_two,
            PrivacyAdminDecision::ReleaseLegalHold,
            "hold_released",
        ))
        .await
        .unwrap();
    let first_approval = database
        .record_privacy_admin_decision(decision(
            actors.admin_one,
            PrivacyAdminDecision::Approve,
            "approve_erasure",
        ))
        .await
        .unwrap();
    assert_eq!(first_approval.status, PrivacyRequestStatus::InReview);
    let repeated_approval = database
        .record_privacy_admin_decision(decision(
            actors.admin_one,
            PrivacyAdminDecision::Approve,
            "approve_erasure",
        ))
        .await
        .unwrap();
    assert_eq!(repeated_approval.status, PrivacyRequestStatus::InReview);
    assert_eq!(
        client
            .query_one(
                "SELECT COUNT(DISTINCT admin_user_id)::BIGINT AS count
                 FROM privacy_admin_decisions
                 WHERE request_id = $1 AND decision = 'approve'",
                &[&erasure.id],
            )
            .await
            .unwrap()
            .get::<_, i64>("count"),
        1
    );
    let queued_erasure = database
        .record_privacy_admin_decision(decision(
            actors.admin_two,
            PrivacyAdminDecision::Approve,
            "approve_erasure",
        ))
        .await
        .unwrap();
    assert_eq!(queued_erasure.status, PrivacyRequestStatus::Queued);
    assert!(database
        .record_privacy_admin_decision(PrivacyAdminDecisionInput {
            universe_id: 1,
            request_id: erasure.id,
            admin_user_id: actors.second_tenant_admin,
            decision: PrivacyAdminDecision::Approve,
            reason_code: "cross_tenant_admin".to_string(),
        })
        .await
        .is_err());
    let erasure_job = database
        .claim_privacy_jobs("erasure-worker", Some(1), 1, 30)
        .await
        .unwrap();
    assert_eq!(erasure_job.len(), 1);
    assert_eq!(erasure_job[0].request_id, erasure.id);
    assert!(database
        .complete_erasure_authorization_job(erasure_job[0].id, "erasure-worker")
        .await
        .unwrap());

    // The subject-access snapshot includes durable gameplay/account categories
    // while omitting password/session/reset/export secrets and other tenants.
    client
        .execute(
            "INSERT INTO planets (
                user_id, universe_id, name, galaxy, system, position, metal, crystal, deuterium
             ) VALUES ($1, 1, 'Privacy Prime', 7, 7, 7, 1234, 567, 89)",
            &[&actors.subject],
        )
        .await
        .unwrap();
    client
        .execute(
            "INSERT INTO research (user_id, energy_technology) VALUES ($1, 3)
             ON CONFLICT (user_id) DO UPDATE SET energy_technology = 3",
            &[&actors.subject],
        )
        .await
        .unwrap();
    seed_extended_export_inventory(&client, actors).await;
    let export_two = database
        .create_privacy_request(request_input(
            1,
            actors.subject,
            PrivacyRequestType::Export,
            "secure-export-delivery",
        ))
        .await
        .unwrap();
    let export_job = database
        .claim_privacy_jobs("export-worker", Some(1), 1, 30)
        .await
        .unwrap();
    assert_eq!(export_job.len(), 1);
    assert_eq!(export_job[0].request_id, export_two.id);
    let snapshot = database
        .privacy_export_snapshot(1, actors.subject)
        .await
        .unwrap();
    let snapshot_text = serde_json::to_string(&snapshot).unwrap();
    assert_eq!(
        snapshot["inventory"].as_array().unwrap().len(),
        PRIVACY_EXPORT_DATA_INVENTORY.len()
    );
    assert_eq!(snapshot["planets"][0]["metal"], 1234);
    assert_eq!(snapshot["research"][0]["energy_technology"], 3);
    for key in [
        "privateConversations",
        "privateMessages",
        "allianceChat",
        "allianceMessages",
        "allianceMemberships",
        "badges",
        "rewards",
        "chatRestrictions",
        "accountBlocks",
        "enhancedPurchases",
        "adminAudit",
    ] {
        assert!(
            snapshot[key]
                .as_array()
                .is_some_and(|rows| !rows.is_empty()),
            "full-schema export source {key} was not populated"
        );
    }
    for section in snapshot.as_object().unwrap().values() {
        assert_export_field_denylist(section);
    }
    assert!(snapshot_text.contains("subject-device"));
    assert!(snapshot_text.contains("subject-private-message"));
    assert!(snapshot_text.contains("received-private-message"));
    assert!(snapshot_text.contains("subject-alliance-chat"));
    assert!(snapshot_text.contains("subject-alliance-message"));
    assert!(snapshot_text.contains("subject-badge-description"));
    assert!(snapshot_text.contains("subject-reward-description"));
    assert!(snapshot_text.contains("subject-chat-restriction"));
    assert!(snapshot_text.contains("subject-account-warning"));
    assert!(snapshot_text.contains("subject-enhanced-purchase"));
    assert!(snapshot_text.contains("subject-user-audit"));
    assert!(!snapshot_text.contains("other-tenant-device"));
    assert!(!snapshot_text.contains("other-tenant-private-message"));
    assert!(!snapshot_text.contains("other-tenant-purchase"));
    assert!(!snapshot_text.contains("numeric-resource-collision"));
    assert!(!snapshot_text.contains("subject-session-secret"));
    assert!(!snapshot_text.contains("nested-message-secret"));
    assert!(!snapshot_text.contains("nested-audit-secret"));
    assert!(!snapshot_text.contains("admin-audit-secret"));
    assert!(!snapshot_text.contains("alliance-admin-secret"));
    assert!(!snapshot_text.contains("alliance-application-secret"));
    assert!(!snapshot_text.contains("account-block-admin-secret"));
    assert!(!snapshot_text.contains("payment-intent-secret"));
    assert!(!snapshot_text.contains("payment-charge-secret"));
    assert!(!snapshot_text.contains("password_hash"));
    assert!(!snapshot_text.contains("verification_token"));
    assert!(!snapshot_text.contains("reset_token"));
    assert!(!snapshot_text.contains("download_token_digest"));
    database
        .complete_privacy_export_job(
            export_job[0].id,
            "export-worker",
            PreparedExportArtifact {
                ciphertext: vec![9, 8, 7, 6],
                encryption_key_id: "export-key-v1".to_string(),
                encryption_nonce: [2; 12],
                plaintext_sha256: [6; 32],
                plaintext_size: snapshot_text.len() as i64,
                expires_in_seconds: 3600,
            },
        )
        .await
        .unwrap();
    let grant = database
        .issue_export_delivery(1, actors.subject, export_two.id, 600)
        .await
        .unwrap();
    let stored_digest: Vec<u8> = client
        .query_one(
            "SELECT download_token_digest FROM privacy_export_artifacts
             WHERE request_id = $1",
            &[&export_two.id],
        )
        .await
        .unwrap()
        .get("download_token_digest");
    assert_eq!(stored_digest.len(), 32);
    assert_ne!(stored_digest, grant.token.as_bytes());
    assert!(matches!(
        database
            .consume_export_delivery(1, actors.subject, export_two.id, "wrong-token")
            .await,
        Err(PrivacyError::DeliveryDenied)
    ));
    let download = database
        .consume_export_delivery(1, actors.subject, export_two.id, &grant.token)
        .await
        .unwrap();
    assert_eq!(download.ciphertext, vec![9, 8, 7, 6]);
    assert!(matches!(
        database
            .consume_export_delivery(1, actors.subject, export_two.id, &grant.token)
            .await,
        Err(PrivacyError::DeliveryDenied)
    ));

    // Retention scrubs expired encrypted content but preserves a request under
    // legal hold. The redaction itself is immutable lifecycle evidence.
    let terminal_payload = database
        .create_privacy_request(encrypted_request_input(
            actors.subject,
            "terminal-encrypted-correction",
        ))
        .await
        .unwrap();
    database
        .record_privacy_admin_decision(PrivacyAdminDecisionInput {
            universe_id: 1,
            request_id: terminal_payload.id,
            admin_user_id: actors.admin_one,
            decision: PrivacyAdminDecision::CompleteCorrection,
            reason_code: "correction_applied".to_string(),
        })
        .await
        .unwrap();
    let held_payload = database
        .create_privacy_request(encrypted_request_input(
            actors.subject,
            "held-encrypted-correction",
        ))
        .await
        .unwrap();
    database
        .record_privacy_admin_decision(PrivacyAdminDecisionInput {
            universe_id: 1,
            request_id: held_payload.id,
            admin_user_id: actors.admin_one,
            decision: PrivacyAdminDecision::ApplyLegalHold,
            reason_code: "regulatory_hold".to_string(),
        })
        .await
        .unwrap();
    client
        .execute(
            "UPDATE gdpr_requests SET retention_until = now() - interval '1 day'
             WHERE id IN ($1, $2)",
            &[&terminal_payload.id, &held_payload.id],
        )
        .await
        .unwrap();
    client
        .execute(
            "UPDATE privacy_export_artifacts
             SET expires_at = now() - interval '1 day'
             WHERE request_id = $1",
            &[&export_two.id],
        )
        .await
        .unwrap();
    let retention = database.purge_privacy_retention(1).await.unwrap();
    assert_eq!(retention.artifacts_purged, 1);
    assert_eq!(retention.request_payloads_redacted, 1);
    let payloads = client
        .query(
            "SELECT id, request_payload_ciphertext IS NOT NULL AS retained
             FROM gdpr_requests WHERE id IN ($1, $2) ORDER BY id",
            &[&terminal_payload.id, &held_payload.id],
        )
        .await
        .unwrap();
    assert!(!payloads
        .iter()
        .find(|row| row.get::<_, i32>("id") == terminal_payload.id)
        .unwrap()
        .get::<_, bool>("retained"));
    assert!(payloads
        .iter()
        .find(|row| row.get::<_, i32>("id") == held_payload.id)
        .unwrap()
        .get::<_, bool>("retained"));
    assert_eq!(
        client
            .query_one(
                "SELECT COUNT(*)::BIGINT AS count FROM privacy_request_events
                 WHERE request_id = $1 AND event_type = 'payload_redacted'",
                &[&terminal_payload.id],
            )
            .await
            .unwrap()
            .get::<_, i64>("count"),
        1
    );
    expect_sql_rejected(
        &client,
        "DELETE FROM privacy_request_events WHERE id = (
            SELECT MIN(id) FROM privacy_request_events
         )",
    )
    .await;
}
