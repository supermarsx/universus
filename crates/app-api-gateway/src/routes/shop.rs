use axum::extract::rejection::JsonRejection;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::response::{bad_request, success};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ShopOffer {
    offer_id: &'static str,
    item: &'static str,
    price_dark_matter: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ShopPackage {
    package_id: &'static str,
    resources: ResourceTriplet,
    price_dark_matter: i64,
}

#[derive(Debug, Serialize)]
struct ResourceTriplet {
    metal: i64,
    crystal: i64,
    deuterium: i64,
}

#[derive(Debug, Deserialize)]
struct PurchasePreviewRequest {
    package_id: String,
    quantity: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PurchasePreview {
    package_id: String,
    quantity: i64,
    total_dark_matter: i64,
}

pub fn router() -> Router {
    Router::new()
        .route("/api/shop/offers", get(list_offers_handler))
        .route("/api/shop/packages", get(list_packages_handler))
        .route("/api/shop/purchase-preview", post(purchase_preview_handler))
}

async fn list_offers_handler() -> Response {
    success(vec![
        ShopOffer {
            offer_id: "of-001",
            item: "Commander",
            price_dark_matter: 2500,
        },
        ShopOffer {
            offer_id: "of-002",
            item: "Merchant",
            price_dark_matter: 1800,
        },
    ])
}

async fn list_packages_handler() -> Response {
    success(vec![
        ShopPackage {
            package_id: "pkg-small",
            resources: ResourceTriplet {
                metal: 100_000,
                crystal: 50_000,
                deuterium: 20_000,
            },
            price_dark_matter: 900,
        },
        ShopPackage {
            package_id: "pkg-large",
            resources: ResourceTriplet {
                metal: 500_000,
                crystal: 250_000,
                deuterium: 120_000,
            },
            price_dark_matter: 3900,
        },
    ])
}

async fn purchase_preview_handler(
    payload: Result<Json<PurchasePreviewRequest>, JsonRejection>,
) -> Response {
    let Json(input) = match payload {
        Ok(value) => value,
        Err(_) => return bad_request("Invalid purchase preview payload"),
    };
    if input.quantity <= 0 {
        return bad_request("Quantity must be greater than zero");
    }

    let price = match input.package_id.as_str() {
        "pkg-small" => 900,
        "pkg-large" => 3900,
        _ => return bad_request("Package not found"),
    };

    success(PurchasePreview {
        package_id: input.package_id,
        quantity: input.quantity,
        total_dark_matter: price * input.quantity,
    })
}
