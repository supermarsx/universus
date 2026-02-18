<#
.SYNOPSIS
Brings up Rust services via Docker Compose, waits for readiness, runs smoke and cutover validation.
#>
param(
  [switch]$NoBuild,
  [string]$AdapterConfigPath = "database/runtime-adapters.json",
  [int]$AdminPort = 3001
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$root = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$adapterConfigPath = Join-Path $root $AdapterConfigPath
$adminBaseUrl = "http://localhost:$AdminPort"

function Get-ConfiguredTenants {
  param([string]$ConfigPath)
  if (-not (Test-Path $ConfigPath)) {
    throw "Adapter registry $ConfigPath not found"
  }
  $raw = Get-Content -Path $ConfigPath -Raw
  if (-not $raw) {
    return @()
  }
  $entries = try {
    ConvertFrom-Json $raw
  } catch {
    throw "Failed to parse adapter registry: $($_.Exception.Message)"
  }
  if (-not $entries) {
    return @()
  }
  @($entries) | ForEach-Object {
    $_.tenant
  } | Where-Object { $_ } | Sort-Object -Unique
}

function Validate-TenantMigrations {
  param(
    [string]$BaseUrl,
    [string[]]$Tenants
  )
  if (-not $Tenants -or $Tenants.Count -eq 0) {
    Write-Host "No tenants configured in adapter registry; skipping migration validation."
    return
  }

  foreach ($tenant in $Tenants) {
    $uri = "$BaseUrl/api/admin/tenants/$tenant/migrations"
    $attempt = 0
    $success = $false
    while (-not $success -and $attempt -lt 3) {
      try {
        $attempt++
        $response = Invoke-WebRequest -Uri $uri -Method GET -TimeoutSec 5
        $success = $true
      } catch {
        Write-Host "Retrying migrations status for tenant '$tenant' (attempt $attempt of 3)..."
        Start-Sleep -Seconds 2
      }
    }
    if (-not $success) {
      throw "Unable to reach migration status for tenant '$tenant'"
    }

    $payload = $response | ConvertFrom-Json
    $items = $payload.data
    if (-not $items) {
      Write-Host "[migrations] tenant '$tenant' has no recorded migrations."
      continue
    }

    $failed = $items | Where-Object { $_.state -eq "Failed" }
    if ($failed) {
      throw "Tenant '$tenant' has failed migrations: $($failed | ForEach-Object { $_.migration_id } -join ', ')"
    }

    $pending = $items | Where-Object { $_.state -eq "Pending" }
    if ($pending) {
      Write-Host "[migrations] tenant '$tenant' has pending migrations: $($pending | ForEach-Object { $_.migration_id } -join ', ')"
    } else {
      Write-Host "[migrations] tenant '$tenant' migrations appear healthy."
    }
  }
}

Push-Location $root
try {
  $upArgs = @("compose", "up", "-d")
  if (-not $NoBuild) {
    $upArgs += "--build"
  }
  Write-Host "Running: docker $($upArgs -join ' ')"
  & docker @upArgs
  if ($LASTEXITCODE -ne 0) {
    throw "docker compose up failed"
  }

  $readyChecks = @(
    @{ Name = "api-gateway"; Url = "http://localhost:3300/health" },
    @{ Name = "realtime-gateway"; Url = "http://localhost:4304/health" },
    @{ Name = "app-core-engine"; Url = "http://localhost:4307/health" }
  )
  $readyChecks += @{ Name = "app-admin-api"; Url = "$adminBaseUrl/health" }

  foreach ($check in $readyChecks) {
    $ok = $false
    for ($i = 0; $i -lt 30; $i++) {
      try {
        $resp = Invoke-WebRequest -Uri $check.Url -Method GET -TimeoutSec 5
        if ([int]$resp.StatusCode -ge 200 -and [int]$resp.StatusCode -lt 300) {
          $ok = $true
          break
        }
      } catch {
      }
      Start-Sleep -Seconds 2
    }
    if (-not $ok) {
      throw "service failed readiness: $($check.Name)"
    }
    Write-Host "[ready] $($check.Name)"
  }

  $tenants = Get-ConfiguredTenants -ConfigPath $adapterConfigPath
  Validate-TenantMigrations -BaseUrl $adminBaseUrl -Tenants $tenants

  & pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\rust\smoke-rust-endpoints.ps1
  if ($LASTEXITCODE -ne 0) {
    throw "smoke checks failed"
  }

  & pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\rust\run-cutover-validation.ps1
  if ($LASTEXITCODE -ne 0) {
    throw "cutover validation failed"
  }

  Write-Host "Live Rust cutover check completed successfully."
}
finally {
  Pop-Location
}
