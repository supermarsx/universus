//! Pluggable database adapter layer driven by JSON configuration.
//! Supports PostgreSQL, MySQL, and a JSON-backed adapter for local dev.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use log::error;
use mysql_async::{prelude::Queryable, Opts, Pool};
use rusqlite::Connection;
use serde::Deserialize;
use std::{
    any::Any,
    collections::HashMap,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{
    fs::{create_dir_all, OpenOptions},
    io::AsyncWriteExt,
    sync::Mutex,
    task,
};
use tokio_postgres::{Config as PgConfig, NoTls};

#[async_trait]
pub trait DatabaseAdapter: Send + Sync {
    fn name(&self) -> &str;
    fn describe(&self) -> String;
    fn connection_info(&self) -> String;
    fn tenant(&self) -> &str;
    fn driver_name(&self) -> &'static str;
    fn as_any(&self) -> &dyn Any;
    async fn execute_script(&self, script: &str) -> Result<String>;
}

#[derive(Debug, Deserialize)]
#[serde(tag = "driver", rename_all = "lowercase")]
pub enum AdapterDriver {
    Postgres {
        url: String,
        tenant: String,
        #[serde(default)]
        log_path: Option<String>,
    },
    Mysql {
        url: String,
        tenant: String,
        #[serde(default)]
        log_path: Option<String>,
    },
    JsonFile {
        path: String,
        tenant: String,
        #[serde(default)]
        log_path: Option<String>,
    },
    Sqlite {
        path: String,
        tenant: String,
        #[serde(default)]
        log_path: Option<String>,
    },
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
            AdapterDriver::Postgres {
                url,
                tenant,
                log_path,
            } => {
                let mut cfg = PgConfig::new();
                cfg.host(&url);
                let (client, connection) = cfg.connect(NoTls).await?;
                tokio::spawn(async move {
                    if let Err(err) = connection.await {
                        error!("postgres connection error: {err}");
                    }
                });
                let log_pathbuf = log_path
                    .map(PathBuf::from)
                    .unwrap_or_else(|| default_log_path("postgres", &tenant));
                Arc::new(PostgresAdapter {
                    name: entry.name.clone(),
                    tenant,
                    info: url.clone(),
                    client: Arc::new(client),
                    log_path: log_pathbuf,
                })
            }
            AdapterDriver::Mysql {
                url,
                tenant,
                log_path,
            } => {
                let opts = Opts::from_url(&url)?;
                let pool = Pool::new(opts);
                let log_pathbuf = log_path
                    .map(PathBuf::from)
                    .unwrap_or_else(|| default_log_path("mysql", &tenant));
                Arc::new(MysqlAdapter {
                    name: entry.name.clone(),
                    tenant,
                    info: url.clone(),
                    pool,
                    log_path: log_pathbuf,
                })
            }
            AdapterDriver::JsonFile {
                path,
                tenant,
                log_path,
            } => {
                let contents = fs::read_to_string(&path)?;
                let pathbuf = PathBuf::from(&path);
                let log_pathbuf = log_path
                    .map(PathBuf::from)
                    .unwrap_or_else(|| pathbuf.with_extension("log.sql"));
                Arc::new(JsonFileAdapter {
                    name: entry.name.clone(),
                    tenant,
                    info: path.clone(),
                    path: pathbuf,
                    data: Arc::new(Mutex::new(contents)),
                    log_path: log_pathbuf,
                })
            }
            AdapterDriver::Sqlite {
                path,
                tenant,
                log_path,
            } => {
                let pathbuf = PathBuf::from(&path);
                let log_pathbuf = log_path
                    .map(PathBuf::from)
                    .unwrap_or_else(|| pathbuf.with_extension("log.sql"));
                Arc::new(SqliteAdapter {
                    name: entry.name.clone(),
                    tenant,
                    info: path.clone(),
                    path: pathbuf,
                    log_path: log_pathbuf,
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
    log_path: PathBuf,
}

impl PostgresAdapter {
    fn log_path(&self) -> &Path {
        &self.log_path
    }
}

#[allow(dead_code)]
pub struct MysqlAdapter {
    name: String,
    tenant: String,
    info: String,
    pool: Pool,
    log_path: PathBuf,
}

impl MysqlAdapter {
    fn log_path(&self) -> &Path {
        &self.log_path
    }
}

#[allow(dead_code)]
pub struct JsonFileAdapter {
    name: String,
    tenant: String,
    info: String,
    path: PathBuf,
    data: Arc<Mutex<String>>,
    log_path: PathBuf,
}

impl JsonFileAdapter {
    fn log_path(&self) -> &Path {
        &self.log_path
    }
}

#[allow(dead_code)]
pub struct SqliteAdapter {
    name: String,
    tenant: String,
    info: String,
    path: PathBuf,
    log_path: PathBuf,
}

impl SqliteAdapter {
    fn log_path(&self) -> &Path {
        &self.log_path
    }
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

    fn driver_name(&self) -> &'static str {
        "postgres"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    async fn execute_script(&self, script: &str) -> Result<String> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let header = format!("-- migration {} @ {} --\n", self.name, timestamp.as_secs());
        self.client.batch_execute(script).await?;
        append_log(&self.log_path, &header, script).await?;
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

    fn driver_name(&self) -> &'static str {
        "mysql"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    async fn execute_script(&self, script: &str) -> Result<String> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let header = format!("-- migration {} @ {} --\n", self.name, timestamp.as_secs());
        let mut conn = self.pool.get_conn().await?;
        conn.query_drop(script).await?;
        append_log(&self.log_path, &header, script).await?;
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

    fn driver_name(&self) -> &'static str {
        "jsonfile"
    }

    fn as_any(&self) -> &dyn Any {
        self
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

        append_log(self.log_path(), &header, script).await?;

        Ok(format!("json:{}:{}", self.tenant, script.len()))
    }
}

#[async_trait]
impl DatabaseAdapter for SqliteAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    fn describe(&self) -> String {
        format!("SQLite (tenant {}) at {}", self.tenant, self.info)
    }

    fn connection_info(&self) -> String {
        self.info.clone()
    }

    fn tenant(&self) -> &str {
        &self.tenant
    }

    fn driver_name(&self) -> &'static str {
        "sqlite"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    async fn execute_script(&self, script: &str) -> Result<String> {
        let path = self.path.clone();
        let tenant = self.tenant.clone();
        let script_owned = script.to_owned();
        let script_for_sql = script_owned.clone();
        let log_path = self.log_path.clone();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let header = format!("-- migration {} @ {} --\n", self.name, timestamp.as_secs());
        let join_result = task::spawn_blocking(move || {
            let conn = Connection::open(&path)?;
            conn.execute_batch(&script_for_sql)?;
            Ok::<(), rusqlite::Error>(())
        })
        .await
        .map_err(|err| anyhow!("sqlite task join failed: {err}"))?;
        join_result.map_err(|err| anyhow!(err))?;

        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&log_path)
            .await?;
        file.write_all(header.as_bytes()).await?;
        file.write_all(script_owned.as_bytes()).await?;
        file.write_all(b"\n").await?;
        file.flush().await?;
        append_log(&log_path, &header, script_owned.as_str()).await?;

        Ok(format!("sqlite:{}:{}", tenant, script_owned.len()))
    }
}

pub async fn load_json_config<P: AsRef<Path>>(path: P) -> Result<String> {
    let contents = fs::read_to_string(path)?;
    Ok(contents)
}

fn default_log_path(driver: &str, tenant: &str) -> PathBuf {
    PathBuf::from("database")
        .join("logs")
        .join(format!("{tenant}-{driver}.sql"))
}

async fn append_log(path: &Path, header: &str, script: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        create_dir_all(parent).await?;
    }
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .await?;
    file.write_all(header.as_bytes()).await?;
    file.write_all(script.as_bytes()).await?;
    file.write_all(b"\n").await?;
    file.flush().await?;
    Ok(())
}

async fn read_log(path: &Path) -> Result<String> {
    match tokio::fs::read_to_string(path).await {
        Ok(contents) => Ok(contents),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(String::new()),
        Err(err) => Err(err.into()),
    }
}

pub struct MigrationSnapshot {
    pub name: String,
    pub tenant: String,
    pub driver: String,
    pub script_log: String,
}

pub async fn export_migration_snapshot(
    adapter: Arc<dyn DatabaseAdapter>,
) -> Result<MigrationSnapshot> {
    let driver = adapter.driver_name().to_string();
    let tenant = adapter.tenant().to_string();
    let name = adapter.name().to_string();

    let script_log = if let Some(json_adapter) = adapter.as_any().downcast_ref::<JsonFileAdapter>()
    {
        read_log(json_adapter.log_path()).await?
    } else if let Some(sqlite_adapter) = adapter.as_any().downcast_ref::<SqliteAdapter>() {
        read_log(sqlite_adapter.log_path()).await?
    } else if let Some(pg_adapter) = adapter.as_any().downcast_ref::<PostgresAdapter>() {
        read_log(pg_adapter.log_path()).await?
    } else if let Some(mysql_adapter) = adapter.as_any().downcast_ref::<MysqlAdapter>() {
        read_log(mysql_adapter.log_path()).await?
    } else {
        return Err(anyhow!("export unsupported for driver {driver}"));
    };

    Ok(MigrationSnapshot {
        name,
        tenant,
        driver,
        script_log,
    })
}

pub async fn import_migration_snapshot(
    adapter: Arc<dyn DatabaseAdapter>,
    snapshot: &MigrationSnapshot,
) -> Result<String> {
    adapter.execute_script(&snapshot.script_log).await
}
