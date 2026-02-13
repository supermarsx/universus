<#
.SYNOPSIS
Runs non-destructive smoke checks against Rust backend endpoints.
#>
param(
  [string]$ApiGatewayBase = "http://localhost:3300",
  [string]$AdminApiBase = "http://localhost:4302",
  [string]$BotApiBase = "http://localhost:4301",
  [string]$SmsApiBase = "http://localhost:4303",
  [string]$RealtimeGatewayBase = "http://localhost:4304",
  [int]$TimeoutSec = 10
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$checks = @(
  @{ Name = "rust-api-gateway health"; Url = "$ApiGatewayBase/health" },
  @{ Name = "rust-api-gateway leaderboard"; Url = "$ApiGatewayBase/api/leaderboard" },
  @{ Name = "rust-admin-api dashboard"; Url = "$AdminApiBase/api/admin/dashboard" },
  @{ Name = "rust-bot-api bots"; Url = "$BotApiBase/api/admin/bots" },
  @{ Name = "rust-sms-api metrics"; Url = "$SmsApiBase/metrics" },
  @{ Name = "rust-realtime-gateway ws-info"; Url = "$RealtimeGatewayBase/ws-info" }
)

$failures = 0

foreach ($check in $checks) {
  try {
    $response = Invoke-WebRequest -Uri $check.Url -Method GET -TimeoutSec $TimeoutSec
    $statusCode = [int]$response.StatusCode
    if ($statusCode -ge 200 -and $statusCode -lt 300) {
      Write-Host "[PASS] $($check.Name) -> HTTP $statusCode ($($check.Url))"
    }
    else {
      Write-Host "[FAIL] $($check.Name) -> HTTP $statusCode ($($check.Url))"
      $failures++
    }
  }
  catch {
    $statusCode = $null
    if ($_.Exception.Response -and $_.Exception.Response.StatusCode) {
      $statusCode = [int]$_.Exception.Response.StatusCode
    }

    if ($null -ne $statusCode) {
      Write-Host "[FAIL] $($check.Name) -> HTTP $statusCode ($($check.Url))"
    }
    else {
      Write-Host "[FAIL] $($check.Name) -> request error: $($_.Exception.Message) ($($check.Url))"
    }
    $failures++
  }
}

if ($failures -gt 0) {
  Write-Host ""
  Write-Host "Smoke check completed with $failures failure(s)."
  exit 1
}

Write-Host ""
Write-Host "Smoke check passed for all endpoints."
exit 0
