//! Migration runner that integrates with the Rust admin interface and multi-tenant DB adapters.

use anyhow::Result;
use serde::Deserialize;
use tokio::sync::Mutex;

#[derive(Debug, Deserialize)]
pub struct MigrationSpec {
    pub id: String,
    pub description: String,
    pub script: String,
}

pub struct MigrationRunner {
    migrations: Mutex<Vec<MigrationSpec>>,
}

impl MigrationRunner {
    pub fn new() -> Self {
        Self {
            migrations: Mutex::new(Vec::new()),
        }
    }

    pub async fn register(&self, migration: MigrationSpec) {
        let mut lock = self.migrations.lock().await;
        lock.push(migration);
    }

    pub async fn run(&self) -> Result<Vec<String>> {
        let lock = self.migrations.lock().await;
        let mut applied = Vec::new();
        for migration in lock.iter() {
            // placeholder: execute migration via adapter-db
            applied.push(format!("{} - {}", migration.id, migration.description));
        }
        Ok(applied)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn register_and_run() {
        let runner = MigrationRunner::new();
        runner
            .register(MigrationSpec {
                id: "001".into(),
                description: "init".into(),
                script: "SELECT 1".into(),
            })
            .await;
        let applied = runner.run().await.expect("runs");
        assert_eq!(applied.len(), 1);
    }
}
