use tokio::time::{sleep, Duration};

const SERVICE_NAME: &str = "app-scheduler-worker";

#[tokio::main]
async fn main() {
    platform_observability::init(SERVICE_NAME);

    let run_once = bool_env("SCHEDULER_RUN_ONCE");
    let game_loop_secs = u64_env("GAME_LOOP_INTERVAL_SECS", 5);
    let fleet_secs = u64_env("FLEET_SCHEDULER_INTERVAL_SECS", 10);
    let moon_secs = u64_env("MOON_DESTROY_INTERVAL_SECS", 10);
    let shard_health_secs = u64_env("SHARD_HEALTH_INTERVAL_SECS", 60);
    let backend_url = std::env::var("RUST_BACKEND_URL")
        .ok()
        .filter(|value| !value.trim().is_empty());

    tracing::info!(
        service = SERVICE_NAME,
        run_once,
        game_loop_secs,
        fleet_secs,
        moon_secs,
        shard_health_secs,
        has_backend_url = backend_url.is_some(),
        "scheduler worker started"
    );

    let mut game_tick = tokio::time::interval(Duration::from_secs(game_loop_secs));
    let mut fleet_tick = tokio::time::interval(Duration::from_secs(fleet_secs));
    let mut moon_tick = tokio::time::interval(Duration::from_secs(moon_secs));
    let mut shard_tick = tokio::time::interval(Duration::from_secs(shard_health_secs));

    let client = reqwest::Client::new();

    loop {
        tokio::select! {
            _ = game_tick.tick() => {
                run_tick("game_loop", backend_url.as_deref(), &client, "/api/universe/1/maintenance/start").await;
            }
            _ = fleet_tick.tick() => {
                run_tick("fleet_scheduler", backend_url.as_deref(), &client, "/api/analytics/events").await;
            }
            _ = moon_tick.tick() => {
                run_tick("moon_destroy", backend_url.as_deref(), &client, "/api/rips/destroyMoon").await;
            }
            _ = shard_tick.tick() => {
                run_tick("shard_health", backend_url.as_deref(), &client, "/api/shards/health/overview").await;
            }
        }

        if run_once {
            break;
        }
    }

    // flush logs in short-lived run_once mode
    sleep(Duration::from_millis(25)).await;
}

async fn run_tick(
    job: &str,
    backend_url: Option<&str>,
    client: &reqwest::Client,
    path: &str,
) {
    tracing::info!(service = SERVICE_NAME, job, "tick start");
    if let Some(base) = backend_url {
        let url = format!("{}{}", base.trim_end_matches('/'), path);
        let response = client.get(url).send().await;
        match response {
            Ok(resp) => tracing::info!(
                service = SERVICE_NAME,
                job,
                status = resp.status().as_u16(),
                "tick request completed"
            ),
            Err(error) => tracing::warn!(service = SERVICE_NAME, job, %error, "tick request failed"),
        }
    } else {
        tracing::info!(service = SERVICE_NAME, job, "backend url missing; running in noop mode");
    }
}

fn bool_env(key: &str) -> bool {
    std::env::var(key)
        .map(|value| value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn u64_env(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}
