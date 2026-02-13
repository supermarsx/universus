# Node vs Rust Contract Diff Harness

This harness compares selected endpoint responses between the Node backend and the Rust API gateway and exits non-zero on mismatch.

## Script

- `scripts/contract-diff.mjs`

## What It Compares

- `health`
  - Node: `GET /api/health`
  - Rust: `GET /health`
- `auth-login`
  - Node: `POST /api/auth/login`
  - Rust: `POST /api/auth/login`
- `fleet-helper-movement`
  - Node: `POST /api/fleet/helpers/movement`
  - Rust: `POST /api/fleet/helpers/movement`
- `galaxy`
  - Node: `GET /api/galaxy?galaxy=1&system=120` (default path)
  - Rust: `GET /api/galaxy/1/120` (default path)

## Usage

From repository root:

```powershell
node .\scripts\contract-diff.mjs
```

Select checks:

```powershell
node .\scripts\contract-diff.mjs --checks health,auth-login
```

Override base URLs:

```powershell
node .\scripts\contract-diff.mjs --node-base http://localhost:3000 --rust-base http://localhost:3300
```

Set timeout:

```powershell
node .\scripts\contract-diff.mjs --timeout-ms 15000
```

## Exit Codes

- `0`: all selected checks matched.
- `1`: one or more checks mismatched or one side failed request execution.
- `2`: invalid arguments or fatal harness error.

## Environment Overrides

Base URLs and timeout:

- `CONTRACT_DIFF_NODE_BASE_URL`
- `CONTRACT_DIFF_RUST_BASE_URL`
- `CONTRACT_DIFF_TIMEOUT_MS`

Payloads (JSON string):

- `CONTRACT_DIFF_AUTH_LOGIN_PAYLOAD`
  - default: `{"email":"contract-diff@example.com","password":"contract-diff-password"}`
- `CONTRACT_DIFF_FLEET_HELPER_PAYLOAD`
  - default: `{"origin":{"galaxy":1,"system":120,"position":8},"target":{"galaxy":1,"system":121,"position":4},"ships":{"lightFighter":10,"cruiser":4}}`

Auth headers/tokens for auth-protected checks:

- Node:
  - `CONTRACT_DIFF_NODE_AUTH_HEADER` (full header value, for example `Bearer <token>`)
  - or `CONTRACT_DIFF_NODE_BEARER_TOKEN` (token only)
- Rust:
  - `CONTRACT_DIFF_RUST_AUTH_HEADER`
  - or `CONTRACT_DIFF_RUST_BEARER_TOKEN`

Galaxy path overrides:

- `CONTRACT_DIFF_NODE_GALAXY_PATH`
- `CONTRACT_DIFF_RUST_GALAXY_PATH`

## Notes

- The harness normalizes JSON before comparison by:
  - unwrapping `{ "success": true, "data": ... }` envelopes;
  - ignoring `timestamp`, `service`, and `engine` fields.
- It still compares HTTP status code and remaining JSON structure/value strictly.
