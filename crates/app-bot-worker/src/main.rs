use std::env;
use std::time::Duration;

use reqwest::{Client, Url};
use serde_json::Value;
use tokio::signal;
use tokio::time;
use tracing_subscriber::EnvFilter;

const DEFAULT_INTERVAL_MS: u64 = 60_000;
const DEFAULT_BOT_API_URL: &str = "http://localhost:4001";
const PROCESS_ALL_PATH: &str = "/api/admin/bots/process-all";

fn parse_interval_ms(raw: Option<&str>) -> u64 {
    raw.and_then(|value| value.parse::<u64>().ok())
        .filter(|milliseconds| *milliseconds > 0)
        .unwrap_or(DEFAULT_INTERVAL_MS)
}

fn parse_bot_api_url(raw: Option<&str>) -> String {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_BOT_API_URL)
        .to_string()
}

fn parse_optional_api_key(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn build_process_all_url(base_url: &str) -> Result<Url, String> {
    let mut parsed = Url::parse(base_url)
        .map_err(|error| format!("invalid BOT_API_URL '{base_url}': {error}"))?;
    parsed.set_path(PROCESS_ALL_PATH);
    parsed.set_query(None);
    parsed.set_fragment(None);
    Ok(parsed)
}

fn extract_processed_count(payload: &Value) -> Option<u64> {
    fn count_from_value(value: &Value) -> Option<u64> {
        const CANDIDATE_KEYS: &[&str] = &[
            "processed",
            "processed_count",
            "processedCount",
            "count",
            "total_processed",
            "totalProcessed",
        ];

        CANDIDATE_KEYS
            .iter()
            .filter_map(|key| value.get(*key))
            .find_map(Value::as_u64)
    }

    count_from_value(payload).or_else(|| payload.get("data").and_then(count_from_value))
}

struct ProcessCallOutcome {
    processed_count: Option<u64>,
    message: Option<String>,
}

async fn call_process_all_bots(
    client: &Client,
    endpoint: &Url,
    api_key: Option<&str>,
) -> Result<ProcessCallOutcome, String> {
    let mut request = client.post(endpoint.clone());
    if let Some(api_key) = api_key {
        request = request.header("x-api-key", api_key);
    }

    let response = request
        .send()
        .await
        .map_err(|error| format!("request failed: {error}"))?;

    let status = response.status();
    let payload = response.json::<Value>().await.ok();
    let message = payload.as_ref().and_then(|json| {
        json.get("message")
            .or_else(|| json.get("error"))
            .and_then(Value::as_str)
            .map(str::to_string)
    });

    if !status.is_success() {
        return Err(match message {
            Some(message) => format!("status {status}: {message}"),
            None => format!("status {status}"),
        });
    }

    if let Some(success) = payload
        .as_ref()
        .and_then(|json| json.get("success"))
        .and_then(Value::as_bool)
    {
        if !success {
            return Err(match message {
                Some(message) => format!("API reported failure: {message}"),
                None => "API reported failure".to_string(),
            });
        }
    }

    Ok(ProcessCallOutcome {
        processed_count: payload.as_ref().and_then(extract_processed_count),
        message,
    })
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let interval_ms = parse_interval_ms(env::var("BOT_WORKER_INTERVAL_MS").ok().as_deref());
    let bot_api_url = parse_bot_api_url(env::var("BOT_API_URL").ok().as_deref());
    let api_key = parse_optional_api_key(env::var("BOT_SERVICE_API_KEY").ok().as_deref());
    let endpoint = match build_process_all_url(&bot_api_url) {
        Ok(endpoint) => endpoint,
        Err(error) => {
            tracing::error!(service = "app-bot-worker", error = %error, "worker startup failed");
            return;
        }
    };
    let client = match Client::builder().timeout(Duration::from_secs(30)).build() {
        Ok(client) => client,
        Err(error) => {
            tracing::error!(
                service = "app-bot-worker",
                error = %error,
                "failed to initialize HTTP client"
            );
            return;
        }
    };
    let mut interval = time::interval(Duration::from_millis(interval_ms));

    tracing::info!(
        service = "app-bot-worker",
        interval_ms,
        endpoint = %endpoint,
        api_key_configured = api_key.is_some(),
        "worker startup"
    );

    loop {
        tokio::select! {
            _ = interval.tick() => {
                tracing::info!(service = "app-bot-worker", endpoint = %endpoint, "processing cycle started");

                match call_process_all_bots(&client, &endpoint, api_key.as_deref()).await {
                    Ok(outcome) => {
                        match outcome.processed_count {
                            Some(processed_count) => {
                                tracing::info!(
                                    service = "app-bot-worker",
                                    processed_bots = processed_count,
                                    message = ?outcome.message,
                                    "processing cycle completed"
                                );
                            }
                            None => {
                                tracing::info!(
                                    service = "app-bot-worker",
                                    message = ?outcome.message,
                                    "processing cycle completed"
                                );
                            }
                        }
                    }
                    Err(error) => {
                        tracing::error!(
                            service = "app-bot-worker",
                            endpoint = %endpoint,
                            error = %error,
                            "processing cycle failed"
                        );
                    }
                }
            }
            _ = signal::ctrl_c() => {
                tracing::info!(service = "app-bot-worker", "shutdown signal received");
                break;
            }
        }
    }

    tracing::info!(service = "app-bot-worker", "worker shutdown complete");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_interval_uses_default_for_invalid_values() {
        assert_eq!(parse_interval_ms(None), DEFAULT_INTERVAL_MS);
        assert_eq!(parse_interval_ms(Some("0")), DEFAULT_INTERVAL_MS);
        assert_eq!(parse_interval_ms(Some("abc")), DEFAULT_INTERVAL_MS);
    }

    #[test]
    fn build_process_all_url_sets_expected_path() {
        let url = build_process_all_url("http://localhost:4001/base/path").unwrap();
        assert_eq!(
            url.as_str(),
            "http://localhost:4001/api/admin/bots/process-all"
        );
    }

    #[test]
    fn extract_processed_count_supports_common_shapes() {
        let top_level = serde_json::json!({ "processedCount": 17 });
        assert_eq!(extract_processed_count(&top_level), Some(17));

        let nested = serde_json::json!({ "data": { "processed": 9 } });
        assert_eq!(extract_processed_count(&nested), Some(9));

        let missing = serde_json::json!({ "success": true });
        assert_eq!(extract_processed_count(&missing), None);
    }
}
