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
  [string]$CoreEngineBase = "http://localhost:4307",
  [string]$DevToken = "dev-token",
  [int]$TimeoutSec = 10
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$checks = @(
  @{ Name = "rust-api-gateway health"; Url = "$ApiGatewayBase/health"; Headers = @{} },
  @{ Name = "rust-api-gateway leaderboard"; Url = "$ApiGatewayBase/api/leaderboard"; Headers = @{} },
  @{ Name = "rust-admin-api dashboard"; Url = "$AdminApiBase/api/admin/dashboard"; Headers = @{} },
  @{ Name = "rust-bot-api bots"; Url = "$BotApiBase/api/admin/bots"; Headers = @{} },
  @{ Name = "rust-sms-api metrics"; Url = "$SmsApiBase/metrics"; Headers = @{} },
  @{ Name = "rust-realtime-gateway ws-info"; Url = "$RealtimeGatewayBase/ws-info"; Headers = @{} },
  @{ Name = "rust-app-core-engine health"; Url = "$CoreEngineBase/health"; Headers = @{} },
  @{ Name = "rust-api notifications unread count"; Url = "$ApiGatewayBase/api/notifications/unread-count"; Headers = @{ authorization = "Bearer $DevToken" } },
  @{ Name = "rust-api shards status"; Url = "$ApiGatewayBase/api/shards/messages/status"; Headers = @{ authorization = "Bearer $DevToken" } },
  @{ Name = "rust-realtime recent events"; Url = "$RealtimeGatewayBase/api/realtime/events/recent?limit=5"; Headers = @{} }
)

$failures = 0

foreach ($check in $checks) {
  try {
    $response = Invoke-WebRequest -Uri $check.Url -Method GET -Headers $check.Headers -TimeoutSec $TimeoutSec
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

try {
  $createBody = @{
    title = "Smoke Notification"
    message = "Smoke flow"
    category = "system"
    priority = 3
  } | ConvertTo-Json

  $create = Invoke-WebRequest -Uri "$ApiGatewayBase/api/notifications" -Method POST -Headers @{ authorization = "Bearer $DevToken"; "content-type" = "application/json" } -Body $createBody -TimeoutSec $TimeoutSec
  if ([int]$create.StatusCode -ge 200 -and [int]$create.StatusCode -lt 300) {
    Write-Host "[PASS] rust-api notification create flow -> HTTP $([int]$create.StatusCode)"
  } else {
    Write-Host "[FAIL] rust-api notification create flow -> HTTP $([int]$create.StatusCode)"
    $failures++
  }
}
catch {
  Write-Host "[FAIL] rust-api notification create flow -> request error: $($_.Exception.Message)"
  $failures++
}

if ($failures -gt 0) {
  Write-Host ""
  Write-Host "Smoke check completed with $failures failure(s)."
  exit 1
}

Write-Host ""
Write-Host "Smoke check passed for all endpoints."
exit 0
