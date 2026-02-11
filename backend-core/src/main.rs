use serde::{Deserialize, Serialize};
use std::io::BufRead;
use std::process::Stdio;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
/// Core gRPC server and worker manager for the Universus simulation core.
///
/// This binary provides two main modes:
/// - Primary server mode: listens for gRPC requests and forwards simulation
///   work to worker processes.
/// - Worker mode (spawned by the manager): runs a local or TCP worker loop
///   that executes simulations synchronously and returns JSON results over
///   the IPC channel.
///
/// The server implements the `core::GameLoop` tonic service and uses a
/// Manager to spawn and select worker processes per "universe". Workers may
/// communicate over local sockets or TCP depending on `CORE_IPC`.
use tonic::{transport::Server, Request, Response, Status};
mod ipc_local;
use interprocess::local_socket::LocalSocketListener;

/// Generated protobuf types from `proto/core.proto`.
pub mod core {
    tonic::include_proto!("core");
}
use core::game_loop_server::{GameLoop, GameLoopServer};
use core::{
    BattleRequest, BattleState, CombatResult, FleetMovementRequest, FleetMovementResult,
    SimulateRequest, StepRequest,
};

mod ships;
mod sim;
use sim::simulate_combat;

/// Lightweight in-memory representation of a live battle.
#[derive(Clone, Debug, Default)]
struct Battle {
    id: String,
    tick: i32,
    json_state: String,
}

/// IPC payload sent from the manager to worker processes when performing a
/// simulation. Serialized as JSON for cross-process communication.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct IPCSimulateRequest {
    cmd: Option<String>,
    battle_id: String,
    attacker_ships: std::collections::HashMap<String, i32>,
    defender_ships: std::collections::HashMap<String, i32>,
    defender_defenses: std::collections::HashMap<String, i32>,
    attacker_tech: std::collections::HashMap<String, i32>,
    defender_tech: std::collections::HashMap<String, i32>,
    planet_metal: i64,
    planet_crystal: i64,
    planet_deuterium: i64,
    seed: String,
    universe: String,
}

/// IPC result returned by worker processes. This mirrors the essential
/// fields returned by the simulation logic, encoded as JSON.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct IPCCombatResult {
    winner: String,
    rounds: Vec<(i32, i32, i32, i32)>,
    attacker_losses: std::collections::HashMap<String, i32>,
    defender_losses: std::collections::HashMap<String, i32>,
    loot: Option<(i64, i64, i64)>,
    debris: Option<(i64, i64)>,
    status: Option<String>,
}

/// Handle describing a spawned worker process: the IPC address, a simple
/// atomic load counter and an optional child process handle (so the manager
/// can terminate or inspect the child).
#[derive(Clone)]
struct WorkerHandle {
    addr: String,
    load: Arc<AtomicUsize>,
    child: Arc<tokio::sync::Mutex<Option<std::process::Child>>>,
}

/// Manager that maintains worker pools per universe and handles spawning
/// new workers when necessary.
struct Manager {
    workers: dashmap::DashMap<String, Vec<WorkerHandle>>,
    max_workers: usize,
    spawn_backoff_secs: u64,
    connect_timeout_ms: u64,
    ipc_mode: String,
}

impl Manager {
    /// Create a new Manager reading configuration from environment
    /// variables with sensible defaults.
    fn new() -> Self {
        let max_workers = std::env::var("CORE_MAX_WORKERS_PER_UNIVERSE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(4usize);
        let spawn_backoff = std::env::var("CORE_SPAWN_BACKOFF_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2u64);
        let connect_to = std::env::var("CORE_WORKER_CONNECT_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(500u64);
        let ipc_mode = std::env::var("CORE_IPC").unwrap_or_else(|_| "tcp".to_string());
        Self {
            workers: dashmap::DashMap::new(),
            max_workers,
            spawn_backoff_secs: spawn_backoff,
            connect_timeout_ms: connect_to,
            ipc_mode,
        }
    }

    /// Spawn a new worker process for the given universe. This runs a
    /// blocking command to start the same binary in `--worker` mode and
    /// reads a handshake line from the child's stdout indicating the
    /// worker's IPC address (either `SOCKET:<path>` or `PORT:<ip:port>`).
    async fn spawn_worker(&self, universe: &str) -> anyhow::Result<WorkerHandle> {
        let mode = self.ipc_mode.clone();
        let uni = universe.to_string();
        let (addr, child) = tokio::task::spawn_blocking(
            move || -> anyhow::Result<(String, std::process::Child)> {
                let exe = std::env::current_exe()?;
                let mut cmd = std::process::Command::new(exe);
                cmd.arg("--worker").arg("--universe").arg(&uni);
                if mode == "local" {
                    cmd.arg("--local");
                } else {
                    cmd.arg("--tcp");
                }
                cmd.stdout(Stdio::piped());
                cmd.stderr(Stdio::inherit());
                let mut child = cmd.spawn()?;
                let stdout = child
                    .stdout
                    .take()
                    .ok_or_else(|| anyhow::anyhow!("missing stdout"))?;
                let mut rdr = std::io::BufReader::new(stdout);
                let mut line = String::new();
                rdr.read_line(&mut line)?;
                let prefix = if mode == "local" { "SOCKET:" } else { "PORT:" };
                let addr = line
                    .trim()
                    .strip_prefix(prefix)
                    .map(|s| s.to_string())
                    .ok_or_else(|| anyhow::anyhow!("no handshake line"))?;
                Ok((addr, child))
            },
        )
        .await??;
        Ok(WorkerHandle {
            addr,
            load: Arc::new(AtomicUsize::new(0)),
            child: Arc::new(tokio::sync::Mutex::new(Some(child))),
        })
    }

    /// Pick the least-loaded available worker for a universe or spawn a new
    /// one if the pool isn't full. Returns an error if the manager cannot
    /// spawn because the per-universe limit has been reached.
    async fn pick_worker_or_spawn(&self, universe: &str) -> anyhow::Result<WorkerHandle> {
        if let Some(entry) = self.workers.get(universe) {
            if !entry.is_empty() {
                let mut best_idx = 0usize;
                let mut best_l = usize::MAX;
                for (i, w) in entry.iter().enumerate() {
                    let l = w.load.load(Ordering::SeqCst);
                    if l < best_l {
                        best_l = l;
                        best_idx = i;
                    }
                }
                return Ok(entry[best_idx].clone());
            }
        }
        let cur = self.workers.get(universe).map(|v| v.len()).unwrap_or(0);
        if cur >= self.max_workers {
            return Err(anyhow::anyhow!("max workers"));
        }
        let wh = self.spawn_worker(universe).await?;
        self.workers
            .entry(universe.to_string())
            .or_insert_with(Vec::new)
            .push(wh.clone());
        Ok(wh)
    }
}

/// Service implementation for the `core::GameLoop` gRPC interface. The
/// implementation stores a lightweight in-memory map of live `Battle`
/// states and delegates heavy simulations to worker processes managed by
/// `Manager`.
#[derive(Clone)]
struct CoreService {
    battles: dashmap::DashMap<String, Battle>,
    manager: Arc<Manager>,
}

#[tonic::async_trait]
impl GameLoop for CoreService {
    /// Start a new live battle. Creates a `BattleState` and stores it in the
    /// in-memory map keyed by `<universe>:<battle_id>`.
    async fn start_battle(
        &self,
        req: Request<BattleRequest>,
    ) -> Result<Response<BattleState>, Status> {
        let r = req.into_inner();
        let key = format!("{}:{}", r.universe, r.battle_id);
        let b = Battle {
            id: r.battle_id.clone(),
            tick: 0,
            json_state: "{\"status\":\"started\"}".to_string(),
        };
        self.battles.insert(key.clone(), b);
        Ok(Response::new(BattleState {
            battle_id: key,
            tick: 0,
            json_state: "{\"status\":\"started\"}".to_string(),
        }))
    }

    /// Advance a live battle by one tick and return the updated state. If the
    /// battle is not found a `not_found` status is returned.
    async fn step_battle(
        &self,
        req: Request<StepRequest>,
    ) -> Result<Response<BattleState>, Status> {
        let r = req.into_inner();
        let key = format!("{}:{}", r.universe, r.battle_id);
        if let Some(mut e) = self.battles.get_mut(&key) {
            e.tick += 1;
            e.json_state = format!("{{\"tick\":{}}}", e.tick);
            Ok(Response::new(BattleState {
                battle_id: e.id.clone(),
                tick: e.tick,
                json_state: e.json_state.clone(),
            }))
        } else {
            Err(Status::not_found("battle not found"))
        }
    }

    type StreamBattleStream = ReceiverStream<Result<BattleState, Status>>;
    /// Stream periodic battle state updates as a server-side stream. This
    /// implementation sends up to 100 ticks at 100ms intervals for a live
    /// battle. If the battle disappears a `not_found` error is sent and the
    /// stream ends.
    async fn stream_battle(
        &self,
        req: Request<StepRequest>,
    ) -> Result<Response<Self::StreamBattleStream>, Status> {
        let r = req.into_inner();
        let (tx, rx) = mpsc::channel(8);
        let battles = self.battles.clone();
        let key = format!("{}:{}", r.universe, r.battle_id);
        tokio::spawn(async move {
            for _ in 0..100 {
                if let Some(mut e) = battles.get_mut(&key) {
                    e.tick += 1;
                    let s = BattleState {
                        battle_id: e.id.clone(),
                        tick: e.tick,
                        json_state: e.json_state.clone(),
                    };
                    if tx.send(Ok(s)).await.is_err() {
                        break;
                    }
                } else {
                    let _ = tx.send(Err(Status::not_found("battle not found"))).await;
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await
            }
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }

    /// Run an isolated simulation by forwarding the `SimulateRequest` to a
    /// worker process over IPC (local socket or TCP). The function serializes
    /// an `IPCSimulateRequest`, sends it to the chosen worker and expects a
    /// single-line JSON `IPCCombatResult` response which it converts into the
    /// protobuf `CombatResult` to return to the gRPC client.
    async fn simulate_battle(
        &self,
        req: Request<SimulateRequest>,
    ) -> Result<Response<CombatResult>, Status> {
        let r = req.into_inner();
        let universe = if r.universe.is_empty() {
            "default".to_string()
        } else {
            r.universe.clone()
        };
        let ipc = IPCSimulateRequest {
            cmd: None,
            battle_id: r.battle_id.clone(),
            attacker_ships: r.attacker_ships.clone(),
            defender_ships: r.defender_ships.clone(),
            defender_defenses: r.defender_defenses.clone(),
            attacker_tech: r.attacker_tech.clone(),
            defender_tech: r.defender_tech.clone(),
            planet_metal: r.planet_metal,
            planet_crystal: r.planet_crystal,
            planet_deuterium: r.planet_deuterium,
            seed: r.seed.clone(),
            universe: universe.clone(),
        };
        let mut worker = match self.manager.pick_worker_or_spawn(&universe).await {
            Ok(w) => w,
            Err(e) => return Err(Status::internal(format!("spawn failed: {}", e))),
        };
        worker.load.fetch_add(1, Ordering::SeqCst);
        // IPC: tcp or local depending on worker.addr prefix
        let payload = serde_json::to_string(&ipc)
            .map_err(|e| Status::internal(format!("serialize: {}", e)))?;
        let resp = if worker.addr.starts_with("SOCKET:") {
            // local socket synchronous call
            let path = worker.addr.trim_start_matches("SOCKET:").to_string();
            ipc_local::call_sync(path, &payload, self.manager.connect_timeout_ms).map_err(|e| {
                worker.load.fetch_sub(1, Ordering::SeqCst);
                Status::internal(format!("local ipc err: {}", e))
            })?
        } else {
            let mut stream = tokio::net::TcpStream::connect(worker.addr.clone())
                .await
                .map_err(|e| {
                    worker.load.fetch_sub(1, Ordering::SeqCst);
                    Status::internal(format!("connect: {}", e))
                })?;
            use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
            stream.write_all(payload.as_bytes()).await.map_err(|e| {
                worker.load.fetch_sub(1, Ordering::SeqCst);
                Status::internal(format!("write: {}", e))
            })?;
            stream.write_all(b"\n").await.map_err(|e| {
                worker.load.fetch_sub(1, Ordering::SeqCst);
                Status::internal(format!("w2: {}", e))
            })?;
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).await.map_err(|e| {
                worker.load.fetch_sub(1, Ordering::SeqCst);
                Status::internal(format!("read: {}", e))
            })?;
            line
        };
        let ipc_res: IPCCombatResult = serde_json::from_str(&resp).map_err(|e| {
            worker.load.fetch_sub(1, Ordering::SeqCst);
            Status::internal(format!("resp parse {}", e))
        })?;
        worker.load.fetch_sub(1, Ordering::SeqCst);
        let mut rounds = Vec::new();
        for (a, b, c, d) in ipc_res.rounds.iter() {
            rounds.push(core::RoundResult {
                attacker_shots: *a,
                defender_shots: *b,
                attacker_destroyed: *c,
                defender_destroyed: *d,
            });
        }
        Ok(Response::new(CombatResult {
            winner: ipc_res.winner,
            rounds,
            attacker_losses: ipc_res.attacker_losses,
            defender_losses: ipc_res.defender_losses,
            loot: None,
            debris: None,
        }))
    }

    async fn calculate_fleet_movement(
        &self,
        req: Request<FleetMovementRequest>,
    ) -> Result<Response<FleetMovementResult>, Status> {
        let r = req.into_inner();

        let distance = if r.origin_galaxy != r.target_galaxy {
            (r.origin_galaxy - r.target_galaxy).abs() * 20000
        } else if r.origin_system != r.target_system {
            (r.origin_system - r.target_system).abs() * 5 * 19 + 2700
        } else {
            (r.origin_position - r.target_position).abs() * 5 + 1000
        };

        let mut min_speed = f64::INFINITY;
        let mut fuel_needed = 0.0f64;
        let mut cargo_capacity = 0.0f64;

        for ship in &r.ships {
            if ship.count <= 0 {
                continue;
            }
            if ship.base_speed > 0.0 {
                min_speed = min_speed.min(ship.base_speed);
            }
            let count = ship.count as f64;
            fuel_needed += ship.fuel_consumption * count * (distance as f64 / 100.0);
            cargo_capacity += ship.cargo * count;
        }

        let fleet_speed = if min_speed.is_finite() { min_speed } else { 0.0 };
        let travel_time_seconds = if fleet_speed > 0.0 {
            ((distance as f64 / fleet_speed) * 3600.0).ceil() as i32
        } else {
            0
        };

        cargo_capacity -= fuel_needed;

        Ok(Response::new(FleetMovementResult {
            distance,
            fleet_speed,
            travel_time_seconds,
            fuel_needed,
            cargo_capacity,
        }))
    }
}

/// Worker main loop used when the binary is invoked with `--worker`.
///
/// The worker can operate in either `local` (platform local socket) or
/// `tcp` mode. It reads single-line JSON requests and writes single-line
/// JSON responses. Special commands `prewarm` and `drain` are handled as
/// control operations.
async fn worker_main(universe: &str) -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let prewarm = args.iter().any(|a| a == "--prewarm");
    let local = args.iter().any(|a| a == "--local");
    if prewarm {
        let _ = ships::load_ships_for_universe(universe);
    }

    if local {
        // Use blocking local socket listener inside spawn_blocking to keep async runtime clean.
        let pid = std::process::id();
        let tmp = std::env::temp_dir();
        let socket_name = format!("universus_core_{}_{}.sock", universe, pid);
        let socket_path = tmp.join(&socket_name);
        let socket_path_s = socket_path.to_string_lossy().to_string();
        let _ = std::fs::remove_file(&socket_path);

        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let listener = LocalSocketListener::bind(socket_path_s.clone())?;
            println!("SOCKET:{}", socket_path_s);
            for conn in listener.incoming() {
                match conn {
                    Ok(mut s) => {
                        std::thread::spawn(move || {
                            use std::io::{BufRead, Write};
                            // move stream into reader so we can use get_mut() to write later
                            let mut rdr = std::io::BufReader::new(s);
                            let mut line = String::new();
                            while rdr.read_line(&mut line).unwrap_or(0) > 0 {
                                if let Ok(req) = serde_json::from_str::<IPCSimulateRequest>(&line) {
                                    if req.cmd.as_deref() == Some("prewarm") {
                                        let _ = ships::load_ships_for_universe(&req.universe);
                                        let res = IPCCombatResult {
                                            winner: String::new(),
                                            rounds: vec![],
                                            attacker_losses: std::collections::HashMap::new(),
                                            defender_losses: std::collections::HashMap::new(),
                                            loot: None,
                                            debris: None,
                                            status: Some("prewarmed".to_string()),
                                        };
                                        let mut out = serde_json::to_string(&res).unwrap();
                                        out.push('\n');
                                        let _ = rdr.get_mut().write_all(out.as_bytes());
                                    } else if req.cmd.as_deref() == Some("drain") {
                                        let res = IPCCombatResult {
                                            winner: String::new(),
                                            rounds: vec![],
                                            attacker_losses: std::collections::HashMap::new(),
                                            defender_losses: std::collections::HashMap::new(),
                                            loot: None,
                                            debris: None,
                                            status: Some("draining".to_string()),
                                        };
                                        let mut out = serde_json::to_string(&res).unwrap();
                                        out.push('\n');
                                        let _ = rdr.get_mut().write_all(out.as_bytes());
                                        std::process::exit(0);
                                    } else {
                                        let mut proto = core::SimulateRequest::default();
                                        proto.battle_id = req.battle_id.clone();
                                        proto.attacker_ships = req.attacker_ships.clone();
                                        proto.defender_ships = req.defender_ships.clone();
                                        proto.defender_defenses = req.defender_defenses.clone();
                                        proto.attacker_tech = req.attacker_tech.clone();
                                        proto.defender_tech = req.defender_tech.clone();
                                        proto.planet_metal = req.planet_metal;
                                        proto.planet_crystal = req.planet_crystal;
                                        proto.planet_deuterium = req.planet_deuterium;
                                        proto.seed = req.seed.clone();
                                        let cr = simulate_combat(&proto);
                                        let mut rounds = vec![];
                                        for r in cr.rounds.iter() {
                                            rounds.push((
                                                r.attacker_shots,
                                                r.defender_shots,
                                                r.attacker_destroyed,
                                                r.defender_destroyed,
                                            ));
                                        }
                                        let res = IPCCombatResult {
                                            winner: cr.winner,
                                            rounds,
                                            attacker_losses: cr.attacker_losses,
                                            defender_losses: cr.defender_losses,
                                            loot: None,
                                            debris: None,
                                            status: None,
                                        };
                                        let mut out = serde_json::to_string(&res).unwrap();
                                        out.push('\n');
                                        let _ = rdr.get_mut().write_all(out.as_bytes());
                                    }
                                }
                                line.clear();
                            }
                        });
                    }
                    Err(e) => eprintln!("local accept err: {}", e),
                }
            }
            Ok(())
        })
        .await??;
        Ok(())
    } else {
        // TCP worker
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        println!("PORT:{}", addr);
        loop {
            let (mut stream, _) = listener.accept().await?;
            tokio::spawn(async move {
                use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
                let mut reader = BufReader::new(&mut stream);
                let mut line = String::new();
                if reader.read_line(&mut line).await.is_ok() {
                    if let Ok(req) = serde_json::from_str::<IPCSimulateRequest>(&line) {
                        if req.cmd.as_deref() == Some("prewarm") {
                            let _ = ships::load_ships_for_universe(&req.universe);
                            let res = IPCCombatResult {
                                winner: String::new(),
                                rounds: vec![],
                                attacker_losses: std::collections::HashMap::new(),
                                defender_losses: std::collections::HashMap::new(),
                                loot: None,
                                debris: None,
                                status: Some("prewarmed".to_string()),
                            };
                            let mut out = serde_json::to_string(&res).unwrap();
                            out.push('\n');
                            let _ = reader.get_mut().write_all(out.as_bytes()).await;
                        } else if req.cmd.as_deref() == Some("drain") {
                            let res = IPCCombatResult {
                                winner: String::new(),
                                rounds: vec![],
                                attacker_losses: std::collections::HashMap::new(),
                                defender_losses: std::collections::HashMap::new(),
                                loot: None,
                                debris: None,
                                status: Some("draining".to_string()),
                            };
                            let mut out = serde_json::to_string(&res).unwrap();
                            out.push('\n');
                            let _ = reader.get_mut().write_all(out.as_bytes()).await;
                            std::process::exit(0);
                        } else {
                            let req2 = req.clone();
                            let res = tokio::task::spawn_blocking(move || {
                                let mut proto = core::SimulateRequest::default();
                                proto.battle_id = req2.battle_id.clone();
                                proto.attacker_ships = req2.attacker_ships.clone();
                                proto.defender_ships = req2.defender_ships.clone();
                                proto.defender_defenses = req2.defender_defenses.clone();
                                proto.attacker_tech = req2.attacker_tech.clone();
                                proto.defender_tech = req2.defender_tech.clone();
                                proto.planet_metal = req2.planet_metal;
                                proto.planet_crystal = req2.planet_crystal;
                                proto.planet_deuterium = req2.planet_deuterium;
                                proto.seed = req2.seed.clone();
                                let cr = simulate_combat(&proto);
                                let mut rounds = Vec::new();
                                for r in cr.rounds.iter() {
                                    rounds.push((
                                        r.attacker_shots,
                                        r.defender_shots,
                                        r.attacker_destroyed,
                                        r.defender_destroyed,
                                    ));
                                }
                                IPCCombatResult {
                                    winner: cr.winner,
                                    rounds,
                                    attacker_losses: cr.attacker_losses,
                                    defender_losses: cr.defender_losses,
                                    loot: None,
                                    debris: None,
                                    status: None,
                                }
                            })
                            .await
                            .unwrap();
                            let mut out = serde_json::to_string(&res).unwrap();
                            out.push('\n');
                            let _ = reader.get_mut().write_all(out.as_bytes()).await;
                        }
                    }
                }
            });
        }
    }
}

/// Entrypoint for the binary. In server mode the program starts the
/// gRPC server and binds the `CoreService`. When invoked with `--worker`
/// the program runs the `worker_main` and acts as a worker process instead
/// of the manager/server.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--worker") {
        let mut universe = "default".to_string();
        for i in 0..args.len() {
            if args[i] == "--universe" && i + 1 < args.len() {
                universe = args[i + 1].clone();
            }
        }
        return worker_main(&universe).await;
    }
    tracing_subscriber::fmt::init();
    let addr = std::env::var("CORE_BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:50051".to_string());
    let addr = addr.parse()?;
    let manager = Arc::new(Manager::new());
    let service = CoreService {
        battles: dashmap::DashMap::new(),
        manager: manager.clone(),
    };
    println!("Starting backend-core at {}", addr);
    Server::builder()
        .add_service(GameLoopServer::new(service))
        .serve(addr)
        .await?;
    Ok(())
}
