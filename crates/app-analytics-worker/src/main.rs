use std::env;

use futures_util::StreamExt;
use lapin::options::{
    BasicAckOptions, BasicConsumeOptions, BasicNackOptions, QueueDeclareOptions,
};
use lapin::types::FieldTable;
use lapin::{Channel, Connection, ConnectionProperties};
use platform_db::Database;
use serde::Deserialize;
use serde_json::Value;
use tokio::signal;
use tracing_subscriber::EnvFilter;

const DEFAULT_ANALYTICS_QUEUE_NAME: &str = "analytics_events";

#[derive(Debug, Deserialize)]
struct AnalyticsMessage {
    #[serde(alias = "eventType", alias = "event_type")]
    event_type: String,
    #[serde(default, alias = "userId", alias = "user_id")]
    user_id: Option<i64>,
    #[serde(default, alias = "sessionId", alias = "session_id")]
    session_id: Option<String>,
    #[serde(default)]
    properties: Option<Value>,
    #[serde(default, alias = "userAgent", alias = "user_agent")]
    user_agent: Option<String>,
    #[serde(default, alias = "ipAddress", alias = "ip_address")]
    ip_address: Option<String>,
}

fn parse_bool(raw: Option<&str>) -> bool {
    matches!(
        raw.map(str::trim)
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("1" | "true" | "yes" | "on")
    )
}

fn parse_queue_name(raw: Option<&str>, default_name: &'static str) -> String {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default_name)
        .to_string()
}

async fn nack_without_requeue(channel: &Channel, delivery_tag: u64) {
    if let Err(error) = channel
        .basic_nack(
            delivery_tag,
            BasicNackOptions {
                multiple: false,
                requeue: false,
            },
        )
        .await
    {
        tracing::error!(
            service = "app-analytics-worker",
            delivery_tag,
            error = %error,
            "failed to nack message"
        );
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let queue_disabled = parse_bool(env::var("ANALYTICS_QUEUE_DISABLED").ok().as_deref());
    let queue_name = parse_queue_name(
        env::var("ANALYTICS_QUEUE_NAME").ok().as_deref(),
        DEFAULT_ANALYTICS_QUEUE_NAME,
    );
    let rabbitmq_url = env::var("RABBITMQ_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    if queue_disabled {
        tracing::info!(
            service = "app-analytics-worker",
            queue_name = %queue_name,
            "analytics queue is disabled, exiting"
        );
        tracing::info!(service = "app-analytics-worker", "worker shutdown complete");
        return;
    }

    let Some(rabbitmq_url) = rabbitmq_url else {
        tracing::info!(
            service = "app-analytics-worker",
            queue_name = %queue_name,
            "RABBITMQ_URL is not configured, analytics queue consumer is disabled"
        );
        tracing::info!(service = "app-analytics-worker", "worker shutdown complete");
        return;
    };

    let database = Database::from_env();

    let connection = match Connection::connect(&rabbitmq_url, ConnectionProperties::default()).await
    {
        Ok(connection) => connection,
        Err(error) => {
            tracing::error!(
                service = "app-analytics-worker",
                queue_name = %queue_name,
                error = %error,
                "failed to connect to rabbitmq"
            );
            return;
        }
    };

    let channel = match connection.create_channel().await {
        Ok(channel) => channel,
        Err(error) => {
            tracing::error!(
                service = "app-analytics-worker",
                queue_name = %queue_name,
                error = %error,
                "failed to create rabbitmq channel"
            );
            return;
        }
    };

    if let Err(error) = channel
        .queue_declare(
            &queue_name,
            QueueDeclareOptions {
                durable: true,
                ..QueueDeclareOptions::default()
            },
            FieldTable::default(),
        )
        .await
    {
        tracing::error!(
            service = "app-analytics-worker",
            queue_name = %queue_name,
            error = %error,
            "failed to declare analytics queue"
        );
        return;
    }

    let mut consumer = match channel
        .basic_consume(
            &queue_name,
            "app-analytics-worker",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await
    {
        Ok(consumer) => consumer,
        Err(error) => {
            tracing::error!(
                service = "app-analytics-worker",
                queue_name = %queue_name,
                error = %error,
                "failed to start analytics queue consumer"
            );
            return;
        }
    };

    tracing::info!(
        service = "app-analytics-worker",
        queue_name = %queue_name,
        "worker startup"
    );

    loop {
        tokio::select! {
            _ = signal::ctrl_c() => {
                tracing::info!(service = "app-analytics-worker", "shutdown signal received");
                break;
            }
            delivery = consumer.next() => {
                let Some(delivery) = delivery else {
                    tracing::warn!(
                        service = "app-analytics-worker",
                        "analytics consumer stream ended"
                    );
                    break;
                };

                let delivery = match delivery {
                    Ok(delivery) => delivery,
                    Err(error) => {
                        tracing::error!(
                            service = "app-analytics-worker",
                            error = %error,
                            "rabbitmq delivery error"
                        );
                        continue;
                    }
                };

                let parsed = serde_json::from_slice::<AnalyticsMessage>(&delivery.data);
                let message = match parsed {
                    Ok(message) => message,
                    Err(error) => {
                        tracing::error!(
                            service = "app-analytics-worker",
                            delivery_tag = delivery.delivery_tag,
                            error = %error,
                            "invalid analytics payload"
                        );
                        nack_without_requeue(&channel, delivery.delivery_tag).await;
                        continue;
                    }
                };

                let event_type = message.event_type.trim();
                if event_type.is_empty() {
                    tracing::error!(
                        service = "app-analytics-worker",
                        delivery_tag = delivery.delivery_tag,
                        "analytics payload missing event type"
                    );
                    nack_without_requeue(&channel, delivery.delivery_tag).await;
                    continue;
                }

                let Some(database) = database.as_ref() else {
                    tracing::error!(
                        service = "app-analytics-worker",
                        delivery_tag = delivery.delivery_tag,
                        "database is not configured; cannot persist analytics event"
                    );
                    nack_without_requeue(&channel, delivery.delivery_tag).await;
                    continue;
                };

                match database
                    .track_analytics_event_detailed(
                        event_type,
                        message.session_id.as_deref(),
                        message.properties,
                        message.user_id,
                        message.user_agent.as_deref(),
                        message.ip_address.as_deref(),
                    )
                    .await
                {
                    Ok(_) => {
                        if let Err(error) = channel
                            .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
                            .await
                        {
                            tracing::error!(
                                service = "app-analytics-worker",
                                delivery_tag = delivery.delivery_tag,
                                error = %error,
                                "failed to ack analytics event"
                            );
                        }
                    }
                    Err(error) => {
                        tracing::error!(
                            service = "app-analytics-worker",
                            delivery_tag = delivery.delivery_tag,
                            error = %error,
                            "failed to persist analytics event"
                        );
                        nack_without_requeue(&channel, delivery.delivery_tag).await;
                    }
                }
            }
        }
    }

    tracing::info!(service = "app-analytics-worker", "worker shutdown complete");
}
