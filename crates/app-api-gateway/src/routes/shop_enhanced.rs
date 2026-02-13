use axum::extract::rejection::JsonRejection;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use serde::{Deserialize, Serialize};

use crate::auth_guard::BearerToken;
use crate::response::{bad_request, success};
use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CosmeticItem {
    id: i64,
    name: &'static str,
    rarity: &'static str,
    matrix_only: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ValidatePromotionRequest {
    promo_code: String,
}

pub fn router() -> Router {
    Router::new()
        .route("/api/shop-enhanced/cosmetics", get(cosmetics_handler))
        .route("/api/shop-enhanced/promotions", get(promotions_handler))
        .route("/api/shop-enhanced/flash-sales", get(flash_sales_handler))
        .route(
            "/api/shop-enhanced/promotions/validate",
            post(validate_promotion_handler),
        )
        .route(
            "/api/shop-enhanced/my-cosmetics",
            get(my_cosmetics_handler),
        )
        .route(
            "/api/shop-enhanced/matrix/progress",
            get(matrix_progress_handler),
        )
}

async fn cosmetics_handler() -> Response {
    success(vec![
        CosmeticItem {
            id: 1,
            name: "Neon Fleet Trail",
            rarity: "rare",
            matrix_only: false,
        },
        CosmeticItem {
            id: 2,
            name: "Matrix Commander Skin",
            rarity: "epic",
            matrix_only: true,
        },
    ])
}

async fn promotions_handler() -> Response {
    success(vec![serde_json::json!({
        "promoCode": "WELCOME10",
        "discountPercent": 10,
        "active": true
    })])
}

async fn flash_sales_handler() -> Response {
    success(vec![serde_json::json!({
        "saleId": "fs-001",
        "itemId": 2,
        "endsInSeconds": 3600,
        "discountPercent": 25
    })])
}

async fn validate_promotion_handler(
    BearerToken(_token): BearerToken,
    payload: Result<Json<ValidatePromotionRequest>, JsonRejection>,
) -> Response {
    let Json(input) = match payload {
        Ok(value) => value,
        Err(_) => return bad_request("Invalid promotion payload"),
    };

    if input.promo_code.trim().is_empty() {
        return bad_request("promoCode is required");
    }

    success(serde_json::json!({
        "promoCode": input.promo_code,
        "valid": true,
        "discountPercent": 10
    }))
}

async fn my_cosmetics_handler(BearerToken(_token): BearerToken) -> Response {
    success(vec![
        serde_json::json!({
            "cosmeticItemId": 1,
            "equipped": true
        }),
        serde_json::json!({
            "cosmeticItemId": 2,
            "equipped": false
        }),
    ])
}

async fn matrix_progress_handler(
    BearerToken(_token): BearerToken,
    Extension(_app_state): Extension<AppState>,
) -> Response {
    success(serde_json::json!({
        "points": 420,
        "level": 3,
        "nextLevelAt": 600
    }))
}
