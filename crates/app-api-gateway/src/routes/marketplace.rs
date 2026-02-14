use axum::extract::{rejection::JsonRejection, Path, Query};
use axum::response::Response;
use axum::routing::{delete, get, post};
use axum::{Extension, Json, Router};
use serde::{Deserialize, Serialize};

use crate::auth_guard::BearerToken;
use crate::response::{bad_request, not_found, success};
use crate::state::{
    AppState, MarketplaceListFilters, MarketplaceListingInput, MarketplaceListingSnapshot,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListingsQuery {
    #[serde(rename = "type")]
    listing_type: Option<String>,
    resource_type: Option<String>,
    fleet_type: Option<String>,
    wanted_type: Option<String>,
    min: Option<i64>,
    max: Option<i64>,
    page: Option<i64>,
    #[serde(rename = "pageSize")]
    page_size: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct MarketplaceCreateRequest {
    #[serde(alias = "listingType")]
    listing_type: Option<String>,
    #[serde(alias = "planetId")]
    planet_id: Option<FlexibleI64>,
    #[serde(alias = "resourceType")]
    resource_type: Option<String>,
    quantity: Option<i64>,
    #[serde(alias = "pricePerUnit")]
    price_per_unit: Option<i64>,
    #[serde(alias = "totalPrice")]
    total_price: Option<i64>,
    #[serde(alias = "fleetType")]
    fleet_type: Option<String>,
    #[serde(alias = "fleetQuantity")]
    fleet_quantity: Option<i64>,
    #[serde(alias = "wantedType")]
    wanted_type: Option<String>,
    #[serde(alias = "wantedAmount")]
    wanted_amount: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct MarketplaceAcceptRequest {
    #[serde(alias = "buyerPlanetId")]
    buyer_planet_id: Option<FlexibleI64>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum FlexibleI64 {
    Number(i64),
    String(String),
}

impl FlexibleI64 {
    fn into_i64(self) -> Option<i64> {
        match self {
            Self::Number(value) => Some(value),
            Self::String(value) => value.trim().parse::<i64>().ok(),
        }
    }
}

#[derive(Debug, Serialize)]
struct ListingsResponse {
    listings: Vec<MarketplaceListingSnapshot>,
    total: i64,
}

#[derive(Debug, Serialize)]
struct ListingResponse {
    listing: MarketplaceListingSnapshot,
}

#[derive(Debug, Serialize)]
struct ListingsCollectionResponse {
    listings: Vec<MarketplaceListingSnapshot>,
}

#[derive(Debug, Serialize)]
struct HistoryResponse {
    transactions: Vec<MarketplaceListingSnapshot>,
}

#[derive(Debug, Serialize)]
struct MarketplaceAcceptResponse {
    success: bool,
    delivery_eta: Option<String>,
    transaction: MarketplaceTransactionResponse,
}

#[derive(Debug, Serialize)]
struct MarketplaceTransactionResponse {
    listing_id: i64,
    buyer_id: i64,
    buyer_planet_id: i64,
    seller_id: i64,
    seller_planet_id: i64,
}

#[derive(Debug, Serialize)]
struct MarketplaceDeleteResponse {
    success: bool,
}

pub fn router() -> Router {
    Router::new()
        .route("/api/marketplace/listings", get(listings_handler))
        .route("/api/marketplace/listings", post(create_listing_handler))
        .route(
            "/api/marketplace/listings/:id/accept",
            post(accept_listing_handler),
        )
        .route("/api/marketplace/listings/:id", get(get_listing_handler))
        .route("/api/marketplace/listings/:id", delete(delete_listing_handler))
        .route("/api/marketplace/my-listings", get(my_listings_handler))
        .route("/api/marketplace/my-history", get(my_history_handler))
}

async fn listings_handler(
    Extension(app_state): Extension<AppState>,
    Query(query): Query<ListingsQuery>,
) -> Response {
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(20).max(1);

    let (listings, total) = app_state.list_marketplace_listings(MarketplaceListFilters {
        listing_type: query.listing_type,
        resource_type: query.resource_type,
        fleet_type: query.fleet_type,
        wanted_type: query.wanted_type,
        min: query.min,
        max: query.max,
        page,
        page_size,
    });

    success(ListingsResponse { listings, total })
}

async fn create_listing_handler(
    BearerToken(token): BearerToken,
    Extension(app_state): Extension<AppState>,
    payload: Result<Json<MarketplaceCreateRequest>, JsonRejection>,
) -> Response {
    let Json(input) = match payload {
        Ok(value) => value,
        Err(_) => return bad_request("Invalid marketplace listing payload"),
    };

    let planet_id = match input.planet_id.and_then(|value| value.into_i64()) {
        Some(value) if value > 0 => value,
        _ => return bad_request("planet_id is required"),
    };

    let listing_type = input
        .listing_type
        .unwrap_or_else(|| "resource".to_string());

    let wanted_type = input
        .wanted_type
        .unwrap_or_else(|| "metal".to_string());
    let wanted_amount = input.wanted_amount.unwrap_or(0);

    let user_id = user_id_from_token(&token);

    let listing_input = match listing_type.as_str() {
        "resource" => {
            let resource_type = match input.resource_type.as_ref() {
                Some(value) if !value.trim().is_empty() => value.clone(),
                _ => return bad_request("Missing resource listing fields"),
            };
            let quantity = match input.quantity {
                Some(value) if value > 0 => value,
                _ => return bad_request("Missing resource listing fields"),
            };
            let price_per_unit = match input.price_per_unit {
                Some(value) if value > 0 => value,
                _ => return bad_request("Missing resource listing fields"),
            };
            let total_price = match input.total_price {
                Some(value) if value > 0 => value,
                _ => return bad_request("Missing resource listing fields"),
            };

            MarketplaceListingInput {
                user_id,
                planet_id,
                listing_type: listing_type.clone(),
                resource_type: Some(resource_type),
                quantity: Some(quantity),
                price_per_unit: Some(price_per_unit),
                total_price: Some(total_price),
                fleet_type: None,
                fleet_quantity: None,
                wanted_type,
                wanted_amount,
            }
        }
        "fleet" => {
            let fleet_type = match input.fleet_type.as_ref() {
                Some(value) if !value.trim().is_empty() => value.clone(),
                _ => return bad_request("Missing fleet listing fields"),
            };
            let fleet_quantity = match input.fleet_quantity {
                Some(value) if value > 0 => value,
                _ => return bad_request("Missing fleet listing fields"),
            };
            let price_per_unit = match input.price_per_unit {
                Some(value) if value > 0 => value,
                _ => return bad_request("Missing fleet listing fields"),
            };
            let total_price = match input.total_price {
                Some(value) if value > 0 => value,
                _ => return bad_request("Missing fleet listing fields"),
            };

            MarketplaceListingInput {
                user_id,
                planet_id,
                listing_type: listing_type.clone(),
                resource_type: None,
                quantity: None,
                price_per_unit: Some(price_per_unit),
                total_price: Some(total_price),
                fleet_type: Some(fleet_type),
                fleet_quantity: Some(fleet_quantity),
                wanted_type,
                wanted_amount,
            }
        }
        _ => return bad_request("Invalid listing_type"),
    };

    let listing = app_state.create_marketplace_listing(listing_input);
    success(ListingResponse { listing })
}

async fn accept_listing_handler(
    BearerToken(token): BearerToken,
    Extension(app_state): Extension<AppState>,
    Path(listing_id): Path<i64>,
    payload: Result<Json<MarketplaceAcceptRequest>, JsonRejection>,
) -> Response {
    let Json(input) = match payload {
        Ok(value) => value,
        Err(_) => return bad_request("Invalid marketplace accept payload"),
    };

    let buyer_planet_id = match input.buyer_planet_id.and_then(|value| value.into_i64()) {
        Some(value) if value > 0 => value,
        _ => return bad_request("buyer_planet_id is required"),
    };

    let user_id = user_id_from_token(&token);
    match app_state.accept_marketplace_listing(user_id, listing_id, buyer_planet_id) {
        Ok(result) => success(MarketplaceAcceptResponse {
            success: true,
            delivery_eta: result.delivery_eta,
            transaction: MarketplaceTransactionResponse {
                listing_id: result.transaction.listing_id,
                buyer_id: result.transaction.buyer_id,
                buyer_planet_id: result.transaction.buyer_planet_id,
                seller_id: result.transaction.seller_id,
                seller_planet_id: result.transaction.seller_planet_id,
            },
        }),
        Err(message) if message == "Listing not found" => not_found(message),
        Err(message) => bad_request(message),
    }
}

async fn delete_listing_handler(
    BearerToken(token): BearerToken,
    Extension(app_state): Extension<AppState>,
    Path(listing_id): Path<i64>,
) -> Response {
    let user_id = user_id_from_token(&token);
    match app_state.cancel_marketplace_listing(user_id, listing_id) {
        Ok(()) => success(MarketplaceDeleteResponse { success: true }),
        Err(message) if message == "Listing not found" => not_found(message),
        Err(message) => bad_request(message),
    }
}

async fn my_listings_handler(
    BearerToken(token): BearerToken,
    Extension(app_state): Extension<AppState>,
) -> Response {
    let user_id = user_id_from_token(&token);
    let listings = app_state.list_marketplace_user_listings(user_id);
    success(ListingsCollectionResponse { listings })
}

async fn my_history_handler(
    BearerToken(token): BearerToken,
    Extension(app_state): Extension<AppState>,
) -> Response {
    let user_id = user_id_from_token(&token);
    let transactions = app_state.list_marketplace_user_history(user_id);
    success(HistoryResponse { transactions })
}

async fn get_listing_handler(
    Extension(app_state): Extension<AppState>,
    Path(listing_id): Path<i64>,
) -> Response {
    match app_state.get_marketplace_listing(listing_id) {
        Some(listing) => success(ListingResponse { listing }),
        None => not_found("Listing not found"),
    }
}

fn user_id_from_token(token: &str) -> i64 {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    token.hash(&mut hasher);
    let value = hasher.finish();
    (value % i64::MAX as u64) as i64 + 1
}
