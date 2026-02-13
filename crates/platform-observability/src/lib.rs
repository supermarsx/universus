//! Shared observability bootstrapping.

use tracing_subscriber::EnvFilter;

/// Initializes tracing with an env filter and logs startup metadata.
pub fn init(service_name: &str) {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let _ = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .try_init();

    tracing::info!(service = service_name, "observability initialized");
}
