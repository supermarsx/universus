use std::net::SocketAddr;

use app_api_gateway::routes;

const SERVICE_NAME: &str = "app-api-gateway";
const DEFAULT_PORT: u16 = 3000;

#[tokio::main]
async fn main() {
    platform_observability::init(SERVICE_NAME);
    app_api_gateway::accounts::validate_runtime_configuration()
        .await
        .expect("invalid authentication/database runtime configuration");

    let app = routes::build_router(SERVICE_NAME);
    let addr = SocketAddr::from((
        [0, 0, 0, 0],
        platform_config::parse_u16_env("PORT", DEFAULT_PORT),
    ));
    tracing::info!(service = SERVICE_NAME, %addr, "startup");

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .expect("server failed");
}
