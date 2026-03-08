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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::env::temp_dir;
    use tokio::fs::{create_dir_all, remove_dir_all, File};
    use tokio::io::AsyncWriteExt;

    async fn recreate_dir(path: &Path) {
        let _ = remove_dir_all(path).await;
        create_dir_all(path).await.unwrap();
    }

    /// Helper: create a temp dir with an empty JSON data file and return its path.
    async fn setup_json_data(dir_name: &str) -> (PathBuf, PathBuf) {
        let dir = temp_dir().join(format!("adapter-db-test-{dir_name}"));
        let data_path = dir.join("data.json");
        recreate_dir(&dir).await;
        let mut f = File::create(&data_path).await.unwrap();
        f.write_all(br#"{}"#).await.unwrap();
        f.flush().await.unwrap();
        (dir, data_path)
    }

    // --- AdapterRegistry: register and get ---

    #[tokio::test]
    async fn registry_register_and_get_by_name() {
        let (dir, data_path) = setup_json_data("reg-name").await;
        let config = json!([{
            "name": "a1",
            "driver": "jsonfile",
            "tenant": "t1",
            "path": data_path.to_string_lossy()
        }])
        .to_string();

        let registry = bootstrap_from_json(&config).await.unwrap();
        let adapter = registry.get("a1").await;
        assert!(adapter.is_some());
        assert_eq!(adapter.unwrap().name(), "a1");

        assert!(registry.get("nonexistent").await.is_none());
        drop(dir);
    }

    #[tokio::test]
    async fn registry_get_for_tenant() {
        let (_dir, data_path) = setup_json_data("reg-tenant").await;
        let config = json!([{
            "name": "a1",
            "driver": "jsonfile",
            "tenant": "t1",
            "path": data_path.to_string_lossy()
        }])
        .to_string();

        let registry = bootstrap_from_json(&config).await.unwrap();
        let adapter = registry.get_for_tenant("t1").await;
        assert!(adapter.is_some());
        assert_eq!(adapter.unwrap().tenant(), "t1");

        assert!(registry.get_for_tenant("no-tenant").await.is_none());
    }

    #[tokio::test]
    async fn registry_new_is_empty() {
        let registry = AdapterRegistry::new();
        assert!(registry.get("anything").await.is_none());
        assert!(registry.get_for_tenant("anyone").await.is_none());
    }

    // --- JsonFileAdapter ---

    #[tokio::test]
    async fn jsonfile_adapter_describe_and_driver() {
        let (_dir, data_path) = setup_json_data("jf-describe").await;
        let config = json!([{
            "name": "jf1",
            "driver": "jsonfile",
            "tenant": "t-jf",
            "path": data_path.to_string_lossy()
        }])
        .to_string();

        let registry = bootstrap_from_json(&config).await.unwrap();
        let adapter = registry.get("jf1").await.unwrap();

        assert_eq!(adapter.driver_name(), "jsonfile");
        assert!(adapter.describe().contains("JSON file"));
        assert!(adapter.describe().contains("t-jf"));
        assert_eq!(adapter.tenant(), "t-jf");
        assert!(!adapter.connection_info().is_empty());
    }

    #[tokio::test]
    async fn jsonfile_adapter_execute_script() {
        let (_dir, data_path) = setup_json_data("jf-exec").await;
        let config = json!([{
            "name": "jf-exec",
            "driver": "jsonfile",
            "tenant": "t-exec",
            "path": data_path.to_string_lossy()
        }])
        .to_string();

        let registry = bootstrap_from_json(&config).await.unwrap();
        let adapter = registry.get("jf-exec").await.unwrap();

        let result = adapter.execute_script("CREATE TABLE test (id INT);").await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.starts_with("json:t-exec:"));

        // Verify the script was appended to the data file
        let contents = tokio::fs::read_to_string(&data_path).await.unwrap();
        assert!(contents.contains("CREATE TABLE test (id INT);"));
    }

    #[tokio::test]
    async fn jsonfile_adapter_multiple_scripts_append() {
        let (_dir, data_path) = setup_json_data("jf-multi").await;
        let config = json!([{
            "name": "jf-multi",
            "driver": "jsonfile",
            "tenant": "t-multi",
            "path": data_path.to_string_lossy()
        }])
        .to_string();

        let registry = bootstrap_from_json(&config).await.unwrap();
        let adapter = registry.get("jf-multi").await.unwrap();

        adapter.execute_script("SCRIPT_ONE").await.unwrap();
        adapter.execute_script("SCRIPT_TWO").await.unwrap();

        let contents = tokio::fs::read_to_string(&data_path).await.unwrap();
        assert!(contents.contains("SCRIPT_ONE"));
        assert!(contents.contains("SCRIPT_TWO"));
    }

    // --- SqliteAdapter ---

    #[tokio::test]
    async fn sqlite_adapter_describe_and_driver() {
        let dir = temp_dir().join("adapter-db-test-sq-describe");
        recreate_dir(&dir).await;
        let db_path = dir.join("test.db");

        let config = json!([{
            "name": "sq1",
            "driver": "sqlite",
            "tenant": "t-sq",
            "path": db_path.to_string_lossy()
        }])
        .to_string();

        let registry = bootstrap_from_json(&config).await.unwrap();
        let adapter = registry.get("sq1").await.unwrap();

        assert_eq!(adapter.driver_name(), "sqlite");
        assert!(adapter.describe().contains("SQLite"));
        assert!(adapter.describe().contains("t-sq"));
        assert_eq!(adapter.tenant(), "t-sq");
    }

    #[tokio::test]
    async fn sqlite_adapter_execute_and_verify() {
        let dir = temp_dir().join("adapter-db-test-sq-exec");
        recreate_dir(&dir).await;
        let db_path = dir.join("exec.db");

        let config = json!([{
            "name": "sq-exec",
            "driver": "sqlite",
            "tenant": "t-sq-exec",
            "path": db_path.to_string_lossy()
        }])
        .to_string();

        let registry = bootstrap_from_json(&config).await.unwrap();
        let adapter = registry.get("sq-exec").await.unwrap();

        let result = adapter
            .execute_script(
                "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT); INSERT INTO items (name) VALUES ('hello');",
            )
            .await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.starts_with("sqlite:t-sq-exec:"));

        // Verify data via direct connection
        let conn = Connection::open(&db_path).unwrap();
        let name: String = conn
            .query_row("SELECT name FROM items LIMIT 1", [], |row| row.get(0))
            .unwrap();
        assert_eq!(name, "hello");
    }

    // --- bootstrap_from_json ---

    #[tokio::test]
    async fn bootstrap_multiple_adapters() {
        let (_dir1, data_path1) = setup_json_data("boot-multi-1").await;
        let dir2 = temp_dir().join("adapter-db-test-boot-multi-2");
        recreate_dir(&dir2).await;
        let db_path2 = dir2.join("boot.db");

        let config = json!([
            {
                "name": "a-json",
                "driver": "jsonfile",
                "tenant": "t-json",
                "path": data_path1.to_string_lossy()
            },
            {
                "name": "a-sqlite",
                "driver": "sqlite",
                "tenant": "t-sqlite",
                "path": db_path2.to_string_lossy()
            }
        ])
        .to_string();

        let registry = bootstrap_from_json(&config).await.unwrap();
        assert!(registry.get("a-json").await.is_some());
        assert!(registry.get("a-sqlite").await.is_some());
        assert!(registry.get_for_tenant("t-json").await.is_some());
        assert!(registry.get_for_tenant("t-sqlite").await.is_some());
    }

    #[tokio::test]
    async fn bootstrap_empty_array() {
        let registry = bootstrap_from_json("[]").await.unwrap();
        assert!(registry.get("anything").await.is_none());
    }

    #[tokio::test]
    async fn bootstrap_invalid_json_errors() {
        let result = bootstrap_from_json("not json").await;
        assert!(result.is_err());
    }

    // --- Config parsing / deserialization ---

    #[test]
    fn adapter_entry_deserializes_jsonfile() {
        let json = json!({
            "name": "a1",
            "driver": "jsonfile",
            "tenant": "t1",
            "path": "/tmp/data.json"
        });
        let entry: AdapterEntry = serde_json::from_value(json).unwrap();
        assert_eq!(entry.name, "a1");
        assert!(matches!(entry.driver, AdapterDriver::JsonFile { .. }));
    }

    #[test]
    fn adapter_entry_deserializes_sqlite() {
        let json = json!({
            "name": "a2",
            "driver": "sqlite",
            "tenant": "t2",
            "path": "/tmp/db.sqlite3"
        });
        let entry: AdapterEntry = serde_json::from_value(json).unwrap();
        assert_eq!(entry.name, "a2");
        assert!(matches!(entry.driver, AdapterDriver::Sqlite { .. }));
    }

    #[test]
    fn adapter_entry_deserializes_postgres() {
        let json = json!({
            "name": "a3",
            "driver": "postgres",
            "tenant": "t3",
            "url": "localhost:5432"
        });
        let entry: AdapterEntry = serde_json::from_value(json).unwrap();
        assert_eq!(entry.name, "a3");
        assert!(matches!(entry.driver, AdapterDriver::Postgres { .. }));
    }

    #[test]
    fn adapter_entry_deserializes_mysql() {
        let json = json!({
            "name": "a4",
            "driver": "mysql",
            "tenant": "t4",
            "url": "mysql://localhost:3306"
        });
        let entry: AdapterEntry = serde_json::from_value(json).unwrap();
        assert_eq!(entry.name, "a4");
        assert!(matches!(entry.driver, AdapterDriver::Mysql { .. }));
    }

    #[test]
    fn adapter_entry_with_log_path() {
        let json = json!({
            "name": "a5",
            "driver": "jsonfile",
            "tenant": "t5",
            "path": "/tmp/data.json",
            "log_path": "/tmp/my-log.sql"
        });
        let entry: AdapterEntry = serde_json::from_value(json).unwrap();
        match entry.driver {
            AdapterDriver::JsonFile { log_path, .. } => {
                assert_eq!(log_path, Some("/tmp/my-log.sql".to_string()));
            }
            _ => panic!("expected JsonFile"),
        }
    }

    #[test]
    fn adapter_entry_unknown_driver_fails() {
        let json = json!({
            "name": "a6",
            "driver": "oracle",
            "tenant": "t6",
            "url": "localhost"
        });
        let result: Result<AdapterEntry, _> = serde_json::from_value(json);
        assert!(result.is_err());
    }

    // --- default_log_path ---

    #[test]
    fn default_log_path_format() {
        let path = default_log_path("postgres", "tenant-a");
        assert!(path.to_string_lossy().contains("tenant-a-postgres.sql"));
        assert!(path.to_string_lossy().contains("database"));
        assert!(path.to_string_lossy().contains("logs"));
    }

    // --- load_json_config ---

    #[tokio::test]
    async fn load_json_config_reads_file() {
        let dir = temp_dir().join("adapter-db-test-load-config");
        create_dir_all(&dir).await.unwrap();
        let path = dir.join("config.json");
        let mut f = File::create(&path).await.unwrap();
        f.write_all(br#"[{"name":"x"}]"#).await.unwrap();
        f.flush().await.unwrap();

        let content = load_json_config(&path).await.unwrap();
        assert!(content.contains("\"name\":\"x\""));
    }

    #[tokio::test]
    async fn load_json_config_missing_file_errors() {
        let result = load_json_config("/tmp/nonexistent-adapter-db-test.json").await;
        assert!(result.is_err());
    }

    // --- export / import migration snapshot ---

    #[tokio::test]
    async fn export_snapshot_from_json_adapter() {
        let (_dir, data_path) = setup_json_data("export-snap").await;
        let config = json!([{
            "name": "snap-json",
            "driver": "jsonfile",
            "tenant": "t-snap",
            "path": data_path.to_string_lossy()
        }])
        .to_string();

        let registry = bootstrap_from_json(&config).await.unwrap();
        let adapter = registry.get("snap-json").await.unwrap();

        // Write some data
        adapter.execute_script("SOME SQL").await.unwrap();

        let snapshot = export_migration_snapshot(adapter).await.unwrap();
        assert_eq!(snapshot.name, "snap-json");
        assert_eq!(snapshot.tenant, "t-snap");
        assert_eq!(snapshot.driver, "jsonfile");
        // The log should contain the script
        assert!(snapshot.script_log.contains("SOME SQL"));
    }

    #[tokio::test]
    async fn import_snapshot_to_json_adapter() {
        let (_dir, data_path) = setup_json_data("import-snap").await;
        let config = json!([{
            "name": "import-json",
            "driver": "jsonfile",
            "tenant": "t-import",
            "path": data_path.to_string_lossy()
        }])
        .to_string();

        let registry = bootstrap_from_json(&config).await.unwrap();
        let adapter = registry.get("import-json").await.unwrap();

        let snapshot = MigrationSnapshot {
            name: "src".into(),
            tenant: "t-import".into(),
            driver: "jsonfile".into(),
            script_log: "IMPORTED SCRIPT".into(),
        };

        let result = import_migration_snapshot(adapter, &snapshot).await;
        assert!(result.is_ok());

        let contents = tokio::fs::read_to_string(&data_path).await.unwrap();
        assert!(contents.contains("IMPORTED SCRIPT"));
    }

    // --- as_any downcast ---

    #[tokio::test]
    async fn json_adapter_downcast() {
        let (_dir, data_path) = setup_json_data("downcast-jf").await;
        let config = json!([{
            "name": "dc-json",
            "driver": "jsonfile",
            "tenant": "t-dc",
            "path": data_path.to_string_lossy()
        }])
        .to_string();

        let registry = bootstrap_from_json(&config).await.unwrap();
        let adapter = registry.get("dc-json").await.unwrap();

        let any = adapter.as_any();
        assert!(any.downcast_ref::<JsonFileAdapter>().is_some());
        assert!(any.downcast_ref::<SqliteAdapter>().is_none());
    }

    #[tokio::test]
    async fn sqlite_adapter_downcast() {
        let dir = temp_dir().join("adapter-db-test-downcast-sq");
        create_dir_all(&dir).await.unwrap();
        let db_path = dir.join("dc.db");

        let config = json!([{
            "name": "dc-sqlite",
            "driver": "sqlite",
            "tenant": "t-dc-sq",
            "path": db_path.to_string_lossy()
        }])
        .to_string();

        let registry = bootstrap_from_json(&config).await.unwrap();
        let adapter = registry.get("dc-sqlite").await.unwrap();

        let any = adapter.as_any();
        assert!(any.downcast_ref::<SqliteAdapter>().is_some());
        assert!(any.downcast_ref::<JsonFileAdapter>().is_none());
    }

    // --- MigrationSnapshot fields ---

    #[test]
    fn migration_snapshot_fields() {
        let snap = MigrationSnapshot {
            name: "n".into(),
            tenant: "t".into(),
            driver: "d".into(),
            script_log: "s".into(),
        };
        assert_eq!(snap.name, "n");
        assert_eq!(snap.tenant, "t");
        assert_eq!(snap.driver, "d");
        assert_eq!(snap.script_log, "s");
    }
}
