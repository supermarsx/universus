#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    tracing::info!(service = "app-sms-api", "startup");
}