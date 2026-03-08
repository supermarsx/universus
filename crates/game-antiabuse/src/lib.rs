#![forbid(unsafe_code)]

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// 1. Noob Protection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NoobProtectionConfig {
    /// Defenders below this score threshold are eligible for noob protection.
    pub min_points_threshold: i64,
    /// Attacker must have more than `multiplier * defender_score` for protection to apply.
    pub multiplier: f64,
    pub enabled: bool,
}

impl Default for NoobProtectionConfig {
    fn default() -> Self {
        Self {
            min_points_threshold: 5000,
            multiplier: 5.0,
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AttackEligibility {
    Allowed,
    NoobProtected,
    StrongPlayerProtected,
    BannedPlayer,
}

/// Returns `true` when the defender is shielded by noob protection.
///
/// Protection applies when **all** of the following hold:
/// - config is enabled
/// - defender score is below `min_points_threshold`
/// - attacker score exceeds `multiplier * defender_score`
pub fn is_noob_protected(
    attacker_score: i64,
    defender_score: i64,
    config: &NoobProtectionConfig,
) -> bool {
    if !config.enabled {
        return false;
    }
    defender_score < config.min_points_threshold
        && (attacker_score as f64) > config.multiplier * (defender_score as f64)
}

/// Determines whether an attack between two players is eligible.
///
/// - If the defender is noob-protected the attacker cannot attack.
/// - If the attacker is so weak relative to the defender (`defender_score >
///   multiplier * attacker_score`) the defender is "strong-player protected"
///   (i.e. too powerful to farm the attacker — this prevents the attacker from
///   deliberately losing fleets to a stronger player to transfer debris).
pub fn can_attack(
    attacker_score: i64,
    defender_score: i64,
    config: &NoobProtectionConfig,
) -> AttackEligibility {
    if !config.enabled {
        return AttackEligibility::Allowed;
    }

    // Noob protection: defender is a small player, attacker is much stronger.
    if defender_score < config.min_points_threshold
        && (attacker_score as f64) > config.multiplier * (defender_score as f64)
    {
        return AttackEligibility::NoobProtected;
    }

    // Strong-player protection: attacker is very weak compared to defender.
    if attacker_score < config.min_points_threshold
        && (defender_score as f64) > config.multiplier * (attacker_score as f64)
    {
        return AttackEligibility::StrongPlayerProtected;
    }

    AttackEligibility::Allowed
}

// ---------------------------------------------------------------------------
// 2. Pushing Detection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PushingConfig {
    /// Maximum fraction of daily production a player may transport to a single
    /// recipient (0.0–1.0). Default 0.5 (50%).
    pub max_daily_transport_ratio: f64,
    /// Sliding window in hours for aggregation. Default 24.
    pub window_hours: i32,
    /// Players below this score are exempt from pushing checks.
    pub min_score_for_check: i64,
}

impl Default for PushingConfig {
    fn default() -> Self {
        Self {
            max_daily_transport_ratio: 0.5,
            window_hours: 24,
            min_score_for_check: 1000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransportRecord {
    pub from_id: i64,
    pub to_id: i64,
    pub metal: i64,
    pub crystal: i64,
    pub deuterium: i64,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PushingResult {
    pub is_suspicious: bool,
    pub total_sent_24h: i64,
    pub transport_count_24h: i32,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushingDetector {
    pub config: PushingConfig,
    pub records: Vec<TransportRecord>,
}

impl PushingDetector {
    pub fn new(config: PushingConfig) -> Self {
        Self {
            config,
            records: Vec::new(),
        }
    }

    pub fn record_transport(&mut self, record: TransportRecord) {
        self.records.push(record);
    }

    /// Check whether the transport flow from `from_id` to `to_id` looks
    /// suspicious within the configured window ending at `now`.
    pub fn check_pushing(&self, from_id: i64, to_id: i64, now: i64) -> PushingResult {
        let window_start = now - (self.config.window_hours as i64) * 3600;

        let mut total: i64 = 0;
        let mut count: i32 = 0;

        for r in &self.records {
            if r.from_id == from_id
                && r.to_id == to_id
                && r.timestamp >= window_start
                && r.timestamp <= now
            {
                total += r.metal + r.crystal + r.deuterium;
                count += 1;
            }
        }

        // A simple heuristic: flag when more than 3 transports in the window
        // **or** the total exceeds an absolute threshold derived from the
        // ratio (we use the ratio against a nominal daily production of
        // 100_000 resources as baseline).
        let threshold = (100_000_f64 * self.config.max_daily_transport_ratio) as i64;
        let is_suspicious = total > threshold;

        let reason = if is_suspicious {
            Some(format!(
                "Total resources sent ({total}) exceeds threshold ({threshold}) in {count} transports over {}h window",
                self.config.window_hours
            ))
        } else {
            None
        };

        PushingResult {
            is_suspicious,
            total_sent_24h: total,
            transport_count_24h: count,
            reason,
        }
    }

    /// Returns all (from_id, to_id, total_amount) pairs that exceed the
    /// pushing threshold within the window ending at `now`.
    pub fn get_suspicious_pairs(&self, now: i64) -> Vec<(i64, i64, i64)> {
        let window_start = now - (self.config.window_hours as i64) * 3600;
        let threshold = (100_000_f64 * self.config.max_daily_transport_ratio) as i64;

        // Aggregate per (from, to) pair.
        let mut agg: HashMap<(i64, i64), i64> = HashMap::new();
        for r in &self.records {
            if r.timestamp >= window_start && r.timestamp <= now {
                *agg.entry((r.from_id, r.to_id)).or_default() += r.metal + r.crystal + r.deuterium;
            }
        }

        let mut result: Vec<(i64, i64, i64)> = agg
            .into_iter()
            .filter(|&(_, total)| total > threshold)
            .map(|((from, to), total)| (from, to, total))
            .collect();

        result.sort_by_key(|&(from, to, _)| (from, to));
        result
    }
}

// ---------------------------------------------------------------------------
// 3. Rate Limiting
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RateLimitConfig {
    /// Maps action name -> (max_count, window_seconds).
    pub limits: HashMap<String, (u32, i64)>,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        let mut limits = HashMap::new();
        limits.insert("fleet_dispatch".into(), (30, 3600));
        limits.insert("message_send".into(), (10, 60));
        limits.insert("market_listing".into(), (20, 3600));
        limits.insert("espionage".into(), (50, 3600));
        Self { limits }
    }
}

/// Per-action, per-player sliding-window rate limiter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRateLimiter {
    pub config: RateLimitConfig,
    /// (player_id, action) -> list of timestamps.
    pub actions: HashMap<(i64, String), Vec<i64>>,
}

impl ActionRateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            actions: HashMap::new(),
        }
    }

    /// Returns `true` if the action is **allowed** (rate limit not exceeded).
    pub fn check_rate_limit(&self, player_id: i64, action: &str, now: i64) -> bool {
        let (max_count, window) = match self.config.limits.get(action) {
            Some(&v) => v,
            None => return true, // Unknown action → not limited
        };

        let key = (player_id, action.to_string());
        let timestamps = match self.actions.get(&key) {
            Some(ts) => ts,
            None => return true,
        };

        let window_start = now - window;
        let count = timestamps
            .iter()
            .filter(|&&t| t >= window_start && t <= now)
            .count() as u32;
        count < max_count
    }

    pub fn record_action(&mut self, player_id: i64, action: &str, now: i64) {
        self.actions
            .entry((player_id, action.to_string()))
            .or_default()
            .push(now);
    }
}

// ---------------------------------------------------------------------------
// 4. IP / Account Monitoring
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoginRecord {
    pub player_id: i64,
    pub ip_address: String,
    pub user_agent: Option<String>,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AccountMonitor {
    pub logins: Vec<LoginRecord>,
}

impl AccountMonitor {
    pub fn new() -> Self {
        Self { logins: Vec::new() }
    }

    pub fn record_login(&mut self, record: LoginRecord) {
        self.logins.push(record);
    }

    /// Returns the IDs of other players that share at least one IP address
    /// with `player_id`.
    pub fn detect_multi_account(&self, player_id: i64) -> Vec<i64> {
        // Collect all IPs used by this player.
        let player_ips: std::collections::HashSet<&str> = self
            .logins
            .iter()
            .filter(|r| r.player_id == player_id)
            .map(|r| r.ip_address.as_str())
            .collect();

        // Find other players that logged in from any of those IPs.
        let mut others: std::collections::HashSet<i64> = std::collections::HashSet::new();
        for r in &self.logins {
            if r.player_id != player_id && player_ips.contains(r.ip_address.as_str()) {
                others.insert(r.player_id);
            }
        }

        let mut result: Vec<i64> = others.into_iter().collect();
        result.sort();
        result
    }

    /// Count of distinct IP addresses used by `player_id` within the window
    /// ending at `now`.
    pub fn suspicious_ip_count(&self, player_id: i64, window_hours: i32, now: i64) -> usize {
        let window_start = now - (window_hours as i64) * 3600;
        let ips: std::collections::HashSet<&str> = self
            .logins
            .iter()
            .filter(|r| {
                r.player_id == player_id && r.timestamp >= window_start && r.timestamp <= now
            })
            .map(|r| r.ip_address.as_str())
            .collect();
        ips.len()
    }

    /// Most recent logins for the player, newest first, capped at `limit`.
    pub fn recent_logins(&self, player_id: i64, limit: usize) -> Vec<LoginRecord> {
        let mut player_logins: Vec<LoginRecord> = self
            .logins
            .iter()
            .filter(|r| r.player_id == player_id)
            .cloned()
            .collect();
        player_logins.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        player_logins.truncate(limit);
        player_logins
    }
}

// ---------------------------------------------------------------------------
// 5. Violation Tracking
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ViolationType {
    Pushing,
    MultiAccount,
    BotBehavior,
    Exploit,
    SpeedHack,
    RateLimitExceeded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Violation {
    pub id: i64,
    pub player_id: i64,
    pub violation_type: ViolationType,
    pub description: String,
    pub severity: i32,
    pub evidence: Option<String>,
    pub detected_at: String,
    pub resolved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ViolationStore {
    pub violations: Vec<Violation>,
    pub next_id: i64,
}

impl ViolationStore {
    pub fn new() -> Self {
        Self {
            violations: Vec::new(),
            next_id: 1,
        }
    }

    /// Records a violation and returns its assigned ID.
    pub fn record_violation(
        &mut self,
        player_id: i64,
        violation_type: ViolationType,
        description: String,
        severity: i32,
        evidence: Option<String>,
    ) -> i64 {
        let severity = severity.clamp(1, 5);
        let id = self.next_id;
        self.next_id += 1;

        let detected_at = format!("unix:{}", timestamp_now_secs());

        self.violations.push(Violation {
            id,
            player_id,
            violation_type,
            description,
            severity,
            evidence,
            detected_at,
            resolved: false,
        });

        id
    }

    /// List violations, optionally filtered by player and/or resolved status.
    pub fn list_violations(
        &self,
        player_id: Option<i64>,
        resolved: Option<bool>,
    ) -> Vec<Violation> {
        self.violations
            .iter()
            .filter(|v| player_id.map_or(true, |pid| v.player_id == pid))
            .filter(|v| resolved.map_or(true, |r| v.resolved == r))
            .cloned()
            .collect()
    }

    /// Mark a violation as resolved. Returns `true` if the violation existed.
    pub fn resolve_violation(&mut self, id: i64) -> bool {
        if let Some(v) = self.violations.iter_mut().find(|v| v.id == id) {
            v.resolved = true;
            true
        } else {
            false
        }
    }

    /// Total number of violations (resolved and unresolved) for a player.
    pub fn player_violation_count(&self, player_id: i64) -> usize {
        self.violations
            .iter()
            .filter(|v| v.player_id == player_id)
            .count()
    }

    /// Returns `true` when the player has **3 or more** unresolved violations
    /// with severity >= 3, indicating they should be auto-banned.
    pub fn should_auto_ban(&self, player_id: i64) -> bool {
        let severe_unresolved = self
            .violations
            .iter()
            .filter(|v| v.player_id == player_id && !v.resolved && v.severity >= 3)
            .count();
        severe_unresolved >= 3
    }
}

// ---------------------------------------------------------------------------
// 6. Behavioral Analysis
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BehaviorMetrics {
    pub actions_per_minute: f64,
    pub average_session_length_minutes: f64,
    pub fleet_dispatch_regularity: f64,
    pub is_likely_bot: bool,
}

/// Analyse a sequence of `(action_name, timestamp)` pairs for bot-like
/// patterns.
///
/// Bot indicators:
/// - Actions per minute exceeding 20 (inhuman speed).
/// - Fleet dispatch interval regularity > 0.9 (too consistent timing).
pub fn analyze_behavior(actions: &[(String, i64)]) -> BehaviorMetrics {
    if actions.is_empty() {
        return BehaviorMetrics {
            actions_per_minute: 0.0,
            average_session_length_minutes: 0.0,
            fleet_dispatch_regularity: 0.0,
            is_likely_bot: false,
        };
    }

    // Sort by timestamp for analysis.
    let mut sorted: Vec<(String, i64)> = actions.to_vec();
    sorted.sort_by_key(|(_, t)| *t);

    // ---- Actions per minute ----
    let first_ts = sorted.first().unwrap().1;
    let last_ts = sorted.last().unwrap().1;
    let duration_secs = (last_ts - first_ts).max(1) as f64;
    let duration_minutes = duration_secs / 60.0;
    let actions_per_minute = sorted.len() as f64 / duration_minutes;

    // ---- Session length approximation ----
    // A "session" is a contiguous run of actions where gaps are < 30 min.
    let session_gap_threshold = 30 * 60; // 30 minutes
    let mut session_lengths: Vec<f64> = Vec::new();
    let mut session_start = sorted[0].1;
    let mut session_end = sorted[0].1;

    for &(_, t) in &sorted[1..] {
        if t - session_end > session_gap_threshold {
            // Close previous session.
            session_lengths.push((session_end - session_start) as f64 / 60.0);
            session_start = t;
        }
        session_end = t;
    }
    // Close last session.
    session_lengths.push((session_end - session_start) as f64 / 60.0);

    let average_session_length_minutes = if session_lengths.is_empty() {
        0.0
    } else {
        session_lengths.iter().sum::<f64>() / session_lengths.len() as f64
    };

    // ---- Fleet dispatch regularity ----
    // Compute coefficient of variation of intervals between fleet_dispatch
    // actions. A low CV (high regularity score) is suspicious.
    let fleet_timestamps: Vec<i64> = sorted
        .iter()
        .filter(|(a, _)| a == "fleet_dispatch")
        .map(|(_, t)| *t)
        .collect();

    let fleet_dispatch_regularity = if fleet_timestamps.len() >= 2 {
        let intervals: Vec<f64> = fleet_timestamps
            .windows(2)
            .map(|w| (w[1] - w[0]) as f64)
            .collect();
        let mean = intervals.iter().sum::<f64>() / intervals.len() as f64;
        if mean <= 0.0 {
            1.0 // All at the same time → maximally regular
        } else {
            let variance =
                intervals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / intervals.len() as f64;
            let std_dev = variance.sqrt();
            let cv = std_dev / mean; // coefficient of variation
                                     // Convert CV to a regularity score in [0, 1]: lower CV → higher regularity.
            (1.0 - cv.min(1.0)).max(0.0)
        }
    } else {
        0.0
    };

    let is_likely_bot = actions_per_minute > 20.0 || fleet_dispatch_regularity > 0.9;

    BehaviorMetrics {
        actions_per_minute: (actions_per_minute * 100.0).round() / 100.0,
        average_session_length_minutes: (average_session_length_minutes * 100.0).round() / 100.0,
        fleet_dispatch_regularity: (fleet_dispatch_regularity * 1000.0).round() / 1000.0,
        is_likely_bot,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn timestamp_now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Noob Protection ---------------------------------------------------

    #[test]
    fn noob_protection_basic() {
        let cfg = NoobProtectionConfig::default();
        // Attacker 30000 vs defender 2000: attacker > 5*2000=10000 and defender < 5000.
        assert!(is_noob_protected(30_000, 2_000, &cfg));
    }

    #[test]
    fn noob_protection_not_triggered_when_defender_above_threshold() {
        let cfg = NoobProtectionConfig::default();
        // Defender has 6000 points → above 5000 threshold.
        assert!(!is_noob_protected(100_000, 6_000, &cfg));
    }

    #[test]
    fn noob_protection_disabled() {
        let cfg = NoobProtectionConfig {
            enabled: false,
            ..Default::default()
        };
        assert!(!is_noob_protected(100_000, 100, &cfg));
    }

    #[test]
    fn noob_protection_not_triggered_when_scores_close() {
        let cfg = NoobProtectionConfig::default();
        // Attacker 4000 vs defender 3000: 4000 < 5*3000=15000 → not protected.
        assert!(!is_noob_protected(4_000, 3_000, &cfg));
    }

    #[test]
    fn can_attack_strong_player_protection() {
        let cfg = NoobProtectionConfig::default();
        // Attacker 500 (< 5000 threshold), defender 50_000 (> 5 * 500).
        assert_eq!(
            can_attack(500, 50_000, &cfg),
            AttackEligibility::StrongPlayerProtected,
        );
    }

    #[test]
    fn can_attack_allowed() {
        let cfg = NoobProtectionConfig::default();
        assert_eq!(can_attack(10_000, 8_000, &cfg), AttackEligibility::Allowed,);
    }

    // -- Pushing Detection -------------------------------------------------

    #[test]
    fn pushing_not_suspicious_small_transports() {
        let mut det = PushingDetector::new(PushingConfig::default());
        det.record_transport(TransportRecord {
            from_id: 1,
            to_id: 2,
            metal: 1000,
            crystal: 500,
            deuterium: 200,
            timestamp: 100,
        });
        let result = det.check_pushing(1, 2, 200);
        assert!(!result.is_suspicious);
        assert_eq!(result.total_sent_24h, 1700);
        assert_eq!(result.transport_count_24h, 1);
    }

    #[test]
    fn pushing_suspicious_large_transports() {
        let mut det = PushingDetector::new(PushingConfig::default());
        // Send a large amount that exceeds 50% of 100k = 50k threshold.
        det.record_transport(TransportRecord {
            from_id: 1,
            to_id: 2,
            metal: 30_000,
            crystal: 15_000,
            deuterium: 10_000,
            timestamp: 100,
        });
        let result = det.check_pushing(1, 2, 200);
        assert!(result.is_suspicious);
        assert!(result.reason.is_some());
    }

    #[test]
    fn pushing_window_expires_old_records() {
        let mut det = PushingDetector::new(PushingConfig::default());
        // Record is older than 24h window.
        det.record_transport(TransportRecord {
            from_id: 1,
            to_id: 2,
            metal: 100_000,
            crystal: 0,
            deuterium: 0,
            timestamp: 0,
        });
        // `now` is 100_000 → window starts at 100_000 - 86400 = 13_600.
        let result = det.check_pushing(1, 2, 100_000);
        assert!(!result.is_suspicious);
        assert_eq!(result.total_sent_24h, 0);
    }

    #[test]
    fn get_suspicious_pairs_returns_correct_set() {
        let mut det = PushingDetector::new(PushingConfig::default());
        let now = 1000;
        // Pair (1,2): exceeds threshold.
        det.record_transport(TransportRecord {
            from_id: 1,
            to_id: 2,
            metal: 60_000,
            crystal: 0,
            deuterium: 0,
            timestamp: now - 100,
        });
        // Pair (3,4): small, under threshold.
        det.record_transport(TransportRecord {
            from_id: 3,
            to_id: 4,
            metal: 100,
            crystal: 0,
            deuterium: 0,
            timestamp: now - 100,
        });

        let pairs = det.get_suspicious_pairs(now);
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0], (1, 2, 60_000));
    }

    // -- Rate Limiting -----------------------------------------------------

    #[test]
    fn rate_limit_allows_under_limit() {
        let mut limiter = ActionRateLimiter::new(RateLimitConfig::default());
        let now = 10_000;
        for i in 0..29 {
            limiter.record_action(1, "fleet_dispatch", now + i);
        }
        assert!(limiter.check_rate_limit(1, "fleet_dispatch", now + 30));
    }

    #[test]
    fn rate_limit_blocks_over_limit() {
        let mut limiter = ActionRateLimiter::new(RateLimitConfig::default());
        let now = 10_000;
        // fleet_dispatch limit: 30 per 3600s
        for i in 0..30 {
            limiter.record_action(1, "fleet_dispatch", now + i);
        }
        assert!(!limiter.check_rate_limit(1, "fleet_dispatch", now + 31));
    }

    #[test]
    fn rate_limit_unknown_action_allowed() {
        let limiter = ActionRateLimiter::new(RateLimitConfig::default());
        assert!(limiter.check_rate_limit(1, "unknown_action", 0));
    }

    // -- Account Monitoring ------------------------------------------------

    #[test]
    fn detect_multi_account_shared_ip() {
        let mut monitor = AccountMonitor::new();
        monitor.record_login(LoginRecord {
            player_id: 1,
            ip_address: "10.0.0.1".into(),
            user_agent: None,
            timestamp: 100,
        });
        monitor.record_login(LoginRecord {
            player_id: 2,
            ip_address: "10.0.0.1".into(),
            user_agent: None,
            timestamp: 200,
        });
        monitor.record_login(LoginRecord {
            player_id: 3,
            ip_address: "192.168.1.1".into(),
            user_agent: None,
            timestamp: 300,
        });

        let others = monitor.detect_multi_account(1);
        assert_eq!(others, vec![2]);
    }

    #[test]
    fn suspicious_ip_count_windowed() {
        let mut monitor = AccountMonitor::new();
        let now = 100_000;
        // 3 unique IPs within 24h window.
        monitor.record_login(LoginRecord {
            player_id: 1,
            ip_address: "1.1.1.1".into(),
            user_agent: None,
            timestamp: now - 1000,
        });
        monitor.record_login(LoginRecord {
            player_id: 1,
            ip_address: "2.2.2.2".into(),
            user_agent: None,
            timestamp: now - 500,
        });
        monitor.record_login(LoginRecord {
            player_id: 1,
            ip_address: "3.3.3.3".into(),
            user_agent: None,
            timestamp: now - 100,
        });
        // Old login, outside window.
        monitor.record_login(LoginRecord {
            player_id: 1,
            ip_address: "4.4.4.4".into(),
            user_agent: None,
            timestamp: 0,
        });

        assert_eq!(monitor.suspicious_ip_count(1, 24, now), 3);
    }

    #[test]
    fn recent_logins_respects_limit_and_order() {
        let mut monitor = AccountMonitor::new();
        for i in 0..10 {
            monitor.record_login(LoginRecord {
                player_id: 1,
                ip_address: format!("10.0.0.{i}"),
                user_agent: None,
                timestamp: i as i64,
            });
        }
        let recent = monitor.recent_logins(1, 3);
        assert_eq!(recent.len(), 3);
        // Newest first.
        assert_eq!(recent[0].timestamp, 9);
        assert_eq!(recent[1].timestamp, 8);
        assert_eq!(recent[2].timestamp, 7);
    }

    // -- Violation Tracking ------------------------------------------------

    #[test]
    fn record_and_list_violations() {
        let mut store = ViolationStore::new();
        let id = store.record_violation(
            1,
            ViolationType::Pushing,
            "Excessive resource transfer".into(),
            4,
            Some("transport log".into()),
        );
        assert_eq!(id, 1);
        assert_eq!(store.player_violation_count(1), 1);

        let list = store.list_violations(Some(1), None);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].violation_type, ViolationType::Pushing);
    }

    #[test]
    fn resolve_violation_works() {
        let mut store = ViolationStore::new();
        let id = store.record_violation(1, ViolationType::Exploit, "Bug exploit".into(), 5, None);
        assert!(store.resolve_violation(id));
        let list = store.list_violations(Some(1), Some(true));
        assert_eq!(list.len(), 1);

        // Non-existent id returns false.
        assert!(!store.resolve_violation(999));
    }

    #[test]
    fn should_auto_ban_triggers_correctly() {
        let mut store = ViolationStore::new();
        // 2 severe violations → not banned yet.
        store.record_violation(1, ViolationType::BotBehavior, "".into(), 3, None);
        store.record_violation(1, ViolationType::SpeedHack, "".into(), 4, None);
        assert!(!store.should_auto_ban(1));

        // 3rd severe violation → auto-ban.
        store.record_violation(1, ViolationType::MultiAccount, "".into(), 3, None);
        assert!(store.should_auto_ban(1));
    }

    #[test]
    fn should_auto_ban_ignores_low_severity() {
        let mut store = ViolationStore::new();
        // 3 low-severity violations should NOT trigger auto-ban.
        store.record_violation(1, ViolationType::RateLimitExceeded, "".into(), 1, None);
        store.record_violation(1, ViolationType::RateLimitExceeded, "".into(), 2, None);
        store.record_violation(1, ViolationType::RateLimitExceeded, "".into(), 2, None);
        assert!(!store.should_auto_ban(1));
    }

    #[test]
    fn should_auto_ban_ignores_resolved() {
        let mut store = ViolationStore::new();
        let id1 = store.record_violation(1, ViolationType::BotBehavior, "".into(), 5, None);
        store.record_violation(1, ViolationType::SpeedHack, "".into(), 4, None);
        store.record_violation(1, ViolationType::MultiAccount, "".into(), 3, None);
        // Resolve one → only 2 unresolved severe → no auto-ban.
        store.resolve_violation(id1);
        assert!(!store.should_auto_ban(1));
    }

    // -- Behavioral Analysis -----------------------------------------------

    #[test]
    fn behavior_analysis_detects_bot() {
        // 100 fleet dispatches every 10 seconds → perfectly regular intervals.
        let actions: Vec<(String, i64)> = (0..100)
            .map(|i| ("fleet_dispatch".to_string(), 1000 + i * 10))
            .collect();

        let metrics = analyze_behavior(&actions);
        // 100 actions over 990s ≈ 6.06 actions/min.
        assert!(metrics.fleet_dispatch_regularity > 0.9);
        assert!(metrics.is_likely_bot);
    }

    #[test]
    fn behavior_analysis_normal_player() {
        // Irregular intervals, mix of actions, human-like pacing.
        let actions: Vec<(String, i64)> = vec![
            ("login".into(), 0),
            ("fleet_dispatch".into(), 120),
            ("build".into(), 300),
            ("fleet_dispatch".into(), 900),
            ("research".into(), 1100),
            ("fleet_dispatch".into(), 2500),
            ("logout".into(), 3000),
        ];

        let metrics = analyze_behavior(&actions);
        assert!(!metrics.is_likely_bot);
        // Regularity should be moderate-to-low with varying intervals.
        assert!(metrics.fleet_dispatch_regularity < 0.9);
    }

    #[test]
    fn behavior_analysis_empty_actions() {
        let metrics = analyze_behavior(&[]);
        assert_eq!(metrics.actions_per_minute, 0.0);
        assert!(!metrics.is_likely_bot);
    }

    // -- Serde round-trip --------------------------------------------------

    #[test]
    fn serde_roundtrip_violation_type() {
        let vt = ViolationType::SpeedHack;
        let json = serde_json::to_string(&vt).unwrap();
        let back: ViolationType = serde_json::from_str(&json).unwrap();
        assert_eq!(vt, back);
    }

    #[test]
    fn serde_roundtrip_noob_protection_config() {
        let cfg = NoobProtectionConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let back: NoobProtectionConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, back);
    }
}
