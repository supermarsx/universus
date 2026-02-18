use adapter_db::bootstrap_from_json;
use anyhow::Result;
use serde_json::json;
use std::env::temp_dir;
use std::time::SystemTime;
use testcontainers::core::WaitFor;
use testcontainers::runners::AsyncRunner;
use testcontainers::GenericImage;

#[tokio::test]
async fn postgres_adapter_executes_script_and_logs() -> Result<()> {
    let unique = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = temp_dir().join(format!("adapter-db-postgres-{unique}"));
    let log_path = dir.join("tenant-pg.log.sql");
    tokio::fs::create_dir_all(&dir).await?;

    let container = match GenericImage::new("postgres", "15")
        .with_env_var("POSTGRES_PASSWORD", "docker")
        .with_wait_for(WaitFor::message_on_stdout(
            "database system is ready to accept connections",
        ))
        .with_exposed_port(5432)
        .start()
        .await
    {
        Ok(container) => container,
        Err(err) => {
            eprintln!("Skipping Postgres integration test: {err}");
            return Ok(());
        }
    };
    let port = container.get_host_port_ipv4(5432).await?;
    let url = format!("postgres://postgres:docker@127.0.0.1:{port}/postgres");

    let config = json!([
        {
            "name": "pg-adapter",
            "driver": "postgres",
            "tenant": "tenant-pg",
            "url": url,
            "logPath": log_path.to_string_lossy()
        }
    ])
    .to_string();

    let registry = bootstrap_from_json(&config).await?;
    let adapter = registry
        .get("pg-adapter")
        .await
        .expect("adapter registered");

    adapter
        .execute_script(
            "CREATE TABLE mig_pg (id SERIAL PRIMARY KEY);\nINSERT INTO mig_pg (id) VALUES (1);",
        )
        .await?;

    let contents = tokio::fs::read_to_string(&log_path).await?;
    assert!(contents.contains("CREATE TABLE"));

    Ok(())
}

#[tokio::test]
async fn mysql_adapter_executes_script_and_logs() -> Result<()> {
    let unique = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = temp_dir().join(format!("adapter-db-mysql-{unique}"));
    let log_path = dir.join("tenant-mysql.log.sql");
    tokio::fs::create_dir_all(&dir).await?;

    let container = match GenericImage::new("mysql", "8")
        .with_env_var("MYSQL_ROOT_PASSWORD", "root")
        .with_env_var("MYSQL_DATABASE", "mysql")
        .with_wait_for(WaitFor::message_on_stdout("ready for connections"))
        .with_exposed_port(3306)
        .start()
        .await
    {
        Ok(container) => container,
        Err(err) => {
            eprintln!("Skipping MySQL integration test: {err}");
            return Ok(());
        }
    };
    let port = container.get_host_port_ipv4(3306).await?;
    let url = format!("mysql://root:root@127.0.0.1:{port}/mysql");

    let config = json!([
        {
            "name": "mysql-adapter",
            "driver": "mysql",
            "tenant": "tenant-mysql",
            "url": url,
            "logPath": log_path.to_string_lossy()
        }
    ])
    .to_string();

    let registry = bootstrap_from_json(&config).await?;
    let adapter = registry
        .get("mysql-adapter")
        .await
        .expect("adapter registered");

    adapter
        .execute_script(
            "CREATE TABLE mig_mysql (id INT PRIMARY KEY AUTO_INCREMENT);\nINSERT INTO mig_mysql (id) VALUES (1);",
        )
        .await?;

    let contents = tokio::fs::read_to_string(&log_path).await?;
    assert!(contents.contains("CREATE TABLE"));

    Ok(())
}
