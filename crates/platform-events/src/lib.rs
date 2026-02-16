//! Core building blocks for the platform-events crate.

#![forbid(unsafe_code)]

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventEnvelope {
    pub event_type: String,
    pub payload: serde_json::Value,
    pub emitted_at_unix: i64,
}

pub fn build_event<T: Serialize>(event_type: &str, payload: &T) -> EventEnvelope {
    EventEnvelope {
        event_type: event_type.to_string(),
        payload: serde_json::to_value(payload).unwrap_or_else(|_| serde_json::json!({})),
        emitted_at_unix: unix_timestamp(),
    }
}

pub fn build_publish_payload(channel: &str, event: &EventEnvelope) -> serde_json::Value {
    serde_json::json!({
        "channel": channel,
        "event": serde_json::to_string(event).unwrap_or_else(|_| "{}".to_string())
    })
}

pub async fn publish_http(base_url: &str, channel: &str, event: &EventEnvelope) -> Result<u16, String> {
    let url = format!(
        "{}/api/realtime/publish",
        base_url.trim_end_matches('/')
    );
    let body = build_publish_payload(channel, event);
    let response = reqwest::Client::new()
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    Ok(response.status().as_u16())
}

/// Returns the crate name for a basic compile-time sanity check.
pub const fn crate_name() -> &'static str {
    "platform-events"
}

fn unix_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{build_event, build_publish_payload};

    #[test]
    fn build_publish_payload_contains_channel_and_event() {
        let event = build_event("scheduler.tick", &serde_json::json!({"job":"fleet"}));
        let payload = build_publish_payload("ops.scheduler", &event);
        assert_eq!(payload["channel"], "ops.scheduler");
        assert!(payload["event"].as_str().unwrap().contains("scheduler.tick"));
    }
}
