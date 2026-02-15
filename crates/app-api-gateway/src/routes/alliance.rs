use axum::response::Response;
use axum::routing::get;
use axum::Router;
use serde::Serialize;

use crate::response::success;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AllianceOverview {
    id: &'static str,
    tag: &'static str,
    name: &'static str,
    member_count: i64,
    rank: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AllianceMember {
    user_id: &'static str,
    username: &'static str,
    role: &'static str,
    points: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AllianceDiplomacy {
    ally_tag: &'static str,
    relation: &'static str,
}

pub fn router() -> Router {
    Router::new()
        .route("/api/alliance", get(get_alliance_handler))
        .route("/api/alliances", get(get_alliance_handler))
        .route("/api/alliance/members", get(list_members_handler))
        .route("/api/alliances/members", get(list_members_handler))
        .route("/api/alliance/diplomacy", get(list_diplomacy_handler))
        .route("/api/alliances/diplomacy", get(list_diplomacy_handler))
}

async fn get_alliance_handler() -> Response {
    success(AllianceOverview {
        id: "a-001",
        tag: "AUR",
        name: "Aurora Dominion",
        member_count: 28,
        rank: 4,
    })
}

async fn list_members_handler() -> Response {
    success(vec![
        AllianceMember {
            user_id: "u-rust-1",
            username: "Commander",
            role: "founder",
            points: 2_445_000,
        },
        AllianceMember {
            user_id: "u-rust-2",
            username: "Vanguard",
            role: "officer",
            points: 1_880_100,
        },
    ])
}

async fn list_diplomacy_handler() -> Response {
    success(vec![
        AllianceDiplomacy {
            ally_tag: "SOL",
            relation: "ally",
        },
        AllianceDiplomacy {
            ally_tag: "RIFT",
            relation: "war",
        },
    ])
}
