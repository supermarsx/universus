#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// The kind of item being offered on the marketplace.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ListingType {
    Resource,
    Fleet,
    Technology,
}

impl fmt::Display for ListingType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource => write!(f, "resource"),
            Self::Fleet => write!(f, "fleet"),
            Self::Technology => write!(f, "technology"),
        }
    }
}

impl FromStr for ListingType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "resource" => Ok(Self::Resource),
            "fleet" => Ok(Self::Fleet),
            "technology" => Ok(Self::Technology),
            other => Err(format!("unknown listing type: {other}")),
        }
    }
}

/// Current status of a marketplace listing.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ListingStatus {
    Active,
    Completed,
    Cancelled,
    Expired,
}

impl fmt::Display for ListingStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Completed => write!(f, "completed"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::Expired => write!(f, "expired"),
        }
    }
}

impl FromStr for ListingStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "active" => Ok(Self::Active),
            "completed" => Ok(Self::Completed),
            "cancelled" => Ok(Self::Cancelled),
            "expired" => Ok(Self::Expired),
            other => Err(format!("unknown listing status: {other}")),
        }
    }
}

/// In-game resource categories.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceType {
    Metal,
    Crystal,
    Deuterium,
}

impl fmt::Display for ResourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Metal => write!(f, "metal"),
            Self::Crystal => write!(f, "crystal"),
            Self::Deuterium => write!(f, "deuterium"),
        }
    }
}

impl FromStr for ResourceType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "metal" => Ok(Self::Metal),
            "crystal" => Ok(Self::Crystal),
            "deuterium" => Ok(Self::Deuterium),
            other => Err(format!("unknown resource type: {other}")),
        }
    }
}

// ---------------------------------------------------------------------------
// Domain structs
// ---------------------------------------------------------------------------

/// A single marketplace listing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketplaceListing {
    pub id: i64,
    pub seller_id: i64,
    pub seller_planet_id: i64,
    pub listing_type: ListingType,
    pub offer_resource_type: Option<String>,
    pub offer_quantity: Option<i64>,
    pub offer_fleet_type: Option<String>,
    pub offer_fleet_quantity: Option<i64>,
    pub price_per_unit: Option<i64>,
    pub total_price: Option<i64>,
    pub wanted_type: String,
    pub wanted_amount: i64,
    pub status: ListingStatus,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub completed_at: Option<String>,
    pub cancelled_at: Option<String>,
    pub buyer_id: Option<i64>,
    pub buyer_planet_id: Option<i64>,
    pub delivery_eta: Option<String>,
    pub tax_rate: f64,
    pub tax_paid: i64,
}

/// Input required to create a new listing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateListingInput {
    pub seller_id: i64,
    pub seller_planet_id: i64,
    pub listing_type: ListingType,
    pub offer_resource_type: Option<String>,
    pub offer_quantity: Option<i64>,
    pub offer_fleet_type: Option<String>,
    pub offer_fleet_quantity: Option<i64>,
    pub price_per_unit: Option<i64>,
    pub total_price: Option<i64>,
    pub wanted_type: String,
    pub wanted_amount: i64,
}

/// Filters for querying listings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListingFilters {
    pub listing_type: Option<ListingType>,
    pub resource_type: Option<String>,
    pub fleet_type: Option<String>,
    pub wanted_type: Option<String>,
    pub min_amount: Option<i64>,
    pub max_amount: Option<i64>,
    pub seller_id: Option<i64>,
    pub page: i64,
    pub page_size: i64,
}

impl Default for ListingFilters {
    fn default() -> Self {
        Self {
            listing_type: None,
            resource_type: None,
            fleet_type: None,
            wanted_type: None,
            min_amount: None,
            max_amount: None,
            seller_id: None,
            page: 1,
            page_size: 20,
        }
    }
}

/// Record of a completed marketplace transaction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transaction {
    pub id: i64,
    pub listing_id: i64,
    pub buyer_id: i64,
    pub buyer_planet_id: i64,
    pub seller_id: i64,
    pub seller_planet_id: i64,
    pub resources_exchanged: i64,
    pub tax_paid: i64,
    pub completed_at: String,
}

/// Aggregate marketplace statistics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketplaceStats {
    pub total_listings: i64,
    pub active_listings: i64,
    pub completed_today: i64,
    pub total_volume: i64,
    pub average_price: f64,
}

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

/// Errors that marketplace operations can produce.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MarketplaceError {
    NotFound,
    NotActive,
    OwnListing,
    NotOwner,
    InvalidInput(String),
    Expired,
}

impl fmt::Display for MarketplaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "listing not found"),
            Self::NotActive => write!(f, "listing is not active"),
            Self::OwnListing => write!(f, "cannot accept your own listing"),
            Self::NotOwner => write!(f, "you do not own this listing"),
            Self::InvalidInput(msg) => write!(f, "invalid input: {msg}"),
            Self::Expired => write!(f, "listing has expired"),
        }
    }
}

// ---------------------------------------------------------------------------
// Price helpers
// ---------------------------------------------------------------------------

/// Standard exchange value ratios: metal=2, crystal=1.5, deuterium=1.
/// Returns a suggested total price denominated in the cheapest unit (deuterium-equivalent).
pub fn suggested_price(resource_type: &str, quantity: i64) -> i64 {
    let multiplier = match resource_type.to_lowercase().as_str() {
        "metal" => 2.0_f64,
        "crystal" => 1.5,
        "deuterium" => 1.0,
        _ => 1.0,
    };
    (quantity as f64 * multiplier).round() as i64
}

/// Calculate tax on the given amount at the given rate (e.g. 0.10 = 10%).
pub fn calculate_tax(amount: i64, rate: f64) -> i64 {
    (amount as f64 * rate).round() as i64
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const DEFAULT_TAX_RATE: f64 = 0.10;
const EXPIRY_SECONDS: i64 = 48 * 3600; // 48 hours

// ---------------------------------------------------------------------------
// MarketplaceStore
// ---------------------------------------------------------------------------

/// In-memory, `HashMap`-backed marketplace store.
#[derive(Debug, Clone)]
pub struct MarketplaceStore {
    listings: HashMap<i64, MarketplaceListing>,
    transactions: Vec<Transaction>,
    next_listing_id: i64,
    next_transaction_id: i64,
    pub tax_rate: f64,
}

impl MarketplaceStore {
    /// Create a new store pre-populated with a handful of seed listings.
    pub fn new() -> Self {
        let mut store = Self {
            listings: HashMap::new(),
            transactions: Vec::new(),
            next_listing_id: 1,
            next_transaction_id: 1,
            tax_rate: DEFAULT_TAX_RATE,
        };

        // Seed listings -------------------------------------------------------
        let seed_now = "2026-02-13T20:00:00Z";
        let seed_expiry = "2026-02-15T20:00:00Z";

        let seed_data: Vec<(
            i64,
            i64,
            ListingType,
            Option<&str>,
            Option<i64>,
            Option<&str>,
            Option<i64>,
            Option<i64>,
            Option<i64>,
            &str,
            i64,
        )> = vec![
            (
                501,
                21,
                ListingType::Resource,
                Some("metal"),
                Some(40_000),
                None,
                None,
                Some(3),
                Some(120_000),
                "crystal",
                75_000,
            ),
            (
                502,
                22,
                ListingType::Fleet,
                None,
                None,
                Some("cruiser"),
                Some(10),
                Some(8_500),
                Some(85_000),
                "metal",
                85_000,
            ),
            (
                503,
                25,
                ListingType::Resource,
                Some("deuterium"),
                Some(12_000),
                None,
                None,
                Some(8),
                Some(96_000),
                "metal",
                96_000,
            ),
        ];

        for (
            seller_id,
            planet_id,
            lt,
            res_type,
            qty,
            fleet_type,
            fleet_qty,
            ppu,
            tp,
            wanted,
            wanted_amt,
        ) in seed_data
        {
            let id = store.next_listing_id;
            store.next_listing_id += 1;
            let tax = calculate_tax(wanted_amt, store.tax_rate);
            store.listings.insert(
                id,
                MarketplaceListing {
                    id,
                    seller_id,
                    seller_planet_id: planet_id,
                    listing_type: lt,
                    offer_resource_type: res_type.map(String::from),
                    offer_quantity: qty,
                    offer_fleet_type: fleet_type.map(String::from),
                    offer_fleet_quantity: fleet_qty,
                    price_per_unit: ppu,
                    total_price: tp,
                    wanted_type: wanted.to_string(),
                    wanted_amount: wanted_amt,
                    status: ListingStatus::Active,
                    created_at: seed_now.to_string(),
                    expires_at: Some(seed_expiry.to_string()),
                    completed_at: None,
                    cancelled_at: None,
                    buyer_id: None,
                    buyer_planet_id: None,
                    delivery_eta: None,
                    tax_rate: store.tax_rate,
                    tax_paid: tax,
                },
            );
        }

        store
    }

    // -- Create ---------------------------------------------------------------

    /// Create a new marketplace listing, returning it on success.
    pub fn create_listing(
        &mut self,
        input: CreateListingInput,
        now: &str,
    ) -> Result<MarketplaceListing, MarketplaceError> {
        // Validation
        if input.seller_id <= 0 {
            return Err(MarketplaceError::InvalidInput(
                "seller_id must be positive".into(),
            ));
        }
        if input.wanted_amount <= 0 {
            return Err(MarketplaceError::InvalidInput(
                "wanted_amount must be positive".into(),
            ));
        }
        let effective_quantity = match input.listing_type {
            ListingType::Resource => input.offer_quantity.unwrap_or(0),
            ListingType::Fleet => input.offer_fleet_quantity.unwrap_or(0),
            ListingType::Technology => 1, // technology listings always offer 1
        };
        if effective_quantity <= 0 {
            return Err(MarketplaceError::InvalidInput(
                "offer quantity must be positive".into(),
            ));
        }

        let id = self.next_listing_id;
        self.next_listing_id += 1;

        let tax = calculate_tax(input.wanted_amount, self.tax_rate);
        let expires_at = add_seconds_iso(now, EXPIRY_SECONDS);

        let listing = MarketplaceListing {
            id,
            seller_id: input.seller_id,
            seller_planet_id: input.seller_planet_id,
            listing_type: input.listing_type,
            offer_resource_type: input.offer_resource_type,
            offer_quantity: input.offer_quantity,
            offer_fleet_type: input.offer_fleet_type,
            offer_fleet_quantity: input.offer_fleet_quantity,
            price_per_unit: input.price_per_unit,
            total_price: input.total_price,
            wanted_type: input.wanted_type,
            wanted_amount: input.wanted_amount,
            status: ListingStatus::Active,
            created_at: now.to_string(),
            expires_at: Some(expires_at),
            completed_at: None,
            cancelled_at: None,
            buyer_id: None,
            buyer_planet_id: None,
            delivery_eta: None,
            tax_rate: self.tax_rate,
            tax_paid: tax,
        };

        self.listings.insert(id, listing.clone());
        Ok(listing)
    }

    // -- Read -----------------------------------------------------------------

    /// List active listings matching the given filters with pagination.
    /// Returns `(page_items, total_matching)`.
    pub fn list_listings(&self, filters: &ListingFilters) -> (Vec<MarketplaceListing>, i64) {
        let mut matched: Vec<&MarketplaceListing> = self
            .listings
            .values()
            .filter(|l| l.status == ListingStatus::Active)
            .filter(|l| {
                filters
                    .listing_type
                    .as_ref()
                    .map_or(true, |ft| l.listing_type == *ft)
            })
            .filter(|l| {
                filters.resource_type.as_ref().map_or(true, |rt| {
                    l.offer_resource_type.as_deref() == Some(rt.as_str())
                })
            })
            .filter(|l| {
                filters.fleet_type.as_ref().map_or(true, |ft| {
                    l.offer_fleet_type.as_deref() == Some(ft.as_str())
                })
            })
            .filter(|l| {
                filters
                    .wanted_type
                    .as_ref()
                    .map_or(true, |wt| l.wanted_type == *wt)
            })
            .filter(|l| {
                filters
                    .min_amount
                    .map_or(true, |min| l.wanted_amount >= min)
            })
            .filter(|l| {
                filters
                    .max_amount
                    .map_or(true, |max| l.wanted_amount <= max)
            })
            .filter(|l| filters.seller_id.map_or(true, |sid| l.seller_id == sid))
            .collect();

        // Sort newest first
        matched.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        let total = matched.len() as i64;
        let page = filters.page.max(1) as usize;
        let page_size = filters.page_size.max(1) as usize;
        let start = (page - 1).saturating_mul(page_size);
        let end = start.saturating_add(page_size).min(matched.len());

        let items = if start >= matched.len() {
            Vec::new()
        } else {
            matched[start..end].iter().map(|l| (*l).clone()).collect()
        };

        (items, total)
    }

    /// Get a single listing by ID.
    pub fn get_listing(&self, id: i64) -> Option<&MarketplaceListing> {
        self.listings.get(&id)
    }

    // -- Accept ---------------------------------------------------------------

    /// Accept an active listing. Creates a `Transaction` and marks the listing
    /// as completed.
    pub fn accept_listing(
        &mut self,
        listing_id: i64,
        buyer_id: i64,
        buyer_planet_id: i64,
        now: &str,
    ) -> Result<Transaction, MarketplaceError> {
        // Borrow-check friendly: read fields first, then mutate.
        let listing = self
            .listings
            .get(&listing_id)
            .ok_or(MarketplaceError::NotFound)?;

        if listing.status != ListingStatus::Active {
            return Err(MarketplaceError::NotActive);
        }
        if listing.seller_id == buyer_id {
            return Err(MarketplaceError::OwnListing);
        }
        // Check expiry
        if let Some(ref exp) = listing.expires_at {
            if now >= exp.as_str() {
                // Mark expired then return error — need mutable access.
                let listing_mut = self.listings.get_mut(&listing_id).unwrap();
                listing_mut.status = ListingStatus::Expired;
                return Err(MarketplaceError::Expired);
            }
        }

        let seller_id = listing.seller_id;
        let seller_planet_id = listing.seller_planet_id;
        let resources_exchanged = listing.wanted_amount;
        let tax = listing.tax_paid;

        // Mutate listing
        let listing_mut = self.listings.get_mut(&listing_id).unwrap();
        listing_mut.status = ListingStatus::Completed;
        listing_mut.completed_at = Some(now.to_string());
        listing_mut.buyer_id = Some(buyer_id);
        listing_mut.buyer_planet_id = Some(buyer_planet_id);

        // Create transaction
        let tx_id = self.next_transaction_id;
        self.next_transaction_id += 1;

        let transaction = Transaction {
            id: tx_id,
            listing_id,
            buyer_id,
            buyer_planet_id,
            seller_id,
            seller_planet_id,
            resources_exchanged,
            tax_paid: tax,
            completed_at: now.to_string(),
        };
        self.transactions.push(transaction.clone());

        Ok(transaction)
    }

    // -- Cancel ---------------------------------------------------------------

    /// Cancel an active listing. Only the owner can cancel.
    pub fn cancel_listing(
        &mut self,
        listing_id: i64,
        user_id: i64,
        now: &str,
    ) -> Result<MarketplaceListing, MarketplaceError> {
        let listing = self
            .listings
            .get(&listing_id)
            .ok_or(MarketplaceError::NotFound)?;

        if listing.seller_id != user_id {
            return Err(MarketplaceError::NotOwner);
        }
        if listing.status != ListingStatus::Active {
            return Err(MarketplaceError::NotActive);
        }

        let listing_mut = self.listings.get_mut(&listing_id).unwrap();
        listing_mut.status = ListingStatus::Cancelled;
        listing_mut.cancelled_at = Some(now.to_string());

        Ok(listing_mut.clone())
    }

    // -- User queries ---------------------------------------------------------

    /// All listings belonging to a user, newest first.
    pub fn user_listings(&self, user_id: i64) -> Vec<MarketplaceListing> {
        let mut items: Vec<MarketplaceListing> = self
            .listings
            .values()
            .filter(|l| l.seller_id == user_id)
            .cloned()
            .collect();
        items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        items
    }

    /// Transaction history for a user (as buyer or seller), most recent first,
    /// limited to `limit` entries.
    pub fn user_history(&self, user_id: i64, limit: usize) -> Vec<Transaction> {
        let mut txs: Vec<Transaction> = self
            .transactions
            .iter()
            .filter(|tx| tx.buyer_id == user_id || tx.seller_id == user_id)
            .cloned()
            .collect();
        txs.sort_by(|a, b| b.completed_at.cmp(&a.completed_at));
        txs.truncate(limit);
        txs
    }

    // -- Maintenance ----------------------------------------------------------

    /// Expire all active listings whose `expires_at` <= `now`.
    /// Returns the number of listings expired.
    pub fn expire_stale_listings(&mut self, now: &str) -> usize {
        let mut count = 0usize;
        for listing in self.listings.values_mut() {
            if listing.status != ListingStatus::Active {
                continue;
            }
            if let Some(ref exp) = listing.expires_at {
                if now >= exp.as_str() {
                    listing.status = ListingStatus::Expired;
                    count += 1;
                }
            }
        }
        count
    }

    // -- Stats ----------------------------------------------------------------

    /// Aggregate statistics across the entire marketplace.
    pub fn stats(&self) -> MarketplaceStats {
        let total_listings = self.listings.len() as i64;
        let active_listings = self
            .listings
            .values()
            .filter(|l| l.status == ListingStatus::Active)
            .count() as i64;

        // "completed today" — count transactions whose completed_at shares the
        // same date prefix as the most recent transaction (simple heuristic
        // without a real clock).
        let today_prefix = self
            .transactions
            .last()
            .map(|tx| &tx.completed_at[..10])
            .unwrap_or("");
        let completed_today = self
            .transactions
            .iter()
            .filter(|tx| tx.completed_at.starts_with(today_prefix) && !today_prefix.is_empty())
            .count() as i64;

        let total_volume: i64 = self
            .transactions
            .iter()
            .map(|tx| tx.resources_exchanged)
            .sum();

        let average_price = if self.transactions.is_empty() {
            0.0
        } else {
            total_volume as f64 / self.transactions.len() as f64
        };

        MarketplaceStats {
            total_listings,
            active_listings,
            completed_today,
            total_volume,
            average_price,
        }
    }

    // -- Search ---------------------------------------------------------------

    /// Simple text search across offer resource type, fleet type, and wanted
    /// type. Returns active listings that match, newest first.
    pub fn search_listings(&self, query: &str) -> Vec<MarketplaceListing> {
        let q = query.to_lowercase();
        let mut results: Vec<MarketplaceListing> = self
            .listings
            .values()
            .filter(|l| l.status == ListingStatus::Active)
            .filter(|l| {
                let in_resource = l
                    .offer_resource_type
                    .as_ref()
                    .map_or(false, |rt| rt.to_lowercase().contains(&q));
                let in_fleet = l
                    .offer_fleet_type
                    .as_ref()
                    .map_or(false, |ft| ft.to_lowercase().contains(&q));
                let in_wanted = l.wanted_type.to_lowercase().contains(&q);
                let in_listing_type = l.listing_type.to_string().contains(&q);
                in_resource || in_fleet || in_wanted || in_listing_type
            })
            .cloned()
            .collect();
        results.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        results
    }
}

impl Default for MarketplaceStore {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Naive ISO 8601 timestamp addition. Parses a `YYYY-MM-DDTHH:MM:SSZ` string,
/// adds `seconds`, and returns the result in the same format. Falls back to
/// appending "+48h" if parsing fails.
fn add_seconds_iso(iso: &str, seconds: i64) -> String {
    // Attempt minimal parse: "2026-02-13T20:00:00Z"
    if iso.len() < 19 {
        return format!("{iso}+{seconds}s");
    }

    let parse = || -> Option<String> {
        let year: i64 = iso[0..4].parse().ok()?;
        let month: i64 = iso[5..7].parse().ok()?;
        let day: i64 = iso[8..10].parse().ok()?;
        let hour: i64 = iso[11..13].parse().ok()?;
        let min: i64 = iso[14..16].parse().ok()?;
        let sec: i64 = iso[17..19].parse().ok()?;

        let total_secs = sec + seconds;
        let extra_min = total_secs.div_euclid(60);
        let new_sec = total_secs.rem_euclid(60);

        let total_min = min + extra_min;
        let extra_hr = total_min.div_euclid(60);
        let new_min = total_min.rem_euclid(60);

        let total_hr = hour + extra_hr;
        let extra_day = total_hr.div_euclid(24);
        let new_hr = total_hr.rem_euclid(24);

        let new_day = day + extra_day; // simplified: ignore month overflow for 48h
        Some(format!(
            "{year:04}-{month:02}-{new_day:02}T{new_hr:02}:{new_min:02}:{new_sec:02}Z"
        ))
    };

    parse().unwrap_or_else(|| format!("{iso}+{seconds}s"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> &'static str {
        "2026-02-14T10:00:00Z"
    }

    fn future() -> &'static str {
        "2026-02-20T10:00:00Z"
    }

    fn make_resource_input(seller_id: i64) -> CreateListingInput {
        CreateListingInput {
            seller_id,
            seller_planet_id: 10,
            listing_type: ListingType::Resource,
            offer_resource_type: Some("metal".into()),
            offer_quantity: Some(5000),
            offer_fleet_type: None,
            offer_fleet_quantity: None,
            price_per_unit: Some(3),
            total_price: Some(15_000),
            wanted_type: "crystal".into(),
            wanted_amount: 10_000,
        }
    }

    fn make_fleet_input(seller_id: i64) -> CreateListingInput {
        CreateListingInput {
            seller_id,
            seller_planet_id: 11,
            listing_type: ListingType::Fleet,
            offer_resource_type: None,
            offer_quantity: None,
            offer_fleet_type: Some("cruiser".into()),
            offer_fleet_quantity: Some(5),
            price_per_unit: Some(10_000),
            total_price: Some(50_000),
            wanted_type: "deuterium".into(),
            wanted_amount: 30_000,
        }
    }

    // -- Enum Display / FromStr -----------------------------------------------

    #[test]
    fn listing_type_display_and_parse() {
        assert_eq!(ListingType::Resource.to_string(), "resource");
        assert_eq!(ListingType::Fleet.to_string(), "fleet");
        assert_eq!(ListingType::Technology.to_string(), "technology");
        assert_eq!(
            "resource".parse::<ListingType>().unwrap(),
            ListingType::Resource
        );
        assert_eq!("Fleet".parse::<ListingType>().unwrap(), ListingType::Fleet);
        assert!("unknown".parse::<ListingType>().is_err());
    }

    #[test]
    fn listing_status_display_and_parse() {
        assert_eq!(ListingStatus::Active.to_string(), "active");
        assert_eq!(ListingStatus::Expired.to_string(), "expired");
        assert_eq!(
            "completed".parse::<ListingStatus>().unwrap(),
            ListingStatus::Completed
        );
        assert_eq!(
            "Cancelled".parse::<ListingStatus>().unwrap(),
            ListingStatus::Cancelled
        );
        assert!("bogus".parse::<ListingStatus>().is_err());
    }

    #[test]
    fn resource_type_display_and_parse() {
        assert_eq!(ResourceType::Metal.to_string(), "metal");
        assert_eq!(ResourceType::Crystal.to_string(), "crystal");
        assert_eq!(ResourceType::Deuterium.to_string(), "deuterium");
        assert_eq!(
            "Metal".parse::<ResourceType>().unwrap(),
            ResourceType::Metal
        );
        assert!("plasma".parse::<ResourceType>().is_err());
    }

    // -- Price helpers --------------------------------------------------------

    #[test]
    fn suggested_price_metal() {
        assert_eq!(suggested_price("metal", 1000), 2000);
    }

    #[test]
    fn suggested_price_crystal() {
        assert_eq!(suggested_price("crystal", 1000), 1500);
    }

    #[test]
    fn suggested_price_deuterium() {
        assert_eq!(suggested_price("deuterium", 1000), 1000);
    }

    #[test]
    fn suggested_price_unknown_defaults_to_one() {
        assert_eq!(suggested_price("dark_matter", 1000), 1000);
    }

    #[test]
    fn calculate_tax_basic() {
        assert_eq!(calculate_tax(10_000, 0.10), 1_000);
        assert_eq!(calculate_tax(10_000, 0.05), 500);
        assert_eq!(calculate_tax(0, 0.10), 0);
    }

    // -- Store seed data ------------------------------------------------------

    #[test]
    fn new_store_has_seed_listings() {
        let store = MarketplaceStore::new();
        assert_eq!(store.listings.len(), 3);
        assert!(store.get_listing(1).is_some());
        assert!(store.get_listing(2).is_some());
        assert!(store.get_listing(3).is_some());
    }

    #[test]
    fn seed_listings_are_active() {
        let store = MarketplaceStore::new();
        for listing in store.listings.values() {
            assert_eq!(listing.status, ListingStatus::Active);
        }
    }

    // -- Create ---------------------------------------------------------------

    #[test]
    fn create_listing_success() {
        let mut store = MarketplaceStore::new();
        let listing = store
            .create_listing(make_resource_input(100), now())
            .unwrap();
        assert_eq!(listing.seller_id, 100);
        assert_eq!(listing.status, ListingStatus::Active);
        assert_eq!(listing.wanted_amount, 10_000);
        assert_eq!(listing.tax_paid, 1_000); // 10% of 10_000
        assert!(listing.expires_at.is_some());
    }

    #[test]
    fn create_listing_assigns_incremental_ids() {
        let mut store = MarketplaceStore::new();
        let l1 = store.create_listing(make_resource_input(1), now()).unwrap();
        let l2 = store.create_listing(make_resource_input(2), now()).unwrap();
        assert_eq!(l2.id, l1.id + 1);
    }

    #[test]
    fn create_listing_invalid_seller_id() {
        let mut store = MarketplaceStore::new();
        let mut input = make_resource_input(0);
        input.seller_id = 0;
        assert_eq!(
            store.create_listing(input, now()),
            Err(MarketplaceError::InvalidInput(
                "seller_id must be positive".into()
            ))
        );
    }

    #[test]
    fn create_listing_invalid_wanted_amount() {
        let mut store = MarketplaceStore::new();
        let mut input = make_resource_input(1);
        input.wanted_amount = 0;
        assert_eq!(
            store.create_listing(input, now()),
            Err(MarketplaceError::InvalidInput(
                "wanted_amount must be positive".into()
            ))
        );
    }

    #[test]
    fn create_listing_invalid_quantity_zero() {
        let mut store = MarketplaceStore::new();
        let mut input = make_resource_input(1);
        input.offer_quantity = Some(0);
        assert_eq!(
            store.create_listing(input, now()),
            Err(MarketplaceError::InvalidInput(
                "offer quantity must be positive".into()
            ))
        );
    }

    #[test]
    fn create_fleet_listing() {
        let mut store = MarketplaceStore::new();
        let listing = store.create_listing(make_fleet_input(200), now()).unwrap();
        assert_eq!(listing.listing_type, ListingType::Fleet);
        assert_eq!(listing.offer_fleet_type, Some("cruiser".into()));
        assert_eq!(listing.offer_fleet_quantity, Some(5));
    }

    // -- List / Filter --------------------------------------------------------

    #[test]
    fn list_listings_default_filters_returns_all_active() {
        let store = MarketplaceStore::new();
        let (items, total) = store.list_listings(&ListingFilters::default());
        assert_eq!(total, 3);
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn list_listings_filter_by_listing_type() {
        let store = MarketplaceStore::new();
        let filters = ListingFilters {
            listing_type: Some(ListingType::Resource),
            ..Default::default()
        };
        let (items, total) = store.list_listings(&filters);
        assert_eq!(total, 2);
        for item in &items {
            assert_eq!(item.listing_type, ListingType::Resource);
        }
    }

    #[test]
    fn list_listings_filter_by_resource_type() {
        let store = MarketplaceStore::new();
        let filters = ListingFilters {
            resource_type: Some("metal".into()),
            ..Default::default()
        };
        let (items, _) = store.list_listings(&filters);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].offer_resource_type.as_deref(), Some("metal"));
    }

    #[test]
    fn list_listings_filter_by_wanted_type() {
        let store = MarketplaceStore::new();
        let filters = ListingFilters {
            wanted_type: Some("metal".into()),
            ..Default::default()
        };
        let (items, total) = store.list_listings(&filters);
        assert_eq!(total, 2); // fleet listing + deuterium listing both want metal
        for item in &items {
            assert_eq!(item.wanted_type, "metal");
        }
    }

    #[test]
    fn list_listings_pagination() {
        let mut store = MarketplaceStore::new();
        for i in 0..10 {
            let mut input = make_resource_input(100 + i);
            input.wanted_amount = 1000 + i as i64;
            store.create_listing(input, now()).unwrap();
        }
        let filters = ListingFilters {
            page: 2,
            page_size: 5,
            ..Default::default()
        };
        let (items, total) = store.list_listings(&filters);
        assert_eq!(total, 13); // 3 seed + 10 new
        assert_eq!(items.len(), 5);
    }

    #[test]
    fn list_listings_min_max_amount() {
        let store = MarketplaceStore::new();
        let filters = ListingFilters {
            min_amount: Some(80_000),
            max_amount: Some(90_000),
            ..Default::default()
        };
        let (items, _) = store.list_listings(&filters);
        for item in &items {
            assert!(item.wanted_amount >= 80_000);
            assert!(item.wanted_amount <= 90_000);
        }
    }

    #[test]
    fn list_listings_filter_by_seller_id() {
        let store = MarketplaceStore::new();
        let filters = ListingFilters {
            seller_id: Some(501),
            ..Default::default()
        };
        let (items, total) = store.list_listings(&filters);
        assert_eq!(total, 1);
        assert_eq!(items[0].seller_id, 501);
    }

    // -- Get ------------------------------------------------------------------

    #[test]
    fn get_listing_existing() {
        let store = MarketplaceStore::new();
        let listing = store.get_listing(1).unwrap();
        assert_eq!(listing.id, 1);
    }

    #[test]
    fn get_listing_nonexistent() {
        let store = MarketplaceStore::new();
        assert!(store.get_listing(999).is_none());
    }

    // -- Accept ---------------------------------------------------------------

    #[test]
    fn accept_listing_success() {
        let mut store = MarketplaceStore::new();
        let tx = store.accept_listing(1, 600, 30, now()).unwrap();
        assert_eq!(tx.listing_id, 1);
        assert_eq!(tx.buyer_id, 600);
        assert_eq!(tx.seller_id, 501);
        assert_eq!(tx.completed_at, now());

        let listing = store.get_listing(1).unwrap();
        assert_eq!(listing.status, ListingStatus::Completed);
        assert_eq!(listing.buyer_id, Some(600));
    }

    #[test]
    fn accept_listing_not_found() {
        let mut store = MarketplaceStore::new();
        assert_eq!(
            store.accept_listing(999, 600, 30, now()),
            Err(MarketplaceError::NotFound)
        );
    }

    #[test]
    fn accept_listing_own_listing() {
        let mut store = MarketplaceStore::new();
        assert_eq!(
            store.accept_listing(1, 501, 30, now()),
            Err(MarketplaceError::OwnListing)
        );
    }

    #[test]
    fn accept_listing_already_completed() {
        let mut store = MarketplaceStore::new();
        store.accept_listing(1, 600, 30, now()).unwrap();
        assert_eq!(
            store.accept_listing(1, 700, 31, now()),
            Err(MarketplaceError::NotActive)
        );
    }

    #[test]
    fn accept_listing_expired() {
        let mut store = MarketplaceStore::new();
        // The seed listings expire at 2026-02-15T20:00:00Z; use a time after that.
        let result = store.accept_listing(1, 600, 30, future());
        assert_eq!(result, Err(MarketplaceError::Expired));
        // Listing should now be marked expired
        assert_eq!(store.get_listing(1).unwrap().status, ListingStatus::Expired);
    }

    // -- Cancel ---------------------------------------------------------------

    #[test]
    fn cancel_listing_success() {
        let mut store = MarketplaceStore::new();
        let cancelled = store.cancel_listing(1, 501, now()).unwrap();
        assert_eq!(cancelled.status, ListingStatus::Cancelled);
        assert_eq!(cancelled.cancelled_at, Some(now().to_string()));
    }

    #[test]
    fn cancel_listing_not_owner() {
        let mut store = MarketplaceStore::new();
        assert_eq!(
            store.cancel_listing(1, 999, now()),
            Err(MarketplaceError::NotOwner)
        );
    }

    #[test]
    fn cancel_listing_not_active() {
        let mut store = MarketplaceStore::new();
        store.cancel_listing(1, 501, now()).unwrap();
        assert_eq!(
            store.cancel_listing(1, 501, now()),
            Err(MarketplaceError::NotActive)
        );
    }

    #[test]
    fn cancel_listing_not_found() {
        let mut store = MarketplaceStore::new();
        assert_eq!(
            store.cancel_listing(999, 501, now()),
            Err(MarketplaceError::NotFound)
        );
    }

    // -- User queries ---------------------------------------------------------

    #[test]
    fn user_listings_returns_all_for_user() {
        let mut store = MarketplaceStore::new();
        store
            .create_listing(make_resource_input(501), now())
            .unwrap();
        let listings = store.user_listings(501);
        assert_eq!(listings.len(), 2); // 1 seed + 1 new
        for l in &listings {
            assert_eq!(l.seller_id, 501);
        }
    }

    #[test]
    fn user_listings_empty_for_unknown_user() {
        let store = MarketplaceStore::new();
        assert!(store.user_listings(999).is_empty());
    }

    #[test]
    fn user_history_returns_transactions() {
        let mut store = MarketplaceStore::new();
        store.accept_listing(1, 600, 30, now()).unwrap();
        // seller history
        let seller_hist = store.user_history(501, 10);
        assert_eq!(seller_hist.len(), 1);
        // buyer history
        let buyer_hist = store.user_history(600, 10);
        assert_eq!(buyer_hist.len(), 1);
    }

    #[test]
    fn user_history_respects_limit() {
        let mut store = MarketplaceStore::new();
        store.accept_listing(1, 600, 30, now()).unwrap();
        store.accept_listing(2, 600, 30, now()).unwrap();
        let hist = store.user_history(600, 1);
        assert_eq!(hist.len(), 1);
    }

    // -- Expire stale ---------------------------------------------------------

    #[test]
    fn expire_stale_listings_expires_old() {
        let mut store = MarketplaceStore::new();
        let expired_count = store.expire_stale_listings(future());
        assert_eq!(expired_count, 3); // all seed listings
        for listing in store.listings.values() {
            assert_eq!(listing.status, ListingStatus::Expired);
        }
    }

    #[test]
    fn expire_stale_listings_none_expired() {
        let mut store = MarketplaceStore::new();
        let expired_count = store.expire_stale_listings("2026-02-14T00:00:00Z");
        assert_eq!(expired_count, 0);
    }

    // -- Stats ----------------------------------------------------------------

    #[test]
    fn stats_basic() {
        let mut store = MarketplaceStore::new();
        store.accept_listing(1, 600, 30, now()).unwrap();
        let stats = store.stats();
        assert_eq!(stats.total_listings, 3);
        assert_eq!(stats.active_listings, 2);
        assert_eq!(stats.completed_today, 1);
        assert!(stats.total_volume > 0);
        assert!(stats.average_price > 0.0);
    }

    #[test]
    fn stats_empty_store() {
        let store = MarketplaceStore {
            listings: HashMap::new(),
            transactions: Vec::new(),
            next_listing_id: 1,
            next_transaction_id: 1,
            tax_rate: DEFAULT_TAX_RATE,
        };
        let stats = store.stats();
        assert_eq!(stats.total_listings, 0);
        assert_eq!(stats.active_listings, 0);
        assert_eq!(stats.average_price, 0.0);
    }

    // -- Search ---------------------------------------------------------------

    #[test]
    fn search_listings_by_resource() {
        let store = MarketplaceStore::new();
        let results = store.search_listings("metal");
        assert!(!results.is_empty());
        // All results should mention metal somewhere
        for r in &results {
            let has_metal =
                r.offer_resource_type.as_deref() == Some("metal") || r.wanted_type == "metal";
            assert!(has_metal);
        }
    }

    #[test]
    fn search_listings_by_fleet() {
        let store = MarketplaceStore::new();
        let results = store.search_listings("cruiser");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].offer_fleet_type.as_deref(), Some("cruiser"));
    }

    #[test]
    fn search_listings_no_match() {
        let store = MarketplaceStore::new();
        let results = store.search_listings("nonexistent_item_xyz");
        assert!(results.is_empty());
    }

    #[test]
    fn search_listings_case_insensitive() {
        let store = MarketplaceStore::new();
        let results = store.search_listings("METAL");
        assert!(!results.is_empty());
    }

    // -- add_seconds_iso helper -----------------------------------------------

    #[test]
    fn add_seconds_iso_48h() {
        let result = add_seconds_iso("2026-02-13T20:00:00Z", 48 * 3600);
        assert_eq!(result, "2026-02-15T20:00:00Z");
    }

    #[test]
    fn add_seconds_iso_partial() {
        let result = add_seconds_iso("2026-02-13T23:30:00Z", 3600);
        assert_eq!(result, "2026-02-14T00:30:00Z");
    }

    // -- Default trait --------------------------------------------------------

    #[test]
    fn default_creates_same_as_new() {
        let a = MarketplaceStore::new();
        let b = MarketplaceStore::default();
        assert_eq!(a.listings.len(), b.listings.len());
        assert_eq!(a.tax_rate, b.tax_rate);
    }

    // -- ListingFilters default -----------------------------------------------

    #[test]
    fn listing_filters_default_values() {
        let f = ListingFilters::default();
        assert_eq!(f.page, 1);
        assert_eq!(f.page_size, 20);
        assert!(f.listing_type.is_none());
        assert!(f.seller_id.is_none());
    }

    // -- Tax on created listing -----------------------------------------------

    #[test]
    fn created_listing_has_correct_tax() {
        let mut store = MarketplaceStore::new();
        store.tax_rate = 0.05;
        let mut input = make_resource_input(100);
        input.wanted_amount = 20_000;
        let listing = store.create_listing(input, now()).unwrap();
        assert_eq!(listing.tax_paid, 1_000); // 5% of 20_000
        assert!((listing.tax_rate - 0.05).abs() < f64::EPSILON);
    }

    // -- MarketplaceError display ---------------------------------------------

    #[test]
    fn marketplace_error_display() {
        assert_eq!(MarketplaceError::NotFound.to_string(), "listing not found");
        assert_eq!(
            MarketplaceError::OwnListing.to_string(),
            "cannot accept your own listing"
        );
        assert_eq!(
            MarketplaceError::InvalidInput("bad".into()).to_string(),
            "invalid input: bad"
        );
    }
}
