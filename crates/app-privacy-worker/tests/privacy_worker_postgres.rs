use aes_gcm::{
    aead::{Aead, Payload},
    Aes256Gcm, KeyInit, Nonce,
};
use app_privacy_worker::{
    ExportEncryptor, JobOutcome, OpsPublisher, PrivacyWorker, ProcessorSettings,
};
use platform_db::{
    CommunicationEvidenceKey, Database, PrivacyAdminDecision, PrivacyAdminDecisionInput,
    PrivacyError, PrivacyRequestCreateInput, PrivacyRequestType,
};
use sha2::{Digest, Sha256};
use std::{path::Path, time::Duration};
use tokio::time::sleep;
use tokio_postgres::{Client, NoTls};
use zeroize::Zeroizing;

const TEST_KEY: [u8; 32] = [0x5a; 32];

#[derive(Debug, Clone, Copy)]
struct Actors {
    export_subject: i32,
    correction_subject: i32,
    restriction_subject: i32,
    erasure_subject: i32,
    failure_subject: i32,
    tenant_two_subject: i32,
    admin_one: i32,
    admin_two: i32,
}

fn request(
    universe_id: i64,
    user_id: i32,
    request_type: PrivacyRequestType,
    idempotency_key: &str,
) -> PrivacyRequestCreateInput {
    PrivacyRequestCreateInput {
        universe_id,
        user_id,
        request_type,
        idempotency_key: idempotency_key.to_string(),
        request_source: "privacy_worker_postgres_test".to_string(),
        requester_ip_digest: None,
        encrypted_payload: None,
        erasure_cooling_off_seconds: Some(0),
    }
}

fn worker(
    database: Database,
    worker_id: &str,
    universe_id: Option<i64>,
    claim_limit: i64,
    lease_seconds: i64,
    max_plaintext_bytes: usize,
) -> PrivacyWorker {
    PrivacyWorker::new(
        database,
        ProcessorSettings {
            worker_id: worker_id.to_string(),
            universe_id,
            claim_limit,
            claim_timeout: Duration::from_secs(5),
            lease_seconds,
            job_timeout: Duration::from_secs((lease_seconds - 1) as u64),
            retry_delay_seconds: 0,
            export_expires_in_seconds: 3600,
            privacy_outbox_retention_days: 30,
        },
        ExportEncryptor::new(
            "v1:postgres-test".to_string(),
            Zeroizing::new(TEST_KEY),
            max_plaintext_bytes,
        )
        .unwrap(),
        CommunicationEvidenceKey::new(vec![0x33; 32]).unwrap(),
        OpsPublisher::disabled(),
    )
    .unwrap()
}

async fn apply_canonical_privacy_schema(client: &Client) {
    client
        .batch_execute("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
        .await
        .expect("reset disposable schema");
    let steps_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../database/sql/steps");
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
                .filter(|version| *version <= 56)
                .map(|version| (version, file_name, path))
        })
        .collect::<Vec<_>>();
    steps.sort_by(|left, right| (left.0, &left.1).cmp(&(right.0, &right.1)));
    assert_eq!(steps.first().map(|step| step.0), Some(1));
    assert_eq!(steps.last().map(|step| step.0), Some(56));
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

async fn seed_actors(client: &Client) -> Actors {
    client
        .execute(
            "INSERT INTO universes (id, name, speed, registration_open)
             VALUES (2, 'Privacy Worker Tenant Two', 1, TRUE)
             ON CONFLICT (id) DO NOTHING",
            &[],
        )
        .await
        .unwrap();
    let rows = client
        .query(
            "INSERT INTO users (username, email, password_hash, universe_id, is_admin)
             VALUES
                ('WorkerExportSubject', 'worker-export@example.test', '!test!', 1, FALSE),
                ('WorkerCorrectionSubject', 'worker-correction@example.test', '!test!', 1, FALSE),
                ('WorkerRestrictionSubject', 'worker-restrict@example.test', '!test!', 1, FALSE),
                ('WorkerErasureSubject', 'worker-erasure@example.test', '!test!', 1, FALSE),
                ('WorkerFailureSubject', 'worker-failure@example.test', '!test!', 1, FALSE),
                ('WorkerTenantTwoSubject', 'worker-tenant-two@example.test', '!test!', 2, FALSE),
                ('WorkerAdminOne', 'worker-admin-one@example.test', '!test!', 1, TRUE),
                ('WorkerAdminTwo', 'worker-admin-two@example.test', '!test!', 1, TRUE)
             RETURNING id, username",
            &[],
        )
        .await
        .unwrap();
    let id = |username: &str| {
        rows.iter()
            .find(|row| row.get::<_, String>("username") == username)
            .unwrap()
            .get::<_, i32>("id")
    };
    Actors {
        export_subject: id("WorkerExportSubject"),
        correction_subject: id("WorkerCorrectionSubject"),
        restriction_subject: id("WorkerRestrictionSubject"),
        erasure_subject: id("WorkerErasureSubject"),
        failure_subject: id("WorkerFailureSubject"),
        tenant_two_subject: id("WorkerTenantTwoSubject"),
        admin_one: id("WorkerAdminOne"),
        admin_two: id("WorkerAdminTwo"),
    }
}

/// This test owns and resets the database named by
/// `UNIVERSUS_TEST_DATABASE_URL`; use only a disposable PostgreSQL database.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires disposable PostgreSQL in UNIVERSUS_TEST_DATABASE_URL"]
async fn privacy_worker_encrypts_recovers_and_dead_letters_without_cross_tenant_claims() {
    let database_url = std::env::var("UNIVERSUS_TEST_DATABASE_URL")
        .expect("UNIVERSUS_TEST_DATABASE_URL must name a disposable PostgreSQL database");
    let (client, connection) = tokio_postgres::connect(&database_url, NoTls)
        .await
        .expect("connect disposable PostgreSQL");
    tokio::spawn(async move {
        connection.await.expect("PostgreSQL test connection");
    });
    apply_canonical_privacy_schema(&client).await;
    let actors = seed_actors(&client).await;
    let database = Database::from_database_url(&database_url).unwrap();
    database.privacy_repository_ready().await.unwrap();

    // A subject-access export is authenticated and encrypted before the
    // repository transaction persists it. No plaintext account value is in
    // the artifact bytes.
    let export_request = database
        .create_privacy_request(request(
            1,
            actors.export_subject,
            PrivacyRequestType::Export,
            "worker-encrypted-export",
        ))
        .await
        .unwrap();
    let export_worker = worker(
        database.clone(),
        "export-worker",
        Some(1),
        4,
        30,
        1024 * 1024,
    );
    let export_report = export_worker.run_cycle().await.unwrap();
    assert_eq!(export_report.claimed, 1);
    assert_eq!(export_report.completed, 1);
    let artifact = client
        .query_one(
            "SELECT ciphertext, encryption_key_id, encryption_nonce,
                    plaintext_sha256, plaintext_size
             FROM privacy_export_artifacts
             WHERE request_id = $1 AND universe_id = 1 AND user_id = $2",
            &[&export_request.id, &actors.export_subject],
        )
        .await
        .unwrap();
    let ciphertext: Vec<u8> = artifact.get("ciphertext");
    let key_id: String = artifact.get("encryption_key_id");
    let nonce: Vec<u8> = artifact.get("encryption_nonce");
    let digest: Vec<u8> = artifact.get("plaintext_sha256");
    assert!(!ciphertext
        .windows("worker-export@example.test".len())
        .any(|window| window == b"worker-export@example.test"));
    let cipher = Aes256Gcm::new_from_slice(&TEST_KEY).unwrap();
    let plaintext = cipher
        .decrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &ciphertext,
                aad: &ExportEncryptor::aad_for_key_id(&key_id),
            },
        )
        .unwrap();
    let snapshot: serde_json::Value = serde_json::from_slice(&plaintext).unwrap();
    assert_eq!(snapshot["profile"]["email"], "worker-export@example.test");
    assert_eq!(digest, Sha256::digest(&plaintext).as_slice());
    assert_eq!(
        artifact.get::<_, i64>("plaintext_size"),
        plaintext.len() as i64
    );
    let grant = database
        .issue_export_delivery(1, actors.export_subject, export_request.id, 600)
        .await
        .unwrap();
    let prepared = database
        .prepare_export_delivery(1, actors.export_subject, export_request.id, &grant.token)
        .await
        .unwrap();
    let wrong_key = ExportEncryptor::new(
        "v1:postgres-test".to_string(),
        Zeroizing::new([0x6b; 32]),
        1024 * 1024,
    )
    .unwrap();
    assert!(wrong_key.decrypt_export(&prepared).is_err());
    let retry = database
        .prepare_export_delivery(1, actors.export_subject, export_request.id, &grant.token)
        .await
        .expect("decrypt failure must not consume the one-time grant");
    ExportEncryptor::new(
        "v1:postgres-test".to_string(),
        Zeroizing::new(TEST_KEY),
        1024 * 1024,
    )
    .unwrap()
    .decrypt_export(&retry)
    .unwrap();
    let first = database.finalize_export_delivery(
        1,
        actors.export_subject,
        export_request.id,
        &grant.token,
    );
    let second = database.finalize_export_delivery(
        1,
        actors.export_subject,
        export_request.id,
        &grant.token,
    );
    let (first, second) = tokio::join!(first, second);
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);

    // Correction values are encrypted with subject-bound AAD before they are
    // persisted, then applied and redacted atomically by the worker.
    let correction_encryptor = ExportEncryptor::new(
        "v1:postgres-test".to_string(),
        Zeroizing::new(TEST_KEY),
        1024 * 1024,
    )
    .unwrap();
    let correction_values = serde_json::json!({
        "username": "WorkerCorrected",
        "email": "worker-corrected@example.test",
        "phoneNumber": "+441234567890"
    });
    let mut correction_input = request(
        1,
        actors.correction_subject,
        PrivacyRequestType::Correction,
        "worker-encrypted-correction",
    );
    correction_input.encrypted_payload = Some(
        correction_encryptor
            .prepare_correction_payload(1, actors.correction_subject, &correction_values)
            .unwrap(),
    );
    let correction_request = database
        .create_privacy_request(correction_input)
        .await
        .unwrap();
    database
        .record_privacy_admin_decision(PrivacyAdminDecisionInput {
            universe_id: 1,
            request_id: correction_request.id,
            admin_user_id: actors.admin_one,
            decision: PrivacyAdminDecision::Approve,
            reason_code: "worker_correction_approved".to_string(),
        })
        .await
        .unwrap();
    let correction_worker = worker(
        database.clone(),
        "correction-worker",
        Some(1),
        1,
        30,
        1024 * 1024,
    );
    let correction_report = correction_worker.run_cycle().await.unwrap();
    assert_eq!(correction_report.completed, 1);
    let corrected = client
        .query_one(
            "SELECT users.username, users.email, users.phone_number,
                    request.status, request.request_payload_ciphertext IS NULL AS payload_redacted,
                    execution.applied_fields
             FROM users
             JOIN gdpr_requests AS request
               ON request.universe_id = users.universe_id AND request.user_id = users.id
             JOIN privacy_correction_executions AS execution
               ON execution.request_id = request.id
             WHERE request.id = $1",
            &[&correction_request.id],
        )
        .await
        .unwrap();
    assert_eq!(corrected.get::<_, String>("username"), "WorkerCorrected");
    assert_eq!(
        corrected.get::<_, String>("email"),
        "worker-corrected@example.test"
    );
    assert_eq!(
        corrected
            .get::<_, Option<String>>("phone_number")
            .as_deref(),
        Some("+441234567890")
    );
    assert_eq!(corrected.get::<_, String>("status"), "completed");
    assert!(corrected.get::<_, bool>("payload_redacted"));
    assert_eq!(
        corrected.get::<_, Vec<String>>("applied_fields"),
        vec!["email", "phone_number", "username"]
    );

    // Concurrent workers use SKIP LOCKED leases; only one can apply the
    // restriction and increment the subject's authentication epoch.
    let restriction_request = database
        .create_privacy_request(request(
            1,
            actors.restriction_subject,
            PrivacyRequestType::Restriction,
            "worker-concurrent-restriction",
        ))
        .await
        .unwrap();
    client
        .execute(
            "INSERT INTO user_sessions (
                user_id, session_token, device_name, ip_address, status, expires_at
             ) VALUES ($1, 'restriction-session-secret', 'worker-test-device',
                       '192.0.2.44', 'active', now() + interval '1 day')",
            &[&actors.restriction_subject],
        )
        .await
        .unwrap();
    let worker_a = worker(
        database.clone(),
        "concurrent-worker-a",
        Some(1),
        1,
        30,
        1024 * 1024,
    );
    let worker_b = worker(
        database.clone(),
        "concurrent-worker-b",
        Some(1),
        1,
        30,
        1024 * 1024,
    );
    let (report_a, report_b) = tokio::join!(worker_a.run_cycle(), worker_b.run_cycle());
    let report_a = report_a.unwrap();
    let report_b = report_b.unwrap();
    assert_eq!(report_a.claimed + report_b.claimed, 1);
    assert_eq!(report_a.completed + report_b.completed, 1);
    let restriction_state = client
        .query_one(
            "SELECT users.auth_epoch, users.privacy_restriction_active,
                    sessions.status AS session_status,
                    (SELECT status FROM privacy_outbox WHERE request_id = $2) AS job_status
             FROM users
             JOIN user_sessions AS sessions ON sessions.user_id = users.id
             WHERE users.id = $1",
            &[&actors.restriction_subject, &restriction_request.id],
        )
        .await
        .unwrap();
    assert_eq!(restriction_state.get::<_, i64>("auth_epoch"), 1);
    assert!(restriction_state.get::<_, bool>("privacy_restriction_active"));
    assert_eq!(
        restriction_state.get::<_, String>("session_status"),
        "revoked"
    );
    assert_eq!(
        restriction_state.get::<_, String>("job_status"),
        "delivered"
    );

    // A process crash leaves a lease behind. Once it expires, another worker
    // recovers the same outbox row; the stale owner cannot commit side effects.
    client
        .execute(
            "INSERT INTO communication_contact_versions (
                universe_id, user_id, channel, version
             ) VALUES (1, $1, 'email', 1)
             ON CONFLICT (universe_id, user_id, channel) DO NOTHING",
            &[&actors.erasure_subject],
        )
        .await
        .unwrap();
    client
        .execute(
            "INSERT INTO communication_verified_contacts (
                universe_id, user_id, channel, destination_hmac,
                destination_masked, verification_method, expires_at,
                retention_until, version
             ) VALUES (
                1, $1, 'email', decode(repeat('ab', 32), 'hex'),
                'w***@example.test', 'worker_test', now() + interval '1 day',
                now() + interval '90 days', 1
             )",
            &[&actors.erasure_subject],
        )
        .await
        .unwrap();
    let erasure_request = database
        .create_privacy_request(request(
            1,
            actors.erasure_subject,
            PrivacyRequestType::Erasure,
            "worker-restart-erasure",
        ))
        .await
        .unwrap();
    for admin_user_id in [actors.admin_one, actors.admin_two] {
        database
            .record_privacy_admin_decision(PrivacyAdminDecisionInput {
                universe_id: 1,
                request_id: erasure_request.id,
                admin_user_id,
                decision: PrivacyAdminDecision::Approve,
                reason_code: "worker_test_approval".to_string(),
            })
            .await
            .unwrap();
    }
    let crashed_claim = database
        .claim_privacy_jobs("crashed-worker", Some(1), 1, 1)
        .await
        .unwrap();
    assert_eq!(crashed_claim.len(), 1);
    sleep(Duration::from_secs(2)).await;
    let recovered = database
        .claim_privacy_jobs("restarted-worker", Some(1), 1, 30)
        .await
        .unwrap();
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].id, crashed_claim[0].id);
    assert_eq!(recovered[0].attempt_count, 2);
    assert!(matches!(
        database
            .complete_erasure_authorization_job(crashed_claim[0].id, "crashed-worker")
            .await,
        Err(PrivacyError::LeaseLost)
    ));
    let restarted_worker = worker(
        database.clone(),
        "restarted-worker",
        Some(1),
        1,
        30,
        1024 * 1024,
    );
    assert_eq!(
        restarted_worker
            .process_claimed_job(recovered[0].clone())
            .await,
        JobOutcome::Completed
    );
    let erasure_state = client
        .query_one(
            "SELECT users.privacy_erasure_pending, users.privacy_restriction_active,
                    users.privacy_erased_at IS NOT NULL AS privacy_erased,
                    users.email, users.phone_number, users.password_hash,
                    outbox.status, outbox.attempt_count,
                    NOT EXISTS (
                        SELECT 1 FROM communication_verified_contacts AS evidence
                        WHERE evidence.universe_id = users.universe_id
                          AND evidence.user_id = users.id
                    ) AS verified_contact_absent,
                    NOT EXISTS (
                        SELECT 1 FROM communication_contact_versions AS versions
                        WHERE versions.universe_id = users.universe_id
                          AND versions.user_id = users.id
                    ) AS contact_versions_absent
             FROM users
             JOIN privacy_outbox AS outbox ON outbox.user_id = users.id
             WHERE users.id = $1 AND outbox.request_id = $2",
            &[&actors.erasure_subject, &erasure_request.id],
        )
        .await
        .unwrap();
    assert!(!erasure_state.get::<_, bool>("privacy_erasure_pending"));
    assert!(erasure_state.get::<_, bool>("privacy_restriction_active"));
    assert!(erasure_state.get::<_, bool>("privacy_erased"));
    assert!(erasure_state
        .get::<_, String>("email")
        .ends_with("@privacy.invalid"));
    assert_eq!(erasure_state.get::<_, Option<String>>("phone_number"), None);
    assert_eq!(
        erasure_state.get::<_, String>("password_hash"),
        "!privacy-erased!"
    );
    assert_eq!(erasure_state.get::<_, String>("status"), "delivered");
    assert_eq!(erasure_state.get::<_, i32>("attempt_count"), 2);
    assert!(erasure_state.get::<_, bool>("verified_contact_absent"));
    assert!(erasure_state.get::<_, bool>("contact_versions_absent"));

    // A tenant-scoped worker ignores another universe. A deterministic export
    // size failure is retried and then dead-lettered using only a stable code.
    let failure_request = database
        .create_privacy_request(request(
            1,
            actors.failure_subject,
            PrivacyRequestType::Export,
            "worker-dead-letter-export",
        ))
        .await
        .unwrap();
    let tenant_two_request = database
        .create_privacy_request(request(
            2,
            actors.tenant_two_subject,
            PrivacyRequestType::Export,
            "worker-tenant-two-export",
        ))
        .await
        .unwrap();
    client
        .execute(
            "UPDATE privacy_outbox SET max_attempts = 2
             WHERE request_id = $1 AND universe_id = 1",
            &[&failure_request.id],
        )
        .await
        .unwrap();
    let failing_worker = worker(database.clone(), "failing-worker", Some(1), 4, 30, 16);
    let first_failure = failing_worker.run_cycle().await.unwrap();
    assert_eq!(first_failure.claimed, 1);
    assert_eq!(first_failure.failure_recorded, 1);
    let tenant_two_status: String = client
        .query_one(
            "SELECT status FROM privacy_outbox
             WHERE request_id = $1 AND universe_id = 2",
            &[&tenant_two_request.id],
        )
        .await
        .unwrap()
        .get("status");
    assert_eq!(tenant_two_status, "pending");
    let second_failure = failing_worker.run_cycle().await.unwrap();
    assert_eq!(second_failure.claimed, 1);
    assert_eq!(second_failure.failure_recorded, 1);
    let dead = client
        .query_one(
            "SELECT outbox.status, outbox.attempt_count, outbox.last_error_code,
                    request.status AS request_status,
                    NOT EXISTS (
                        SELECT 1 FROM privacy_export_artifacts
                        WHERE request_id = outbox.request_id
                    ) AS artifact_absent
             FROM privacy_outbox AS outbox
             JOIN gdpr_requests AS request ON request.id = outbox.request_id
             WHERE outbox.request_id = $1 AND outbox.universe_id = 1",
            &[&failure_request.id],
        )
        .await
        .unwrap();
    assert_eq!(dead.get::<_, String>("status"), "dead");
    assert_eq!(dead.get::<_, i32>("attempt_count"), 2);
    assert_eq!(dead.get::<_, String>("last_error_code"), "export_too_large");
    assert_eq!(dead.get::<_, String>("request_status"), "failed");
    assert!(dead.get::<_, bool>("artifact_absent"));

    let tenant_two_worker = worker(
        database.clone(),
        "tenant-two-worker",
        Some(2),
        1,
        30,
        1024 * 1024,
    );
    let tenant_two_report = tenant_two_worker.run_cycle().await.unwrap();
    assert_eq!(tenant_two_report.claimed, 1);
    assert_eq!(tenant_two_report.completed, 1);
    assert_eq!(
        client
            .query_one(
                "SELECT COUNT(*)::BIGINT FROM privacy_export_artifacts
                 WHERE request_id = $1 AND universe_id = 2 AND user_id = $2",
                &[&tenant_two_request.id, &actors.tenant_two_subject],
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
}
