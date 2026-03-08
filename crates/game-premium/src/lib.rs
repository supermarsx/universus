#![forbid(unsafe_code)]

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// =============================================================================
// Dark Matter (Premium Currency)
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DarkMatterBalance {
    pub player_id: String,
    pub amount: u64,
    pub lifetime_earned: u64,
    pub lifetime_spent: u64,
    pub last_updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DarkMatterTransaction {
    pub id: u64,
    pub player_id: String,
    pub amount: i64,
    pub reason: TransactionReason,
    pub balance_after: u64,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransactionReason {
    Purchase,
    ExpeditionReward,
    AdminGrant,
    OfficerHire,
    BoosterPurchase,
    ResourcePackPurchase,
    CosmeticPurchase,
    Refund,
    EventReward,
}

// =============================================================================
// Officers
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum OfficerType {
    Commander,
    Admiral,
    Engineer,
    Geologist,
    Technocrat,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Officer {
    pub officer_type: OfficerType,
    pub name: &'static str,
    pub description: &'static str,
    pub cost_per_week: u64,
    pub bonuses: Vec<OfficerBonus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OfficerBonus {
    pub bonus_type: BonusType,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BonusType {
    BuildQueueSlots,
    FleetSlots,
    DefenseBonus,
    MineProductionBonus,
    ResearchSpeedBonus,
    ReducedFleetLoss,
    EnergyBonus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OfficerHire {
    pub id: u64,
    pub player_id: String,
    pub officer_type: OfficerType,
    pub hired_at: String,
    pub expires_at: String,
    pub is_active: bool,
}

pub fn officer_catalog() -> Vec<Officer> {
    vec![
        Officer {
            officer_type: OfficerType::Commander,
            name: "Commander",
            description: "Grants additional building queue slots for parallel construction.",
            cost_per_week: 5000,
            bonuses: vec![OfficerBonus {
                bonus_type: BonusType::BuildQueueSlots,
                value: 2.0,
            }],
        },
        Officer {
            officer_type: OfficerType::Admiral,
            name: "Admiral",
            description: "Expands your fleet capacity and reduces losses in combat.",
            cost_per_week: 5000,
            bonuses: vec![
                OfficerBonus {
                    bonus_type: BonusType::FleetSlots,
                    value: 2.0,
                },
                OfficerBonus {
                    bonus_type: BonusType::ReducedFleetLoss,
                    value: 5.0,
                },
            ],
        },
        Officer {
            officer_type: OfficerType::Engineer,
            name: "Engineer",
            description: "Strengthens planetary defenses and improves energy output.",
            cost_per_week: 5000,
            bonuses: vec![
                OfficerBonus {
                    bonus_type: BonusType::DefenseBonus,
                    value: 10.0,
                },
                OfficerBonus {
                    bonus_type: BonusType::EnergyBonus,
                    value: 10.0,
                },
            ],
        },
        Officer {
            officer_type: OfficerType::Geologist,
            name: "Geologist",
            description: "Boosts mine production across all your planets.",
            cost_per_week: 5000,
            bonuses: vec![OfficerBonus {
                bonus_type: BonusType::MineProductionBonus,
                value: 10.0,
            }],
        },
        Officer {
            officer_type: OfficerType::Technocrat,
            name: "Technocrat",
            description: "Accelerates research speed for all technologies.",
            cost_per_week: 5000,
            bonuses: vec![OfficerBonus {
                bonus_type: BonusType::ResearchSpeedBonus,
                value: 25.0,
            }],
        },
    ]
}

// =============================================================================
// Boosters
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BoosterType {
    ProductionBoost,
    ResearchSpeed,
    BuildingSpeed,
    FleetSpeed,
    ResourceProtection,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Booster {
    pub id: u64,
    pub name: String,
    pub booster_type: BoosterType,
    pub multiplier: f64,
    pub duration_hours: u32,
    pub cost_dm: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActiveBooster {
    pub id: u64,
    pub player_id: String,
    pub booster: Booster,
    pub activated_at: String,
    pub expires_at: String,
}

pub fn default_boosters() -> Vec<Booster> {
    vec![
        Booster {
            id: 1,
            name: "Bronze Production Boost".to_string(),
            booster_type: BoosterType::ProductionBoost,
            multiplier: 1.1,
            duration_hours: 24,
            cost_dm: 500,
        },
        Booster {
            id: 2,
            name: "Silver Production Boost".to_string(),
            booster_type: BoosterType::ProductionBoost,
            multiplier: 1.2,
            duration_hours: 72,
            cost_dm: 1200,
        },
        Booster {
            id: 3,
            name: "Gold Production Boost".to_string(),
            booster_type: BoosterType::ProductionBoost,
            multiplier: 1.3,
            duration_hours: 168,
            cost_dm: 3000,
        },
        Booster {
            id: 4,
            name: "Research Accelerator".to_string(),
            booster_type: BoosterType::ResearchSpeed,
            multiplier: 1.25,
            duration_hours: 48,
            cost_dm: 1000,
        },
        Booster {
            id: 5,
            name: "Rapid Construction".to_string(),
            booster_type: BoosterType::BuildingSpeed,
            multiplier: 1.2,
            duration_hours: 48,
            cost_dm: 1000,
        },
        Booster {
            id: 6,
            name: "Hyperspace Fuel".to_string(),
            booster_type: BoosterType::FleetSpeed,
            multiplier: 1.3,
            duration_hours: 24,
            cost_dm: 800,
        },
        Booster {
            id: 7,
            name: "Bunker Shield".to_string(),
            booster_type: BoosterType::ResourceProtection,
            multiplier: 1.5,
            duration_hours: 72,
            cost_dm: 1500,
        },
        Booster {
            id: 8,
            name: "Elite Research Accelerator".to_string(),
            booster_type: BoosterType::ResearchSpeed,
            multiplier: 1.5,
            duration_hours: 24,
            cost_dm: 2000,
        },
    ]
}

// =============================================================================
// Shop Items
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ShopCategory {
    Officers,
    Boosters,
    ResourcePacks,
    Cosmetics,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShopItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: ShopCategory,
    pub cost_dm: u64,
    pub is_available: bool,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResourcePack {
    pub metal: i64,
    pub crystal: i64,
    pub deuterium: i64,
}

pub fn default_shop_items() -> Vec<ShopItem> {
    vec![
        // Officers
        ShopItem {
            id: "officer-commander".to_string(),
            name: "Commander".to_string(),
            description: "Hire the Commander for 1 week. Grants +2 building queue slots."
                .to_string(),
            category: ShopCategory::Officers,
            cost_dm: 5000,
            is_available: true,
            metadata: serde_json::json!({"officer_type": "Commander", "duration_weeks": 1}),
        },
        ShopItem {
            id: "officer-admiral".to_string(),
            name: "Admiral".to_string(),
            description: "Hire the Admiral for 1 week. Grants +2 fleet slots and -5% fleet losses."
                .to_string(),
            category: ShopCategory::Officers,
            cost_dm: 5000,
            is_available: true,
            metadata: serde_json::json!({"officer_type": "Admiral", "duration_weeks": 1}),
        },
        ShopItem {
            id: "officer-engineer".to_string(),
            name: "Engineer".to_string(),
            description: "Hire the Engineer for 1 week. Grants +10% defense and +10% energy."
                .to_string(),
            category: ShopCategory::Officers,
            cost_dm: 5000,
            is_available: true,
            metadata: serde_json::json!({"officer_type": "Engineer", "duration_weeks": 1}),
        },
        // Boosters
        ShopItem {
            id: "booster-production-bronze".to_string(),
            name: "Bronze Production Boost".to_string(),
            description: "+10% production for 24 hours.".to_string(),
            category: ShopCategory::Boosters,
            cost_dm: 500,
            is_available: true,
            metadata: serde_json::json!({"booster_id": 1, "multiplier": 1.1, "duration_hours": 24}),
        },
        ShopItem {
            id: "booster-research-accelerator".to_string(),
            name: "Research Accelerator".to_string(),
            description: "+25% research speed for 48 hours.".to_string(),
            category: ShopCategory::Boosters,
            cost_dm: 1000,
            is_available: true,
            metadata: serde_json::json!({"booster_id": 4, "multiplier": 1.25, "duration_hours": 48}),
        },
        // Resource Packs
        ShopItem {
            id: "pack-starter".to_string(),
            name: "Starter Resource Pack".to_string(),
            description: "A small bundle of resources to kickstart your empire.".to_string(),
            category: ShopCategory::ResourcePacks,
            cost_dm: 1000,
            is_available: true,
            metadata: serde_json::json!({"metal": 50000, "crystal": 30000, "deuterium": 10000}),
        },
        ShopItem {
            id: "pack-advanced".to_string(),
            name: "Advanced Resource Pack".to_string(),
            description: "A hefty resource supply for mid-game expansion.".to_string(),
            category: ShopCategory::ResourcePacks,
            cost_dm: 5000,
            is_available: true,
            metadata: serde_json::json!({"metal": 300000, "crystal": 150000, "deuterium": 75000}),
        },
        ShopItem {
            id: "pack-elite".to_string(),
            name: "Elite Resource Pack".to_string(),
            description: "Massive resources for late-game supremacy.".to_string(),
            category: ShopCategory::ResourcePacks,
            cost_dm: 15000,
            is_available: true,
            metadata: serde_json::json!({"metal": 1000000, "crystal": 500000, "deuterium": 250000}),
        },
        // Cosmetics
        ShopItem {
            id: "cosmetic-planet-skin-lava".to_string(),
            name: "Lava Planet Skin".to_string(),
            description: "Transform the appearance of your planet to a fiery lava world."
                .to_string(),
            category: ShopCategory::Cosmetics,
            cost_dm: 2000,
            is_available: true,
            metadata: serde_json::json!({"skin_id": "lava", "type": "planet_skin"}),
        },
        ShopItem {
            id: "cosmetic-fleet-trail-neon".to_string(),
            name: "Neon Fleet Trail".to_string(),
            description: "Leave a glowing neon trail as your fleets travel through space."
                .to_string(),
            category: ShopCategory::Cosmetics,
            cost_dm: 1500,
            is_available: true,
            metadata: serde_json::json!({"trail_id": "neon", "type": "fleet_trail"}),
        },
        ShopItem {
            id: "cosmetic-avatar-frame-gold".to_string(),
            name: "Gold Avatar Frame".to_string(),
            description: "A prestigious golden frame for your player avatar.".to_string(),
            category: ShopCategory::Cosmetics,
            cost_dm: 3000,
            is_available: false,
            metadata: serde_json::json!({"frame_id": "gold", "type": "avatar_frame"}),
        },
    ]
}

// =============================================================================
// Purchase Flow
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PurchaseRequest {
    pub player_id: String,
    pub item_id: String,
    pub quantity: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PurchaseResult {
    pub success: bool,
    pub transaction_id: Option<u64>,
    pub error: Option<String>,
    pub items_granted: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PremiumError {
    InsufficientDarkMatter,
    ItemNotFound,
    ItemUnavailable,
    InvalidQuantity,
    AlreadyActive,
}

impl std::fmt::Display for PremiumError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PremiumError::InsufficientDarkMatter => {
                write!(f, "Insufficient Dark Matter balance")
            }
            PremiumError::ItemNotFound => write!(f, "Item not found"),
            PremiumError::ItemUnavailable => write!(f, "Item is currently unavailable"),
            PremiumError::InvalidQuantity => write!(f, "Invalid quantity"),
            PremiumError::AlreadyActive => write!(f, "Item is already active"),
        }
    }
}

// =============================================================================
// Dark Matter Store
// =============================================================================

#[derive(Debug, Clone)]
pub struct DarkMatterStore {
    balances: HashMap<String, DarkMatterBalance>,
    transactions: Vec<DarkMatterTransaction>,
    next_tx_id: u64,
}

impl DarkMatterStore {
    pub fn new() -> Self {
        Self {
            balances: HashMap::new(),
            transactions: Vec::new(),
            next_tx_id: 1,
        }
    }

    pub fn get_balance(&self, player_id: &str) -> u64 {
        self.balances.get(player_id).map(|b| b.amount).unwrap_or(0)
    }

    pub fn credit(
        &mut self,
        player_id: &str,
        amount: u64,
        reason: TransactionReason,
    ) -> DarkMatterTransaction {
        let balance = self
            .balances
            .entry(player_id.to_string())
            .or_insert_with(|| DarkMatterBalance {
                player_id: player_id.to_string(),
                amount: 0,
                lifetime_earned: 0,
                lifetime_spent: 0,
                last_updated: String::new(),
            });

        balance.amount += amount;
        balance.lifetime_earned += amount;
        balance.last_updated = now_placeholder();

        let tx = DarkMatterTransaction {
            id: self.next_tx_id,
            player_id: player_id.to_string(),
            amount: amount as i64,
            reason,
            balance_after: balance.amount,
            timestamp: now_placeholder(),
        };
        self.next_tx_id += 1;
        self.transactions.push(tx.clone());
        tx
    }

    pub fn debit(
        &mut self,
        player_id: &str,
        amount: u64,
        reason: TransactionReason,
    ) -> Result<DarkMatterTransaction, PremiumError> {
        let current = self.get_balance(player_id);
        if current < amount {
            return Err(PremiumError::InsufficientDarkMatter);
        }

        let balance = self.balances.get_mut(player_id).unwrap();
        balance.amount -= amount;
        balance.lifetime_spent += amount;
        balance.last_updated = now_placeholder();

        let tx = DarkMatterTransaction {
            id: self.next_tx_id,
            player_id: player_id.to_string(),
            amount: -(amount as i64),
            reason,
            balance_after: balance.amount,
            timestamp: now_placeholder(),
        };
        self.next_tx_id += 1;
        self.transactions.push(tx.clone());
        Ok(tx)
    }

    pub fn get_transactions(&self, player_id: &str, limit: usize) -> Vec<&DarkMatterTransaction> {
        self.transactions
            .iter()
            .rev()
            .filter(|tx| tx.player_id == player_id)
            .take(limit)
            .collect()
    }

    pub fn get_balance_record(&self, player_id: &str) -> Option<&DarkMatterBalance> {
        self.balances.get(player_id)
    }
}

impl Default for DarkMatterStore {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Officer Store
// =============================================================================

#[derive(Debug, Clone)]
pub struct OfficerStore {
    hires: Vec<OfficerHire>,
    next_id: u64,
}

impl OfficerStore {
    pub fn new() -> Self {
        Self {
            hires: Vec::new(),
            next_id: 1,
        }
    }

    pub fn hire(
        &mut self,
        player_id: &str,
        officer_type: OfficerType,
        duration_weeks: u32,
        now: &str,
    ) -> OfficerHire {
        let expires_at = add_weeks_to_timestamp(now, duration_weeks);
        let hire = OfficerHire {
            id: self.next_id,
            player_id: player_id.to_string(),
            officer_type,
            hired_at: now.to_string(),
            expires_at,
            is_active: true,
        };
        self.next_id += 1;
        self.hires.push(hire.clone());
        hire
    }

    pub fn get_active_officers(&self, player_id: &str, now: &str) -> Vec<&OfficerHire> {
        self.hires
            .iter()
            .filter(|h| h.player_id == player_id && h.is_active && h.expires_at.as_str() > now)
            .collect()
    }

    pub fn is_officer_active(
        &self,
        player_id: &str,
        officer_type: &OfficerType,
        now: &str,
    ) -> bool {
        self.hires.iter().any(|h| {
            h.player_id == player_id
                && &h.officer_type == officer_type
                && h.is_active
                && h.expires_at.as_str() > now
        })
    }

    pub fn get_combined_bonuses(&self, player_id: &str, now: &str) -> Vec<OfficerBonus> {
        let active = self.get_active_officers(player_id, now);
        let catalog = officer_catalog();
        let mut bonus_map: HashMap<String, f64> = HashMap::new();

        for hire in active {
            if let Some(officer) = catalog.iter().find(|o| o.officer_type == hire.officer_type) {
                for bonus in &officer.bonuses {
                    let key = format!("{:?}", bonus.bonus_type);
                    let entry = bonus_map.entry(key).or_insert(0.0);
                    *entry += bonus.value;
                }
            }
        }

        bonus_map
            .into_iter()
            .map(|(key, value)| {
                let bonus_type = match key.as_str() {
                    "BuildQueueSlots" => BonusType::BuildQueueSlots,
                    "FleetSlots" => BonusType::FleetSlots,
                    "DefenseBonus" => BonusType::DefenseBonus,
                    "MineProductionBonus" => BonusType::MineProductionBonus,
                    "ResearchSpeedBonus" => BonusType::ResearchSpeedBonus,
                    "ReducedFleetLoss" => BonusType::ReducedFleetLoss,
                    "EnergyBonus" => BonusType::EnergyBonus,
                    _ => unreachable!(),
                };
                OfficerBonus { bonus_type, value }
            })
            .collect()
    }

    pub fn expire_officers(&mut self, now: &str) -> usize {
        let mut count = 0;
        for hire in &mut self.hires {
            if hire.is_active && hire.expires_at.as_str() <= now {
                hire.is_active = false;
                count += 1;
            }
        }
        count
    }
}

impl Default for OfficerStore {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Booster Store
// =============================================================================

#[derive(Debug, Clone)]
pub struct BoosterStore {
    active_boosters: Vec<ActiveBooster>,
    next_id: u64,
}

impl BoosterStore {
    pub fn new() -> Self {
        Self {
            active_boosters: Vec::new(),
            next_id: 1,
        }
    }

    pub fn activate(&mut self, player_id: &str, booster: Booster, now: &str) -> ActiveBooster {
        let expires_at = add_hours_to_timestamp(now, booster.duration_hours);
        let active = ActiveBooster {
            id: self.next_id,
            player_id: player_id.to_string(),
            booster,
            activated_at: now.to_string(),
            expires_at,
        };
        self.next_id += 1;
        self.active_boosters.push(active.clone());
        active
    }

    pub fn get_active_boosters(&self, player_id: &str, now: &str) -> Vec<&ActiveBooster> {
        self.active_boosters
            .iter()
            .filter(|b| b.player_id == player_id && b.expires_at.as_str() > now)
            .collect()
    }

    pub fn get_multiplier(&self, player_id: &str, booster_type: &BoosterType, now: &str) -> f64 {
        let active = self.get_active_boosters(player_id, now);
        let mut multiplier = 1.0;
        for ab in active {
            if &ab.booster.booster_type == booster_type {
                multiplier *= ab.booster.multiplier;
            }
        }
        multiplier
    }

    pub fn expire_boosters(&mut self, now: &str) -> usize {
        let before = self.active_boosters.len();
        self.active_boosters.retain(|b| b.expires_at.as_str() > now);
        before - self.active_boosters.len()
    }
}

impl Default for BoosterStore {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Shop Store
// =============================================================================

#[derive(Debug, Clone)]
pub struct ShopStore {
    items: Vec<ShopItem>,
}

impl ShopStore {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn with_defaults() -> Self {
        Self {
            items: default_shop_items(),
        }
    }

    pub fn list_items(&self) -> Vec<&ShopItem> {
        self.items.iter().collect()
    }

    pub fn list_by_category(&self, category: &ShopCategory) -> Vec<&ShopItem> {
        self.items
            .iter()
            .filter(|i| &i.category == category)
            .collect()
    }

    pub fn get_item(&self, id: &str) -> Option<&ShopItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn add_item(&mut self, item: ShopItem) {
        self.items.push(item);
    }

    pub fn remove_item(&mut self, id: &str) -> bool {
        let before = self.items.len();
        self.items.retain(|i| i.id != id);
        self.items.len() < before
    }

    pub fn update_availability(&mut self, id: &str, available: bool) -> bool {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.is_available = available;
            true
        } else {
            false
        }
    }
}

impl Default for ShopStore {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Timestamp Helpers (simple ISO 8601 string manipulation)
// =============================================================================

fn now_placeholder() -> String {
    "2026-01-01T00:00:00Z".to_string()
}

/// Add weeks to an ISO 8601 timestamp string.
fn add_weeks_to_timestamp(timestamp: &str, weeks: u32) -> String {
    add_hours_to_timestamp(timestamp, weeks * 7 * 24)
}

/// Add hours to an ISO 8601 timestamp string.
/// Performs basic parsing of "YYYY-MM-DDThh:mm:ssZ" format.
fn add_hours_to_timestamp(timestamp: &str, hours: u32) -> String {
    let parts: Vec<&str> = timestamp.split('T').collect();
    if parts.len() != 2 {
        return timestamp.to_string();
    }
    let date_parts: Vec<u32> = parts[0].split('-').filter_map(|s| s.parse().ok()).collect();
    let time_str = parts[1].trim_end_matches('Z');
    let time_parts: Vec<u32> = time_str.split(':').filter_map(|s| s.parse().ok()).collect();

    if date_parts.len() != 3 || time_parts.len() != 3 {
        return timestamp.to_string();
    }

    let (mut year, mut month, mut day) = (date_parts[0], date_parts[1], date_parts[2]);
    let (mut hour, min, sec) = (time_parts[0], time_parts[1], time_parts[2]);

    let total_hours = hour + hours;
    let extra_days = total_hours / 24;
    hour = total_hours % 24;
    day += extra_days;

    loop {
        let dim = days_in_month(year, month);
        if day <= dim {
            break;
        }
        day -= dim;
        month += 1;
        if month > 12 {
            month = 1;
            year += 1;
        }
    }

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hour, min, sec
    )
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 => 31,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        3 => 31,
        4 => 30,
        5 => 31,
        6 => 30,
        7 => 31,
        8 => 31,
        9 => 30,
        10 => 31,
        11 => 30,
        12 => 31,
        _ => 30,
    }
}

fn is_leap_year(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Timestamp helpers ----

    #[test]
    fn test_add_hours_basic() {
        let result = add_hours_to_timestamp("2026-01-01T00:00:00Z", 5);
        assert_eq!(result, "2026-01-01T05:00:00Z");
    }

    #[test]
    fn test_add_hours_day_rollover() {
        let result = add_hours_to_timestamp("2026-01-01T23:00:00Z", 2);
        assert_eq!(result, "2026-01-02T01:00:00Z");
    }

    #[test]
    fn test_add_hours_month_rollover() {
        let result = add_hours_to_timestamp("2026-01-31T12:00:00Z", 24);
        assert_eq!(result, "2026-02-01T12:00:00Z");
    }

    #[test]
    fn test_add_hours_year_rollover() {
        let result = add_hours_to_timestamp("2026-12-31T23:00:00Z", 2);
        assert_eq!(result, "2027-01-01T01:00:00Z");
    }

    #[test]
    fn test_add_weeks() {
        let result = add_weeks_to_timestamp("2026-01-01T00:00:00Z", 1);
        assert_eq!(result, "2026-01-08T00:00:00Z");
    }

    #[test]
    fn test_add_weeks_multiple() {
        let result = add_weeks_to_timestamp("2026-01-01T00:00:00Z", 4);
        assert_eq!(result, "2026-01-29T00:00:00Z");
    }

    #[test]
    fn test_leap_year() {
        assert!(is_leap_year(2024));
        assert!(!is_leap_year(2023));
        assert!(!is_leap_year(1900));
        assert!(is_leap_year(2000));
    }

    #[test]
    fn test_add_hours_leap_february() {
        let result = add_hours_to_timestamp("2024-02-28T12:00:00Z", 24);
        assert_eq!(result, "2024-02-29T12:00:00Z");
    }

    // ---- DarkMatterStore ----

    #[test]
    fn test_dm_store_new_balance_is_zero() {
        let store = DarkMatterStore::new();
        assert_eq!(store.get_balance("player1"), 0);
    }

    #[test]
    fn test_dm_store_credit() {
        let mut store = DarkMatterStore::new();
        let tx = store.credit("player1", 1000, TransactionReason::Purchase);
        assert_eq!(tx.amount, 1000);
        assert_eq!(tx.balance_after, 1000);
        assert_eq!(store.get_balance("player1"), 1000);
    }

    #[test]
    fn test_dm_store_multiple_credits() {
        let mut store = DarkMatterStore::new();
        store.credit("player1", 1000, TransactionReason::Purchase);
        store.credit("player1", 500, TransactionReason::ExpeditionReward);
        assert_eq!(store.get_balance("player1"), 1500);
    }

    #[test]
    fn test_dm_store_debit_success() {
        let mut store = DarkMatterStore::new();
        store.credit("player1", 1000, TransactionReason::Purchase);
        let tx = store
            .debit("player1", 300, TransactionReason::OfficerHire)
            .unwrap();
        assert_eq!(tx.amount, -300);
        assert_eq!(tx.balance_after, 700);
        assert_eq!(store.get_balance("player1"), 700);
    }

    #[test]
    fn test_dm_store_debit_insufficient() {
        let mut store = DarkMatterStore::new();
        store.credit("player1", 100, TransactionReason::Purchase);
        let result = store.debit("player1", 200, TransactionReason::OfficerHire);
        assert_eq!(result, Err(PremiumError::InsufficientDarkMatter));
    }

    #[test]
    fn test_dm_store_debit_zero_balance() {
        let store = DarkMatterStore::new();
        assert_eq!(store.get_balance("nobody"), 0);
    }

    #[test]
    fn test_dm_store_debit_empty_player() {
        let mut store = DarkMatterStore::new();
        let result = store.debit("nobody", 100, TransactionReason::OfficerHire);
        assert_eq!(result, Err(PremiumError::InsufficientDarkMatter));
    }

    #[test]
    fn test_dm_store_debit_exact_balance() {
        let mut store = DarkMatterStore::new();
        store.credit("player1", 500, TransactionReason::Purchase);
        let tx = store
            .debit("player1", 500, TransactionReason::BoosterPurchase)
            .unwrap();
        assert_eq!(tx.balance_after, 0);
        assert_eq!(store.get_balance("player1"), 0);
    }

    #[test]
    fn test_dm_store_transactions_order() {
        let mut store = DarkMatterStore::new();
        store.credit("player1", 1000, TransactionReason::Purchase);
        store.credit("player1", 500, TransactionReason::ExpeditionReward);
        store
            .debit("player1", 200, TransactionReason::OfficerHire)
            .unwrap();
        let txs = store.get_transactions("player1", 10);
        assert_eq!(txs.len(), 3);
        // Most recent first
        assert_eq!(txs[0].amount, -200);
        assert_eq!(txs[1].amount, 500);
        assert_eq!(txs[2].amount, 1000);
    }

    #[test]
    fn test_dm_store_transactions_limit() {
        let mut store = DarkMatterStore::new();
        for _ in 0..10 {
            store.credit("player1", 100, TransactionReason::Purchase);
        }
        let txs = store.get_transactions("player1", 3);
        assert_eq!(txs.len(), 3);
    }

    #[test]
    fn test_dm_store_transactions_player_filter() {
        let mut store = DarkMatterStore::new();
        store.credit("player1", 1000, TransactionReason::Purchase);
        store.credit("player2", 500, TransactionReason::Purchase);
        let txs = store.get_transactions("player1", 10);
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0].player_id, "player1");
    }

    #[test]
    fn test_dm_store_balance_record() {
        let mut store = DarkMatterStore::new();
        assert!(store.get_balance_record("player1").is_none());
        store.credit("player1", 1000, TransactionReason::Purchase);
        let record = store.get_balance_record("player1").unwrap();
        assert_eq!(record.amount, 1000);
        assert_eq!(record.lifetime_earned, 1000);
        assert_eq!(record.lifetime_spent, 0);
    }

    #[test]
    fn test_dm_store_lifetime_tracking() {
        let mut store = DarkMatterStore::new();
        store.credit("player1", 1000, TransactionReason::Purchase);
        store.credit("player1", 500, TransactionReason::ExpeditionReward);
        store
            .debit("player1", 300, TransactionReason::OfficerHire)
            .unwrap();
        let record = store.get_balance_record("player1").unwrap();
        assert_eq!(record.lifetime_earned, 1500);
        assert_eq!(record.lifetime_spent, 300);
        assert_eq!(record.amount, 1200);
    }

    #[test]
    fn test_dm_store_tx_ids_increment() {
        let mut store = DarkMatterStore::new();
        let tx1 = store.credit("player1", 100, TransactionReason::Purchase);
        let tx2 = store.credit("player1", 200, TransactionReason::Purchase);
        assert_eq!(tx1.id, 1);
        assert_eq!(tx2.id, 2);
    }

    #[test]
    fn test_dm_store_default() {
        let store = DarkMatterStore::default();
        assert_eq!(store.get_balance("player1"), 0);
    }

    #[test]
    fn test_dm_store_multiple_players() {
        let mut store = DarkMatterStore::new();
        store.credit("player1", 1000, TransactionReason::Purchase);
        store.credit("player2", 2000, TransactionReason::Purchase);
        assert_eq!(store.get_balance("player1"), 1000);
        assert_eq!(store.get_balance("player2"), 2000);
    }

    // ---- TransactionReason serialization ----

    #[test]
    fn test_transaction_reason_serde() {
        let reason = TransactionReason::Purchase;
        let json = serde_json::to_string(&reason).unwrap();
        let deserialized: TransactionReason = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, TransactionReason::Purchase);
    }

    #[test]
    fn test_transaction_reason_all_variants() {
        let reasons = vec![
            TransactionReason::Purchase,
            TransactionReason::ExpeditionReward,
            TransactionReason::AdminGrant,
            TransactionReason::OfficerHire,
            TransactionReason::BoosterPurchase,
            TransactionReason::ResourcePackPurchase,
            TransactionReason::CosmeticPurchase,
            TransactionReason::Refund,
            TransactionReason::EventReward,
        ];
        for reason in reasons {
            let json = serde_json::to_string(&reason).unwrap();
            let deserialized: TransactionReason = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, reason);
        }
    }

    // ---- Officer Catalog ----

    #[test]
    fn test_officer_catalog_has_five() {
        let catalog = officer_catalog();
        assert_eq!(catalog.len(), 5);
    }

    #[test]
    fn test_officer_catalog_commander() {
        let catalog = officer_catalog();
        let cmd = catalog
            .iter()
            .find(|o| o.officer_type == OfficerType::Commander)
            .unwrap();
        assert_eq!(cmd.cost_per_week, 5000);
        assert_eq!(cmd.bonuses.len(), 1);
        assert_eq!(cmd.bonuses[0].bonus_type, BonusType::BuildQueueSlots);
        assert_eq!(cmd.bonuses[0].value, 2.0);
    }

    #[test]
    fn test_officer_catalog_admiral() {
        let catalog = officer_catalog();
        let adm = catalog
            .iter()
            .find(|o| o.officer_type == OfficerType::Admiral)
            .unwrap();
        assert_eq!(adm.cost_per_week, 5000);
        assert_eq!(adm.bonuses.len(), 2);
    }

    #[test]
    fn test_officer_catalog_engineer() {
        let catalog = officer_catalog();
        let eng = catalog
            .iter()
            .find(|o| o.officer_type == OfficerType::Engineer)
            .unwrap();
        assert_eq!(eng.bonuses.len(), 2);
        let defense = eng
            .bonuses
            .iter()
            .find(|b| b.bonus_type == BonusType::DefenseBonus)
            .unwrap();
        assert_eq!(defense.value, 10.0);
    }

    #[test]
    fn test_officer_catalog_geologist() {
        let catalog = officer_catalog();
        let geo = catalog
            .iter()
            .find(|o| o.officer_type == OfficerType::Geologist)
            .unwrap();
        assert_eq!(geo.bonuses.len(), 1);
        assert_eq!(geo.bonuses[0].bonus_type, BonusType::MineProductionBonus);
        assert_eq!(geo.bonuses[0].value, 10.0);
    }

    #[test]
    fn test_officer_catalog_technocrat() {
        let catalog = officer_catalog();
        let tech = catalog
            .iter()
            .find(|o| o.officer_type == OfficerType::Technocrat)
            .unwrap();
        assert_eq!(tech.bonuses.len(), 1);
        assert_eq!(tech.bonuses[0].bonus_type, BonusType::ResearchSpeedBonus);
        assert_eq!(tech.bonuses[0].value, 25.0);
    }

    #[test]
    fn test_officer_catalog_all_cost_5000() {
        let catalog = officer_catalog();
        for officer in &catalog {
            assert_eq!(officer.cost_per_week, 5000);
        }
    }

    // ---- Officer Store ----

    #[test]
    fn test_officer_store_hire() {
        let mut store = OfficerStore::new();
        let hire = store.hire("player1", OfficerType::Commander, 1, "2026-01-01T00:00:00Z");
        assert_eq!(hire.player_id, "player1");
        assert_eq!(hire.officer_type, OfficerType::Commander);
        assert_eq!(hire.hired_at, "2026-01-01T00:00:00Z");
        assert_eq!(hire.expires_at, "2026-01-08T00:00:00Z");
        assert!(hire.is_active);
    }

    #[test]
    fn test_officer_store_hire_multiple_weeks() {
        let mut store = OfficerStore::new();
        let hire = store.hire("player1", OfficerType::Admiral, 4, "2026-01-01T00:00:00Z");
        assert_eq!(hire.expires_at, "2026-01-29T00:00:00Z");
    }

    #[test]
    fn test_officer_store_get_active() {
        let mut store = OfficerStore::new();
        store.hire("player1", OfficerType::Commander, 1, "2026-01-01T00:00:00Z");
        store.hire("player1", OfficerType::Admiral, 1, "2026-01-01T00:00:00Z");
        let active = store.get_active_officers("player1", "2026-01-05T00:00:00Z");
        assert_eq!(active.len(), 2);
    }

    #[test]
    fn test_officer_store_get_active_expired() {
        let mut store = OfficerStore::new();
        store.hire("player1", OfficerType::Commander, 1, "2026-01-01T00:00:00Z");
        let active = store.get_active_officers("player1", "2026-01-10T00:00:00Z");
        assert_eq!(active.len(), 0);
    }

    #[test]
    fn test_officer_store_is_active() {
        let mut store = OfficerStore::new();
        store.hire("player1", OfficerType::Commander, 1, "2026-01-01T00:00:00Z");
        assert!(store.is_officer_active(
            "player1",
            &OfficerType::Commander,
            "2026-01-05T00:00:00Z"
        ));
        assert!(!store.is_officer_active(
            "player1",
            &OfficerType::Commander,
            "2026-01-10T00:00:00Z"
        ));
    }

    #[test]
    fn test_officer_store_is_active_wrong_type() {
        let mut store = OfficerStore::new();
        store.hire("player1", OfficerType::Commander, 1, "2026-01-01T00:00:00Z");
        assert!(!store.is_officer_active("player1", &OfficerType::Admiral, "2026-01-05T00:00:00Z"));
    }

    #[test]
    fn test_officer_store_combined_bonuses() {
        let mut store = OfficerStore::new();
        store.hire("player1", OfficerType::Commander, 1, "2026-01-01T00:00:00Z");
        store.hire("player1", OfficerType::Geologist, 1, "2026-01-01T00:00:00Z");
        let bonuses = store.get_combined_bonuses("player1", "2026-01-05T00:00:00Z");
        assert_eq!(bonuses.len(), 2);
    }

    #[test]
    fn test_officer_store_combined_bonuses_values() {
        let mut store = OfficerStore::new();
        store.hire("player1", OfficerType::Commander, 1, "2026-01-01T00:00:00Z");
        let bonuses = store.get_combined_bonuses("player1", "2026-01-05T00:00:00Z");
        let queue_bonus = bonuses
            .iter()
            .find(|b| b.bonus_type == BonusType::BuildQueueSlots)
            .unwrap();
        assert_eq!(queue_bonus.value, 2.0);
    }

    #[test]
    fn test_officer_store_combined_bonuses_none_active() {
        let store = OfficerStore::new();
        let bonuses = store.get_combined_bonuses("player1", "2026-01-05T00:00:00Z");
        assert!(bonuses.is_empty());
    }

    #[test]
    fn test_officer_store_expire() {
        let mut store = OfficerStore::new();
        store.hire("player1", OfficerType::Commander, 1, "2026-01-01T00:00:00Z");
        store.hire("player1", OfficerType::Admiral, 2, "2026-01-01T00:00:00Z");
        // Commander expires at 2026-01-08, Admiral at 2026-01-15
        let expired = store.expire_officers("2026-01-09T00:00:00Z");
        assert_eq!(expired, 1);
    }

    #[test]
    fn test_officer_store_expire_all() {
        let mut store = OfficerStore::new();
        store.hire("player1", OfficerType::Commander, 1, "2026-01-01T00:00:00Z");
        store.hire("player1", OfficerType::Admiral, 1, "2026-01-01T00:00:00Z");
        let expired = store.expire_officers("2026-01-10T00:00:00Z");
        assert_eq!(expired, 2);
    }

    #[test]
    fn test_officer_store_expire_none() {
        let mut store = OfficerStore::new();
        store.hire("player1", OfficerType::Commander, 1, "2026-01-01T00:00:00Z");
        let expired = store.expire_officers("2026-01-02T00:00:00Z");
        assert_eq!(expired, 0);
    }

    #[test]
    fn test_officer_store_expire_idempotent() {
        let mut store = OfficerStore::new();
        store.hire("player1", OfficerType::Commander, 1, "2026-01-01T00:00:00Z");
        store.expire_officers("2026-01-10T00:00:00Z");
        let expired_again = store.expire_officers("2026-01-10T00:00:00Z");
        assert_eq!(expired_again, 0);
    }

    #[test]
    fn test_officer_store_default() {
        let store = OfficerStore::default();
        let active = store.get_active_officers("player1", "2026-01-01T00:00:00Z");
        assert!(active.is_empty());
    }

    #[test]
    fn test_officer_store_hire_ids_increment() {
        let mut store = OfficerStore::new();
        let h1 = store.hire("player1", OfficerType::Commander, 1, "2026-01-01T00:00:00Z");
        let h2 = store.hire("player1", OfficerType::Admiral, 1, "2026-01-01T00:00:00Z");
        assert_eq!(h1.id, 1);
        assert_eq!(h2.id, 2);
    }

    #[test]
    fn test_officer_store_different_players() {
        let mut store = OfficerStore::new();
        store.hire("player1", OfficerType::Commander, 1, "2026-01-01T00:00:00Z");
        store.hire("player2", OfficerType::Admiral, 1, "2026-01-01T00:00:00Z");
        let p1 = store.get_active_officers("player1", "2026-01-05T00:00:00Z");
        let p2 = store.get_active_officers("player2", "2026-01-05T00:00:00Z");
        assert_eq!(p1.len(), 1);
        assert_eq!(p2.len(), 1);
        assert_eq!(p1[0].officer_type, OfficerType::Commander);
        assert_eq!(p2[0].officer_type, OfficerType::Admiral);
    }

    // ---- Booster Catalog ----

    #[test]
    fn test_default_boosters_count() {
        let boosters = default_boosters();
        assert!(boosters.len() >= 6);
    }

    #[test]
    fn test_default_boosters_types_covered() {
        let boosters = default_boosters();
        let types: Vec<&BoosterType> = boosters.iter().map(|b| &b.booster_type).collect();
        assert!(types.contains(&&BoosterType::ProductionBoost));
        assert!(types.contains(&&BoosterType::ResearchSpeed));
        assert!(types.contains(&&BoosterType::BuildingSpeed));
        assert!(types.contains(&&BoosterType::FleetSpeed));
        assert!(types.contains(&&BoosterType::ResourceProtection));
    }

    #[test]
    fn test_default_boosters_positive_costs() {
        let boosters = default_boosters();
        for booster in &boosters {
            assert!(booster.cost_dm > 0);
            assert!(booster.multiplier > 1.0);
            assert!(booster.duration_hours > 0);
        }
    }

    // ---- Booster Store ----

    #[test]
    fn test_booster_store_activate() {
        let mut store = BoosterStore::new();
        let booster = Booster {
            id: 1,
            name: "Test Boost".to_string(),
            booster_type: BoosterType::ProductionBoost,
            multiplier: 1.1,
            duration_hours: 24,
            cost_dm: 500,
        };
        let active = store.activate("player1", booster, "2026-01-01T00:00:00Z");
        assert_eq!(active.player_id, "player1");
        assert_eq!(active.activated_at, "2026-01-01T00:00:00Z");
        assert_eq!(active.expires_at, "2026-01-02T00:00:00Z");
    }

    #[test]
    fn test_booster_store_get_active() {
        let mut store = BoosterStore::new();
        let booster = default_boosters().into_iter().next().unwrap();
        store.activate("player1", booster, "2026-01-01T00:00:00Z");
        let active = store.get_active_boosters("player1", "2026-01-01T12:00:00Z");
        assert_eq!(active.len(), 1);
    }

    #[test]
    fn test_booster_store_get_active_expired() {
        let mut store = BoosterStore::new();
        let booster = Booster {
            id: 1,
            name: "Short Boost".to_string(),
            booster_type: BoosterType::ProductionBoost,
            multiplier: 1.1,
            duration_hours: 1,
            cost_dm: 100,
        };
        store.activate("player1", booster, "2026-01-01T00:00:00Z");
        let active = store.get_active_boosters("player1", "2026-01-01T02:00:00Z");
        assert_eq!(active.len(), 0);
    }

    #[test]
    fn test_booster_store_multiplier_none() {
        let store = BoosterStore::new();
        let mult = store.get_multiplier(
            "player1",
            &BoosterType::ProductionBoost,
            "2026-01-01T00:00:00Z",
        );
        assert_eq!(mult, 1.0);
    }

    #[test]
    fn test_booster_store_multiplier_single() {
        let mut store = BoosterStore::new();
        let booster = Booster {
            id: 1,
            name: "Prod Boost".to_string(),
            booster_type: BoosterType::ProductionBoost,
            multiplier: 1.2,
            duration_hours: 24,
            cost_dm: 500,
        };
        store.activate("player1", booster, "2026-01-01T00:00:00Z");
        let mult = store.get_multiplier(
            "player1",
            &BoosterType::ProductionBoost,
            "2026-01-01T12:00:00Z",
        );
        assert!((mult - 1.2).abs() < f64::EPSILON);
    }

    #[test]
    fn test_booster_store_multiplier_stacking() {
        let mut store = BoosterStore::new();
        let b1 = Booster {
            id: 1,
            name: "Prod Boost 1".to_string(),
            booster_type: BoosterType::ProductionBoost,
            multiplier: 1.1,
            duration_hours: 24,
            cost_dm: 500,
        };
        let b2 = Booster {
            id: 2,
            name: "Prod Boost 2".to_string(),
            booster_type: BoosterType::ProductionBoost,
            multiplier: 1.2,
            duration_hours: 24,
            cost_dm: 1000,
        };
        store.activate("player1", b1, "2026-01-01T00:00:00Z");
        store.activate("player1", b2, "2026-01-01T00:00:00Z");
        let mult = store.get_multiplier(
            "player1",
            &BoosterType::ProductionBoost,
            "2026-01-01T12:00:00Z",
        );
        assert!((mult - 1.32).abs() < 0.001); // 1.1 * 1.2 = 1.32
    }

    #[test]
    fn test_booster_store_multiplier_different_type() {
        let mut store = BoosterStore::new();
        let booster = Booster {
            id: 1,
            name: "Prod Boost".to_string(),
            booster_type: BoosterType::ProductionBoost,
            multiplier: 1.2,
            duration_hours: 24,
            cost_dm: 500,
        };
        store.activate("player1", booster, "2026-01-01T00:00:00Z");
        let mult = store.get_multiplier(
            "player1",
            &BoosterType::ResearchSpeed,
            "2026-01-01T12:00:00Z",
        );
        assert_eq!(mult, 1.0);
    }

    #[test]
    fn test_booster_store_expire() {
        let mut store = BoosterStore::new();
        let b1 = Booster {
            id: 1,
            name: "Short".to_string(),
            booster_type: BoosterType::ProductionBoost,
            multiplier: 1.1,
            duration_hours: 1,
            cost_dm: 100,
        };
        let b2 = Booster {
            id: 2,
            name: "Long".to_string(),
            booster_type: BoosterType::ResearchSpeed,
            multiplier: 1.2,
            duration_hours: 48,
            cost_dm: 500,
        };
        store.activate("player1", b1, "2026-01-01T00:00:00Z");
        store.activate("player1", b2, "2026-01-01T00:00:00Z");
        let expired = store.expire_boosters("2026-01-01T02:00:00Z");
        assert_eq!(expired, 1);
    }

    #[test]
    fn test_booster_store_expire_none() {
        let mut store = BoosterStore::new();
        let booster = Booster {
            id: 1,
            name: "Long".to_string(),
            booster_type: BoosterType::ProductionBoost,
            multiplier: 1.1,
            duration_hours: 168,
            cost_dm: 3000,
        };
        store.activate("player1", booster, "2026-01-01T00:00:00Z");
        let expired = store.expire_boosters("2026-01-02T00:00:00Z");
        assert_eq!(expired, 0);
    }

    #[test]
    fn test_booster_store_default() {
        let store = BoosterStore::default();
        assert_eq!(
            store.get_multiplier("p1", &BoosterType::ProductionBoost, "2026-01-01T00:00:00Z"),
            1.0
        );
    }

    #[test]
    fn test_booster_store_ids_increment() {
        let mut store = BoosterStore::new();
        let b1 = Booster {
            id: 10,
            name: "A".to_string(),
            booster_type: BoosterType::ProductionBoost,
            multiplier: 1.1,
            duration_hours: 24,
            cost_dm: 500,
        };
        let b2 = Booster {
            id: 20,
            name: "B".to_string(),
            booster_type: BoosterType::ResearchSpeed,
            multiplier: 1.2,
            duration_hours: 24,
            cost_dm: 500,
        };
        let a1 = store.activate("player1", b1, "2026-01-01T00:00:00Z");
        let a2 = store.activate("player1", b2, "2026-01-01T00:00:00Z");
        assert_eq!(a1.id, 1);
        assert_eq!(a2.id, 2);
    }

    // ---- Shop Items ----

    #[test]
    fn test_default_shop_items_count() {
        let items = default_shop_items();
        assert!(items.len() >= 10);
    }

    #[test]
    fn test_default_shop_items_categories() {
        let items = default_shop_items();
        let has_officers = items.iter().any(|i| i.category == ShopCategory::Officers);
        let has_boosters = items.iter().any(|i| i.category == ShopCategory::Boosters);
        let has_packs = items
            .iter()
            .any(|i| i.category == ShopCategory::ResourcePacks);
        let has_cosmetics = items.iter().any(|i| i.category == ShopCategory::Cosmetics);
        assert!(has_officers);
        assert!(has_boosters);
        assert!(has_packs);
        assert!(has_cosmetics);
    }

    #[test]
    fn test_default_shop_items_unique_ids() {
        let items = default_shop_items();
        let ids: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
        let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len());
    }

    // ---- Shop Store ----

    #[test]
    fn test_shop_store_new_empty() {
        let store = ShopStore::new();
        assert!(store.list_items().is_empty());
    }

    #[test]
    fn test_shop_store_with_defaults() {
        let store = ShopStore::with_defaults();
        assert!(store.list_items().len() >= 10);
    }

    #[test]
    fn test_shop_store_add_item() {
        let mut store = ShopStore::new();
        let item = ShopItem {
            id: "test-item".to_string(),
            name: "Test".to_string(),
            description: "A test item".to_string(),
            category: ShopCategory::Cosmetics,
            cost_dm: 100,
            is_available: true,
            metadata: serde_json::json!({}),
        };
        store.add_item(item);
        assert_eq!(store.list_items().len(), 1);
    }

    #[test]
    fn test_shop_store_get_item() {
        let store = ShopStore::with_defaults();
        let item = store.get_item("officer-commander");
        assert!(item.is_some());
        assert_eq!(item.unwrap().name, "Commander");
    }

    #[test]
    fn test_shop_store_get_item_not_found() {
        let store = ShopStore::with_defaults();
        assert!(store.get_item("nonexistent").is_none());
    }

    #[test]
    fn test_shop_store_list_by_category() {
        let store = ShopStore::with_defaults();
        let officers = store.list_by_category(&ShopCategory::Officers);
        assert!(officers.len() >= 3);
        for item in officers {
            assert_eq!(item.category, ShopCategory::Officers);
        }
    }

    #[test]
    fn test_shop_store_remove_item() {
        let mut store = ShopStore::with_defaults();
        let before = store.list_items().len();
        let removed = store.remove_item("officer-commander");
        assert!(removed);
        assert_eq!(store.list_items().len(), before - 1);
        assert!(store.get_item("officer-commander").is_none());
    }

    #[test]
    fn test_shop_store_remove_nonexistent() {
        let mut store = ShopStore::with_defaults();
        let removed = store.remove_item("nonexistent");
        assert!(!removed);
    }

    #[test]
    fn test_shop_store_update_availability() {
        let mut store = ShopStore::with_defaults();
        let updated = store.update_availability("officer-commander", false);
        assert!(updated);
        let item = store.get_item("officer-commander").unwrap();
        assert!(!item.is_available);
    }

    #[test]
    fn test_shop_store_update_availability_nonexistent() {
        let mut store = ShopStore::with_defaults();
        let updated = store.update_availability("nonexistent", false);
        assert!(!updated);
    }

    #[test]
    fn test_shop_store_default() {
        let store = ShopStore::default();
        assert!(store.list_items().is_empty());
    }

    // ---- Purchase flow structs ----

    #[test]
    fn test_purchase_request_serde() {
        let req = PurchaseRequest {
            player_id: "player1".to_string(),
            item_id: "officer-commander".to_string(),
            quantity: 1,
        };
        let json = serde_json::to_string(&req).unwrap();
        let deserialized: PurchaseRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, req);
    }

    #[test]
    fn test_purchase_result_success() {
        let result = PurchaseResult {
            success: true,
            transaction_id: Some(42),
            error: None,
            items_granted: vec!["officer-commander".to_string()],
        };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: PurchaseResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, result);
    }

    #[test]
    fn test_purchase_result_failure() {
        let result = PurchaseResult {
            success: false,
            transaction_id: None,
            error: Some("Insufficient Dark Matter balance".to_string()),
            items_granted: vec![],
        };
        assert!(!result.success);
        assert!(result.transaction_id.is_none());
    }

    // ---- PremiumError Display ----

    #[test]
    fn test_premium_error_display_insufficient() {
        let err = PremiumError::InsufficientDarkMatter;
        assert_eq!(format!("{}", err), "Insufficient Dark Matter balance");
    }

    #[test]
    fn test_premium_error_display_not_found() {
        let err = PremiumError::ItemNotFound;
        assert_eq!(format!("{}", err), "Item not found");
    }

    #[test]
    fn test_premium_error_display_unavailable() {
        let err = PremiumError::ItemUnavailable;
        assert_eq!(format!("{}", err), "Item is currently unavailable");
    }

    #[test]
    fn test_premium_error_display_invalid_quantity() {
        let err = PremiumError::InvalidQuantity;
        assert_eq!(format!("{}", err), "Invalid quantity");
    }

    #[test]
    fn test_premium_error_display_already_active() {
        let err = PremiumError::AlreadyActive;
        assert_eq!(format!("{}", err), "Item is already active");
    }

    // ---- ResourcePack ----

    #[test]
    fn test_resource_pack_serde() {
        let pack = ResourcePack {
            metal: 50000,
            crystal: 30000,
            deuterium: 10000,
        };
        let json = serde_json::to_string(&pack).unwrap();
        let deserialized: ResourcePack = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, pack);
    }

    // ---- DarkMatterBalance serde ----

    #[test]
    fn test_dark_matter_balance_serde() {
        let balance = DarkMatterBalance {
            player_id: "player1".to_string(),
            amount: 5000,
            lifetime_earned: 10000,
            lifetime_spent: 5000,
            last_updated: "2026-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&balance).unwrap();
        let deserialized: DarkMatterBalance = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, balance);
    }

    // ---- OfficerHire serde ----

    #[test]
    fn test_officer_hire_serde() {
        let hire = OfficerHire {
            id: 1,
            player_id: "player1".to_string(),
            officer_type: OfficerType::Commander,
            hired_at: "2026-01-01T00:00:00Z".to_string(),
            expires_at: "2026-01-08T00:00:00Z".to_string(),
            is_active: true,
        };
        let json = serde_json::to_string(&hire).unwrap();
        let deserialized: OfficerHire = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, hire);
    }

    // ---- ActiveBooster serde ----

    #[test]
    fn test_active_booster_serde() {
        let booster = Booster {
            id: 1,
            name: "Test".to_string(),
            booster_type: BoosterType::ProductionBoost,
            multiplier: 1.1,
            duration_hours: 24,
            cost_dm: 500,
        };
        let active = ActiveBooster {
            id: 1,
            player_id: "player1".to_string(),
            booster,
            activated_at: "2026-01-01T00:00:00Z".to_string(),
            expires_at: "2026-01-02T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&active).unwrap();
        let deserialized: ActiveBooster = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, active);
    }

    // ---- ShopItem serde ----

    #[test]
    fn test_shop_item_serde() {
        let item = ShopItem {
            id: "test".to_string(),
            name: "Test Item".to_string(),
            description: "A test".to_string(),
            category: ShopCategory::Cosmetics,
            cost_dm: 100,
            is_available: true,
            metadata: serde_json::json!({"key": "value"}),
        };
        let json = serde_json::to_string(&item).unwrap();
        let deserialized: ShopItem = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, item);
    }

    // ---- Integration-style tests ----

    #[test]
    fn test_full_purchase_flow_simulation() {
        let mut dm_store = DarkMatterStore::new();
        let mut officer_store = OfficerStore::new();
        let shop = ShopStore::with_defaults();

        // Player buys dark matter
        dm_store.credit("player1", 20000, TransactionReason::Purchase);
        assert_eq!(dm_store.get_balance("player1"), 20000);

        // Player checks shop for commander
        let item = shop.get_item("officer-commander").unwrap();
        assert_eq!(item.cost_dm, 5000);

        // Player purchases commander
        let tx = dm_store
            .debit("player1", item.cost_dm, TransactionReason::OfficerHire)
            .unwrap();
        assert_eq!(tx.balance_after, 15000);

        // Hire the officer
        let hire = officer_store.hire("player1", OfficerType::Commander, 1, "2026-01-01T00:00:00Z");
        assert!(hire.is_active);

        // Verify officer is active
        assert!(officer_store.is_officer_active(
            "player1",
            &OfficerType::Commander,
            "2026-01-05T00:00:00Z"
        ));
    }

    #[test]
    fn test_full_booster_flow_simulation() {
        let mut dm_store = DarkMatterStore::new();
        let mut booster_store = BoosterStore::new();

        dm_store.credit("player1", 5000, TransactionReason::Purchase);

        let booster = default_boosters().into_iter().next().unwrap();
        let cost = booster.cost_dm;

        // Purchase booster
        dm_store
            .debit("player1", cost, TransactionReason::BoosterPurchase)
            .unwrap();

        // Activate booster
        let active = booster_store.activate(
            "player1",
            default_boosters().into_iter().next().unwrap(),
            "2026-01-01T00:00:00Z",
        );
        assert_eq!(active.player_id, "player1");

        // Check multiplier
        let mult = booster_store.get_multiplier(
            "player1",
            &BoosterType::ProductionBoost,
            "2026-01-01T12:00:00Z",
        );
        assert!(mult > 1.0);
    }

    #[test]
    fn test_insufficient_funds_flow() {
        let mut dm_store = DarkMatterStore::new();
        dm_store.credit("player1", 1000, TransactionReason::ExpeditionReward);

        let result = dm_store.debit("player1", 5000, TransactionReason::OfficerHire);
        assert_eq!(result, Err(PremiumError::InsufficientDarkMatter));
        // Balance unchanged
        assert_eq!(dm_store.get_balance("player1"), 1000);
    }

    #[test]
    fn test_officer_all_five_hired() {
        let mut store = OfficerStore::new();
        store.hire("player1", OfficerType::Commander, 1, "2026-01-01T00:00:00Z");
        store.hire("player1", OfficerType::Admiral, 1, "2026-01-01T00:00:00Z");
        store.hire("player1", OfficerType::Engineer, 1, "2026-01-01T00:00:00Z");
        store.hire("player1", OfficerType::Geologist, 1, "2026-01-01T00:00:00Z");
        store.hire(
            "player1",
            OfficerType::Technocrat,
            1,
            "2026-01-01T00:00:00Z",
        );
        let active = store.get_active_officers("player1", "2026-01-05T00:00:00Z");
        assert_eq!(active.len(), 5);
        let bonuses = store.get_combined_bonuses("player1", "2026-01-05T00:00:00Z");
        // Commander: BuildQueueSlots
        // Admiral: FleetSlots, ReducedFleetLoss
        // Engineer: DefenseBonus, EnergyBonus
        // Geologist: MineProductionBonus
        // Technocrat: ResearchSpeedBonus
        assert_eq!(bonuses.len(), 7);
    }

    #[test]
    fn test_shop_store_list_by_resource_packs() {
        let store = ShopStore::with_defaults();
        let packs = store.list_by_category(&ShopCategory::ResourcePacks);
        assert!(packs.len() >= 3);
    }

    #[test]
    fn test_shop_store_list_by_cosmetics() {
        let store = ShopStore::with_defaults();
        let cosmetics = store.list_by_category(&ShopCategory::Cosmetics);
        assert!(cosmetics.len() >= 3);
    }

    #[test]
    fn test_dm_store_credit_refund() {
        let mut store = DarkMatterStore::new();
        store.credit("player1", 1000, TransactionReason::Purchase);
        store
            .debit("player1", 500, TransactionReason::OfficerHire)
            .unwrap();
        store.credit("player1", 500, TransactionReason::Refund);
        assert_eq!(store.get_balance("player1"), 1000);
        let record = store.get_balance_record("player1").unwrap();
        assert_eq!(record.lifetime_earned, 1500);
        assert_eq!(record.lifetime_spent, 500);
    }

    #[test]
    fn test_booster_store_different_players() {
        let mut store = BoosterStore::new();
        let b1 = Booster {
            id: 1,
            name: "A".to_string(),
            booster_type: BoosterType::ProductionBoost,
            multiplier: 1.1,
            duration_hours: 24,
            cost_dm: 500,
        };
        let b2 = Booster {
            id: 2,
            name: "B".to_string(),
            booster_type: BoosterType::ResearchSpeed,
            multiplier: 1.2,
            duration_hours: 24,
            cost_dm: 500,
        };
        store.activate("player1", b1, "2026-01-01T00:00:00Z");
        store.activate("player2", b2, "2026-01-01T00:00:00Z");
        let p1 = store.get_active_boosters("player1", "2026-01-01T12:00:00Z");
        let p2 = store.get_active_boosters("player2", "2026-01-01T12:00:00Z");
        assert_eq!(p1.len(), 1);
        assert_eq!(p2.len(), 1);
    }
}
