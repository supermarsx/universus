#![forbid(unsafe_code)]

use std::collections::HashMap;

use serde::Serialize;

pub fn crate_name() -> &'static str {
    "game-economy"
}

// ---------------------------------------------------------------------------
// Production
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionRate {
    pub metal_per_hour: f64,
    pub crystal_per_hour: f64,
    pub deuterium_per_hour: f64,
    pub energy_produced: i64,
    pub energy_consumed: i64,
}

pub fn calculate_mine_production(mine_type: &str, level: i32, universe_speed: i32) -> f64 {
    let base_rate: f64 = match mine_type {
        "metal" => 30.0,
        "crystal" => 20.0,
        "deuterium" => 10.0,
        _ => 0.0,
    };
    base_rate * level as f64 * 1.1_f64.powi(level) * universe_speed as f64
}

pub fn calculate_energy_production(level: i32) -> i64 {
    (20.0 * level as f64 * 1.1_f64.powi(level)) as i64
}

pub fn calculate_energy_consumption(mine_type: &str, level: i32) -> i64 {
    let base: f64 = match mine_type {
        "metal" => 10.0,
        "crystal" => 10.0,
        "deuterium" => 20.0,
        _ => 0.0,
    };
    (base * level as f64 * 1.1_f64.powi(level)) as i64
}

// ---------------------------------------------------------------------------
// Trade
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradeRate {
    pub metal_to_crystal: f64,
    pub metal_to_deuterium: f64,
    pub crystal_to_deuterium: f64,
}

pub fn default_trade_rates() -> TradeRate {
    TradeRate {
        metal_to_crystal: 2.0,
        metal_to_deuterium: 3.0,
        crystal_to_deuterium: 1.5,
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradeOffer {
    pub id: i64,
    pub seller_id: i64,
    pub offer_type: String,
    pub offer_amount: i64,
    pub request_type: String,
    pub request_amount: i64,
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// Marketplace (in-memory store)
// ---------------------------------------------------------------------------

pub struct Marketplace {
    next_id: i64,
    offers: HashMap<i64, TradeOffer>,
}

impl Marketplace {
    pub fn new() -> Self {
        Self {
            next_id: 0,
            offers: HashMap::new(),
        }
    }

    pub fn create_offer(
        &mut self,
        seller_id: i64,
        offer_type: String,
        offer_amount: i64,
        request_type: String,
        request_amount: i64,
    ) -> TradeOffer {
        self.next_id += 1;
        let offer = TradeOffer {
            id: self.next_id,
            seller_id,
            offer_type,
            offer_amount,
            request_type,
            request_amount,
            created_at: now_timestamp(),
        };
        self.offers.insert(offer.id, offer.clone());
        offer
    }

    pub fn list_offers(&self) -> Vec<TradeOffer> {
        let mut list: Vec<TradeOffer> = self.offers.values().cloned().collect();
        list.sort_by_key(|o| o.id);
        list
    }

    pub fn cancel_offer(&mut self, offer_id: i64, seller_id: i64) -> bool {
        if let Some(offer) = self.offers.get(&offer_id) {
            if offer.seller_id == seller_id {
                self.offers.remove(&offer_id);
                return true;
            }
        }
        false
    }

    pub fn accept_offer(&mut self, offer_id: i64) -> Option<TradeOffer> {
        self.offers.remove(&offer_id)
    }
}

impl Default for Marketplace {
    fn default() -> Self {
        Self::new()
    }
}

fn now_timestamp() -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("unix:{ts}")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_returns_expected() {
        assert_eq!(crate_name(), "game-economy");
    }

    #[test]
    fn metal_mine_production_level_1_speed_1() {
        let prod = calculate_mine_production("metal", 1, 1);
        let expected = 30.0 * 1.0 * 1.1_f64.powi(1) * 1.0;
        assert!((prod - expected).abs() < 1e-9);
    }

    #[test]
    fn crystal_mine_production_scales_with_speed() {
        let speed1 = calculate_mine_production("crystal", 5, 1);
        let speed3 = calculate_mine_production("crystal", 5, 3);
        assert!((speed3 - speed1 * 3.0).abs() < 1e-9);
    }

    #[test]
    fn unknown_mine_type_returns_zero() {
        assert_eq!(calculate_mine_production("unknown", 5, 1), 0.0);
    }

    #[test]
    fn energy_production_level_5() {
        let energy = calculate_energy_production(5);
        let expected = (20.0 * 5.0 * 1.1_f64.powi(5)) as i64;
        assert_eq!(energy, expected);
    }

    #[test]
    fn energy_consumption_deuterium_higher_base() {
        let metal = calculate_energy_consumption("metal", 3);
        let deuterium = calculate_energy_consumption("deuterium", 3);
        assert!(deuterium > metal);
    }

    #[test]
    fn default_trade_rates_values() {
        let rates = default_trade_rates();
        assert!((rates.metal_to_crystal - 2.0).abs() < f64::EPSILON);
        assert!((rates.metal_to_deuterium - 3.0).abs() < f64::EPSILON);
        assert!((rates.crystal_to_deuterium - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn marketplace_create_and_list() {
        let mut mp = Marketplace::new();
        mp.create_offer(1, "metal".into(), 1000, "crystal".into(), 500);
        mp.create_offer(2, "crystal".into(), 200, "deuterium".into(), 100);
        let offers = mp.list_offers();
        assert_eq!(offers.len(), 2);
        assert_eq!(offers[0].seller_id, 1);
        assert_eq!(offers[1].seller_id, 2);
    }

    #[test]
    fn marketplace_cancel_own_offer() {
        let mut mp = Marketplace::new();
        let offer = mp.create_offer(42, "metal".into(), 500, "crystal".into(), 250);
        assert!(mp.cancel_offer(offer.id, 42));
        assert!(mp.list_offers().is_empty());
    }

    #[test]
    fn marketplace_cancel_other_seller_fails() {
        let mut mp = Marketplace::new();
        let offer = mp.create_offer(42, "metal".into(), 500, "crystal".into(), 250);
        assert!(!mp.cancel_offer(offer.id, 99));
        assert_eq!(mp.list_offers().len(), 1);
    }

    #[test]
    fn marketplace_accept_removes_offer() {
        let mut mp = Marketplace::new();
        let offer = mp.create_offer(10, "deuterium".into(), 300, "metal".into(), 900);
        let accepted = mp.accept_offer(offer.id);
        assert!(accepted.is_some());
        assert_eq!(accepted.unwrap().seller_id, 10);
        assert!(mp.list_offers().is_empty());
    }

    #[test]
    fn marketplace_accept_missing_returns_none() {
        let mut mp = Marketplace::new();
        assert!(mp.accept_offer(999).is_none());
    }

    #[test]
    fn production_rate_default_is_zero() {
        let rate = ProductionRate::default();
        assert!((rate.metal_per_hour).abs() < f64::EPSILON);
        assert_eq!(rate.energy_produced, 0);
        assert_eq!(rate.energy_consumed, 0);
    }
}
