use std::path::Path;

use platform_db::{
    CommunicationActor, CommunicationCategory, CommunicationChannel, CommunicationEnqueueInput,
    CommunicationError, CommunicationEvidenceKey, CommunicationPolicyInput, CommunicationState,
    Database, COMMUNICATION_SCOPE_AUDIT_READ, COMMUNICATION_SCOPE_CONTACT_VERIFY,
    COMMUNICATION_SCOPE_DISPATCH, COMMUNICATION_SCOPE_ENQUEUE, COMMUNICATION_SCOPE_GLOBAL,
    COMMUNICATION_SCOPE_POLICY_WRITE, COMMUNICATION_SCOPE_RETENTION,
};
use tokio_postgres::{Client, NoTls};
use zeroize::Zeroizing;

async fn connect(database_url: &str) -> (Client, tokio::task::JoinHandle<()>) {
    let (client, connection) = tokio_postgres::connect(database_url, NoTls)
        .await
        .expect("connect to disposable PostgreSQL");
    let connection = tokio::spawn(async move {
        connection
            .await
            .expect("PostgreSQL connection remains healthy");
    });
    (client, connection)
}

async fn reset_and_apply_through_communications(client: &Client) {
    client
        .batch_execute("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
        .await
        .expect("reset disposable schema");
    let steps_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../database/sql/steps");
    let mut steps = std::fs::read_dir(&steps_dir)
        .expect("read migration directory")
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            let name = path.file_name()?.to_str()?.to_string();
            let version = name.split_once('_')?.0.parse::<u32>().ok()?;
            (version <= 54).then_some((version, name, path))
        })
        .collect::<Vec<_>>();
    steps.sort_by(|left, right| (left.0, &left.1).cmp(&(right.0, &right.1)));
    for (version, name, path) in steps {
        let sql = std::fs::read_to_string(path).expect("read migration SQL");
        client
            .batch_execute(&sql)
            .await
            .unwrap_or_else(|error| panic!("apply migration {version} {name}: {error:?}"));
    }

    // The schema step must be safe to replay during interrupted deploy recovery.
    client
        .batch_execute(include_str!(
            "../../../database/sql/steps/54_durable_communications_outbox.sql"
        ))
        .await
        .expect("repeat communication migration");
}

async fn seed_subjects(client: &Client) -> (i32, i32, i32) {
    let rows = client
        .query(
            "INSERT INTO users (username, email, password_hash, universe_id, is_admin)
             VALUES
                ('CommunicationHeld', 'held-person@example.test', '!test!', 1, FALSE),
                ('CommunicationFree', 'free-person@example.test', '!test!', 1, FALSE),
                ('CommunicationAdmin', 'communication-admin@example.test', '!test!', 1, TRUE)
             RETURNING id, username",
            &[],
        )
        .await
        .expect("seed communication subjects");
    let id = |name: &str| {
        rows.iter()
            .find(|row| row.get::<_, String>("username") == name)
            .expect("seeded actor")
            .get::<_, i32>("id")
    };
    (
        id("CommunicationHeld"),
        id("CommunicationFree"),
        id("CommunicationAdmin"),
    )
}

fn tenant_actor() -> CommunicationActor {
    CommunicationActor::authenticated_service(
        "service:communications-test",
        1,
        [
            COMMUNICATION_SCOPE_ENQUEUE,
            COMMUNICATION_SCOPE_DISPATCH,
            COMMUNICATION_SCOPE_AUDIT_READ,
            COMMUNICATION_SCOPE_POLICY_WRITE,
            COMMUNICATION_SCOPE_CONTACT_VERIFY,
        ],
    )
    .expect("tenant communication actor")
}

fn global_retention_actor() -> CommunicationActor {
    CommunicationActor::authenticated_global_service(
        "service:communications-retention-test",
        [COMMUNICATION_SCOPE_RETENTION, COMMUNICATION_SCOPE_GLOBAL],
    )
    .expect("global retention actor")
}

fn enqueue_input(user_id: i32, suffix: &str, max_attempts: i32) -> CommunicationEnqueueInput {
    let authoritative_suffix = suffix
        .chars()
        .filter(|character| character.is_ascii_hexdigit() || *character == '-')
        .collect::<String>();
    CommunicationEnqueueInput {
        universe_id: 1,
        user_id,
        channel: CommunicationChannel::Email,
        category: CommunicationCategory::Transactional,
        template_key: "welcome".to_string(),
        payload_identity: format!("account_event:{authoritative_suffix}"),
        idempotency_key: format!("communication:test:{suffix}"),
        max_attempts,
    }
}

/// Destructively validates migration replay, tenant authority, dedupe, policy
/// CAS, leases/restart/reclaim, snapshot tamper resistance, contact evidence,
/// append-only audit, legal holds, and bounded retention against real PostgreSQL.
#[tokio::test]
#[ignore = "requires disposable PostgreSQL in UNIVERSUS_TEST_DATABASE_URL"]
async fn durable_communications_are_tenant_scoped_restart_safe_and_privacy_enforced() {
    let database_url = std::env::var("UNIVERSUS_TEST_DATABASE_URL")
        .expect("UNIVERSUS_TEST_DATABASE_URL must name a disposable PostgreSQL database");
    let (client, connection) = connect(&database_url).await;
    reset_and_apply_through_communications(&client).await;
    let (held_user, free_user, admin_user) = seed_subjects(&client).await;
    let database = Database::from_database_url(&database_url).expect("database pool");
    database
        .communication_repository_ready()
        .await
        .expect("communication repository ready");
    let actor = tenant_actor();
    let evidence_key = CommunicationEvidenceKey::new(vec![17; 32]).expect("evidence key");
    let invalid_hold_keys = client
        .query_one(
            "SELECT
                privacy_subject_has_active_legal_hold(NULL, 1) AS null_tenant,
                privacy_subject_has_active_legal_hold(1, 0) AS invalid_user",
            &[],
        )
        .await
        .expect("legal-hold predicate is available in migration 54");
    assert!(invalid_hold_keys.get::<_, bool>("null_tenant"));
    assert!(invalid_hold_keys.get::<_, bool>("invalid_user"));

    let wrong_tenant = CommunicationActor::authenticated_service(
        "service:wrong-tenant",
        2,
        [COMMUNICATION_SCOPE_ENQUEUE],
    )
    .unwrap();
    assert_eq!(
        database
            .enqueue_communication(
                enqueue_input(free_user, "authority-0001", 3),
                &wrong_tenant,
                &evidence_key
            )
            .await,
        Err(CommunicationError::Unauthorized)
    );

    let policy_version = database
        .set_communication_policy(
            CommunicationPolicyInput {
                universe_id: 1,
                channel: CommunicationChannel::Email,
                category: CommunicationCategory::Transactional,
                provider_key: "email_http".to_string(),
                enabled: true,
                expected_version: None,
            },
            "initial_provider_policy",
            &actor,
            &evidence_key,
        )
        .await
        .expect("create channel policy");
    assert_eq!(policy_version, 1);
    assert!(matches!(
        database
            .set_communication_policy(
                CommunicationPolicyInput {
                    universe_id: 1,
                    channel: CommunicationChannel::Email,
                    category: CommunicationCategory::Transactional,
                    provider_key: "email_http".to_string(),
                    enabled: false,
                    expected_version: None,
                },
                "stale_policy_write",
                &actor,
                &evidence_key,
            )
            .await,
        Err(CommunicationError::Conflict(_))
    ));
    assert_eq!(
        database
            .set_communication_policy(
                CommunicationPolicyInput {
                    universe_id: 1,
                    channel: CommunicationChannel::Email,
                    category: CommunicationCategory::Transactional,
                    provider_key: "email_http".to_string(),
                    enabled: true,
                    expected_version: Some(policy_version),
                },
                "confirmed_provider_policy",
                &actor,
                &evidence_key,
            )
            .await
            .expect("CAS policy update"),
        2
    );

    for user_id in [held_user, free_user] {
        let masked = database
            .record_current_verified_contact(
                1,
                user_id,
                CommunicationChannel::Email,
                "account_challenge",
                "challenge_completed",
                3_600,
                &actor,
                &evidence_key,
            )
            .await
            .expect("record durable verified contact");
        assert!(masked.ends_with("@e***.t***"));
        assert!(!masked.contains("person"));
    }

    let unaudited_change = client
        .execute(
            "UPDATE users SET email = 'unaudited@example.test' WHERE universe_id = 1 AND id = $1",
            &[&free_user],
        )
        .await
        .expect_err("unaudited contact mutation must fail closed");
    assert_eq!(
        unaudited_change.code().map(|code| code.code()),
        Some("42501")
    );

    let first = database
        .enqueue_communication(
            enqueue_input(free_user, "dedupe-0001", 3),
            &actor,
            &evidence_key,
        )
        .await
        .expect("enqueue communication");
    assert!(!first.idempotent_replay);
    let replay = database
        .enqueue_communication(
            enqueue_input(free_user, "dedupe-0001", 3),
            &actor,
            &evidence_key,
        )
        .await
        .expect("idempotent replay");
    assert!(replay.idempotent_replay);
    assert_eq!(replay.job.id, first.job.id);
    assert!(matches!(
        database
            .enqueue_communication(
                enqueue_input(free_user, "dedupe-0001", 4),
                &actor,
                &evidence_key
            )
            .await,
        Err(CommunicationError::Conflict(_))
    ));
    let other_user_same_key = database
        .enqueue_communication(
            enqueue_input(held_user, "dedupe-0001", 3),
            &actor,
            &evidence_key,
        )
        .await
        .expect("idempotency is user scoped");
    assert_ne!(other_user_same_key.job.id, first.job.id);

    let left_database = database.clone();
    let right_database = database.clone();
    let left_actor = actor.clone();
    let right_actor = actor.clone();
    let left_key = evidence_key.clone();
    let right_key = evidence_key.clone();
    let (left, right) = tokio::join!(
        left_database.claim_communications(
            1,
            CommunicationChannel::Email,
            "worker-left",
            1,
            30,
            &left_actor,
            &left_key
        ),
        right_database.claim_communications(
            1,
            CommunicationChannel::Email,
            "worker-right",
            1,
            30,
            &right_actor,
            &right_key
        )
    );
    let mut claimed = left.unwrap();
    claimed.extend(right.unwrap());
    assert_eq!(
        claimed.len(),
        2,
        "each worker atomically claims a distinct job"
    );
    assert_ne!(claimed[0].id, claimed[1].id);

    let active = claimed
        .iter()
        .find(|job| job.id == first.job.id)
        .cloned()
        .expect("free subject job claimed");
    let worker = active.lease_owner.clone().expect("lease owner");
    let renewed = database
        .renew_communication_lease(&active, &worker, 30, &actor)
        .await
        .expect("renew active lease before dispatch");
    let policy = database
        .communication_delivery_policy(&renewed, &actor)
        .await
        .expect("read delivery policy")
        .expect("policy enabled");
    assert_eq!(policy.provider_key, "email_http");
    let contact = database
        .resolve_verified_communication_contact(&renewed, &actor, &evidence_key)
        .await
        .expect("resolve verified contact")
        .expect("verified evidence matches current contact");
    assert_eq!(contact.destination.as_str(), "free-person@example.test");

    let mut tampered = renewed.clone();
    tampered.template_key = "password_reset".to_string();
    assert!(matches!(
        database
            .communication_delivery_policy(&tampered, &actor)
            .await,
        Err(CommunicationError::Conflict(_))
    ));
    assert!(matches!(
        database
            .mark_communication_sent(
                &tampered,
                &worker,
                "email_http",
                "provider-tampered",
                contact.destination_hmac,
                &contact.destination_masked,
                &actor,
                &evidence_key,
            )
            .await,
        Err(CommunicationError::Conflict(_))
    ));

    client
        .execute(
            "UPDATE communication_outbox SET lease_until = now() - INTERVAL '1 second' WHERE id = $1",
            &[&renewed.id],
        )
        .await
        .expect("expire worker lease");
    let restarted_database = Database::from_database_url(&database_url).expect("restarted pool");
    let reclaimed = restarted_database
        .claim_communications(
            1,
            CommunicationChannel::Email,
            "worker-restarted",
            1,
            30,
            &actor,
            &evidence_key,
        )
        .await
        .expect("claim after restart")
        .pop()
        .expect("expired lease reclaimed");
    assert_eq!(reclaimed.id, renewed.id);
    assert!(matches!(
        database
            .mark_communication_sent(
                &renewed,
                &worker,
                "email_http",
                "provider-stale",
                contact.destination_hmac,
                &contact.destination_masked,
                &actor,
                &evidence_key,
            )
            .await,
        Err(CommunicationError::Conflict(_))
    ));
    let reclaimed_contact = restarted_database
        .resolve_verified_communication_contact(&reclaimed, &actor, &evidence_key)
        .await
        .expect("re-evaluate contact after reclaim")
        .expect("contact remains verified");
    let sent = restarted_database
        .mark_communication_sent(
            &reclaimed,
            "worker-restarted",
            "email_http",
            "provider-durable-idempotent-receipt",
            reclaimed_contact.destination_hmac,
            &reclaimed_contact.destination_masked,
            &actor,
            &evidence_key,
        )
        .await
        .expect("finish reclaimed communication");
    assert_eq!(sent.state, CommunicationState::Sent);

    let exhausted = database
        .enqueue_communication(
            enqueue_input(free_user, "attempts-0001", 1),
            &actor,
            &evidence_key,
        )
        .await
        .unwrap();
    let exhausted = database
        .claim_communications(
            1,
            CommunicationChannel::Email,
            "worker-attempts",
            10,
            30,
            &actor,
            &evidence_key,
        )
        .await
        .unwrap()
        .into_iter()
        .find(|job| job.id == exhausted.job.id)
        .expect("attempt-limited job claimed");
    let dead = database
        .fail_communication_attempt(
            &exhausted,
            "worker-attempts",
            "email_http",
            "provider_unreachable",
            0,
            &actor,
            &evidence_key,
        )
        .await
        .expect("maximum attempt transitions dead");
    assert_eq!(dead.state, CommunicationState::Dead);

    let expiry_limited = database
        .enqueue_communication(
            enqueue_input(free_user, "expiry-attempts-0001", 1),
            &actor,
            &evidence_key,
        )
        .await
        .unwrap();
    let expiry_limited = database
        .claim_communications(
            1,
            CommunicationChannel::Email,
            "worker-expiry-attempts",
            10,
            30,
            &actor,
            &evidence_key,
        )
        .await
        .unwrap()
        .into_iter()
        .find(|job| job.id == expiry_limited.job.id)
        .expect("expiry-limited job claimed once");
    client
        .execute(
            "UPDATE communication_outbox SET lease_until = now() - INTERVAL '1 second'
             WHERE id = $1",
            &[&expiry_limited.id],
        )
        .await
        .unwrap();
    let past_limit = database
        .claim_communications(
            1,
            CommunicationChannel::Email,
            "worker-must-not-dispatch",
            10,
            30,
            &actor,
            &evidence_key,
        )
        .await
        .expect("expired maximum attempt is dead-lettered");
    assert!(!past_limit.iter().any(|job| job.id == expiry_limited.id));
    assert_eq!(
        client
            .query_one(
                "SELECT state FROM communication_outbox WHERE id = $1",
                &[&expiry_limited.id],
            )
            .await
            .unwrap()
            .get::<_, String>("state"),
        "dead"
    );

    database
        .replace_communication_contact(
            1,
            free_user,
            CommunicationChannel::Email,
            Zeroizing::new("replacement@example.test".to_string()),
            "gdpr_erasure_applied",
            &actor,
            &evidence_key,
        )
        .await
        .expect("audited contact change");
    let contact_state = client
        .query_one(
            "SELECT email_verified,
                (SELECT revoked_at IS NOT NULL FROM communication_verified_contacts
                 WHERE universe_id = 1 AND user_id = $1 AND channel = 'email') AS revoked
             FROM users WHERE universe_id = 1 AND id = $1",
            &[&free_user],
        )
        .await
        .unwrap();
    assert!(!contact_state.get::<_, bool>("email_verified"));
    assert!(contact_state.get::<_, bool>("revoked"));
    let leaked_session_actor = client
        .execute(
            "UPDATE users SET email = 'session-leak@example.test'
             WHERE universe_id = 1 AND id = $1",
            &[&free_user],
        )
        .await
        .expect_err("audited actor settings must clear at transaction end");
    assert_eq!(
        leaked_session_actor.code().map(|code| code.code()),
        Some("42501")
    );

    let held_job = other_user_same_key.job.id;
    client
        .execute(
            "INSERT INTO gdpr_requests (
                user_id, universe_id, idempotency_key, request_source, request_type,
                status, legal_hold_active, legal_hold_at, legal_hold_by_admin_id,
                legal_hold_reason_code, retention_until
             ) VALUES ($1, 1, 'communications:legal-hold', 'integration_test', 'erasure',
                'blocked_legal_hold', TRUE, now(), $2, 'litigation_hold', now() + INTERVAL '1 year')",
            &[&held_user, &admin_user],
        )
        .await
        .expect("create active legal hold");
    client
        .execute(
            "UPDATE communication_outbox
             SET destination_hmac = decode(repeat('11', 32), 'hex'),
                 destination_masked = 'h***@e***.t***',
                 provider_message_hmac = decode(repeat('22', 32), 'hex'),
                 retention_until = now() - INTERVAL '1 day'
             WHERE id = $1",
            &[&held_job],
        )
        .await
        .unwrap();
    client
        .execute(
            "UPDATE communication_outbox
             SET destination_hmac = decode(repeat('33', 32), 'hex'),
                 destination_masked = 'f***@e***.t***',
                 provider_message_hmac = decode(repeat('44', 32), 'hex'),
                 retention_until = now() - INTERVAL '1 day'
             WHERE id = $1",
            &[&sent.id],
        )
        .await
        .unwrap();
    client
        .execute(
            "UPDATE communication_verified_contacts
             SET verified_at = now() - INTERVAL '2 days',
                 retention_until = now() - INTERVAL '1 day'
             WHERE universe_id = 1 AND user_id IN ($1, $2)",
            &[&held_user, &free_user],
        )
        .await
        .unwrap();

    assert_eq!(
        database
            .apply_communication_retention(&actor, &evidence_key)
            .await,
        Err(CommunicationError::Unauthorized)
    );
    let (redacted, _) = database
        .apply_communication_retention(&global_retention_actor(), &evidence_key)
        .await
        .expect("apply legal-hold-aware retention");
    assert!(redacted >= 1);
    let evidence = client
        .query(
            "SELECT id, destination_hmac IS NOT NULL AS retained
             FROM communication_outbox WHERE id IN ($1, $2) ORDER BY id",
            &[&held_job, &sent.id],
        )
        .await
        .unwrap();
    let retained = |job_id| {
        evidence
            .iter()
            .find(|row| row.get::<_, i64>("id") == job_id)
            .unwrap()
            .get::<_, bool>("retained")
    };
    assert!(
        retained(held_job),
        "held subject evidence survives retention"
    );
    assert!(!retained(sent.id), "unheld expired evidence is redacted");
    assert_eq!(
        client
            .query_one(
                "SELECT COUNT(*) FROM communication_verified_contacts
                 WHERE universe_id = 1 AND user_id = $1",
                &[&held_user],
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    assert_eq!(
        client
            .query_one(
                "SELECT COUNT(*) FROM communication_verified_contacts
                 WHERE universe_id = 1 AND user_id = $1",
                &[&free_user],
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );

    let audit = database
        .communication_audit_events(1, 200, &actor)
        .await
        .expect("aggregate-safe delivery audit");
    assert!(audit
        .iter()
        .any(|event| event.event_type == "lease_reclaimed"));
    let controls = database
        .communication_control_audit_events(1, 200, &actor)
        .await
        .expect("aggregate-safe control audit");
    assert!(controls.iter().any(|event| {
        event.control_type == "verified_contact"
            && event.action == "revoked"
            && event.reason_code == "gdpr_erasure_applied"
    }));
    let immutable = client
        .execute(
            "UPDATE communication_control_events SET reason_code = 'tampered' WHERE id = $1",
            &[&controls[0].id],
        )
        .await
        .expect_err("control audit is append-only");
    assert_eq!(immutable.code().map(|code| code.code()), Some("55000"));

    let mismatched_tenant_event = client
        .execute(
            "INSERT INTO communication_outbox_events (
                outbox_id, universe_id, channel, category, event_type, state,
                attempt, actor_subject_hmac
             ) VALUES ($1, 2, 'email', 'transactional', 'enqueued', 'pending', 0,
                decode(repeat('55', 32), 'hex'))",
            &[&sent.id],
        )
        .await
        .expect_err("event tenant must match its outbox parent");
    assert_eq!(
        mismatched_tenant_event.code().map(|code| code.code()),
        Some("23503")
    );
    let malformed_control = client
        .execute(
            "INSERT INTO communication_control_events (
                universe_id, user_id, control_type, channel, category, action,
                reason_code, control_version, actor_subject_hmac
             ) VALUES (1, $1, 'channel_policy', 'email', 'transactional', 'enabled',
                'invalid_shape', 1, decode(repeat('66', 32), 'hex'))",
            &[&free_user],
        )
        .await
        .expect_err("control event shape must match its control type");
    assert_eq!(
        malformed_control.code().map(|code| code.code()),
        Some("23514")
    );

    let leaked = client
        .query_one(
            "SELECT CONCAT_WS(' ',
                (SELECT string_agg(COALESCE(destination_masked, ''), ' ') FROM communication_outbox),
                (SELECT string_agg(COALESCE(reason_code, ''), ' ') FROM communication_outbox_events),
                (SELECT string_agg(COALESCE(reason_code, ''), ' ') FROM communication_control_events)
             )",
            &[],
        )
        .await
        .unwrap()
        .get::<_, String>(0);
    assert!(!leaked.contains("held-person@example.test"));
    assert!(!leaked.contains("free-person@example.test"));
    assert!(!leaked.contains("replacement@example.test"));

    drop(database);
    drop(restarted_database);
    drop(client);
    connection.await.expect("join PostgreSQL connection task");
}
