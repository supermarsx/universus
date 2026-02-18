//! Pluggable database adapter layer driven by JSON configuration.
//! Supports PostgreSQL, MySQL, and a JSON-backed adapter for local dev.

use anyhow::Result;
use async_trait::async_trait;
use log::error;
use mysql_async::{prelude::Queryable, Opts, Pool};
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{fs::OpenOptions, io::AsyncWriteExt, sync::Mutex};
use tokio_postgres::{Config as PgConfig, NoTls};

#[async_trait]
pub trait DatabaseAdapter: Send + Sync {
    fn name(&self) -> &str;
    fn describe(&self) -> String;
    fn connection_info(&self) -> String;
    fn tenant(&self) -> &str;
    async fn execute_script(&self, script: &str) -> Result<String>;
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
                    info: url.clone(),
                    client: Arc::new(client),
                })
            }
            AdapterDriver::Mysql { url, tenant } => {
                let opts = Opts::from_url(&url)?;
                let pool = Pool::new(opts);
                Arc::new(MysqlAdapter {
                    name: entry.name.clone(),
                    tenant,
                    info: url.clone(),
                    pool,
                })
            }
            AdapterDriver::JsonFile { path, tenant } => {
                let contents = fs::read_to_string(&path)?;
                let pathbuf = PathBuf::from(&path);
                Arc::new(JsonFileAdapter {
                    name: entry.name.clone(),
                    tenant,
                    info: path.clone(),
                    path: pathbuf,
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

#[allow(dead_code)]
pub struct MysqlAdapter {
    name: String,
    tenant: String,
    info: String,
    pool: Pool,
}

#[allow(dead_code)]
pub struct JsonFileAdapter {
    name: String,
    tenant: String,
    info: String,
    path: PathBuf,
    data: Arc<Mutex<String>>,
}

#[async_trait]
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

    async fn execute_script(&self, script: &str) -> Result<String> {
        self.client.batch_execute(script).await?;
        Ok(format!("postgres:{}:{}", self.tenant, script.len()))
    }
}

#[async_trait]
impl DatabaseAdapter for MysqlAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    fn describe(&self) -> String {
        format!("MySQL (tenant {}) at {}", self.tenant, self.info)
    }

    fn connection_info(&self) -> String {
        self.info.clone()
    }

    fn tenant(&self) -> &str {
        &self.tenant
    }

    async fn execute_script(&self, script: &str) -> Result<String> {
        let mut conn = self.pool.get_conn().await?;
        conn.query_drop(script).await?;
        Ok(format!("mysql:{}:{}", self.tenant, script.len()))
    }
}

#[async_trait]
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

    async fn execute_script(&self, script: &str) -> Result<String> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let header = format!("-- migration {} @ {} --\n", self.name, timestamp.as_secs());

        {
            let mut data = self.data.lock().await;
            data.push_str(&header);
            data.push_str(script);
            data.push('\n');
        }

        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&self.path)
            .await?;
        file.write_all(header.as_bytes()).await?;
        file.write_all(script.as_bytes()).await?;
        file.write_all(b"\n").await?;
        file.flush().await?;

        Ok(format!("json:{}:{}", self.tenant, script.len()))
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
    async fn json_adapter_execute_appends_file() -> Result<()> {
        let dir = temp_dir().join("adapter-db-test-json");
        let path = dir.join("tenant-data.json");
        tokio::fs::create_dir_all(&dir).await?;
        let mut file = tokio::fs::File::create(&path).await?;
        file.write_all(br#"{"seed":true}"#).await?;
        file.flush().await?;
        drop(file);

        let config = serde_json::json!([
            {
                "name": "local-json",
                "driver": "jsonfile",
                "tenant": "default",
                "path": path.to_string_lossy()
            }
        ])
        .to_string();

        let registry: AdapterRegistry = bootstrap_from_json(&config).await?;
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
