#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    tracing::info!(service = "app-admin-api", "startup");
}