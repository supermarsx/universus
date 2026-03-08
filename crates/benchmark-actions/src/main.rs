#![forbid(unsafe_code)]

//! Benchmarks for core game-domain computations: combat simulation, economy formulas,
//! fleet routing, queue processing, and serialization. Prints timing results in a
//! table format suitable for CI or manual comparison.

use game_combat::{simulate_combat, CombatInput};
use game_economy::{
    building_construction_time, building_cost, calculate_accumulated_resources, crystal_production,
    defense_cost, deuterium_production, metal_production, research_cost, research_time, ship_cost,
    shipyard_construction_time, solar_plant_energy, storage_capacity, LazyResourceState,
};
use game_fleet::{
    calculate_distance, calculate_movement, process_arrival, FleetComposition, FleetMission,
    FleetMissionType, FleetMovementInput, FleetShipInput, FleetStore, MissionStatus,
    ALL_SHIP_TYPES,
};
use game_queue::{BuildingQueue, LazyResourceState as QueueResourceState, QueueManager};
use std::collections::HashMap;
use std::time::Instant;

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

struct BenchResult {
    name: String,
    iterations: usize,
    total_ms: f64,
    per_iter_us: f64,
}

fn bench<F: FnMut()>(name: &str, iterations: usize, mut f: F) -> BenchResult {
    // Warm-up
    for _ in 0..iterations.min(100) {
        f();
    }
    let start = Instant::now();
    for _ in 0..iterations {
        f();
    }
    let elapsed = start.elapsed();
    let total_ms = elapsed.as_secs_f64() * 1000.0;
    let per_iter_us = (elapsed.as_nanos() as f64) / (iterations as f64) / 1000.0;
    BenchResult {
        name: name.to_string(),
        iterations,
        total_ms,
        per_iter_us,
    }
}

fn print_results(results: &[BenchResult]) {
    println!();
    println!(
        "{:<45} {:>10} {:>12} {:>14}",
        "Benchmark", "Iters", "Total (ms)", "Per-iter (us)"
    );
    println!("{}", "-".repeat(85));
    for r in results {
        println!(
            "{:<45} {:>10} {:>12.2} {:>14.2}",
            r.name, r.iterations, r.total_ms, r.per_iter_us
        );
    }
    println!("{}", "-".repeat(85));
    println!();
}

// ---------------------------------------------------------------------------
// Combat benchmarks
// ---------------------------------------------------------------------------

fn bench_combat_small() -> BenchResult {
    let input = CombatInput {
        attacker_ships: [("light_fighter".into(), 50)].into_iter().collect(),
        defender_ships: [("heavy_fighter".into(), 30)].into_iter().collect(),
        defender_defenses: [("rocket_launcher".into(), 20)].into_iter().collect(),
        attacker_tech: [
            ("weapons".into(), 5),
            ("shielding".into(), 5),
            ("armour".into(), 5),
        ]
        .into_iter()
        .collect(),
        defender_tech: [
            ("weapons".into(), 3),
            ("shielding".into(), 3),
            ("armour".into(), 3),
        ]
        .into_iter()
        .collect(),
        planet_metal: 500_000,
        planet_crystal: 300_000,
        planet_deuterium: 100_000,
        seed: "bench-small".into(),
        universe: "uni1".into(),
        max_rounds: None,
    };
    bench("combat::small (50 LF vs 30 HF + 20 RL)", 10_000, || {
        let _ = simulate_combat(&input);
    })
}

fn bench_combat_large() -> BenchResult {
    let input = CombatInput {
        attacker_ships: [
            ("light_fighter".into(), 1000),
            ("heavy_fighter".into(), 500),
            ("cruiser".into(), 200),
            ("battleship".into(), 100),
            ("battlecruiser".into(), 50),
        ]
        .into_iter()
        .collect(),
        defender_ships: [
            ("light_fighter".into(), 800),
            ("heavy_fighter".into(), 400),
            ("cruiser".into(), 150),
            ("battleship".into(), 75),
        ]
        .into_iter()
        .collect(),
        defender_defenses: [
            ("rocket_launcher".into(), 500),
            ("light_laser".into(), 200),
            ("heavy_laser".into(), 50),
            ("gauss_cannon".into(), 10),
            ("ion_cannon".into(), 20),
        ]
        .into_iter()
        .collect(),
        attacker_tech: [
            ("weapons".into(), 12),
            ("shielding".into(), 12),
            ("armour".into(), 12),
        ]
        .into_iter()
        .collect(),
        defender_tech: [
            ("weapons".into(), 10),
            ("shielding".into(), 10),
            ("armour".into(), 10),
        ]
        .into_iter()
        .collect(),
        planet_metal: 50_000_000,
        planet_crystal: 30_000_000,
        planet_deuterium: 10_000_000,
        seed: "bench-large".into(),
        universe: "uni1".into(),
        max_rounds: None,
    };
    bench("combat::large (1850 vs 1425 + 780 def)", 1_000, || {
        let _ = simulate_combat(&input);
    })
}

// ---------------------------------------------------------------------------
// Economy benchmarks
// ---------------------------------------------------------------------------

fn bench_production_formulas() -> BenchResult {
    bench(
        "economy::production (all formulas x30 lvl)",
        100_000,
        || {
            for level in 1..=30 {
                let _ = metal_production(level, 1);
                let _ = crystal_production(level, 1);
                let _ = deuterium_production(level, 50, 1);
                let _ = solar_plant_energy(level);
            }
        },
    )
}

fn bench_building_costs() -> BenchResult {
    let building_types = [
        "metal_mine",
        "crystal_mine",
        "deuterium_synthesizer",
        "solar_plant",
        "fusion_reactor",
        "robotics_factory",
        "nanite_factory",
        "shipyard",
        "metal_storage",
        "crystal_storage",
        "deuterium_tank",
        "research_lab",
        "terraformer",
        "alliance_depot",
        "missile_silo",
    ];
    bench("economy::building_cost (15 types x 20 lvl)", 50_000, || {
        for bt in &building_types {
            for level in 1..=20 {
                let _ = building_cost(bt, level);
            }
        }
    })
}

fn bench_research_costs() -> BenchResult {
    let research_types = [
        "espionage_technology",
        "computer_technology",
        "weapons_technology",
        "shielding_technology",
        "armour_technology",
        "energy_technology",
        "hyperspace_technology",
        "combustion_drive",
        "impulse_drive",
        "hyperspace_drive",
        "laser_technology",
        "ion_technology",
        "plasma_technology",
        "intergalactic_research_network",
        "astrophysics",
        "graviton_technology",
    ];
    bench("economy::research_cost (16 types x 15 lvl)", 50_000, || {
        for rt in &research_types {
            for level in 1..=15 {
                let _ = research_cost(rt, level);
            }
        }
    })
}

fn bench_ship_defense_costs() -> BenchResult {
    let ships = [
        "small_cargo",
        "large_cargo",
        "light_fighter",
        "heavy_fighter",
        "cruiser",
        "battleship",
        "colony_ship",
        "recycler",
        "espionage_probe",
        "bomber",
        "destroyer",
        "deathstar",
        "battlecruiser",
        "solar_satellite",
    ];
    let defenses = [
        "rocket_launcher",
        "light_laser",
        "heavy_laser",
        "gauss_cannon",
        "ion_cannon",
        "plasma_turret",
        "small_shield_dome",
        "large_shield_dome",
        "anti_ballistic_missiles",
        "interplanetary_missiles",
    ];
    bench("economy::ship+defense costs (24 types)", 100_000, || {
        for s in &ships {
            let _ = ship_cost(s);
        }
        for d in &defenses {
            let _ = defense_cost(d);
        }
    })
}

fn bench_construction_time() -> BenchResult {
    bench(
        "economy::construction_time (build+research+ship)",
        100_000,
        || {
            let _ = building_construction_time(5000.0, 3000.0, 10, 2, 1);
            let _ = research_time(8000.0, 5000.0, 8, 1);
            let _ = shipyard_construction_time(15000.0, 10000.0, 8, 1, 1);
        },
    )
}

fn bench_storage_capacity() -> BenchResult {
    bench("economy::storage_capacity (levels 0-20)", 200_000, || {
        for level in 0..=20 {
            let _ = storage_capacity(level);
        }
    })
}

fn bench_lazy_resource_accumulation() -> BenchResult {
    let state = LazyResourceState {
        metal: 100_000.0,
        crystal: 80_000.0,
        deuterium: 30_000.0,
        metal_per_hour: 5000.0,
        crystal_per_hour: 3000.0,
        deuterium_per_hour: 1000.0,
        metal_storage_cap: 500_000.0,
        crystal_storage_cap: 400_000.0,
        deuterium_storage_cap: 200_000.0,
        last_update: 1_700_000_000,
    };
    bench("economy::lazy_resource_accum (1-hr eval)", 500_000, || {
        let _ = calculate_accumulated_resources(&state, 1_700_003_600);
    })
}

// ---------------------------------------------------------------------------
// Fleet benchmarks
// ---------------------------------------------------------------------------

fn bench_distance_calculation() -> BenchResult {
    bench("fleet::distance (mixed galaxy/system/pos)", 500_000, || {
        let _ = calculate_distance(1, 100, 5, 1, 200, 10);
        let _ = calculate_distance(1, 100, 5, 2, 50, 3);
        let _ = calculate_distance(1, 100, 5, 1, 100, 12);
    })
}

fn bench_fleet_movement() -> BenchResult {
    let input = FleetMovementInput {
        origin_galaxy: 1,
        origin_system: 100,
        origin_position: 5,
        target_galaxy: 1,
        target_system: 250,
        target_position: 8,
        ships: vec![
            FleetShipInput {
                count: 200,
                base_speed: 12500.0,
                fuel_consumption: 20.0,
                cargo: 5000.0,
            },
            FleetShipInput {
                count: 50,
                base_speed: 10000.0,
                fuel_consumption: 50.0,
                cargo: 25000.0,
            },
            FleetShipInput {
                count: 30,
                base_speed: 7500.0,
                fuel_consumption: 300.0,
                cargo: 1500.0,
            },
        ],
    };
    bench(
        "fleet::movement (3 ship types, same galaxy)",
        200_000,
        || {
            let _ = calculate_movement(&input);
        },
    )
}

fn bench_fleet_composition_stats() -> BenchResult {
    let mut ships = HashMap::new();
    for ship_type in ALL_SHIP_TYPES {
        ships.insert(ship_type.to_string(), 100);
    }
    let comp = FleetComposition { ships };
    bench("fleet::composition stats (15 types x100)", 200_000, || {
        let _ = comp.total_ships();
        let _ = comp.cargo_capacity();
        let _ = comp.min_speed();
        let _ = comp.fuel_consumption();
        let _ = comp.combat_power();
    })
}

fn bench_fleet_dispatch() -> BenchResult {
    bench("fleet::dispatch + process_arrival", 50_000, || {
        let mut store = FleetStore::new(1.0);
        let mut comp = FleetComposition {
            ships: HashMap::new(),
        };
        comp.ships.insert("light_fighter".to_string(), 100);
        comp.ships.insert("small_cargo".to_string(), 50);

        let origin = game_fleet::Coordinates {
            galaxy: 1,
            system: 100,
            position: 5,
        };
        let target = game_fleet::Coordinates {
            galaxy: 1,
            system: 200,
            position: 8,
        };
        let resources = game_fleet::Resources {
            metal: 10000,
            crystal: 5000,
            deuterium: 2000,
        };

        if let Ok(fleet_id) = store.dispatch_fleet(
            "player1",
            FleetMissionType::Attack,
            origin.clone(),
            target.clone(),
            comp,
            resources,
            1_700_000_000,
        ) {
            if let Some(mission) = store.get_fleet(fleet_id) {
                let _ = process_arrival(mission);
            }
        }
    })
}

// ---------------------------------------------------------------------------
// Queue benchmarks
// ---------------------------------------------------------------------------

fn bench_queue_enqueue_building() -> BenchResult {
    bench("queue::enqueue_building (100 planets)", 5_000, || {
        let mut queue = BuildingQueue::new();
        for planet_id in 0..100 {
            let avail = (1_000_000i64, 800_000i64, 400_000i64);
            let _ = queue.enqueue(
                planet_id,
                "metal_mine",
                5,
                avail,
                5,
                0,
                "2025-01-01T00:00:00Z",
            );
        }
    })
}

fn bench_queue_check_completion() -> BenchResult {
    let mut queue = BuildingQueue::new();
    for planet_id in 0..100 {
        let avail = (1_000_000i64, 800_000i64, 400_000i64);
        let _ = queue.enqueue(
            planet_id,
            "metal_mine",
            5,
            avail,
            5,
            0,
            "2025-01-01T00:00:00Z",
        );
    }
    bench("queue::check_completion (100 planets)", 50_000, || {
        for planet_id in 0..100 {
            let _ = queue.check_completion(planet_id, "2025-01-02T00:00:00Z");
        }
    })
}

fn bench_queue_manager_batch() -> BenchResult {
    bench("queue::manager process_all (50 planets)", 2_000, || {
        let mut manager = QueueManager::new();
        let planets: Vec<i32> = (0..50).collect();
        let players: Vec<i32> = (0..10).collect();

        for &pid in &planets {
            let avail = (5_000_000i64, 3_000_000i64, 1_500_000i64);
            let _ = manager.enqueue_building(
                pid,
                "crystal_mine",
                3,
                avail,
                3,
                0,
                "2025-01-01T00:00:00Z",
            );
        }
        let _ = manager.process_all_completions(&planets, &players, "2025-01-02T00:00:00Z");
    })
}

fn bench_lazy_resource_spend() -> BenchResult {
    bench("queue::lazy_resource spend+evaluate", 200_000, || {
        let mut state = QueueResourceState {
            metal: 500_000,
            crystal: 400_000,
            deuterium: 200_000,
            metal_per_hour: 5000.0,
            crystal_per_hour: 3000.0,
            deuterium_per_hour: 1000.0,
            storage_metal: 1_000_000,
            storage_crystal: 800_000,
            storage_deuterium: 400_000,
            last_updated: "2025-01-01T00:00:00Z".to_string(),
        };
        let _ = state.evaluate("2025-01-01T01:00:00Z");
        let _ = state.spend(50_000, 30_000, 10_000, "2025-01-01T01:00:00Z");
    })
}

// ---------------------------------------------------------------------------
// Serialization benchmarks
// ---------------------------------------------------------------------------

fn bench_serde_combat_result() -> BenchResult {
    let input = CombatInput {
        attacker_ships: [("light_fighter".into(), 100)].into_iter().collect(),
        defender_ships: [("heavy_fighter".into(), 50)].into_iter().collect(),
        defender_defenses: [("rocket_launcher".into(), 30)].into_iter().collect(),
        attacker_tech: [
            ("weapons".into(), 8),
            ("shielding".into(), 8),
            ("armour".into(), 8),
        ]
        .into_iter()
        .collect(),
        defender_tech: [
            ("weapons".into(), 6),
            ("shielding".into(), 6),
            ("armour".into(), 6),
        ]
        .into_iter()
        .collect(),
        planet_metal: 1_000_000,
        planet_crystal: 500_000,
        planet_deuterium: 200_000,
        seed: "serde-bench".into(),
        universe: "uni1".into(),
        max_rounds: None,
    };
    let result = simulate_combat(&input);
    bench("serde::combat_result round-trip", 50_000, || {
        let serialized = serde_json::to_string(&result).unwrap();
        let _: game_combat::CombatResult = serde_json::from_str(&serialized).unwrap();
    })
}

fn bench_serde_fleet_mission() -> BenchResult {
    let mission = FleetMission {
        id: 1,
        owner_id: "player1".to_string(),
        mission_type: FleetMissionType::Attack,
        origin: game_fleet::Coordinates {
            galaxy: 1,
            system: 100,
            position: 5,
        },
        target: game_fleet::Coordinates {
            galaxy: 1,
            system: 200,
            position: 8,
        },
        composition: FleetComposition {
            ships: [
                ("light_fighter".to_string(), 500),
                ("cruiser".to_string(), 100),
                ("battleship".to_string(), 50),
            ]
            .into_iter()
            .collect(),
        },
        resources_carried: game_fleet::Resources {
            metal: 100_000,
            crystal: 50_000,
            deuterium: 25_000,
        },
        departure_time: "2025-01-01T00:00:00Z".to_string(),
        arrival_time: "2025-01-01T01:30:00Z".to_string(),
        return_time: "2025-01-01T03:00:00Z".to_string(),
        status: MissionStatus::Outbound,
        fuel_consumed: 15_000.0,
    };
    bench("serde::fleet_mission round-trip", 100_000, || {
        let serialized = serde_json::to_string(&mission).unwrap();
        let _: FleetMission = serde_json::from_str(&serialized).unwrap();
    })
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    println!("=== Universus Game Benchmark Suite ===");
    println!();

    let mut results = Vec::new();

    // Combat
    println!("Running combat benchmarks...");
    results.push(bench_combat_small());
    results.push(bench_combat_large());

    // Economy
    println!("Running economy benchmarks...");
    results.push(bench_production_formulas());
    results.push(bench_building_costs());
    results.push(bench_research_costs());
    results.push(bench_ship_defense_costs());
    results.push(bench_construction_time());
    results.push(bench_storage_capacity());
    results.push(bench_lazy_resource_accumulation());

    // Fleet
    println!("Running fleet benchmarks...");
    results.push(bench_distance_calculation());
    results.push(bench_fleet_movement());
    results.push(bench_fleet_composition_stats());
    results.push(bench_fleet_dispatch());

    // Queue
    println!("Running queue benchmarks...");
    results.push(bench_queue_enqueue_building());
    results.push(bench_queue_check_completion());
    results.push(bench_queue_manager_batch());
    results.push(bench_lazy_resource_spend());

    // Serialization
    println!("Running serialization benchmarks...");
    results.push(bench_serde_combat_result());
    results.push(bench_serde_fleet_mission());

    print_results(&results);

    // Summary
    let total_iters: usize = results.iter().map(|r| r.iterations).sum();
    let total_ms: f64 = results.iter().map(|r| r.total_ms).sum();
    println!(
        "Total: {} iterations across {} benchmarks in {:.1} ms",
        total_iters,
        results.len(),
        total_ms
    );
}
