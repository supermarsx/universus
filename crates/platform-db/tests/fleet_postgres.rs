use std::collections::BTreeMap;
use std::path::Path;

use platform_db::{
    AccountCreateInput, Database, FleetLaunchInput, FleetSourceKind, FleetWriteError,
};
use tokio_postgres::{Client, NoTls};

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

async fn reset_and_apply_through_fleet(client: &Client) {
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
            (version <= 55).then_some((version, name, path))
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

    // Interrupted deployments may replay the current step.
    client
        .batch_execute(include_str!(
            "../../../database/sql/steps/55_durable_fleet_missions.sql"
        ))
        .await
        .expect("repeat durable fleet migration");
}

fn account_input(name: &str) -> AccountCreateInput {
    AccountCreateInput {
        username: name.to_string(),
        email: format!("{}@example.test", name.to_ascii_lowercase()),
        password_hash: "$argon2id$v=19$m=19456,t=2,p=1$c2FsdA$aGFzaA".to_string(),
    }
}

fn transport_input(
    user_id: &str,
    universe_id: i64,
    command_id: &str,
    origin_planet_id: i32,
    target: (i32, i32, i32),
) -> FleetLaunchInput {
    FleetLaunchInput {
        user_id: user_id.to_string(),
        universe_id,
        command_id: command_id.to_string(),
        mission_type: "transport".to_string(),
        source_kind: FleetSourceKind::Planet,
        origin_planet_id: origin_planet_id.to_string(),
        origin_moon_id: None,
        target_kind: "planet".to_string(),
        target_galaxy: target.0,
        target_system: target.1,
        target_position: target.2,
        acs_group_id: None,
        ships: BTreeMap::from([("small_cargo".to_string(), 1)]),
        cargo_metal: 200,
        cargo_crystal: 100,
        cargo_deuterium: 50,
        speed_percent: 100,
        hold_seconds: 0,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Inventory {
    metal: i64,
    crystal: i64,
    deuterium: i64,
    small_cargo: i64,
}

async fn inventory(client: &Client, planet_id: i32) -> Inventory {
    let row = client
        .query_one(
            "SELECT metal, crystal, deuterium, small_cargo
             FROM planets WHERE id = $1",
            &[&planet_id],
        )
        .await
        .expect("planet inventory");
    Inventory {
        metal: row.get("metal"),
        crystal: row.get("crystal"),
        deuterium: row.get("deuterium"),
        small_cargo: row.get("small_cargo"),
    }
}

struct DueTransportFixture<'a> {
    user_id: i32,
    universe_id: i64,
    command_id: &'a str,
    origin_planet_id: i32,
    target_planet_id: i32,
    target: (i32, i32, i32),
    return_is_due: bool,
}

async fn seed_due_transport(client: &Client, fixture: DueTransportFixture<'_>) -> i32 {
    let DueTransportFixture {
        user_id,
        universe_id,
        command_id,
        origin_planet_id,
        target_planet_id,
        target,
        return_is_due,
    } = fixture;
    let origin = client
        .query_one(
            "SELECT galaxy, system, position FROM planets WHERE id = $1",
            &[&origin_planet_id],
        )
        .await
        .expect("origin coordinates");
    client
        .execute(
            "UPDATE planets
             SET small_cargo = small_cargo - 1,
                 metal = metal - 200,
                 crystal = crystal - 100,
                 deuterium = deuterium - 51
             WHERE id = $1 AND small_cargo >= 1
               AND metal >= 200 AND crystal >= 100 AND deuterium >= 51",
            &[&origin_planet_id],
        )
        .await
        .expect("model direct launch inventory deduction");
    let fleet_id = client
        .query_one(
            "INSERT INTO fleets
                (user_id, universe_id, command_id, request_fingerprint, resolution_seed,
                 mission_type, origin_kind, origin_planet_id, origin_moon_id,
                 origin_galaxy, origin_system, origin_position,
                 target_kind, target_planet_id, target_moon_id,
                 target_galaxy, target_system, target_position, acs_group_id,
                 departure_time, arrival_time, return_time,
                 departed_at, unadjusted_arrives_at, arrives_at, returns_at, phase_due_at,
                 distance, fleet_speed, duration_seconds, hold_seconds,
                 movement_fuel_consumed, holding_fuel_consumed, fuel_consumed,
                 cargo_capacity, ships, cargo_metal, cargo_crystal, cargo_deuterium,
                 launched_cargo_metal, launched_cargo_crystal, launched_cargo_deuterium,
                 applied_universe_speed, applied_speed_percent,
                 applied_fuel_multiplier_milli, applied_cargo_multiplier_milli,
                 applied_max_galaxies, applied_max_systems, applied_max_positions,
                 status, result)
             VALUES
                ($1, $2, $3,
                 decode(md5($3) || md5($3 || ':request'), 'hex'),
                 decode(repeat('07', 32), 'hex'),
                 'transport', 'planet', $4, NULL, $5, $6, $7,
                 'planet', $8, NULL, $9, $10, $11, NULL,
                 clock_timestamp() - interval '5 minutes',
                 clock_timestamp() - interval '4 minutes',
                 CASE WHEN $12 THEN clock_timestamp() - interval '3 minutes'
                      ELSE clock_timestamp() + interval '1 hour' END,
                 clock_timestamp() - interval '5 minutes',
                 clock_timestamp() - interval '4 minutes',
                 clock_timestamp() - interval '4 minutes',
                 CASE WHEN $12 THEN clock_timestamp() - interval '3 minutes'
                      ELSE clock_timestamp() + interval '1 hour' END,
                 clock_timestamp() - interval '4 minutes',
                 1000, 5000, 60, 0, 1, 0, 1, 5000,
                 '{\"small_cargo\": 1}'::jsonb, 200, 100, 50, 200, 100, 50,
                 1, 100, 1000, 1000, 9, 499, 15, 'outbound', '{}'::jsonb)
             RETURNING id",
            &[
                &user_id,
                &universe_id,
                &command_id,
                &origin_planet_id,
                &origin.get::<_, i32>("galaxy"),
                &origin.get::<_, i32>("system"),
                &origin.get::<_, i32>("position"),
                &target_planet_id,
                &target.0,
                &target.1,
                &target.2,
                &return_is_due,
            ],
        )
        .await
        .expect("insert valid due fleet")
        .get::<_, i32>("id");
    client
        .execute(
            "INSERT INTO fleet_mission_ships
                (fleet_id, ship_type, initial_count, current_count)
             VALUES ($1, 'small_cargo', 1, 1)",
            &[&fleet_id],
        )
        .await
        .expect("insert normalized fleet ship");
    client
        .execute(
            "INSERT INTO fleet_mission_events
                (universe_id, fleet_id, sequence, event_key, event_type,
                 phase_generation, actor_user_id, payload)
             VALUES ($1, $2, 1, 'launch:dispatched', 'dispatched', 0, $3,
                     '{\"testFixture\": true}'::jsonb)",
            &[&universe_id, &fleet_id, &user_id],
        )
        .await
        .expect("insert dispatch event");
    fleet_id
}

/// Destructively validates migration replay, launch idempotency and locking,
/// tenant isolation, ACS closure, lease expiry/reclaim, exact-once phase
/// transitions, event immutability, restart processing, and conservation.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires disposable PostgreSQL in UNIVERSUS_TEST_DATABASE_URL"]
async fn durable_fleet_is_idempotent_tenant_safe_and_restart_deterministic() {
    let database_url = std::env::var("UNIVERSUS_TEST_DATABASE_URL")
        .expect("UNIVERSUS_TEST_DATABASE_URL must name a disposable PostgreSQL database");
    let (client, connection) = connect(&database_url).await;
    reset_and_apply_through_fleet(&client).await;
    let database = Database::from_database_url(&database_url).expect("database pool");
    database
        .fleet_repository_ready()
        .await
        .expect("fleet repository ready");

    let sender = database
        .register_account_with_starting_state(account_input("FleetSender"))
        .await
        .expect("sender account");
    let receiver = database
        .register_account_with_starting_state(account_input("FleetReceiver"))
        .await
        .expect("receiver account");
    let sender_id = sender.id.parse::<i32>().unwrap();
    let receiver_id = receiver.id.parse::<i32>().unwrap();
    let universe_id = sender.universe_id.expect("sender universe");
    assert_eq!(receiver.universe_id, Some(universe_id));
    let sender_planet = client
        .query_one(
            "SELECT id, galaxy, system, position FROM planets WHERE user_id = $1",
            &[&sender_id],
        )
        .await
        .unwrap();
    let receiver_planet = client
        .query_one(
            "SELECT id, galaxy, system, position FROM planets WHERE user_id = $1",
            &[&receiver_id],
        )
        .await
        .unwrap();
    let sender_planet_id = sender_planet.get::<_, i32>("id");
    let receiver_planet_id = receiver_planet.get::<_, i32>("id");
    let receiver_coordinates = (
        receiver_planet.get::<_, i32>("galaxy"),
        receiver_planet.get::<_, i32>("system"),
        receiver_planet.get::<_, i32>("position"),
    );
    client
        .execute(
            "UPDATE planets
             SET metal = 100000, crystal = 100000, deuterium = 100000,
                 small_cargo = CASE WHEN id = $1 THEN 20 ELSE small_cargo END,
                 metal_mine = 0, crystal_mine = 0, deuterium_synthesizer = 0,
                 last_resource_update = clock_timestamp()
             WHERE id IN ($1, $2)",
            &[&sender_planet_id, &receiver_planet_id],
        )
        .await
        .unwrap();
    client
        .execute(
            "UPDATE config_parameters SET current_value = 'false'
             WHERE parameter_key = 'combat.noob_protection_enabled'",
            &[],
        )
        .await
        .unwrap();

    let launch = transport_input(
        &sender.id,
        universe_id,
        "fleet-concurrent-idempotency",
        sender_planet_id,
        receiver_coordinates,
    );
    let before_launch = inventory(&client, sender_planet_id).await;
    let restarted = Database::from_database_url(&database_url).expect("restarted pool");
    let (left, right) = tokio::join!(
        database.launch_fleet(launch.clone()),
        restarted.launch_fleet(launch.clone())
    );
    let left = left.expect("first concurrent launch");
    let right = right.expect("second concurrent launch");
    assert_eq!(left.mission.id, right.mission.id);
    assert_ne!(left.idempotent_replay, right.idempotent_replay);
    let after_launch = inventory(&client, sender_planet_id).await;
    assert_eq!(after_launch.small_cargo, before_launch.small_cargo - 1);
    assert_eq!(after_launch.metal, before_launch.metal - 200);
    assert_eq!(after_launch.crystal, before_launch.crystal - 100);
    assert_eq!(
        after_launch.deuterium,
        before_launch.deuterium - 50 - left.mission.fuel_consumed
    );
    assert_eq!(
        client
            .query_one(
                "SELECT count(*)::BIGINT AS count FROM fleets
                 WHERE universe_id = $1 AND user_id = $2 AND command_id = $3",
                &[&universe_id, &sender_id, &launch.command_id],
            )
            .await
            .unwrap()
            .get::<_, i64>("count"),
        1
    );
    let replay = database
        .launch_fleet(launch.clone())
        .await
        .expect("post-restart idempotent replay");
    assert!(replay.idempotent_replay);
    let mut conflicting = launch.clone();
    conflicting.speed_percent = 90;
    assert_eq!(
        database.launch_fleet(conflicting).await.unwrap_err(),
        FleetWriteError::IdempotencyConflict
    );

    let inventory_before_acs = inventory(&client, sender_planet_id).await;
    let mut acs = launch.clone();
    acs.command_id = "fleet-acs-disabled".to_string();
    acs.mission_type = "acs_attack".to_string();
    let acs_error = database.launch_fleet(acs).await.unwrap_err();
    assert!(
        matches!(acs_error, FleetWriteError::Invalid(message) if message.contains("ACS launch is disabled"))
    );
    assert_eq!(
        inventory(&client, sender_planet_id).await,
        inventory_before_acs
    );

    assert!(database
        .fleet_missions_for_user(&sender.id, universe_id + 1)
        .await
        .unwrap()
        .is_empty());
    assert!(database
        .fleet_mission_for_user(&sender.id, universe_id + 1, &left.mission.id)
        .await
        .unwrap()
        .is_none());
    assert!(database
        .fleet_mission_for_user(&receiver.id, universe_id, &left.mission.id)
        .await
        .unwrap()
        .is_none());

    let conservation_before_source = inventory(&client, sender_planet_id).await;
    let conservation_before_target = inventory(&client, receiver_planet_id).await;
    let due_fleet = seed_due_transport(
        &client,
        DueTransportFixture {
            user_id: sender_id,
            universe_id,
            command_id: "fleet-lease-reclaim",
            origin_planet_id: sender_planet_id,
            target_planet_id: receiver_planet_id,
            target: receiver_coordinates,
            return_is_due: true,
        },
    )
    .await;
    let first_claim = database
        .claim_due_fleet_missions("fleet-worker-a", 1, 1)
        .await
        .unwrap()
        .into_iter()
        .find(|claim| claim.fleet_id == due_fleet)
        .expect("first due fleet claim");
    tokio::time::sleep(std::time::Duration::from_millis(1_100)).await;
    let second_claim = restarted
        .claim_due_fleet_missions("fleet-worker-b", 10, 30)
        .await
        .unwrap()
        .into_iter()
        .find(|claim| claim.fleet_id == due_fleet)
        .expect("reclaimed expired fleet lease");
    assert_eq!(second_claim.generation, first_claim.generation);
    assert_eq!(second_claim.claim_attempt, first_claim.claim_attempt + 1);
    assert!(database
        .process_claimed_fleet_mission("fleet-worker-a", &first_claim)
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        restarted
            .process_claimed_fleet_mission("fleet-worker-b", &second_claim)
            .await
            .unwrap(),
        Some("arrival")
    );
    let after_arrival = database
        .fleet_mission_for_user(&sender.id, universe_id, &due_fleet.to_string())
        .await
        .unwrap()
        .expect("arrived fleet");
    assert_eq!(after_arrival.status, "returning");
    assert_eq!(after_arrival.result["mission"], "transport");

    let return_result = Database::from_database_url(&database_url)
        .unwrap()
        .process_due_fleet_missions("fleet-worker-after-restart", 10, 30)
        .await
        .unwrap();
    assert_eq!(return_result.returns, 1);
    let no_replay = database
        .process_due_fleet_missions("fleet-worker-replay", 10, 30)
        .await
        .unwrap();
    assert_eq!(no_replay.arrivals + no_replay.returns, 0);
    let completed = database
        .fleet_mission_for_user(&sender.id, universe_id, &due_fleet.to_string())
        .await
        .unwrap()
        .expect("completed fleet");
    assert_eq!(completed.status, "completed");
    assert_eq!(completed.ships["small_cargo"], 0);
    assert_eq!(
        inventory(&client, sender_planet_id).await.small_cargo,
        conservation_before_source.small_cargo
    );
    let conservation_after_source = inventory(&client, sender_planet_id).await;
    let conservation_after_target = inventory(&client, receiver_planet_id).await;
    assert_eq!(
        conservation_after_source.metal + conservation_after_target.metal,
        conservation_before_source.metal + conservation_before_target.metal
    );
    assert_eq!(
        conservation_after_source.crystal + conservation_after_target.crystal,
        conservation_before_source.crystal + conservation_before_target.crystal
    );
    assert_eq!(
        conservation_after_source.deuterium + conservation_after_target.deuterium,
        conservation_before_source.deuterium + conservation_before_target.deuterium - 1
    );

    let events = database
        .fleet_mission_events_for_user(&sender.id, universe_id, &due_fleet.to_string())
        .await
        .unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|event| event.event_type == "arrival_resolved")
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| event.event_type == "returned")
            .count(),
        1
    );
    assert!(events.iter().all(|event| {
        let payload = serde_json::to_string(&event.payload).unwrap();
        !payload.contains(&"07".repeat(32)) && !payload.contains("resolution_seed")
    }));
    assert!(client
        .execute(
            "UPDATE fleet_mission_events SET event_type = 'tampered'
             WHERE fleet_id = $1",
            &[&due_fleet],
        )
        .await
        .is_err());
    assert!(client
        .execute(
            "INSERT INTO fleet_mission_events
                (universe_id, fleet_id, sequence, event_key, event_type,
                 phase_generation, payload)
             VALUES ($1, $2, 999, repeat('x', 161), 'oversized', 0, '{}'::jsonb)",
            &[&universe_id, &due_fleet],
        )
        .await
        .is_err());

    // A separate future-return mission proves two concurrent workers cannot
    // apply its arrival cargo or event twice.
    let concurrent_before_source = inventory(&client, sender_planet_id).await;
    let concurrent_before_target = inventory(&client, receiver_planet_id).await;
    let concurrent_fleet = seed_due_transport(
        &client,
        DueTransportFixture {
            user_id: sender_id,
            universe_id,
            command_id: "fleet-concurrent-resolution",
            origin_planet_id: sender_planet_id,
            target_planet_id: receiver_planet_id,
            target: receiver_coordinates,
            return_is_due: false,
        },
    )
    .await;
    let worker_one = Database::from_database_url(&database_url).unwrap();
    let worker_two = Database::from_database_url(&database_url).unwrap();
    let (one, two) = tokio::join!(
        worker_one.process_due_fleet_missions("fleet-concurrent-a", 10, 30),
        worker_two.process_due_fleet_missions("fleet-concurrent-b", 10, 30)
    );
    let one = one.unwrap();
    let two = two.unwrap();
    assert_eq!(one.arrivals + two.arrivals, 1);
    assert_eq!(one.returns + two.returns, 0);
    let concurrent_after_source = inventory(&client, sender_planet_id).await;
    let concurrent_after_target = inventory(&client, receiver_planet_id).await;
    assert_eq!(
        concurrent_after_target.metal,
        concurrent_before_target.metal + 200
    );
    assert_eq!(
        concurrent_after_target.crystal,
        concurrent_before_target.crystal + 100
    );
    assert_eq!(
        concurrent_after_target.deuterium,
        concurrent_before_target.deuterium + 50
    );
    assert_eq!(
        concurrent_after_source.small_cargo + 1,
        concurrent_before_source.small_cargo
    );
    assert_eq!(
        client
            .query_one(
                "SELECT count(*)::BIGINT AS count FROM fleet_mission_events
                 WHERE fleet_id = $1 AND event_type = 'arrival_resolved'",
                &[&concurrent_fleet],
            )
            .await
            .unwrap()
            .get::<_, i64>("count"),
        1
    );

    drop(database);
    drop(restarted);
    drop(client);
    connection.abort();
}
