//! Shared observability bootstrapping.

use platform_consensus::LeaseCoordinator;
use tracing_subscriber::EnvFilter;

/// Initializes tracing with an env filter and logs startup metadata.
pub fn init(service_name: &str) {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let _ = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .try_init();

    tracing::info!(service = service_name, "observability initialized");
}

/// Emit lease metrics/events from a consensus coordinator into tracing logs.
pub async fn emit_consensus_snapshot(
    service_name: &str,
    coordinator: &LeaseCoordinator,
    event_limit: usize,
) {
    let metrics = coordinator.metrics_snapshot().await;
    tracing::info!(
        service = service_name,
        acquired = metrics.acquired,
        acquire_failed = metrics.acquire_failed,
        renewed = metrics.renewed,
        released = metrics.released,
        release_rejected = metrics.release_rejected,
        expired = metrics.expired,
        "consensus lease metrics snapshot"
    );

    for event in coordinator.recent_events(event_limit).await {
        tracing::debug!(
            service = service_name,
            kind = ?event.kind,
            resource = %event.resource,
            owner = %event.owner,
            observed_at = ?event.observed_at,
            "consensus lease event"
        );
    }
}
