use adapter_db::bootstrap_from_json;
use anyhow::{anyhow, Context, Result};
use platform_migrations::MigrationTransfer;
use std::{collections::HashMap, env, fs, path::PathBuf};

struct Args {
    source_config: PathBuf,
    source_tenant: String,
    target_config: PathBuf,
    target_tenant: String,
}

fn parse_args() -> Result<Args> {
    let mut values = HashMap::new();
    let mut iter = env::args().skip(1);
    while let Some(key) = iter.next() {
        if !key.starts_with("--") {
            return Err(anyhow!("unexpected argument {key}"));
        }
        let value = iter
            .next()
            .ok_or_else(|| anyhow!("missing value for {key}"))?;
        values.insert(key, value);
    }

    let source_config = values
        .remove("--source-config")
        .ok_or_else(|| anyhow!("--source-config is required"))?;
    let source_tenant = values
        .remove("--source-tenant")
        .ok_or_else(|| anyhow!("--source-tenant is required"))?;
    let target_config = values
        .remove("--target-config")
        .ok_or_else(|| anyhow!("--target-config is required"))?;
    let target_tenant = values
        .remove("--target-tenant")
        .ok_or_else(|| anyhow!("--target-tenant is required"))?;

    if !values.is_empty() {
        return Err(anyhow!("unexpected flags: {:?}", values.keys()));
    }

    Ok(Args {
        source_config: PathBuf::from(source_config),
        source_tenant,
        target_config: PathBuf::from(target_config),
        target_tenant,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = parse_args()?;

    let source_config = fs::read_to_string(&args.source_config)
        .with_context(|| format!("reading {}", args.source_config.display()))?;
    let target_config = fs::read_to_string(&args.target_config)
        .with_context(|| format!("reading {}", args.target_config.display()))?;

    let source_registry = bootstrap_from_json(&source_config).await?;
    let target_registry = bootstrap_from_json(&target_config).await?;

    let source_adapter = source_registry
        .get_for_tenant(&args.source_tenant)
        .await
        .ok_or_else(|| anyhow!("source tenant {} not found", args.source_tenant))?;
    let target_adapter = target_registry
        .get_for_tenant(&args.target_tenant)
        .await
        .ok_or_else(|| anyhow!("target tenant {} not found", args.target_tenant))?;

    let status = MigrationTransfer::new()
        .transfer(source_adapter, target_adapter)
        .await?;

    println!("Migrated tenant {}", status.tenant_id);
    println!(
        "Source adapter: {}, driver: {}",
        status.source_adapter, status.source_driver
    );
    println!(
        "Target adapter: {}, driver: {}",
        status.target_adapter, status.target_driver
    );
    println!("Script length: {} bytes", status.script_size);
    println!("Import result: {}", status.message);

    Ok(())
}
