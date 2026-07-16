use platform_db::{
    ChatRestrictionUpsert, CrossServerMessageCreateInput, Database, ScheduledTaskCreateInput,
};
use tokio::time::{sleep, Duration};
use tokio_postgres::NoTls;

fn unix_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

async fn reset_disposable_schema(database_url: &str) {
    let (client, connection) = tokio_postgres::connect(database_url, NoTls)
        .await
        .expect("connect to disposable PostgreSQL");
    let connection_task = tokio::spawn(async move {
        connection
            .await
            .expect("disposable PostgreSQL connection remains healthy");
    });
    client
        .batch_execute("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
        .await
        .expect("reset disposable schema");
    drop(client);
    connection_task
        .await
        .expect("join PostgreSQL connection task");
}

async fn expire_scheduler_lease(database_url: &str, task_id: i64) {
    let (client, connection) = tokio_postgres::connect(database_url, NoTls)
        .await
        .expect("connect to expire scheduler lease");
    let connection_task = tokio::spawn(async move {
        connection
            .await
            .expect("lease-expiry PostgreSQL connection remains healthy");
    });
    assert_eq!(
        client
            .execute(
                "UPDATE scheduled_tasks SET lease_until = now() - INTERVAL '1 second' WHERE id = $1",
                &[&task_id],
            )
            .await
            .expect("expire claimed scheduler lease"),
        1
    );
    drop(client);
    connection_task.await.expect("join lease-expiry task");
}

/// Exercises every dynamic SQL path that converts an i64 Unix timestamp or
/// duration into PostgreSQL temporal types. The database is destroyed/reset;
/// `UNIVERSUS_TEST_DATABASE_URL` must point at a disposable instance.
#[tokio::test]
#[ignore = "requires disposable PostgreSQL in UNIVERSUS_TEST_DATABASE_URL"]
async fn bigint_timestamp_bindings_support_durable_scheduler_shard_and_chat_lifecycles() {
    let database_url = std::env::var("UNIVERSUS_TEST_DATABASE_URL")
        .expect("UNIVERSUS_TEST_DATABASE_URL must name a disposable PostgreSQL database");
    reset_disposable_schema(&database_url).await;
    let database = Database::from_database_url(&database_url).expect("database pool");

    let now = unix_timestamp();
    let scheduled = database
        .enqueue_scheduled_task(ScheduledTaskCreateInput {
            task_type: "scheduler.binding_test".to_string(),
            payload: serde_json::json!({"contract": "bigint-epoch"}),
            run_at_unix: now - 1,
            task_key: Some("scheduler-binding-test".to_string()),
        })
        .await
        .expect("enqueue i64 epoch without float inference");
    assert_eq!(scheduled.run_at_unix, now - 1);

    let claimed = database
        .claim_due_scheduled_tasks("scheduler-binding-worker", 1, 7)
        .await
        .expect("claim with an i64 lease duration")
        .pop()
        .expect("due scheduled task");
    assert_eq!(claimed.id, scheduled.id);
    assert_eq!(claimed.status, "running");
    assert_eq!(
        claimed.lease_owner.as_deref(),
        Some("scheduler-binding-worker")
    );
    assert!(claimed
        .lease_until_unix
        .is_some_and(|lease| lease >= now + 5));

    expire_scheduler_lease(&database_url, claimed.id).await;
    let rolled_over = database
        .claim_due_scheduled_tasks("scheduler-binding-rollover", 1, 7)
        .await
        .expect("re-claim scheduler task after lease expiry")
        .pop()
        .expect("rolled-over scheduled task");
    assert_eq!(rolled_over.id, scheduled.id);
    assert_eq!(
        rolled_over.lease_owner.as_deref(),
        Some("scheduler-binding-rollover")
    );
    assert!(!database
        .complete_scheduled_task_for_owner(rolled_over.id, "scheduler-binding-worker")
        .await
        .expect("stale owner completion is rejected"));
    assert!(!database
        .fail_scheduled_task_for_owner(
            rolled_over.id,
            "scheduler-binding-worker",
            "stale failure",
            1,
            3,
        )
        .await
        .expect("stale owner failure is rejected"));
    assert!(database
        .fail_scheduled_task_for_owner(
            rolled_over.id,
            "scheduler-binding-rollover",
            "retry me",
            1,
            3,
        )
        .await
        .expect("current lease owner persists an i64 retry delay"));
    sleep(Duration::from_millis(1_100)).await;
    let retried = database
        .claim_due_scheduled_tasks("scheduler-binding-retry", 1, 7)
        .await
        .expect("claim due scheduler retry")
        .pop()
        .expect("retry scheduled task");
    assert_eq!(retried.id, scheduled.id);
    assert_eq!(retried.attempt_count, 1);
    assert!(database
        .complete_scheduled_task_for_owner(retried.id, "scheduler-binding-retry")
        .await
        .expect("complete retried scheduled task"));

    let legacy = database
        .enqueue_scheduled_task(ScheduledTaskCreateInput {
            task_type: "scheduler.legacy_binding_test".to_string(),
            payload: serde_json::json!({}),
            run_at_unix: unix_timestamp() - 1,
            task_key: Some("scheduler-legacy-binding-test".to_string()),
        })
        .await
        .expect("enqueue legacy retry binding task");
    let legacy_claim = database
        .claim_due_scheduled_tasks("scheduler-legacy-worker", 1, 7)
        .await
        .expect("claim legacy binding task")
        .pop()
        .expect("legacy binding task");
    assert_eq!(legacy_claim.id, legacy.id);
    assert!(database
        .fail_scheduled_task(legacy.id, "legacy retry", 1, 3)
        .await
        .expect("legacy id-only retry also accepts an i64 delay"));

    let message = database
        .enqueue_cross_server_message(CrossServerMessageCreateInput {
            source_server_id: "source-a".to_string(),
            target_server_id: "target-b".to_string(),
            message_type: "binding.test".to_string(),
            payload: serde_json::json!({"sequence": 1}),
        })
        .await
        .expect("enqueue cross-server message");
    let claimed_message = database
        .claim_cross_server_messages("target-b", "shard-binding-worker", 1, 7)
        .await
        .expect("claim cross-server message with an i64 lease duration")
        .pop()
        .expect("queued cross-server message");
    assert_eq!(claimed_message.id, message.id);
    assert_eq!(claimed_message.status, "processing");
    assert!(database
        .fail_cross_server_message(claimed_message.id, "retry shard message", 1, 3)
        .await
        .expect("persist cross-server retry using an i64 delay"));
    sleep(Duration::from_millis(1_100)).await;
    let retried_message = database
        .claim_cross_server_messages("target-b", "shard-binding-retry", 1, 7)
        .await
        .expect("claim cross-server retry")
        .pop()
        .expect("retry cross-server message");
    assert_eq!(retried_message.id, message.id);
    assert_eq!(retried_message.attempt_count, 1);
    assert!(database
        .ack_cross_server_message(retried_message.id)
        .await
        .expect("acknowledge retried cross-server message"));

    let expires_at = unix_timestamp() + 60;
    let restriction = database
        .upsert_chat_restriction(ChatRestrictionUpsert {
            user_id: 41,
            channel_id: Some(7),
            restriction_type: "mute".to_string(),
            reason: "binding regression".to_string(),
            restricted_by: 99,
            expires_at_unix: Some(expires_at),
        })
        .await
        .expect("store optional i64 chat expiry without float inference");
    assert_eq!(restriction.expires_at_unix, Some(expires_at));
    assert_eq!(
        database
            .cleanup_expired_chat_restrictions(expires_at - 1)
            .await
            .expect("compare cleanup epoch using an i64 binding"),
        0
    );
    assert_eq!(
        database
            .cleanup_expired_chat_restrictions(expires_at)
            .await
            .expect("delete restriction at its exact i64 expiry"),
        1
    );
}
