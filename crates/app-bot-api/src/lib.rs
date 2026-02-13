use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

pub const SERVICE_NAME: &str = "app-bot-api";
pub const DEFAULT_PORT: u16 = 3002;

#[derive(Clone)]
pub struct AppState {
    inner: Arc<Mutex<BotStore>>,
}

impl AppState {
    fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(BotStore::default())),
        }
    }
}

#[derive(Default)]
struct BotStore {
    bots: HashMap<u64, Bot>,
    next_id: u64,
}

#[derive(Clone, Serialize, Deserialize)]
struct Bot {
    id: u64,
    username: String,
    email: String,
    personality_type: String,
    is_active: bool,
    difficulty_level: u8,
    think_interval_minutes: u16,
    total_resources_plundered: u64,
    win_rate: f32,
}

#[derive(Serialize)]
struct ServiceStatus {
    status: &'static str,
    service: &'static str,
}

#[derive(Serialize)]
struct ApiResponse<T: Serialize> {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            message: None,
            error: None,
        }
    }

    fn ok_message(message: impl Into<String>) -> Self {
        Self {
            success: true,
            data: None,
            message: Some(message.into()),
            error: None,
        }
    }

    fn err(error: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            message: None,
            error: Some(error.into()),
        }
    }
}

#[derive(Deserialize)]
struct ListBotsQuery {
    is_active: Option<bool>,
    personality_type: Option<String>,
    min_difficulty: Option<u8>,
    max_difficulty: Option<u8>,
}

#[derive(Deserialize)]
struct CreateBotRequest {
    username: String,
    email: String,
    personality_type: String,
    difficulty_level: Option<u8>,
}

#[derive(Deserialize)]
struct UpdateBotRequest {
    is_active: Option<bool>,
    difficulty_level: Option<u8>,
    think_interval_minutes: Option<u16>,
}

#[derive(Deserialize)]
struct LeaderboardQuery {
    limit: Option<usize>,
}

#[derive(Serialize)]
struct BotDetails {
    bot: Bot,
    #[serde(rename = "recentActions")]
    recent_actions: Vec<serde_json::Value>,
    statistics: Vec<serde_json::Value>,
    targets: Vec<serde_json::Value>,
}

#[derive(Serialize)]
struct Personality {
    r#type: &'static str,
    name: &'static str,
    description: &'static str,
    traits: serde_json::Value,
}

fn valid_personality(personality: &str) -> bool {
    matches!(
        personality,
        "aggressive_conqueror"
            | "strategic_builder"
            | "diplomatic_negotiator"
            | "resource_hoarder"
            | "speed_rusher"
            | "tech_enthusiast"
            | "alliance_focused"
            | "solo_survivor"
    )
}

fn personalities() -> Vec<Personality> {
    vec![
        Personality {
            r#type: "aggressive_conqueror",
            name: "Aggressive Conqueror",
            description: "Prioritizes military expansion, frequent attacks, rapid fleet building",
            traits: serde_json::json!({"aggression": 90, "military": 95, "economy": 40}),
        },
        Personality {
            r#type: "strategic_builder",
            name: "Strategic Builder",
            description: "Focuses on infrastructure, balanced development, defensive strategies",
            traits: serde_json::json!({"aggression": 30, "economy": 85, "research": 75}),
        },
        Personality {
            r#type: "diplomatic_negotiator",
            name: "Diplomatic Negotiator",
            description: "Alliance-focused, trade-oriented, peaceful expansion",
            traits: serde_json::json!({"diplomacy": 95, "aggression": 15, "economy": 70}),
        },
        Personality {
            r#type: "resource_hoarder",
            name: "Resource Hoarder",
            description: "Maximum resource gathering, conservative playstyle, long-term planning",
            traits: serde_json::json!({"economy": 95, "risk_tolerance": 15, "aggression": 10}),
        },
        Personality {
            r#type: "speed_rusher",
            name: "Speed Rusher",
            description: "Early game aggression, rapid technology advancement, timing-based attacks",
            traits: serde_json::json!({"aggression": 95, "military": 90, "risk_tolerance": 90}),
        },
        Personality {
            r#type: "tech_enthusiast",
            name: "Tech Enthusiast",
            description: "Research-focused, advanced technology, innovative strategies",
            traits: serde_json::json!({"research": 95, "economy": 75, "aggression": 35}),
        },
        Personality {
            r#type: "alliance_focused",
            name: "Alliance-Focused",
            description: "Team player, supports allies, coordinated attacks",
            traits: serde_json::json!({"diplomacy": 90, "military": 65, "economy": 65}),
        },
        Personality {
            r#type: "solo_survivor",
            name: "Solo Survivor",
            description: "Independent play, self-sufficiency, defensive positioning",
            traits: serde_json::json!({"economy": 80, "military": 70, "diplomacy": 30}),
        },
    ]
}

pub fn build_router() -> Router {
    let state = AppState::new();

    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/api/admin/bots/health", get(bots_health))
        .route("/api/admin/bots/process/all", post(process_all_bots))
        .route("/api/admin/bots/leaderboard/top", get(leaderboard_top))
        .route("/api/admin/bots/personalities/list", get(list_personalities))
        .route("/api/admin/bots/think/:id", post(think_by_id))
        .route("/api/admin/bots/:id/think", post(think_by_id))
        .route("/api/admin/bots", get(list_bots).post(create_bot))
        .route(
            "/api/admin/bots/:id",
            get(get_bot).put(update_bot).delete(delete_bot),
        )
        .with_state(state)
}

pub fn listen_port(default_port: u16) -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(default_port)
}

pub async fn serve() {
    tracing_subscriber::fmt::init();

    let addr = SocketAddr::from(([0, 0, 0, 0], listen_port(DEFAULT_PORT)));
    tracing::info!(service = SERVICE_NAME, %addr, "startup");

    axum::Server::bind(&addr)
        .serve(build_router().into_make_service())
        .await
        .expect("server failed");
}

async fn health() -> Json<ServiceStatus> {
    Json(ServiceStatus {
        status: "ok",
        service: SERVICE_NAME,
    })
}

async fn ready() -> Json<ServiceStatus> {
    Json(ServiceStatus {
        status: "ok",
        service: SERVICE_NAME,
    })
}

async fn bots_health() -> Json<ServiceStatus> {
    Json(ServiceStatus {
        status: "ok",
        service: SERVICE_NAME,
    })
}

async fn list_bots(
    State(state): State<AppState>,
    Query(filters): Query<ListBotsQuery>,
) -> Json<ApiResponse<Vec<Bot>>> {
    let store = state.inner.lock().expect("state lock poisoned");
    let mut bots: Vec<Bot> = store.bots.values().cloned().collect();

    bots.retain(|bot| {
        filters
            .is_active
            .map(|value| bot.is_active == value)
            .unwrap_or(true)
            && filters
                .personality_type
                .as_ref()
                .map(|value| &bot.personality_type == value)
                .unwrap_or(true)
            && filters
                .min_difficulty
                .map(|value| bot.difficulty_level >= value)
                .unwrap_or(true)
            && filters
                .max_difficulty
                .map(|value| bot.difficulty_level <= value)
                .unwrap_or(true)
    });

    bots.sort_by_key(|bot| bot.id);

    Json(ApiResponse::ok(bots))
}

async fn get_bot(
    State(state): State<AppState>,
    Path(bot_id): Path<u64>,
) -> (StatusCode, Json<ApiResponse<BotDetails>>) {
    let store = state.inner.lock().expect("state lock poisoned");
    let Some(bot) = store.bots.get(&bot_id).cloned() else {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::err("Bot not found")),
        );
    };

    (
        StatusCode::OK,
        Json(ApiResponse::ok(BotDetails {
            bot,
            recent_actions: Vec::new(),
            statistics: Vec::new(),
            targets: Vec::new(),
        })),
    )
}

async fn create_bot(
    State(state): State<AppState>,
    Json(request): Json<CreateBotRequest>,
) -> (StatusCode, Json<ApiResponse<Bot>>) {
    if request.username.trim().is_empty()
        || request.email.trim().is_empty()
        || request.personality_type.trim().is_empty()
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::err(
                "Missing required fields: username, email, personality_type",
            )),
        );
    }

    if !valid_personality(&request.personality_type) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::err("Invalid personality type")),
        );
    }

    let difficulty_level = request.difficulty_level.unwrap_or(5);
    if !(1..=10).contains(&difficulty_level) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::err("Difficulty level must be between 1 and 10")),
        );
    }

    let mut store = state.inner.lock().expect("state lock poisoned");
    let duplicate = store
        .bots
        .values()
        .any(|bot| bot.username == request.username || bot.email == request.email);
    if duplicate {
        return (
            StatusCode::CONFLICT,
            Json(ApiResponse::err("Username or email already exists")),
        );
    }

    store.next_id += 1;
    let id = store.next_id;

    let bot = Bot {
        id,
        username: request.username,
        email: request.email,
        personality_type: request.personality_type,
        is_active: true,
        difficulty_level,
        think_interval_minutes: 20,
        total_resources_plundered: 0,
        win_rate: 0.0,
    };

    store.bots.insert(bot.id, bot.clone());

    (StatusCode::CREATED, Json(ApiResponse::ok(bot)))
}

async fn update_bot(
    State(state): State<AppState>,
    Path(bot_id): Path<u64>,
    Json(request): Json<UpdateBotRequest>,
) -> (StatusCode, Json<ApiResponse<Bot>>) {
    if let Some(difficulty_level) = request.difficulty_level {
        if !(1..=10).contains(&difficulty_level) {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::err("Difficulty level must be between 1 and 10")),
            );
        }
    }

    let mut store = state.inner.lock().expect("state lock poisoned");
    let Some(bot) = store.bots.get_mut(&bot_id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::err("Bot not found")),
        );
    };

    if request.is_active.is_none()
        && request.difficulty_level.is_none()
        && request.think_interval_minutes.is_none()
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::err("No valid fields to update")),
        );
    }

    if let Some(is_active) = request.is_active {
        bot.is_active = is_active;
    }
    if let Some(difficulty_level) = request.difficulty_level {
        bot.difficulty_level = difficulty_level;
    }
    if let Some(think_interval_minutes) = request.think_interval_minutes {
        bot.think_interval_minutes = think_interval_minutes;
    }

    (StatusCode::OK, Json(ApiResponse::ok(bot.clone())))
}

async fn delete_bot(
    State(state): State<AppState>,
    Path(bot_id): Path<u64>,
) -> (StatusCode, Json<ApiResponse<()>>) {
    let mut store = state.inner.lock().expect("state lock poisoned");
    if store.bots.remove(&bot_id).is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::err("Bot not found")),
        );
    }

    (
        StatusCode::OK,
        Json(ApiResponse::ok_message("Bot deleted successfully")),
    )
}

async fn process_all_bots() -> Json<ApiResponse<()>> {
    Json(ApiResponse::ok_message("Bot processing triggered"))
}

async fn think_by_id(
    State(state): State<AppState>,
    Path(bot_id): Path<u64>,
) -> (StatusCode, Json<ApiResponse<()>>) {
    let store = state.inner.lock().expect("state lock poisoned");
    if !store.bots.contains_key(&bot_id) {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::err("Bot not found")),
        );
    }

    (
        StatusCode::OK,
        Json(ApiResponse::ok_message("Bot think cycle triggered")),
    )
}

async fn leaderboard_top(
    State(state): State<AppState>,
    Query(query): Query<LeaderboardQuery>,
) -> Json<ApiResponse<Vec<Bot>>> {
    let limit = query.limit.unwrap_or(20);
    let store = state.inner.lock().expect("state lock poisoned");
    let mut leaderboard: Vec<Bot> = store.bots.values().filter(|bot| bot.is_active).cloned().collect();

    leaderboard.sort_by(|a, b| {
        b.total_resources_plundered
            .cmp(&a.total_resources_plundered)
            .then_with(|| b.win_rate.partial_cmp(&a.win_rate).unwrap_or(std::cmp::Ordering::Equal))
    });

    leaderboard.truncate(limit);

    Json(ApiResponse::ok(leaderboard))
}

async fn list_personalities() -> Json<ApiResponse<Vec<Personality>>> {
    Json(ApiResponse::ok(personalities()))
}
