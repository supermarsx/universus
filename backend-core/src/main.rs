use tonic::{transport::Server, Request, Response, Status};
use tokio::sync::{mpsc, Mutex};
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
use tokio_stream::wrappers::ReceiverStream;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use std::time::{Duration, Instant};
use std::process::Stdio;
use serde::{Serialize, Deserialize};
use std::io::BufRead;
use prometheus::Encoder;

pub mod core {
    tonic::include_proto!("core");
}

use core::game_loop_server::{GameLoop, GameLoopServer};
use core::{BattleRequest, StepRequest, BattleState, SimulateRequest, CombatResult};

mod ships;
mod sim;
use sim::simulate_combat;

#[derive(Debug, Default, Clone)]
struct Battle {
    id: String,
    tick: i32,
    json_state: String,
}

// Serializable structs for IPC between manager and worker
#[derive(Serialize, Deserialize)]
struct IPCSimulateRequest {
    cmd: Option<String>,
    battle_id: String,
    attacker_ships: std::collections::HashMap<String,i32>,
    defender_ships: std::collections::HashMap<String,i32>,
    defender_defenses: std::collections::HashMap<String,i32>,
    attacker_tech: std::collections::HashMap<String,i32>,
    defender_tech: std::collections::HashMap<String,i32>,
    planet_metal: i64,
    planet_crystal: i64,
    planet_deuterium: i64,
    seed: String,
    universe: String,
}

#[derive(Serialize, Deserialize)]
struct IPCCombatResult {
    winner: String,
    rounds: Vec<(i32,i32,i32,i32)>,
    attacker_losses: std::collections::HashMap<String,i32>,
    defender_losses: std::collections::HashMap<String,i32>,
    loot: Option<(i64,i64,i64)>,
    debris: Option<(i64,i64)>,
    // optional status for control commands
    status: Option<String>,
}

struct CoreService {
    // simple in-memory store for battles
    battles: dashmap::DashMap<String, Battle>,
    // process manager for universe workers
    manager: Arc<Manager>,
}

#[derive(Clone)]
struct WorkerHandle {
    port: u16,
    load: Arc<AtomicUsize>,
    // child process handle held only to allow killing if needed
    // store as Option so cloning WorkerHandle is cheap
    _child: Arc<Mutex<Option<std::process::Child>>>,
}

struct Manager {
    // map universe -> vec of workers
    workers: dashmap::DashMap<String, Vec<WorkerHandle>>,
    // track last spawn timestamps per universe to avoid spawn storms
    last_spawn: dashmap::DashMap<String, Instant>,
    // maximum workers per universe
    max_workers_per_universe: usize,
    // spawn backoff seconds
    spawn_backoff_secs: u64,
    // connect timeout ms
    worker_connect_timeout_ms: u64,
    // metrics registry
    metrics: prometheus::Registry,
    workers_gauge: prometheus::IntGaugeVec,
    // additional metrics
    in_flight: prometheus::IntGauge,
    spawn_counter: prometheus::IntCounter,
    request_duration: prometheus::Histogram,
    workers_load_gauge: prometheus::IntGaugeVec,
}

impl Manager {
    fn new() -> Self {
        let max_workers = std::env::var("CORE_MAX_WORKERS_PER_UNIVERSE").ok().and_then(|v| v.parse().ok()).unwrap_or(4usize);
        let spawn_backoff = std::env::var("CORE_SPAWN_BACKOFF_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(2u64);
        let connect_to = std::env::var("CORE_WORKER_CONNECT_TIMEOUT_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(500u64);
        let min_workers = std::env::var("CORE_MIN_WORKERS_PER_UNIVERSE").ok().and_then(|v| v.parse().ok()).unwrap_or(1usize);
        let registry = prometheus::Registry::new();
        let workers_gauge = prometheus::IntGaugeVec::new(
            prometheus::opts!("core_workers_total", "number of workers per universe"),
            &["universe"],
        ).unwrap();
        registry.register(Box::new(workers_gauge.clone())).unwrap();
        let in_flight = prometheus::IntGauge::new("core_in_flight_requests", "current in-flight simulate requests").unwrap();
        let spawn_counter = prometheus::IntCounter::new("core_spawn_total", "total worker spawns").unwrap();
        let request_duration = prometheus::Histogram::with_opts(prometheus::opts!("core_request_duration_seconds", "simulate request duration seconds").into()).unwrap();
        registry.register(Box::new(in_flight.clone())).unwrap();
        registry.register(Box::new(spawn_counter.clone())).unwrap();
        registry.register(Box::new(request_duration.clone())).unwrap();
        let worker_load_gauge = prometheus::IntGaugeVec::new(
            prometheus::opts!("core_worker_load", "current load per worker"),
            &["universe","port"],
        ).unwrap();
        registry.register(Box::new(worker_load_gauge.clone())).unwrap();
        let m = Self { workers: dashmap::DashMap::new(), last_spawn: dashmap::DashMap::new(), max_workers_per_universe: max_workers, spawn_backoff_secs: spawn_backoff, worker_connect_timeout_ms: connect_to, metrics: registry, workers_gauge, in_flight, spawn_counter, request_duration, workers_load_gauge: worker_load_gauge };
        // spawn pruning background task
        {
            let mm = m.workers.clone();
            let mlast = m.last_spawn.clone();
            let wg = m.workers_gauge.clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(10));
                    // collect keys first
                    let keys: Vec<String> = mm.iter().map(|e| e.key().clone()).collect();
                    for k in keys {
                                        if let Some(mut entry) = mm.get_mut(&k) {
                            let mut vec = entry.value().clone();
                            vec.retain(|wh| {
                                // remove if child process has exited
                                if let Ok(mut guard) = wh._child.try_lock() {
                                    if let Some(child) = guard.as_mut() {
                                        match child.try_wait() {
                                            Ok(Some(_status)) => return false,
                                            Ok(None) => (),
                                            Err(_) => (),
                                        }
                                    }
                                }
                                // also probe TCP port with short timeout; if unreachable, drop it
                                if let Ok(addr) = format!("127.0.0.1:{}", wh.port).parse::<std::net::SocketAddr>() {
                                    if let Err(_) = std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(200)) {
                                        return false;
                                    }
                                }
                                true
                            });
                            let count = vec.len() as i64;
                            wg.with_label_values(&[&k]).set(count);
                            // update per-worker load metric
                            for wh in vec.iter() {
                                wg.with_label_values(&[&k]).set(vec.len() as i64);
                            }
                            *entry = vec;
                            // cleanup last_spawn for empty entries
                            if entry.is_empty() { mlast.remove(&k); wg.with_label_values(&[&k]).set(0); }
                        }
                    }
                }
            });
        }
        m
    }

    fn pick_worker_or_spawn(&self, universe: &str) -> anyhow::Result<WorkerHandle> {
        // try to find least-loaded worker
        if let Some(mut entry) = self.workers.get_mut(universe) {
            if !entry.is_empty() {
                // pick min load
                let mut best_idx = 0usize;
                let mut best_load = usize::MAX;
                for (i,w) in entry.iter().enumerate() {
                    let l = w.load.load(Ordering::SeqCst);
                    if l < best_load { best_load = l; best_idx = i; }
                }
                return Ok(entry[best_idx].clone());
            }
        }
        // enforce max workers per universe
        let cur = self.workers.get(universe).map(|v| v.len()).unwrap_or(0);
        if cur >= self.max_workers_per_universe {
            return Err(anyhow::anyhow!("max workers reached"));
        }
        // spawn backoff
        if let Some(ts) = self.last_spawn.get(universe) {
            if ts.elapsed() < Duration::from_secs(self.spawn_backoff_secs) {
                return Err(anyhow::anyhow!("spawn backoff"));
            }
        }
        // spawn a new worker
        let wh = self.spawn_worker(universe)?;
        self.last_spawn.insert(universe.to_string(), Instant::now());
        self.spawn_counter.inc();
        self.workers.entry(universe.to_string()).or_insert_with(Vec::new).push(wh.clone());
        Ok(wh)
    }

    fn spawn_worker(&self, universe: &str) -> anyhow::Result<WorkerHandle> {
        // spawn the same binary with --worker --universe <u>
        let exe = std::env::current_exe()?;
        let mut cmd = std::process::Command::new(exe);
        cmd.arg("--worker").arg("--universe").arg(universe.to_string());
        // instruct worker to prewarm
        cmd.arg("--prewarm");
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::inherit());
        let mut child = cmd.spawn()?;
        // read first stdout line to get PORT:<port>
        let stdout = child.stdout.take().expect("child stdout");
        let mut reader = std::io::BufReader::new(stdout);
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let port = if let Some(p) = line.strip_prefix("PORT:") { p.trim().parse()? } else { return Err(anyhow::anyhow!("failed to get port from worker")); };
        let handle = WorkerHandle { port, load: Arc::new(AtomicUsize::new(0)), _child: Arc::new(Mutex::new(Some(child))) };
        Ok(handle)
    }
}

#[tonic::async_trait]
impl GameLoop for CoreService {
    async fn start_battle(&self, req: Request<BattleRequest>) -> Result<Response<BattleState>, Status> {
        let r = req.into_inner();
        let key = format!("{}:{}", r.universe, r.battle_id);
        let b = Battle { id: r.battle_id.clone(), tick: 0, json_state: "{\"status\":\"started\"}".to_string() };
        self.battles.insert(key, b);
        let state = BattleState { battle_id: r.battle_id, tick: 0, json_state: "{\"status\":\"started\"}".to_string() };
        Ok(Response::new(state))
    }

    async fn step_battle(&self, req: Request<StepRequest>) -> Result<Response<BattleState>, Status> {
        let r = req.into_inner();
        let key = format!("{}:{}", r.universe, r.battle_id);
        if let Some(mut entry) = self.battles.get_mut(&key) {
            entry.tick += 1;
            entry.json_state = format!("{{\"status\":\"tick\",\"tick\":{}}}", entry.tick);
            let state = BattleState { battle_id: entry.id.clone(), tick: entry.tick, json_state: entry.json_state.clone() };
            Ok(Response::new(state))
        } else {
            Err(Status::not_found("battle not found"))
        }
    }

    type StreamBattleStream = ReceiverStream<Result<BattleState, Status>>;

    async fn stream_battle(&self, req: Request<StepRequest>) -> Result<Response<Self::StreamBattleStream>, Status> {
        let r = req.into_inner();
        let (mut tx, rx) = mpsc::channel(8);
        let battles = self.battles.clone();
        let key = format!("{}:{}", r.universe, r.battle_id);
        tokio::spawn(async move {
            for i in 0..1000u32 {
                if let Some(mut entry) = battles.get_mut(&key) {
                    entry.tick += 1;
                    entry.json_state = format!("{{\"status\":\"stream\",\"tick\":{}}}", entry.tick);
                    let state = BattleState { battle_id: entry.id.clone(), tick: entry.tick, json_state: entry.json_state.clone() };
                    if tx.send(Ok(state)).await.is_err() {
                        break;
                    }
                } else {
                    let _ = tx.send(Err(Status::not_found("battle not found"))).await;
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn simulate_battle(&self, req: Request<SimulateRequest>) -> Result<Response<CombatResult>, Status> {
        let r = req.into_inner();
        // dispatch to a universe worker (spawn on demand)
        let universe = if r.universe.is_empty() { "default".to_string() } else { r.universe.clone() };
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
        // start duration timer (observed on drop)
        let timer = self.manager.request_duration.start_timer();
        // pick or spawn (with retry on spawn backoff)
        let mut worker = match self.manager.pick_worker_or_spawn(&universe) {
            Ok(w) => w,
            Err(e) => {
                // try once more after short sleep if spawn backoff
               tokio::time::sleep(Duration::from_millis(200)).await;
                match self.manager.pick_worker_or_spawn(&universe) {
                    Ok(w2) => w2,
                    Err(e2) => return Err(Status::internal(format!("failed spawning worker: {} / {}", e, e2)))
                }
            }
        };
        // increment load & metrics
        worker.load.fetch_add(1, Ordering::SeqCst);
        self.manager.in_flight.inc();
        self.manager.workers_load_gauge.with_label_values(&[&universe, &worker.port.to_string()]).set(worker.load.load(Ordering::SeqCst) as i64);

        // connect and send; if connect fails, remove worker from list and retry once
        let addr = format!("127.0.0.1:{}", worker.port);
        let conn_future = TcpStream::connect(&addr);
        let to_ms = Duration::from_millis(self.manager.worker_connect_timeout_ms);
        let mut stream = match tokio::time::timeout(to_ms, conn_future).await {
            Ok(Ok(s)) => s,
            Ok(Err(ioe)) => {
                // io error
                let e_msg = format!("{}", ioe);
                worker.load.fetch_sub(1, Ordering::SeqCst);
                self.manager.in_flight.dec();
                self.manager.workers_load_gauge.with_label_values(&[&universe, &worker.port.to_string()]).set(worker.load.load(Ordering::SeqCst) as i64);
                if let Some(mut v) = self.manager.workers.get_mut(&universe) { v.retain(|w| w.port != worker.port); }
                // retry pick/spawn once
                let worker2 = match self.manager.pick_worker_or_spawn(&universe) {
                    Ok(w) => w,
                    Err(e2) => return Err(Status::internal(format!("connect worker failed and respawn failed: {} / {}", e_msg, e2)))
                };
                worker = worker2;
                worker.load.fetch_add(1, Ordering::SeqCst);
                let addr2 = format!("127.0.0.1:{}", worker.port);
                let conn2 = tokio::time::timeout(to_ms, TcpStream::connect(&addr2)).await;
                match conn2 {
                    Ok(Ok(s2)) => s2,
                    Ok(Err(ioe2)) => { worker.load.fetch_sub(1, Ordering::SeqCst); return Err(Status::internal(format!("connect worker failed (2): {}", ioe2))); }
                    Err(elapsed2) => { worker.load.fetch_sub(1, Ordering::SeqCst); return Err(Status::internal(format!("connect worker timed out (2): {}", elapsed2))); }
                }
            }
            Err(elapsed) => {
                // timeout
                let e_msg = format!("{}", elapsed);
                worker.load.fetch_sub(1, Ordering::SeqCst);
                if let Some(mut v) = self.manager.workers.get_mut(&universe) { v.retain(|w| w.port != worker.port); }
                let worker2 = match self.manager.pick_worker_or_spawn(&universe) {
                    Ok(w) => w,
                    Err(e2) => return Err(Status::internal(format!("connect worker failed and respawn failed: {} / {}", e_msg, e2)))
                };
                worker = worker2;
                worker.load.fetch_add(1, Ordering::SeqCst);
                let addr2 = format!("127.0.0.1:{}", worker.port);
                let conn2 = tokio::time::timeout(to_ms, TcpStream::connect(&addr2)).await;
                match conn2 {
                    Ok(Ok(s2)) => s2,
                    Ok(Err(ioe2)) => { worker.load.fetch_sub(1, Ordering::SeqCst); return Err(Status::internal(format!("connect worker failed (2): {}", ioe2))); }
                    Err(elapsed2) => { worker.load.fetch_sub(1, Ordering::SeqCst); return Err(Status::internal(format!("connect worker timed out (2): {}", elapsed2))); }
                }
            }
        };
        let mut line = serde_json::to_string(&ipc).map_err(|e| Status::internal(format!("serialize ipc req: {}", e)))?;
        line.push('\n');
        if let Err(e) = stream.write_all(line.as_bytes()).await { worker.load.fetch_sub(1, Ordering::SeqCst); self.manager.in_flight.dec(); self.manager.workers_load_gauge.with_label_values(&[&universe, &worker.port.to_string()]).set(worker.load.load(Ordering::SeqCst) as i64); timer.observe_duration(); return Err(Status::internal(format!("write to worker: {}", e))); }
        let mut reader = BufReader::new(stream);
        let mut resp_line = String::new();
        if let Err(e) = reader.read_line(&mut resp_line).await { 
            worker.load.fetch_sub(1, Ordering::SeqCst); 
            self.manager.in_flight.dec(); 
            self.manager.workers_load_gauge.with_label_values(&[&universe, &worker.port.to_string()]).set(worker.load.load(Ordering::SeqCst) as i64); 
            timer.observe_duration(); 
            return Err(Status::internal(format!("read from worker: {}", e))); 
        }
        worker.load.fetch_sub(1, Ordering::SeqCst);
        self.manager.in_flight.dec();
        self.manager.workers_load_gauge.with_label_values(&[&universe, &worker.port.to_string()]).set(worker.load.load(Ordering::SeqCst) as i64);
        let ipc_res = match serde_json::from_str::<IPCCombatResult>(&resp_line) {
            Ok(v) => v,
            Err(e) => { timer.observe_duration(); return Err(Status::internal(format!("ipc resp parse: {}", e))); }
        };
        timer.observe_duration();
        // convert IPCCombatResult into proto CombatResult
        let mut rounds = Vec::new();
        for (a,b,c,d) in ipc_res.rounds.iter() {
            rounds.push(core::RoundResult { attacker_shots: *a, defender_shots: *b, attacker_destroyed: *c, defender_destroyed: *d });
        }
        let attacker_losses = ipc_res.attacker_losses;
        let defender_losses = ipc_res.defender_losses;
        let loot = ipc_res.loot.map(|(m,c,d)| core::Loot { metal: m, crystal: c, deuterium: d });
        let debris = ipc_res.debris.map(|(m,c)| core::Debris { metal: m, crystal: c });
        let result = CombatResult { winner: ipc_res.winner, rounds, attacker_losses, defender_losses, loot, debris };
        Ok(Response::new(result))
    }
}

async fn worker_main(universe: &str) -> Result<(), Box<dyn std::error::Error>> {
    // check if we should prewarm before accepting connections
    let args: Vec<String> = std::env::args().collect();
    let prewarm = args.iter().any(|a| a == "--prewarm");
    if prewarm {
        // load ship defs and other caches
        let _ = ships::load_ships_for_universe(universe);
    }
    // bind to ephemeral port and print it to stdout for the parent AFTER prewarm
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    println!("PORT:{}", port);
    // track last request for idle shutdown
    let last_req = Arc::new(tokio::sync::Mutex::new(Instant::now()));
    let last_req_clone = last_req.clone();
    // idle shutdown task
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
            let lr = last_req_clone.lock().await;
            if lr.elapsed() > Duration::from_secs(60) {
                // exit process
                std::process::exit(0);
            }
        }
    });

    loop {
        let (socket, _) = listener.accept().await?;
        let last_req = last_req.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_conn(socket, last_req).await { eprintln!("worker handle error: {}", e); }
        });
    }
}

async fn handle_conn(mut socket: TcpStream, last_req: Arc<tokio::sync::Mutex<Instant>>) -> Result<(), Box<dyn std::error::Error>> {
    let (r,w) = socket.split();
    let mut reader = BufReader::new(r);
    let mut writer = w; // move writer into this scope once
    let mut line = String::new();
    while reader.read_line(&mut line).await? > 0 {
        let req: IPCSimulateRequest = serde_json::from_str(&line)?;
        // update last_req
        {
            let mut lr = last_req.lock().await; *lr = Instant::now();
        }
        // run simulation in blocking task
        // allow special control commands from manager, e.g., prewarm or health-check
        if let Some(cmd) = req.cmd.as_ref() {
            if cmd == "prewarm" {
                // perform prewarm: load ship defs for universe
                let _ = ships::load_ships_for_universe(&req.universe);
                let status = IPCCombatResult { winner: "".to_string(), rounds: vec![], attacker_losses: std::collections::HashMap::new(), defender_losses: std::collections::HashMap::new(), loot: None, debris: None, status: Some("prewarmed".to_string()) };
                let mut out = serde_json::to_string(&status)?;
                out.push('\n');
                writer.write_all(out.as_bytes()).await?;
                line.clear();
                continue;
            }
        }
        let res = tokio::task::spawn_blocking(move || {
            // convert IPCSimulateRequest into core::SimulateRequest
            let mut proto_req = core::SimulateRequest::default();
            proto_req.battle_id = req.battle_id.clone();
            proto_req.attacker_ships = req.attacker_ships.clone();
            proto_req.defender_ships = req.defender_ships.clone();
            proto_req.defender_defenses = req.defender_defenses.clone();
            proto_req.attacker_tech = req.attacker_tech.clone();
            proto_req.defender_tech = req.defender_tech.clone();
            proto_req.planet_metal = req.planet_metal;
            proto_req.planet_crystal = req.planet_crystal;
            proto_req.planet_deuterium = req.planet_deuterium;
            proto_req.seed = req.seed.clone();
            let cr = simulate_combat(&proto_req);
            // convert CombatResult into IPCCombatResult
            let mut rounds = Vec::new();
            for r in cr.rounds.iter() { rounds.push((r.attacker_shots, r.defender_shots, r.attacker_destroyed, r.defender_destroyed)); }
            let loot = cr.loot.map(|l| (l.metal, l.crystal, l.deuterium));
            let debris = cr.debris.map(|d| (d.metal, d.crystal));
            IPCCombatResult { winner: cr.winner, rounds, attacker_losses: cr.attacker_losses, defender_losses: cr.defender_losses, loot, debris, status: None }
        }).await?;
        let mut out = serde_json::to_string(&res)?;
        out.push('\n');
        writer.write_all(out.as_bytes()).await?;
        line.clear();
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // check args for worker mode
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--worker") {
        // find --universe <u>
        let mut universe = "default".to_string();
        for i in 0..args.len() {
            if args[i] == "--universe" && i + 1 < args.len() { universe = args[i+1].clone(); }
        }
        return worker_main(&universe).await;
    }

    tracing_subscriber::fmt::init();
    let addr = std::env::var("CORE_BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:50051".to_string());
    let addr = addr.parse()?;
    let manager = Arc::new(Manager::new());
    let service = CoreService { battles: dashmap::DashMap::new(), manager: manager.clone() };

    // spawn metrics server
    let registry = manager.metrics.clone();
    tokio::spawn(async move {
        let make_svc = hyper::service::make_service_fn(move |_| {
            let reg = registry.clone();
            async move {
                Ok::<_, hyper::Error>(hyper::service::service_fn(move |_req| {
                    let reg = reg.clone();
                    async move {
                        let encoder = prometheus::TextEncoder::new();
                        let mf = reg.gather();
                        let mut buffer = Vec::new();
                        encoder.encode(&mf, &mut buffer).unwrap();
                        Ok::<_, hyper::Error>(hyper::Response::builder().status(200).body(hyper::Body::from(buffer)).unwrap())
                    }
                }))
            }
        });
        let addr = ([0,0,0,0], 9090).into();
        let server = hyper::Server::bind(&addr).serve(make_svc);
        if let Err(e) = server.await { eprintln!("metrics server error: {}", e); }
    });

        println!("Starting backend-core at {}", addr);

    // start prewarm controller to keep a minimal healthy pool per universe
    let manager_clone = manager.clone();
    tokio::spawn(async move {
        let min_workers = std::env::var("CORE_MIN_WORKERS_PER_UNIVERSE").ok().and_then(|v| v.parse().ok()).unwrap_or(1usize);
        loop {
            // discover universes from assets folder
            let assets_root = format!("{}/assets", env!("CARGO_MANIFEST_DIR"));
            let mut universes: Vec<String> = Vec::new();
            if let Ok(entries) = std::fs::read_dir(&assets_root) {
                for e in entries.flatten() {
                    if let Ok(ft) = e.file_type() {
                        if ft.is_dir() { if let Some(name) = e.file_name().to_str() { universes.push(name.to_string()); } }
                    }
                }
            }
            if universes.is_empty() { universes.push("default".to_string()); }

            for u in universes.iter() {
                // ensure at least min_workers are prewarmed
                let cur = manager_clone.workers.get(u).map(|v| v.len()).unwrap_or(0);
                for _ in 0..(min_workers.saturating_sub(cur)) {
                    if let Ok(wh) = manager_clone.spawn_worker(u) {
                        manager_clone.workers.entry(u.clone()).or_insert_with(Vec::new).push(wh);
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });

    Server::builder()
        .add_service(GameLoopServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}

