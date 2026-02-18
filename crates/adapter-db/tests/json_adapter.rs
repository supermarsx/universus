use adapter_db::bootstrap_from_json;
use anyhow::Result;
use serde_json::json;
use std::env::temp_dir;
use tokio::io::AsyncWriteExt;

#[tokio::test]
async fn json_adapter_execute_appends_file() -> Result<()> {
    let dir = temp_dir().join("adapter-db-test-json");
    let path = dir.join("tenant-data.json");
    tokio::fs::create_dir_all(&dir).await?;
    let mut file = tokio::fs::File::create(&path).await?;
    file.write_all(br#"{"seed":true}"#).await?;
    file.flush().await?;
    drop(file);

    let config = json!([
        {
            "name": "local-json",
            "driver": "jsonfile",
            "tenant": "default",
            "path": path.to_string_lossy()
        }
    ])
    .to_string();

    let registry: adapter_db::AdapterRegistry = bootstrap_from_json(&config).await?;
    let adapter = registry
        .get("local-json")
        .await
        .expect("adapter registered");

    let result = adapter
        .execute_script("INSERT INTO noop VALUES (1);")
        .await?;
    assert!(result.contains("json:default"));

    let contents = tokio::fs::read_to_string(&path).await?;
    assert!(contents.contains("INSERT INTO noop VALUES (1);"));
    Ok(())
}

#[tokio::test]
async fn registry_maps_tenants_and_names() -> Result<()> {
    let dir = temp_dir().join("adapter-db-test");
    let path = dir.join("data.json");
    tokio::fs::create_dir_all(&dir).await?;
    let mut file = tokio::fs::File::create(&path).await?;
    file.write_all(br#"{}"#).await?;
    file.flush().await?;
    drop(file);

    let config = json!([
        { "name": "local-json", "driver": "jsonfile", "tenant": "default", "path": path.to_string_lossy() }
    ])
    .to_string();

    let registry: adapter_db::AdapterRegistry = bootstrap_from_json(&config).await?;
    let adapter = registry
        .get("local-json")
        .await
        .expect("adapter registered");
    assert!(adapter.describe().contains("JSON file"));
    let tenant_adapter = registry
        .get_for_tenant("default")
        .await
        .expect("tenant adapter registered");
    assert_eq!(tenant_adapter.name(), "local-json");
    Ok(())
}
