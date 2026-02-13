<#
.SYNOPSIS
Starts the Rust-only Docker Compose profile for local bringup.
#>
param(
  [switch]$NoBuild,
  [switch]$Foreground
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$services = @(
  "database",
  "redis",
  "rabbitmq",
  "rust-core-engine",
  "rust-api-gateway",
  "rust-admin-api",
  "rust-bot-api",
  "rust-sms-api",
  "rust-realtime-gateway",
  "frontend"
)

$composeArgs = @("compose", "--profile", "rust-only", "up")
if (-not $Foreground) {
  $composeArgs += "-d"
}
if (-not $NoBuild) {
  $composeArgs += "--build"
}
$composeArgs += $services

Write-Host "Running: docker $($composeArgs -join ' ')"

Push-Location $repoRoot
try {
  & docker @composeArgs
  if ($LASTEXITCODE -ne 0) {
    throw "docker compose failed with exit code $LASTEXITCODE"
  }
}
finally {
  Pop-Location
}
