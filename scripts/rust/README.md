# Rust Bringup and Smoke Scripts

These scripts are non-destructive operational helpers for local Rust-only bringup and endpoint smoke checks.

## Files

- `start-rust-only.ps1`: starts the Docker Compose `rust-only` profile for Rust backend services.
- `smoke-rust-endpoints.ps1`: checks key Rust service endpoints over HTTP.

## Usage

Run from repository root:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\rust\start-rust-only.ps1
```

Optional flags:

```powershell
# Skip image build step
powershell -ExecutionPolicy Bypass -File .\scripts\rust\start-rust-only.ps1 -NoBuild

# Run attached (foreground) instead of detached
powershell -ExecutionPolicy Bypass -File .\scripts\rust\start-rust-only.ps1 -Foreground
```

Run smoke checks:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\rust\smoke-rust-endpoints.ps1
```

Optional base URL overrides:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\rust\smoke-rust-endpoints.ps1 `
  -ApiGatewayBase "http://localhost:3300" `
  -AdminApiBase "http://localhost:4302" `
  -BotApiBase "http://localhost:4301" `
  -SmsApiBase "http://localhost:4303" `
  -RealtimeGatewayBase "http://localhost:4304"
```

## Endpoints Checked

- `rust-api-gateway`: `/health`, `/api/leaderboard`
- `rust-admin-api`: `/api/admin/dashboard`
- `rust-bot-api`: `/api/admin/bots`
- `rust-sms-api`: `/metrics`
- `rust-realtime-gateway`: `/ws-info`
