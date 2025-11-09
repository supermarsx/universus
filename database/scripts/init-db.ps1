<#
PowerShell equivalent of init-db.sh used in Docker entrypoint initialization.
This script expects environment variables inside the container:
 - POSTGRES_USER
 - POSTGRES_DB
 - The SQL files are expected under /docker-entrypoint-initdb.d/sql/steps
#>
param()

$PostgresUser = if ($env:POSTGRES_USER) { $env:POSTGRES_USER } else { 'postgres' }
$PostgresDb = if ($env:POSTGRES_DB) { $env:POSTGRES_DB } else { 'postgres' }

# run_sql function
function Run-SqlFile($file) {
  if (Test-Path $file) {
    Write-Host "Applying schema file: $(Split-Path $file -Leaf)"
    & psql -v ON_ERROR_STOP=1 -U $PostgresUser -d $PostgresDb -f $file
  }
}

$SQL_DIR = '/docker-entrypoint-initdb.d/sql'
$STEPS_DIR = Join-Path $SQL_DIR 'steps'

if (Test-Path $STEPS_DIR) {
  Write-Host "Applying ordered schema steps..."
  Get-ChildItem -Path $STEPS_DIR -Filter '*.sql' -File | Sort-Object Name | ForEach-Object {
    Run-SqlFile $_.FullName
  }
} else {
  Write-Host "No steps directory found; skipping structured schema application."
}

Write-Host "Finished applying schema steps."
