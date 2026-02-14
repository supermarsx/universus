use std::collections::HashMap;

pub const DEFAULT_CHANNEL_FAILURE_THRESHOLD: u64 = 3;
pub const DEFAULT_CHANNEL_COOLDOWN_MS: u128 = 30_000;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChannelCircuitState {
    pub consecutive_failures: u64,
    pub open_until_ms: Option<u128>,
}

#[derive(Clone, Debug)]
pub struct CircuitBreaker {
    failure_threshold: u64,
    cooldown_ms: u128,
    channels: HashMap<String, ChannelCircuitState>,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u64, cooldown_ms: u128) -> Self {
        Self {
            failure_threshold,
            cooldown_ms,
            channels: HashMap::new(),
        }
    }

    pub fn from_env() -> Self {
        let failure_threshold = std::env::var("SMS_CHANNEL_FAILURE_THRESHOLD")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(DEFAULT_CHANNEL_FAILURE_THRESHOLD);

        let cooldown_ms = std::env::var("SMS_CHANNEL_COOLDOWN_MS")
            .ok()
            .and_then(|value| value.parse::<u128>().ok())
            .unwrap_or(DEFAULT_CHANNEL_COOLDOWN_MS);

        Self::new(failure_threshold, cooldown_ms)
    }

    pub fn failure_threshold(&self) -> u64 {
        self.failure_threshold
    }

    pub fn cooldown_ms(&self) -> u128 {
        self.cooldown_ms
    }

    pub fn record_failure(&mut self, channel: &str, now_ms: u128) {
        if self.failure_threshold == 0 {
            return;
        }

        let state = self.channels.entry(channel.to_string()).or_default();
        state.consecutive_failures = state.consecutive_failures.saturating_add(1);

        if state.consecutive_failures >= self.failure_threshold {
            state.consecutive_failures = 0;
            state.open_until_ms = Some(now_ms.saturating_add(self.cooldown_ms));
        }
    }

    pub fn record_success(&mut self, channel: &str) {
        self.reset(channel);
    }

    pub fn reset(&mut self, channel: &str) {
        if let Some(state) = self.channels.get_mut(channel) {
            state.consecutive_failures = 0;
            state.open_until_ms = None;
        }
    }

    pub fn is_open(&mut self, channel: &str, now_ms: u128) -> bool {
        let Some(state) = self.channels.get_mut(channel) else {
            return false;
        };

        let Some(open_until_ms) = state.open_until_ms else {
            return false;
        };

        if open_until_ms > now_ms {
            return true;
        }

        state.open_until_ms = None;
        state.consecutive_failures = 0;
        false
    }

    pub fn state(&self, channel: &str) -> Option<&ChannelCircuitState> {
        self.channels.get(channel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_after_threshold_and_closes_after_cooldown() {
        let mut breaker = CircuitBreaker::new(2, 50);

        breaker.record_failure("sms_twilio", 1000);
        assert_eq!(
            breaker
                .state("sms_twilio")
                .expect("channel state")
                .consecutive_failures,
            1
        );
        assert!(!breaker.is_open("sms_twilio", 1000));

        breaker.record_failure("sms_twilio", 1010);
        assert!(breaker.is_open("sms_twilio", 1011));
        assert!(!breaker.is_open("sms_twilio", 1060));
        assert!(!breaker.is_open("sms_twilio", 1061));
    }

    #[test]
    fn threshold_zero_disables_opening() {
        let mut breaker = CircuitBreaker::new(0, 50);

        breaker.record_failure("sms_twilio", 1000);
        breaker.record_failure("sms_twilio", 1010);

        assert!(!breaker.is_open("sms_twilio", 1011));
        assert!(breaker.state("sms_twilio").is_none());
    }

    #[test]
    fn success_resets_failure_state() {
        let mut breaker = CircuitBreaker::new(3, 50);

        breaker.record_failure("sms_twilio", 1000);
        breaker.record_failure("sms_twilio", 1010);
        assert_eq!(
            breaker
                .state("sms_twilio")
                .expect("channel state")
                .consecutive_failures,
            2
        );

        breaker.record_success("sms_twilio");
        let state = breaker.state("sms_twilio").expect("channel state");
        assert_eq!(state.consecutive_failures, 0);
        assert_eq!(state.open_until_ms, None);
    }
}
