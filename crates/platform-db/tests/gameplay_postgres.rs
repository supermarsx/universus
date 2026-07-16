use platform_db::{
    AccountCreateInput, Database, GameplayCompletionKind, GameplayQueueInput, GameplayWriteError,
    STARTING_CRYSTAL, STARTING_DEUTERIUM, STARTING_METAL,
};
use tokio_postgres::NoTls;

const CORE_SCHEMA: &str = include_str!("../../../database/sql/steps/01_core_schema.sql");
const MOON_SCHEMA: &str = include_str!("../../../database/sql/steps/30_moon_schema.sql");
const AUTH_SCHEMA: &str =
    include_str!("../../../database/sql/steps/48_auth_accounts_hardening.sql");
const GAMEPLAY_SCHEMA: &str =
    include_str!("../../../database/sql/steps/49_durable_gameplay_loop.sql");
const AUTHORITATIVE_GAMEPLAY_SCHEMA: &str =
    include_str!("../../../database/sql/steps/50_authoritative_gameplay_state.sql");

fn account_input(username: &str, email: &str) -> AccountCreateInput {
    AccountCreateInput {
        username: username.to_string(),
        email: email.to_string(),
        password_hash: "$argon2id$v=19$m=19456,t=2,p=1$c2FsdA$aGFzaA".to_string(),
    }
}

fn building_input(user_id: &str, planet_id: &str) -> GameplayQueueInput {
    GameplayQueueInput {
        user_id: user_id.to_string(),
        planet_id: planet_id.to_string(),
        item_type: "metal_mine".to_string(),
        target_level: Some(1),
        quantity: None,
        metal_cost: 60,
        crystal_cost: 15,
        deuterium_cost: 0,
        energy_required: 0,
        duration_seconds: 0,
    }
}

/// This test intentionally owns and resets the database named by
/// `UNIVERSUS_TEST_DATABASE_URL`; use only a disposable PostgreSQL database.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires disposable PostgreSQL in UNIVERSUS_TEST_DATABASE_URL"]
async fn durable_gameplay_schema_and_repository_round_trip() {
    let database_url = std::env::var("UNIVERSUS_TEST_DATABASE_URL")
        .expect("UNIVERSUS_TEST_DATABASE_URL must name a disposable PostgreSQL database");
    let (client, connection) = tokio_postgres::connect(&database_url, NoTls)
        .await
        .expect("connect disposable PostgreSQL");
    tokio::spawn(async move {
        connection.await.expect("PostgreSQL test connection");
    });

    client
        .batch_execute("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
        .await
        .expect("reset disposable schema");
    client
        .batch_execute(CORE_SCHEMA)
        .await
        .expect("core schema");
    client
        .batch_execute(MOON_SCHEMA)
        .await
        .expect("moon schema");
    client
        .batch_execute(AUTH_SCHEMA)
        .await
        .expect("auth schema");

    // Seed pre-status prototype rows. Valid unknown rows must be quarantined,
    // malformed rows terminalized, and moon work isolated from planet workers.
    client
        .batch_execute(
            "ALTER TABLE shipyard_queue ADD COLUMN status VARCHAR(20);
             INSERT INTO users (username, email, password_hash)
             VALUES ('Legacy', 'legacy@example.com', '!legacy!');
             INSERT INTO planets
                (user_id, name, galaxy, system, position, metal, crystal, deuterium,
                 small_cargo)
             VALUES (1, 'Legacy Prime', 9, 499, 15, 1000, 1000, 1000, 10);
             INSERT INTO moons
                (planet_id, user_id, name, diameter, total_fields, used_fields)
             VALUES (1, 1, 'Legacy Moon', 5000, 20, 0);
             INSERT INTO construction_queue
                (planet_id, moon_id, location_type, building_type, level, end_time,
                 metal_cost, crystal_cost, deuterium_cost)
             VALUES (NULL, 1, 'moon', 'lunar_base', 1, now() - interval '1 hour',
                     100, 100, 0);
             INSERT INTO shipyard_queue
                (planet_id, location_type, unit_type, quantity, end_time,
                 metal_cost, crystal_cost, deuterium_cost, status)
             VALUES
                (1, 'planet', 'small_cargo', 2, now() - interval '1 hour', 10, 10, 0, NULL),
                (1, 'planet', 'small_cargo', -5, now() - interval '1 hour', 0, 0, 0, NULL),
                (1, 'planet', 'small_cargo', 5, now() - interval '2 hours', 0, 0, 0,
                 'completed');",
        )
        .await
        .expect("seed legacy queue rows");

    client
        .batch_execute(GAMEPLAY_SCHEMA)
        .await
        .expect("gameplay migration first application");
    client
        .batch_execute(GAMEPLAY_SCHEMA)
        .await
        .expect("gameplay migration repeat application");
    client
        .batch_execute(AUTHORITATIVE_GAMEPLAY_SCHEMA)
        .await
        .expect("authoritative gameplay migration first application");
    client
        .batch_execute(AUTHORITATIVE_GAMEPLAY_SCHEMA)
        .await
        .expect("authoritative gameplay migration repeat application");

    let legacy_rows = client
        .query(
            "SELECT quantity, status FROM shipyard_queue ORDER BY id",
            &[],
        )
        .await
        .expect("read migrated legacy queues");
    assert_eq!(
        legacy_rows[0].get::<_, String>("status"),
        "legacy_unclassified"
    );
    assert_eq!(legacy_rows[1].get::<_, String>("status"), "failed");
    assert_eq!(legacy_rows[2].get::<_, String>("status"), "completed");
    assert_eq!(
        client
            .query_one("SELECT status FROM construction_queue", &[])
            .await
            .unwrap()
            .get::<_, String>("status"),
        "legacy_unclassified"
    );

    let database = Database::from_database_url(&database_url).expect("test database pool");
    database
        .gameplay_repository_ready()
        .await
        .expect("repository readiness");
    client
        .batch_execute(
            "DROP INDEX idx_shipyard_queue_due;
             CREATE INDEX idx_shipyard_queue_due ON shipyard_queue (id)
             WHERE status = 'queued';",
        )
        .await
        .unwrap();
    assert!(database.gameplay_repository_ready().await.is_err());
    client
        .batch_execute("DROP INDEX idx_shipyard_queue_due;")
        .await
        .unwrap();
    client
        .batch_execute(GAMEPLAY_SCHEMA)
        .await
        .expect("restore validated gameplay index definition");
    database
        .gameplay_repository_ready()
        .await
        .expect("readiness after index restoration");
    let legacy_result = database
        .process_due_gameplay_queues(10)
        .await
        .expect("legacy quarantine processing pass");
    assert_eq!(legacy_result, Default::default());
    assert_eq!(
        client
            .query_one(
                "SELECT small_cargo::BIGINT AS count FROM planets WHERE id = 1",
                &[]
            )
            .await
            .unwrap()
            .get::<_, i64>("count"),
        10,
        "malformed legacy quantity must never subtract inventory"
    );

    client
        .execute(
            "INSERT INTO shipyard_queue
                (planet_id, location_type, unit_type, quantity, start_time, end_time,
                 metal_cost, crystal_cost, deuterium_cost, status, processing_started_at)
             VALUES (1, 'planet', 'small_cargo', 1, now() - interval '1 hour',
                     now() - interval '30 minutes', 0, 0, 0, 'processing',
                     now() - interval '30 minutes')",
            &[],
        )
        .await
        .expect("seed stale external processing claim");
    assert_eq!(
        database.process_due_gameplay_queues(10).await.unwrap(),
        Default::default()
    );
    assert_eq!(
        client
            .query_one(
                "SELECT status FROM shipyard_queue ORDER BY id DESC LIMIT 1",
                &[]
            )
            .await
            .unwrap()
            .get::<_, String>("status"),
        "stale_processing",
        "stale external claims must stop blocking the active queue index"
    );

    // A failure after the user insert must roll the whole registration back.
    client
        .batch_execute(
            "CREATE FUNCTION reject_starting_planet() RETURNS trigger
             LANGUAGE plpgsql AS $$ BEGIN
                 RAISE EXCEPTION 'injected starting planet failure';
             END $$;
             CREATE TRIGGER reject_starting_planet
             BEFORE INSERT ON planets FOR EACH ROW
             WHEN (NEW.name = 'New Terra')
             EXECUTE FUNCTION reject_starting_planet();",
        )
        .await
        .unwrap();
    assert!(database
        .register_account_with_starting_state(account_input(
            "RollbackCommander",
            "rollback@example.com"
        ))
        .await
        .is_err());
    client
        .batch_execute(
            "DROP TRIGGER reject_starting_planet ON planets;
             DROP FUNCTION reject_starting_planet();",
        )
        .await
        .unwrap();
    assert_eq!(
        client
            .query_one(
                "SELECT COUNT(*)::BIGINT AS count FROM users
                 WHERE email = 'rollback@example.com'",
                &[]
            )
            .await
            .unwrap()
            .get::<_, i64>("count"),
        0
    );

    let first_registration = database
        .register_account_with_starting_state(account_input("CommanderOne", "one@example.com"));
    let second_registration = database
        .register_account_with_starting_state(account_input("CommanderTwo", "two@example.com"));
    let (first, second) = tokio::join!(first_registration, second_registration);
    let first = first.expect("concurrent first registration");
    let second = second.expect("concurrent second registration");
    assert_eq!(first.universe_id, Some(1));
    assert_eq!(second.universe_id, Some(1));
    let first_planet = database
        .gameplay_planets_for_user(&first.id)
        .await
        .unwrap()
        .remove(0);
    let second_planet = database
        .gameplay_planets_for_user(&second.id)
        .await
        .unwrap()
        .remove(0);
    assert_ne!(
        (
            first_planet.galaxy,
            first_planet.system,
            first_planet.position
        ),
        (
            second_planet.galaxy,
            second_planet.system,
            second_planet.position
        ),
        "registration coordinate allocation must be unique"
    );
    assert_eq!(first_planet.metal, STARTING_METAL);
    assert_eq!(first_planet.crystal, STARTING_CRYSTAL);
    assert_eq!(first_planet.deuterium, STARTING_DEUTERIUM);
    assert!(database
        .gameplay_research_for_user(&first.id)
        .await
        .unwrap()
        .is_some());
    assert_eq!(first_planet.universe_id, 1);
    assert_eq!(
        database
            .gameplay_score_for_user(&first.id)
            .await
            .unwrap()
            .unwrap()
            .total_score,
        0,
        "registration must provision score state"
    );

    let provisioned = database
        .gameplay_provision_planet_at_next_coordinate(&first.id, "Silent Colony")
        .await
        .expect("shared coordinate provisioning contract");
    assert_eq!(provisioned.user_id, first.id);
    assert_eq!(provisioned.universe_id, first_planet.universe_id);
    assert_eq!(
        (
            provisioned.metal,
            provisioned.crystal,
            provisioned.deuterium
        ),
        (0, 0, 0)
    );
    assert_ne!(
        (provisioned.galaxy, provisioned.system, provisioned.position),
        (
            first_planet.galaxy,
            first_planet.system,
            first_planet.position
        )
    );

    client
        .batch_execute(
            "INSERT INTO universes (id, name, speed, registration_open)
             VALUES (2, 'Parallel Universe', 1, FALSE);",
        )
        .await
        .expect("create a second universe");
    let parallel_user = client
        .query_one(
            "INSERT INTO users
                (username, email, password_hash, universe_id)
             VALUES ('Parallel', 'parallel@example.com', '!parallel!', 2)
             RETURNING id",
            &[],
        )
        .await
        .unwrap();
    client
        .execute(
            "INSERT INTO planets
                (user_id, universe_id, name, galaxy, system, position)
             VALUES ($1, 2, 'Parallel Prime', $2, $3, $4)",
            &[
                &parallel_user.get::<_, i32>("id"),
                &first_planet.galaxy,
                &first_planet.system,
                &first_planet.position,
            ],
        )
        .await
        .expect("same coordinate is valid in a different universe");

    let wrong_owner = building_input(&second.id, &first_planet.id);
    assert_eq!(
        database.gameplay_enqueue_building(&wrong_owner).await,
        Err(GameplayWriteError::NotFound)
    );
    let mut unaffordable = building_input(&first.id, &first_planet.id);
    unaffordable.metal_cost = i64::MAX;
    assert_eq!(
        database.gameplay_enqueue_building(&unaffordable).await,
        Err(GameplayWriteError::InsufficientResources)
    );

    let building = building_input(&first.id, &first_planet.id);
    database
        .gameplay_enqueue_building(&building)
        .await
        .expect("queue building");
    assert_eq!(
        database.gameplay_enqueue_building(&building).await,
        Err(GameplayWriteError::QueueBusy)
    );
    let research = GameplayQueueInput {
        user_id: first.id.clone(),
        planet_id: first_planet.id.clone(),
        item_type: "energy_technology".to_string(),
        target_level: Some(1),
        quantity: None,
        metal_cost: 0,
        crystal_cost: 800,
        deuterium_cost: 400,
        energy_required: 0,
        duration_seconds: 0,
    };
    database
        .gameplay_enqueue_research(&research)
        .await
        .expect("queue research");
    let ships = GameplayQueueInput {
        user_id: first.id.clone(),
        planet_id: first_planet.id.clone(),
        item_type: "small_cargo".to_string(),
        target_level: None,
        quantity: Some(2),
        metal_cost: 4_000,
        crystal_cost: 4_000,
        deuterium_cost: 0,
        energy_required: 0,
        duration_seconds: 0,
    };
    database
        .gameplay_enqueue_ships(&ships)
        .await
        .expect("queue ships");

    let processed = database
        .process_due_gameplay_queues(1)
        .await
        .expect("fair due processing");
    assert_eq!(processed.buildings, 1);
    assert_eq!(processed.research, 1);
    assert_eq!(processed.ships, 1);
    assert_eq!(processed.failed, 0);
    assert_eq!(processed.completions.len(), 3);
    assert!(processed.completions.iter().any(|completion| {
        completion.kind == GameplayCompletionKind::Building
            && completion.item_type == "metal_mine"
            && completion.target_level == Some(1)
    }));
    assert!(processed.completions.iter().any(|completion| {
        completion.kind == GameplayCompletionKind::Research
            && completion.item_type == "energy_technology"
            && completion.score_delta == 1
    }));
    assert!(processed.completions.iter().any(|completion| {
        completion.kind == GameplayCompletionKind::Shipyard
            && completion.item_type == "small_cargo"
            && completion.quantity == Some(2)
            && completion.score_delta == 8
    }));
    let initial_score = database
        .gameplay_score_for_user(&first.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(initial_score.total_score, 9);
    assert_eq!(initial_score.economy_score, 0);
    assert_eq!(initial_score.research_score, 1);
    assert_eq!(initial_score.military_score, 8);
    assert_eq!(
        database.process_due_gameplay_queues(10).await.unwrap(),
        Default::default(),
        "completed rows must be exactly-once"
    );

    let restarted = Database::from_database_url(&database_url).expect("restart database pool");
    let persisted_planet = restarted
        .gameplay_planet_for_user(&first.id, &first_planet.id)
        .await
        .unwrap()
        .expect("persisted owned planet");
    assert_eq!(persisted_planet.buildings["metal_mine"], 1);
    assert_eq!(persisted_planet.ships["small_cargo"], 2);
    assert_eq!(
        restarted
            .gameplay_research_for_user(&first.id)
            .await
            .unwrap()
            .unwrap()
            .levels["energy_technology"],
        1
    );

    // Same-kind concurrent enqueues serialize on the owned planet and deduct
    // exactly once; one wins and the other observes the active queue.
    let second_building = building_input(&second.id, &second_planet.id);
    let (same_kind_a, same_kind_b) = tokio::join!(
        database.gameplay_enqueue_building(&second_building),
        database.gameplay_enqueue_building(&second_building)
    );
    assert!(matches!(
        (&same_kind_a, &same_kind_b),
        (Ok(_), Err(GameplayWriteError::QueueBusy)) | (Err(GameplayWriteError::QueueBusy), Ok(_))
    ));
    let second_after_race = database
        .gameplay_planet_for_user(&second.id, &second_planet.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(second_after_race.metal, STARTING_METAL - 60);
    assert_eq!(second_after_race.crystal, STARTING_CRYSTAL - 15);

    // Different queue kinds still share the same resource lock. Two
    // individually affordable 100k orders cannot jointly overspend 125k.
    let third = database
        .register_account_with_starting_state(account_input("CommanderThree", "three@example.com"))
        .await
        .unwrap();
    let third_planet = database
        .gameplay_planets_for_user(&third.id)
        .await
        .unwrap()
        .remove(0);
    let mut expensive_building = building_input(&third.id, &third_planet.id);
    expensive_building.metal_cost = 100_000;
    expensive_building.crystal_cost = 0;
    let expensive_research = GameplayQueueInput {
        user_id: third.id.clone(),
        planet_id: third_planet.id.clone(),
        item_type: "energy_technology".to_string(),
        target_level: Some(1),
        quantity: None,
        metal_cost: 100_000,
        crystal_cost: 0,
        deuterium_cost: 0,
        energy_required: 0,
        duration_seconds: 0,
    };
    let (cross_kind_a, cross_kind_b) = tokio::join!(
        database.gameplay_enqueue_building(&expensive_building),
        database.gameplay_enqueue_research(&expensive_research)
    );
    assert!(matches!(
        (&cross_kind_a, &cross_kind_b),
        (Ok(_), Err(GameplayWriteError::InsufficientResources))
            | (Err(GameplayWriteError::InsufficientResources), Ok(_))
    ));
    assert_eq!(
        database
            .gameplay_planet_for_user(&third.id, &third_planet.id)
            .await
            .unwrap()
            .unwrap()
            .metal,
        STARTING_METAL - 100_000
    );

    let second_ships = GameplayQueueInput {
        user_id: second.id.clone(),
        planet_id: second_planet.id.clone(),
        item_type: "small_cargo".to_string(),
        target_level: None,
        quantity: Some(3),
        metal_cost: 0,
        crystal_cost: 0,
        deuterium_cost: 0,
        energy_required: 0,
        duration_seconds: 0,
    };
    database
        .gameplay_enqueue_ships(&second_ships)
        .await
        .unwrap();
    let worker_a = database.clone();
    let worker_b = database.clone();
    let (worker_a, worker_b) = tokio::join!(
        worker_a.process_due_gameplay_queues(10),
        worker_b.process_due_gameplay_queues(10)
    );
    let worker_a = worker_a.unwrap();
    let worker_b = worker_b.unwrap();
    assert_eq!(worker_a.failed + worker_b.failed, 0);
    assert_eq!(
        worker_a.buildings
            + worker_a.research
            + worker_a.ships
            + worker_b.buildings
            + worker_b.research
            + worker_b.ships,
        3,
        "two workers must collectively apply each due row once"
    );
    let second_completed = database
        .gameplay_planet_for_user(&second.id, &second_planet.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(second_completed.buildings["metal_mine"], 1);
    assert_eq!(second_completed.ships["small_cargo"], 3);
    let third_completed = database
        .gameplay_planet_for_user(&third.id, &third_planet.id)
        .await
        .unwrap()
        .unwrap();
    let third_research = database
        .gameplay_research_for_user(&third.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        third_completed.buildings["metal_mine"] + third_research.levels["energy_technology"],
        1
    );

    // A poison overflow row is terminal-failed without aborting peer queue
    // completion in the same transaction/batch.
    client
        .execute(
            "UPDATE planets SET small_cargo = $2 WHERE id = $1",
            &[&first_planet.id.parse::<i32>().unwrap(), &i64::MAX],
        )
        .await
        .unwrap();
    let metal_before_overflow_rejection = restarted
        .gameplay_planet_for_user(&first.id, &first_planet.id)
        .await
        .unwrap()
        .unwrap()
        .metal;
    let overflow_request = GameplayQueueInput {
        user_id: first.id.clone(),
        planet_id: first_planet.id.clone(),
        item_type: "small_cargo".to_string(),
        target_level: None,
        quantity: Some(1),
        metal_cost: 100,
        crystal_cost: 0,
        deuterium_cost: 0,
        energy_required: 0,
        duration_seconds: 0,
    };
    assert!(matches!(
        restarted.gameplay_enqueue_ships(&overflow_request).await,
        Err(GameplayWriteError::Invalid(message)) if message.contains("overflow")
    ));
    assert_eq!(
        restarted
            .gameplay_planet_for_user(&first.id, &first_planet.id)
            .await
            .unwrap()
            .unwrap()
            .metal,
        metal_before_overflow_rejection,
        "overflow must be rejected before resource deduction"
    );
    client
        .execute(
            "INSERT INTO shipyard_queue
                (planet_id, location_type, unit_type, quantity, end_time,
                 metal_cost, crystal_cost, deuterium_cost, status)
             VALUES ($1, 'planet', 'small_cargo', 1, now() - interval '1 second',
                     0, 0, 0, 'queued')",
            &[&first_planet.id.parse::<i32>().unwrap()],
        )
        .await
        .unwrap();
    client
        .execute(
            "INSERT INTO research_queue
                (user_id, planet_id, research_type, level, end_time,
                 metal_cost, crystal_cost, deuterium_cost, status)
             VALUES ($1, $2, 'energy_technology', 2, now() - interval '1 second',
                     0, 0, 0, 'queued')",
            &[
                &first.id.parse::<i32>().unwrap(),
                &first_planet.id.parse::<i32>().unwrap(),
            ],
        )
        .await
        .unwrap();
    let poison = restarted.process_due_gameplay_queues(10).await.unwrap();
    assert_eq!(poison.ships, 0);
    assert_eq!(poison.research, 1);
    assert_eq!(poison.failed, 1);
    let overflow_row = client
        .query_one(
            "SELECT status FROM shipyard_queue
             WHERE status = 'failed' ORDER BY id DESC LIMIT 1",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(overflow_row.get::<_, String>("status"), "failed");
    assert_eq!(
        restarted
            .gameplay_planet_for_user(&first.id, &first_planet.id)
            .await
            .unwrap()
            .unwrap()
            .ships["small_cargo"],
        i64::MAX
    );
}
