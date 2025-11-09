<#
.SYNOPSIS
Apply SQL migration steps to a running Postgres DB using psql (PowerShell version).

ENV VARIABLES (optional):
 - PGHOST (default: localhost)
 - PGPORT (default: 5432)
 - PGUSER (default: postgres)
 - PGPASSWORD (default: postgres)
 - PGDATABASE (default: testdb)
 - VERBOSE_MIGRATE (if 'true', prints sample rows)
#>

param()

$PGHOST = if ($env:PGHOST) { $env:PGHOST } else { 'localhost' }
$PGPORT = if ($env:PGPORT) { $env:PGPORT } else { '5432' }
$PGUSER = if ($env:PGUSER) { $env:PGUSER } else { 'postgres' }
$PGPASSWORD = if ($env:PGPASSWORD) { $env:PGPASSWORD } else { 'postgres' }
$PGDATABASE = if ($env:PGDATABASE) { $env:PGDATABASE } else { 'testdb' }

# Ensure psql is available
if (-not (Get-Command psql -ErrorAction SilentlyContinue)) {
  Write-Host "psql not found in PATH. Please install Postgres client tools and ensure psql is available." -ForegroundColor Red
  exit 1
}

$env:PGPASSWORD = $PGPASSWORD

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$SQL_DIR = Join-Path $ScriptDir '..\sql\steps' | Resolve-Path -ErrorAction Stop
$SQL_DIR = $SQL_DIR.Path

Write-Host "Applying SQL files from $SQL_DIR to $PGDATABASE@$PGHOST:$PGPORT"

function Run-PsqlCommandForFile([string]$filePath) {
  $attempt = 0
  $maxAttempts = 3
  $delay = 2
  $psqlArgs = @('-v', 'ON_ERROR_STOP=1', '-h', $PGHOST, '-p', $PGPORT, '-U', $PGUSER, '-d', $PGDATABASE, '-f', $filePath)
  while ($attempt -lt $maxAttempts) {
    & psql @psqlArgs
    if ($LASTEXITCODE -eq 0) { return $true }
    $attempt++
    Write-Host "psql command failed (attempt $attempt/$maxAttempts). Retrying in $delay seconds..." -ForegroundColor Yellow
    Start-Sleep -Seconds $delay
  }
  Write-Host "psql command failed after $maxAttempts attempts." -ForegroundColor Red
  return $false
}

function Run-PsqlCommandCapture([string[]]$args) {
  & psql @args 2>$null | ForEach-Object { $_ }
  return $LASTEXITCODE
}

# Apply each .sql file in the steps directory in alphabetical order
Get-ChildItem -Path $SQL_DIR -Filter '*.sql' -File | Sort-Object Name | ForEach-Object {
  $file = $_.FullName
  Write-Host "Applying $($_.Name)"
  if (-not (Run-PsqlCommandForFile $file)) { exit 1 }
}

Write-Host "Applying schema completed. Running sanity checks..."

function Check-Table($tbl) {
  $psqlArgs = @('-h', $PGHOST, '-p', $PGPORT, '-U', $PGUSER, '-d', $PGDATABASE, '-t', '-c', "SELECT COUNT(*) FROM $tbl;")
  $exit = Run-PsqlCommandCapture $psqlArgs
  if ($exit -ne 0) {
    Write-Host "Sanity check failed: $tbl table missing or psql error" -ForegroundColor Red
    return $false
  }
  # Capture output for display
  $output = & psql @psqlArgs 2>$null
  $value = ($output -join "").Trim()
  Write-Host "$tbl: $value"
  return $true
}

if (-not (Check-Table 'users')) { exit 1 }
if (-not (Check-Table 'planets')) { exit 1 }
if (-not (Check-Table 'fleets')) { exit 1 }

if ($env:VERBOSE_MIGRATE -eq 'true') {
  Write-Host "Listing sample rows from users, planets, fleets for verification:"
  & psql -h $PGHOST -p $PGPORT -U $PGUSER -d $PGDATABASE -c "SELECT * FROM users LIMIT 3;"
  & psql -h $PGHOST -p $PGPORT -U $PGUSER -d $PGDATABASE -c "SELECT * FROM planets LIMIT 3;"
  & psql -h $PGHOST -p $PGPORT -U $PGUSER -d $PGDATABASE -c "SELECT * FROM fleets LIMIT 3;"
}

Write-Host "Sanity checks passed." -ForegroundColor Green

