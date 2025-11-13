use tonic::{transport::Server, Request, Response, Status};
use tokio::sync::{mpsc, Mutex};
use std::sync::Arc;
use tokio_stream::wrappers::ReceiverStream;

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

#[derive(Debug, Default)]
struct CoreService {
    // simple in-memory store for battles
    battles: dashmap::DashMap<String, Battle>,
}

#[tonic::async_trait]
impl GameLoop for CoreService {
    async fn start_battle(&self, req: Request<BattleRequest>) -> Result<Response<BattleState>, Status> {
        let r = req.into_inner();
        let b = Battle { id: r.battle_id.clone(), tick: 0, json_state: "{\"status\":\"started\"}".to_string() };
        self.battles.insert(r.battle_id.clone(), b);
        let state = BattleState { battle_id: r.battle_id, tick: 0, json_state: "{\"status\":\"started\"}".to_string() };
        Ok(Response::new(state))
    }

    async fn step_battle(&self, req: Request<StepRequest>) -> Result<Response<BattleState>, Status> {
        let r = req.into_inner();
        if let Some(mut entry) = self.battles.get_mut(&r.battle_id) {
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
        tokio::spawn(async move {
            for i in 0..1000u32 {
                if let Some(mut entry) = battles.get_mut(&r.battle_id) {
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
        // run deterministic simulation
        let result = simulate_combat(&r);
        Ok(Response::new(result))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let addr = std::env::var("CORE_BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:50051".to_string());
    let addr = addr.parse()?;
    let service = CoreService::default();

    println!("Starting backend-core at {}", addr);

    Server::builder()
        .add_service(GameLoopServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
