use app_privacy_worker::{
    healthcheck_from_env, spawn_privacy_service_server, DeliveryState, ExportEncryptor,
    HealthState, OpsPublisher, PrivacyWorker, ProcessorSettings, WorkerConfig, WorkerError,
    SERVICE_NAME,
};
use std::{process::ExitCode, sync::Arc};
use tokio::{
    sync::watch,
    time::{sleep, Instant},
};

#[tokio::main]
async fn main() -> ExitCode {
    if std::env::args().nth(1).as_deref() == Some("healthcheck") {
        return if healthcheck_from_env().is_ok() {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        };
    }

    platform_observability::init(SERVICE_NAME);
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(
                service = SERVICE_NAME,
                error_code = error.code(),
                "privacy worker stopped with a fatal error"
            );
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), WorkerError> {
    let config = WorkerConfig::from_env()?;
    let database = platform_db::Database::try_from_env()
        .map_err(|_| WorkerError::DatabaseConfiguration)?
        .ok_or(WorkerError::MissingConfiguration)?;
    database
        .privacy_repository_ready()
        .await
        .map_err(|_| WorkerError::RepositoryNotReady)?;

    let encryptor = ExportEncryptor::from_keyring(
        config.export_keyring.clone(),
        config.export_max_plaintext_bytes,
    )?;
    let delivery = DeliveryState::new(
        database.clone(),
        Arc::new(encryptor.clone()),
        config.export_delivery_token_ttl_seconds,
    )?;
    let ops = match (config.realtime_url, config.realtime_token) {
        (Some(url), Some(token)) => OpsPublisher::new(url, token)?,
        (None, None) => OpsPublisher::disabled(),
        _ => return Err(WorkerError::InvalidConfiguration),
    };
    let worker = PrivacyWorker::new(
        database,
        ProcessorSettings {
            worker_id: config.worker_id,
            universe_id: config.universe_id,
            claim_limit: config.claim_limit,
            claim_timeout: config.claim_timeout,
            lease_seconds: config.lease_seconds,
            job_timeout: config.job_timeout,
            retry_delay_seconds: config.retry_delay_seconds,
            export_expires_in_seconds: config.export_expires_in_seconds,
            privacy_outbox_retention_days: config.privacy_outbox_retention_days,
        },
        encryptor,
        config.communication_evidence_key.clone(),
        ops.clone(),
    )?;

    let health = HealthState::new(config.readiness_stale_after);
    health.mark_database_success();
    let health_server =
        spawn_privacy_service_server(config.health_addr, Arc::clone(&health), delivery)?;
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    tokio::spawn(async move {
        shutdown_signal().await;
        let _ = shutdown_tx.send(true);
    });

    tracing::info!(
        service = SERVICE_NAME,
        run_once = config.run_once,
        claim_limit = config.claim_limit,
        claim_timeout_seconds = config.claim_timeout.as_secs(),
        lease_seconds = config.lease_seconds,
        job_timeout_seconds = config.job_timeout.as_secs(),
        poll_interval_ms = config.poll_interval.as_millis() as u64,
        tenant_scoped = config.universe_id.is_some(),
        has_realtime_url = ops.is_enabled(),
        "privacy worker started"
    );
    ops.publish(
        "privacy.worker.started",
        &serde_json::json!({
            "claimLimit": config.claim_limit,
            "tenantScoped": config.universe_id.is_some()
        }),
    )
    .await;

    let mut next_retention = Instant::now();
    loop {
        match worker.run_cycle().await {
            Ok(report) => {
                if report.failure_unrecorded == 0 {
                    health.mark_database_success();
                } else {
                    health.mark_not_ready();
                }
                tracing::info!(
                    service = SERVICE_NAME,
                    claimed = report.claimed,
                    completed = report.completed,
                    failure_recorded = report.failure_recorded,
                    lease_lost = report.lease_lost,
                    failure_unrecorded = report.failure_unrecorded,
                    "privacy worker cycle completed"
                );
                ops.publish("privacy.worker.cycle_completed", &report).await;
            }
            Err(error) => {
                health.mark_not_ready();
                tracing::error!(
                    service = SERVICE_NAME,
                    error_code = error.code(),
                    "privacy worker cycle failed"
                );
                ops.publish(
                    "privacy.worker.cycle_failed",
                    &serde_json::json!({"errorCode": error.code()}),
                )
                .await;
                if config.run_once {
                    health.mark_shutdown();
                    health_server.shutdown().await?;
                    return Err(error);
                }
            }
        }

        if Instant::now() >= next_retention {
            match worker.run_retention().await {
                Ok(()) => {
                    tracing::info!(service = SERVICE_NAME, "privacy retention completed");
                    next_retention = Instant::now() + config.retention_interval;
                }
                Err(error) => {
                    health.mark_not_ready();
                    tracing::error!(
                        service = SERVICE_NAME,
                        error_code = error.code(),
                        "privacy retention failed"
                    );
                    if config.run_once {
                        health.mark_shutdown();
                        health_server.shutdown().await?;
                        return Err(error);
                    }
                    next_retention = Instant::now() + config.retention_interval;
                }
            }
        }

        if config.run_once || *shutdown_rx.borrow() {
            break;
        }
        tokio::select! {
            _ = sleep(config.poll_interval) => {}
            changed = shutdown_rx.changed() => {
                if changed.is_err() || *shutdown_rx.borrow() {
                    break;
                }
            }
        }
    }

    health.mark_shutdown();
    ops.publish("privacy.worker.stopped", &serde_json::json!({}))
        .await;
    health_server.shutdown().await?;
    tracing::info!(service = SERVICE_NAME, "privacy worker shutdown complete");
    Ok(())
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        if let Ok(mut terminate) = signal(SignalKind::terminate()) {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = terminate.recv() => {}
            }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}
