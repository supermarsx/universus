#[tokio::main]
async fn main() {
    app_realtime_gateway::serve().await;
}
