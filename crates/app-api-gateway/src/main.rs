#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    tracing::info!(service = "app-api-gateway", "startup");
}