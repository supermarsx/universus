//! Pluggable database adapter layer driven by JSON configuration.
//! Supports PostgreSQL, MySQL, and a JSON-backed adapter for local dev.

use anyhow::Result;
use log::error;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_postgres::{Config as PgConfig, NoTls};

pub trait DatabaseAdapter: Send + Sync {
    fn name(&self) -> &str;
    fn describe(&self) -> String;
    fn connection_info(&self) -> String;
    fn tenant(&self) -> &str;
}

#[derive(Debug, Deserialize)]
#[serde(tag = "driver", rename_all = "lowercase")]
pub enum AdapterDriver {
    Postgres { url: String, tenant: String },
    Mysql { url: String, tenant: String },
    JsonFile { path: String, tenant: String },
}

#[derive(Debug, Deserialize)]
pub struct AdapterEntry {
    pub name: String,
    #[serde(flatten)]
    pub driver: AdapterDriver,
}

pub struct AdapterRegistry {
    adapters: Mutex<HashMap<String, Arc<dyn DatabaseAdapter>>>,
    tenants: Mutex<HashMap<String, Arc<dyn DatabaseAdapter>>>,
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self {
            adapters: Mutex::new(HashMap::new()),
            tenants: Mutex::new(HashMap::new()),
        }
    }

    pub async fn register(&self, adapter: Arc<dyn DatabaseAdapter>) {
        let mut adapters = self.adapters.lock().await;
        adapters.insert(adapter.name().to_string(), adapter.clone());
        drop(adapters);
        let mut tenants = self.tenants.lock().await;
        tenants.insert(adapter.tenant().to_string(), adapter);
    }

    pub async fn get(&self, name: &str) -> Option<Arc<dyn DatabaseAdapter>> {
        let lock = self.adapters.lock().await;
        lock.get(name).cloned()
    }

    pub async fn get_for_tenant(&self, tenant: &str) -> Option<Arc<dyn DatabaseAdapter>> {
        let lock = self.tenants.lock().await;
        lock.get(tenant).cloned()
    }
}

pub async fn bootstrap_from_json(config_json: &str) -> Result<AdapterRegistry> {
    let entries: Vec<AdapterEntry> = serde_json::from_str(config_json)?;
    let registry = AdapterRegistry::new();
    for entry in entries {
        let adapter: Arc<dyn DatabaseAdapter> = match entry.driver {
            AdapterDriver::Postgres { url, tenant } => {
                let mut cfg = PgConfig::new();
                cfg.host(&url);
                let (client, connection) = cfg.connect(NoTls).await?;
                tokio::spawn(async move {
                    if let Err(err) = connection.await {
                        error!("postgres connection error: {err}");
                    }
                });
                Arc::new(PostgresAdapter {
                    name: entry.name.clone(),
                    tenant,
                    info: url,
                    client: Arc::new(client),
                })
            }
            AdapterDriver::Mysql { url, tenant } => {
                let mut cfg = PgConfig::new();
                cfg.host(&url);
                let (client, connection) = cfg.connect(NoTls).await?;
                tokio::spawn(async move {
                    if let Err(err) = connection.await {
                        error!("mysql connection error: {err}");
                    }
                });
                Arc::new(PostgresAdapter {
                    name: entry.name.clone(),
                    tenant,
                    info: url,
                    client: Arc::new(client),
                })
            }
            AdapterDriver::JsonFile { path, tenant } => {
                let contents = fs::read_to_string(&path)?;
                Arc::new(JsonFileAdapter {
                    name: entry.name.clone(),
                    tenant,
                    info: path.clone(),
                    data: Arc::new(Mutex::new(contents)),
                })
            }
        };
        registry.register(adapter).await;
    }
    Ok(registry)
}

#[allow(dead_code)]
pub struct PostgresAdapter {
    name: String,
    tenant: String,
    info: String,
    client: Arc<tokio_postgres::Client>,
}

impl DatabaseAdapter for PostgresAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    fn describe(&self) -> String {
        format!("Postgres (tenant {}) at {}", self.tenant, self.info)
    }

    fn connection_info(&self) -> String {
        self.info.clone()
    }

    fn tenant(&self) -> &str {
        &self.tenant
    }
}

#[allow(dead_code)]
pub struct JsonFileAdapter {
    name: String,
    tenant: String,
    info: String,
    data: Arc<Mutex<String>>,
}

impl DatabaseAdapter for JsonFileAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    fn describe(&self) -> String {
        format!("JSON file (tenant {}) at {}", self.tenant, self.info)
    }

    fn connection_info(&self) -> String {
        self.info.clone()
    }

    fn tenant(&self) -> &str {
        &self.tenant
    }
}

pub async fn load_json_config<P: AsRef<Path>>(path: P) -> Result<String> {
    let contents = fs::read_to_string(path)?;
    Ok(contents)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn bootstrap_json_registers_jsonfile_adapter() -> Result<()> {
        let dir = temp_dir().join("adapter-db-test");
        let path = dir.join("data.json");
        tokio::fs::create_dir_all(&dir).await?;
        let mut file: tokio::fs::File = tokio::fs::File::create(&path).await?;
        file.write_all(br#"{}"#).await?;

        let config = serde_json::json!([
            { "name": "local-json", "driver": "jsonfile", "tenant": "default", "path": path.to_string_lossy() }
        ])
        .to_string();

        let registry: AdapterRegistry = bootstrap_from_json(&config).await?;
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
}
