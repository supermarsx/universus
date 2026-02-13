use axum::response::Response;
use axum::routing::get;
use axum::Router;
use serde::Serialize;

use crate::response::success;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LeaderboardEntry {
    rank: i64,
    name: &'static str,
    points: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LeaderboardSnapshot {
    scope: &'static str,
    entries: Vec<LeaderboardEntry>,
}

pub fn router() -> Router {
    Router::new()
        .route("/api/leaderboard", get(global_leaderboard_handler))
        .route("/api/leaderboard/players", get(players_leaderboard_handler))
        .route(
            "/api/leaderboard/alliances",
            get(alliances_leaderboard_handler),
        )
}

async fn global_leaderboard_handler() -> Response {
    success(LeaderboardSnapshot {
        scope: "global",
        entries: top_players(),
    })
}

async fn players_leaderboard_handler() -> Response {
    success(LeaderboardSnapshot {
        scope: "players",
        entries: top_players(),
    })
}

async fn alliances_leaderboard_handler() -> Response {
    success(LeaderboardSnapshot {
        scope: "alliances",
        entries: vec![
            LeaderboardEntry {
                rank: 1,
                name: "SOL",
                points: 22_400_000,
            },
            LeaderboardEntry {
                rank: 2,
                name: "AUR",
                points: 19_700_000,
            },
        ],
    })
}

fn top_players() -> Vec<LeaderboardEntry> {
    vec![
        LeaderboardEntry {
            rank: 1,
            name: "AdmiralNova",
            points: 4_300_000,
        },
        LeaderboardEntry {
            rank: 2,
            name: "Commander",
            points: 2_445_000,
        },
    ]
}
