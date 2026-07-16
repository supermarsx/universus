use app_api_gateway::accounts::AccountRepository;
use app_api_gateway::routes::build_router_with_dependencies;
use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use hyper::body::to_bytes;
use platform_db::{AccountCreateInput, Database};
use serde_json::{json, Value};
use tokio_postgres::NoTls;
use tower::ServiceExt;

const CORE_SCHEMA: &str = include_str!("../../../database/sql/steps/01_core_schema.sql");
const MOON_SCHEMA: &str = include_str!("../../../database/sql/steps/30_moon_schema.sql");
const AUTH_SCHEMA: &str =
    include_str!("../../../database/sql/steps/48_auth_accounts_hardening.sql");
const QUEUE_SCHEMA: &str = include_str!("../../../database/sql/steps/49_durable_gameplay_loop.sql");
const GAMEPLAY_SCHEMA: &str =
    include_str!("../../../database/sql/steps/50_authoritative_gameplay_state.sql");
const RESOURCE_SCHEMA: &str =
    include_str!("../../../database/sql/steps/51_resource_accrual_remainders.sql");

fn account_input(username: &str, email: &str) -> AccountCreateInput {
    AccountCreateInput {
        username: username.to_string(),
        email: email.to_string(),
        password_hash: "$argon2id$v=19$m=19456,t=2,p=1$c2FsdA$aGFzaA".to_string(),
    }
}

fn token(subject: &str) -> String {
    let config = platform_auth::AuthConfig {
        jwt_secret: "default-secret".to_string(),
        jwt_expiry_seconds: 86_400,
        ..platform_auth::AuthConfig::default()
    };
    platform_auth::generate_token(&config, subject, "Commander", "player", Some(1)).unwrap()
}

fn request(method: &str, path: &str, token: &str, body: Option<Value>) -> Request<Body> {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"));
    if body.is_some() {
        request = request.header(header::CONTENT_TYPE, "application/json");
    }
    request
        .body(
            body.map(|value| Body::from(value.to_string()))
                .unwrap_or_else(Body::empty),
        )
        .unwrap()
}

async fn json_body(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body()).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// Owns and resets `UNIVERSUS_TEST_DATABASE_URL`; the database must be disposable.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires disposable PostgreSQL in UNIVERSUS_TEST_DATABASE_URL"]
async fn signed_routes_use_authoritative_persisted_gameplay_state() {
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
        .unwrap();
    for schema in [
        CORE_SCHEMA,
        MOON_SCHEMA,
        AUTH_SCHEMA,
        QUEUE_SCHEMA,
        GAMEPLAY_SCHEMA,
        RESOURCE_SCHEMA,
    ] {
        client.batch_execute(schema).await.unwrap();
    }

    let database = Database::from_database_url(&database_url).unwrap();
    let first = database
        .register_account_with_starting_state(account_input("Commander", "one@example.com"))
        .await
        .unwrap();
    let second = database
        .register_account_with_starting_state(account_input("Rival", "two@example.com"))
        .await
        .unwrap();
    let first_planet = database
        .gameplay_planets_for_user(&first.id)
        .await
        .unwrap()
        .remove(0);
    let app = build_router_with_dependencies(
        "gameplay-test",
        Some(database.clone()),
        AccountRepository::from_environment(Some(database.clone())),
    );
    let first_token = token(&first.id);
    let second_token = token(&second.id);

    let planets = app
        .clone()
        .oneshot(request("GET", "/api/planets", &first_token, None))
        .await
        .unwrap();
    assert_eq!(planets.status(), StatusCode::OK);
    let planets = json_body(planets).await;
    assert_eq!(planets["data"].as_array().unwrap().len(), 1);
    assert_eq!(planets["data"][0]["id"], first_planet.id);

    let forbidden = app
        .clone()
        .oneshot(request(
            "GET",
            &format!("/api/planets/{}", first_planet.id),
            &second_token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(forbidden.status(), StatusCode::NOT_FOUND);

    for (method, path, body) in [
        (
            "GET",
            format!("/api/planets/{}/buildings", first_planet.id),
            None,
        ),
        (
            "GET",
            format!("/api/planets/{}/build-queue", first_planet.id),
            None,
        ),
        (
            "POST",
            format!("/api/planets/{}/build-quote", first_planet.id),
            Some(json!({"buildingType": "metalMine"})),
        ),
    ] {
        let response = app
            .clone()
            .oneshot(request(method, &path, &second_token, body))
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "{method} {path} exposed another player's planet"
        );
        let response = json_body(response).await;
        assert_eq!(response["success"], false);
        assert_eq!(response["error"], "Planet not found");
        assert!(response.get("data").is_none());
    }

    let catalog = app
        .clone()
        .oneshot(request(
            "GET",
            &format!("/api/planets/{}/buildings", first_planet.id),
            &first_token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(catalog.status(), StatusCode::OK);
    let catalog = json_body(catalog).await;
    assert_eq!(catalog["data"].as_array().unwrap().len(), 16);
    let metal_mine = catalog["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|building| building["buildingType"] == "metalMine")
        .unwrap();
    assert_eq!(metal_mine["available"], true);
    assert_eq!(metal_mine["quote"]["currentLevel"], 0);
    assert_eq!(metal_mine["quote"]["nextLevel"], 1);
    assert_eq!(metal_mine["quote"]["metal"], 60);
    assert_eq!(metal_mine["quote"]["crystal"], 15);
    assert_eq!(metal_mine["quote"]["timeSeconds"], 108);
    let fusion_reactor = catalog["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|building| building["buildingType"] == "fusionReactor")
        .unwrap();
    assert_eq!(fusion_reactor["available"], false);
    assert!(fusion_reactor["unavailableReason"]
        .as_str()
        .unwrap()
        .contains("Missing prerequisite"));

    let quote = app
        .clone()
        .oneshot(request(
            "POST",
            &format!("/api/planets/{}/build-quote", first_planet.id),
            &first_token,
            Some(json!({"buildingType": "metalMine"})),
        ))
        .await
        .unwrap();
    assert_eq!(quote.status(), StatusCode::OK);
    let quote = json_body(quote).await;
    assert_eq!(quote["data"]["planetId"], first_planet.id);
    assert_eq!(quote["data"]["buildingType"], "metalMine");
    assert_eq!(quote["data"]["metal"], 60);
    assert_eq!(quote["data"]["crystal"], 15);
    assert_eq!(quote["data"]["timeSeconds"], 108);

    // Spoofed prices and durations are ignored; the canonical level-one metal
    // mine quote is 60 metal, 15 crystal, and 108 seconds at speed one.
    let building = app
        .clone()
        .oneshot(request(
            "POST",
            &format!("/api/planets/{}/build", first_planet.id),
            &first_token,
            Some(json!({
                "buildingType": "metalMine",
                "metalCost": 0,
                "crystalCost": 0,
                "durationSeconds": 0
            })),
        ))
        .await
        .unwrap();
    assert_eq!(building.status(), StatusCode::OK);
    let building = json_body(building).await;
    assert_eq!(building["data"]["levelTarget"], 1);
    assert_eq!(building["data"]["finishesInSeconds"], 108);
    let construction_queue = app
        .clone()
        .oneshot(request(
            "GET",
            &format!("/api/planets/{}/build-queue", first_planet.id),
            &first_token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(construction_queue.status(), StatusCode::OK);
    let construction_queue = json_body(construction_queue).await;
    assert_eq!(construction_queue["data"].as_array().unwrap().len(), 1);
    assert_eq!(construction_queue["data"][0]["buildingType"], "metalMine");
    assert_eq!(construction_queue["data"][0]["name"], "Metal Mine");
    assert_eq!(construction_queue["data"][0]["levelTarget"], 1);
    let after_build = database
        .gameplay_planet_for_user(&first.id, &first_planet.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after_build.metal, first_planet.metal - 60);
    assert_eq!(after_build.crystal, first_planet.crystal - 15);

    client
        .execute(
            "UPDATE construction_queue SET end_time = now() - interval '1 second'
             WHERE planet_id = $1 AND status = 'queued'",
            &[&first_planet.id.parse::<i32>().unwrap()],
        )
        .await
        .unwrap();
    database.process_due_gameplay_queues(10).await.unwrap();

    let missing_research_prerequisite = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/research/start",
            &first_token,
            Some(json!({
                "planetId": first_planet.id,
                "technologyType": "energyTechnology"
            })),
        ))
        .await
        .unwrap();
    assert_eq!(
        missing_research_prerequisite.status(),
        StatusCode::BAD_REQUEST
    );
    let missing_research_prerequisite = json_body(missing_research_prerequisite).await;
    let prerequisite_error = missing_research_prerequisite["error"].as_str().unwrap();
    let normalized_prerequisite = prerequisite_error
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    assert!(normalized_prerequisite.contains("researchlab"));
    assert!(prerequisite_error.contains("level 1"));

    let research_planet = database
        .gameplay_provision_planet_at_next_coordinate(&first.id, "Research Nexus")
        .await
        .unwrap();
    client
        .execute(
            "UPDATE planets SET research_lab = 1 WHERE id = $1",
            &[&first_planet.id.parse::<i32>().unwrap()],
        )
        .await
        .unwrap();
    client
        .execute(
            "UPDATE planets
             SET research_lab = 5, metal = 0, crystal = 800, deuterium = 400,
                 last_resource_update = clock_timestamp()
             WHERE id = $1",
            &[&research_planet.id.parse::<i32>().unwrap()],
        )
        .await
        .unwrap();
    let research_cost = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/research/energyTechnology/cost",
            &first_token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(research_cost.status(), StatusCode::OK);
    let research_cost = json_body(research_cost).await;
    assert_eq!(research_cost["data"]["planetId"], research_planet.id);
    assert_eq!(research_cost["data"]["nextLevel"], 1);
    assert_eq!(research_cost["data"]["metal"], 0);
    assert_eq!(research_cost["data"]["crystal"], 800);
    assert_eq!(research_cost["data"]["deuterium"], 400);
    assert_eq!(research_cost["data"]["timeSeconds"], 480);

    let before_research = database
        .gameplay_planet_for_user(&first.id, &research_planet.id)
        .await
        .unwrap()
        .unwrap();
    let research_payload = json!({
        // Backwards-compatible stale/incorrect selectors cannot override the
        // server-owned highest-lab choice used by the displayed quote.
        "planetId": first_planet.id,
        "technologyType": "energyTechnology",
        "metalCost": 999999999,
        "crystalCost": 0,
        "deuteriumCost": 0,
        "durationSeconds": 0
    });
    let research_a = app.clone().oneshot(request(
        "POST",
        "/api/research/start",
        &first_token,
        Some(research_payload.clone()),
    ));
    let research_b = app.clone().oneshot(request(
        "POST",
        "/api/research/start",
        &first_token,
        Some(research_payload),
    ));
    let (research_a, research_b) = tokio::join!(research_a, research_b);
    let research_a = research_a.unwrap();
    let research_b = research_b.unwrap();
    assert!(matches!(
        (research_a.status(), research_b.status()),
        (StatusCode::OK, StatusCode::CONFLICT) | (StatusCode::CONFLICT, StatusCode::OK)
    ));
    let after_research = database
        .gameplay_planet_for_user(&first.id, &research_planet.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after_research.metal, before_research.metal);
    assert_eq!(after_research.crystal, before_research.crystal - 800);
    assert_eq!(after_research.deuterium, before_research.deuterium - 400);
    let research_queue = app
        .clone()
        .oneshot(request("GET", "/api/research/queue", &first_token, None))
        .await
        .unwrap();
    assert_eq!(research_queue.status(), StatusCode::OK);
    let research_queue = json_body(research_queue).await;
    assert_eq!(research_queue["data"].as_array().unwrap().len(), 1);
    assert_eq!(research_queue["data"][0]["techId"], "energy_technology");
    assert_eq!(research_queue["data"][0]["levelTarget"], 1);
    assert_eq!(research_queue["data"][0]["planetId"], research_planet.id);
    client
        .execute(
            "UPDATE research_queue SET end_time = now() - interval '1 second'
             WHERE user_id = $1 AND status = 'queued'",
            &[&first.id.parse::<i32>().unwrap()],
        )
        .await
        .unwrap();
    database.process_due_gameplay_queues(10).await.unwrap();

    // The resource payload exposes the exact signed fusion/energy vector used
    // by persistence, so a draining stockpile and throttled mines are visible
    // instead of being mislabeled as zero production.
    client
        .execute(
            "UPDATE planets
             SET metal = 0, crystal = 0, deuterium = 1000,
                 metal_mine = 20, crystal_mine = 0,
                 deuterium_synthesizer = 0, solar_plant = 0,
                 fusion_reactor = 10, solar_satellite = 0,
                 metal_production_remainder = 0,
                 crystal_production_remainder = 0,
                 deuterium_production_remainder = 0,
                 last_resource_update = clock_timestamp()
             WHERE id = $1",
            &[&first_planet.id.parse::<i32>().unwrap()],
        )
        .await
        .unwrap();
    let projection = app
        .clone()
        .oneshot(request(
            "GET",
            &format!("/api/planets/{}/resources", first_planet.id),
            &first_token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(projection.status(), StatusCode::OK);
    let projection = json_body(projection).await;
    assert!(
        projection["data"]["productionPerHour"]["deuterium"]
            .as_i64()
            .unwrap()
            < 0
    );
    assert_eq!(
        projection["data"]["productionBreakdown"]["deuteriumGrossPerHour"],
        0
    );
    assert!(
        projection["data"]["productionBreakdown"]["fusionFuelPerHour"]
            .as_i64()
            .unwrap()
            > 0
    );
    assert!(projection["data"]["energy"]["supply"].as_i64().unwrap() > 0);
    assert!(
        projection["data"]["energy"]["demand"].as_i64().unwrap()
            > projection["data"]["energy"]["supply"].as_i64().unwrap()
    );
    assert!(projection["data"]["energy"]["net"].as_i64().unwrap() < 0);
    assert!(projection["data"]["productionFactor"].as_f64().unwrap() > 0.0);
    assert!(projection["data"]["productionFactor"].as_f64().unwrap() < 1.0);
    assert_eq!(projection["data"]["fusionOnline"], true);

    client
        .execute(
            "UPDATE planets
             SET metal_mine = 1, crystal_mine = 0,
                 deuterium_synthesizer = 0, solar_plant = 0,
                 fusion_reactor = 0, solar_satellite = 0,
                 research_lab = 12, shipyard = 12, robotics_factory = 10,
                  nanite_factory = 2, energy = 1000000,
                  metal = 100000000, crystal = 100000000, deuterium = 100000000,
                  last_resource_update = clock_timestamp()
             WHERE id = $1",
            &[&first_planet.id.parse::<i32>().unwrap()],
        )
        .await
        .unwrap();
    client
        .execute(
            "UPDATE research
             SET energy_technology = 20, combustion_drive = 20,
                 impulse_drive = 20, hyperspace_technology = 20,
                 hyperspace_drive = 20, espionage_technology = 20,
                 computer_technology = 20, laser_technology = 20,
                 ion_technology = 20, plasma_technology = 20,
                 graviton_technology = 1, shielding_technology = 20,
                 armor_technology = 20
             WHERE user_id = $1",
            &[&first.id.parse::<i32>().unwrap()],
        )
        .await
        .unwrap();

    let preview = app
        .clone()
        .oneshot(request(
            "POST",
            &format!("/api/shipyard/{}/build-preview", first_planet.id),
            &first_token,
            Some(json!({"ship_type": "smallCargo", "count": 2})),
        ))
        .await
        .unwrap();
    assert_eq!(preview.status(), StatusCode::OK);
    let preview = json_body(preview).await;
    assert_eq!(preview["data"]["totalMetal"], 4_000);
    assert_eq!(preview["data"]["totalCrystal"], 4_000);

    let ships = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/shipyard/build",
            &first_token,
            Some(json!({
                "planetId": first_planet.id,
                "shipType": "smallCargo",
                "quantity": 2,
                "totalMetal": 0,
                "completesInSeconds": 0
            })),
        ))
        .await
        .unwrap();
    assert_eq!(ships.status(), StatusCode::OK);
    let ships = json_body(ships).await;
    assert_eq!(ships["data"]["quantity"], 2);
    assert_eq!(
        ships["data"]["completesInSeconds"],
        preview["data"]["totalBuildTimeSeconds"]
    );

    let resources = app
        .clone()
        .oneshot(request("GET", "/api/account/resources", &first_token, None))
        .await
        .unwrap();
    assert_eq!(resources.status(), StatusCode::OK);
    let resources = json_body(resources).await;
    assert_eq!(resources["data"]["metal"], 99_996_000);

    let restarted = Database::from_database_url(&database_url).unwrap();
    let restarted_app = build_router_with_dependencies(
        "gameplay-restart-test",
        Some(restarted.clone()),
        AccountRepository::from_environment(Some(restarted)),
    );
    let queue = restarted_app
        .oneshot(request(
            "GET",
            &format!("/api/shipyard/{}/queue", first_planet.id),
            &first_token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(queue.status(), StatusCode::OK);
    let queue = json_body(queue).await;
    assert_eq!(queue["data"][0]["shipType"], "smallCargo");
    assert_eq!(queue["data"][0]["count"], 2);
}
