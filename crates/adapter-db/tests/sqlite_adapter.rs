use adapter_db::{bootstrap_from_json, export_migration_snapshot, import_migration_snapshot};
use anyhow::Result;
use rusqlite::Connection;
use serde_json::json;
use std::env::temp_dir;
use std::time::SystemTime;
use tokio::io::AsyncWriteExt;

#[tokio::test]
async fn sqlite_adapter_executes_script() -> Result<()> {
    let unique = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = temp_dir().join(format!("adapter-db-sqlite-{unique}"));
    let _ = tokio::fs::create_dir_all(&dir).await;
    let db_path = dir.join("tenant.db");

    let config = json!([
        {
            "name": "local-sqlite",
            "driver": "sqlite",
            "tenant": "sqlite-tenant",
            "path": db_path.to_string_lossy()
        }
    ])
    .to_string();

    let registry: adapter_db::AdapterRegistry = bootstrap_from_json(&config).await?;
    let adapter = registry
        .get("local-sqlite")
        .await
        .expect("adapter registered");

    adapter
        .execute_script(
            "CREATE TABLE IF NOT EXISTS debug (id INTEGER PRIMARY KEY);\nINSERT INTO debug (id) VALUES (1);",
        )
        .await?;

    let conn = Connection::open(db_path)?;
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM debug;", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1);

    Ok(())
}

#[tokio::test]
async fn export_snapshot_and_import_between_json_and_sqlite() -> Result<()> {
    let unique = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = temp_dir().join(format!("adapter-db-migration-{unique}"));
    let json_path = dir.join("tenant-data.json");
    tokio::fs::create_dir_all(&dir).await?;
    let mut file = tokio::fs::File::create(&json_path).await?;
    file.write_all(br#"{}"#).await?;
    file.flush().await?;

    let json_config = json!([
        {
            "name": "tenant-json",
            "driver": "jsonfile",
            "tenant": "tenant-a",
            "path": json_path.to_string_lossy()
        }
    ])
    .to_string();

    let registry = bootstrap_from_json(&json_config).await?;
    let json_adapter = registry
        .get_for_tenant("tenant-a")
        .await
        .expect("json adapter available");

    json_adapter
        .execute_script(
            "CREATE TABLE IF NOT EXISTS mig_test (id INTEGER PRIMARY KEY);\nINSERT INTO mig_test (id) VALUES (1);",
        )
        .await?;

    let snapshot = export_migration_snapshot(json_adapter.clone()).await?;
    assert!(snapshot.script_log.contains("CREATE TABLE"));

    let sqlite_path = dir.join("tenant-target.sqlite3");
    let sqlite_log = dir.join("tenant-target.log.sql");
    let sqlite_config = json!([
        {
            "name": "tenant-sqlite",
            "driver": "sqlite",
            "tenant": "tenant-a",
            "path": sqlite_path.to_string_lossy(),
            "logPath": sqlite_log.to_string_lossy()
        }
    ])
    .to_string();

    let target_registry = bootstrap_from_json(&sqlite_config).await?;
    let sqlite_adapter = target_registry
        .get_for_tenant("tenant-a")
        .await
        .expect("sqlite adapter available");

    let import_result = import_migration_snapshot(sqlite_adapter, &snapshot).await?;
    assert!(import_result.starts_with("sqlite:tenant-a:"));

    let conn = Connection::open(&sqlite_path)?;
    let value: i64 =
        conn.query_row("SELECT id FROM mig_test LIMIT 1;", [], |row| row.get(0))?;
    assert_eq!(value, 1);

    Ok(())
}
