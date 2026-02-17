<#
.SYNOPSIS
Brings up Rust services via Docker Compose, waits for readiness, runs smoke and cutover validation.
#>
param(
  [switch]$NoBuild
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$root = Resolve-Path (Join-Path $PSScriptRoot "..\..")

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
