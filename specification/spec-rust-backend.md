# Universus Rust Backend Core Specification

## Status
Active as of February 11, 2026.

## Purpose
Define the Rust-native backend responsibilities and the Node.js/Rust boundary for performance-critical systems.

## Architecture Boundary
- Node.js backend owns:
  - Authentication, authorization, REST APIs, WebSocket orchestration.
  - Persistence orchestration and business workflow coordination.
  - Realtime fanout and notification delivery.
- Rust backend-core owns:
  - Deterministic combat simulation execution.
  - Worker-managed simulation scheduling for per-universe compute isolation.

## Transport
- Protocols: gRPC, N-API, HTTP (internal)
- Contract source: `backend/src/coreAdapter/proto/core.proto`
- Rust service endpoint: `BACKEND_CORE_ADDR` (default `backend-core:50051`)
- Rust HTTP helper endpoint base: `RUST_HTTP_HELPER_URL` (example `http://backend-core:50052`)

## Current Rust-Delegated Flows
- Combat simulation (`SimulateBattle`) via:
  - Node adapter: `backend/src/coreAdapter/rustCoreClient.ts`
  - Node caller: `backend/src/services/combatService.ts`
  - HTTP internal route (when `CORE_TRANSPORT=http`):
    - `POST /api/combat/simulate` on Rust HTTP helper service.
    - Called via `backend/src/services/rustHttpHelperClientService.ts`.
    - Protected by `x-core-helper-token` when `CORE_HTTP_HELPER_TOKEN` is configured.
- Fleet movement math (`CalculateFleetMovement`) via:
  - Node adapter: `backend/src/coreAdapter/rustCoreClient.ts`
  - Node caller: `backend/src/services/fleetService.ts`
  - Scope: distance, travel time, fuel consumption, cargo capacity calculations.
- Fleet movement by-type kernel (N-API-first):
  - Node adapter: `backend/src/coreAdapter/rustCoreNapiClient.ts` (`calculateFleetMovementByTypeNapi`)
  - Node callers:
    - `backend/src/services/fleetService.ts`
    - `backend/src/services/fleetHelperService.ts`
  - Scope: movement math keyed by ship type/count map (Rust-owned ship stats), with cache key using deterministic ship map ordering.
- Fleet post-combat distribution (N-API):
  - Node adapter: `backend/src/coreAdapter/rustCoreNapiClient.ts`
  - Node caller: `backend/src/services/fleetService.ts`
  - Scope: attacker loss allocation + loot split + defender rebuild resolution kernels.
- Espionage outcome kernel (N-API):
  - Node adapter: `backend/src/coreAdapter/rustCoreNapiClient.ts`
  - Node caller: `backend/src/services/fleetService.ts`
  - Scope: intel level + detection chance/decision computation.
  - Deterministic seed: `${fleet.id}:${target.galaxy}:${target.system}:${target.position}:${probes}`.
- Fleet/combat helper REST shims (Rust HTTP proxy optional):
  - Node routes: `backend/src/routes/fleet.ts`
  - Node callers:
    - `backend/src/services/rustHttpHelperClientService.ts` (proxy-first when configured)
    - `backend/src/services/fleetHelperService.ts` (local fallback path)
  - Endpoints:
    - `POST /api/fleet/helpers/movement`
    - `POST /api/fleet/helpers/combat/defense-rebuild`
    - `POST /api/fleet/helpers/combat/attacker-distribution`
    - `POST /api/fleet/helpers/espionage-outcome`
    - `POST /api/fleet/helpers/mission-cargo-transfer`
    - `POST /api/fleet/helpers/harvest-collection`
  - Scope: expose low-risk calculator kernels to clients/admin tooling while keeping DB mutation out of the shim paths.
  - Migration path:
    - Set `RUST_HTTP_HELPER_URL` to proxy helper requests to a Rust HTTP service.
    - If unset or the proxy call fails, Node falls back to local `FleetHelperService`.

## Runtime Controls
- `CORE_ENGINE`:
  - `rust` (default in non-test environments): Rust-first delegation with TS fallback.
  - `ts` / `typescript` / `js`: force TypeScript simulation path.
- `CORE_TRANSPORT`:
  - `auto` (default): prefer N-API when available, otherwise gRPC.
  - `grpc`: Node -> Rust core gRPC path.
  - `napi`: Node -> in-process Rust addon path (`backend-core-napi`) with fallback to gRPC, then TS.
  - `http`: Node -> Rust HTTP helper path for combat simulation (`POST /api/combat/simulate`), then fallback to auto/N-API/gRPC/TS.
- `CORE_UNIVERSE`: universe label passed to Rust for worker routing.
- `BACKEND_CORE_ADDR`: Rust gRPC target address.
- `CORE_NAPI_BINDING_PATH`: optional absolute path to compiled N-API `.node` module.
- `RUST_HTTP_HELPER_URL`: optional HTTP base URL for Rust helper proxy (example: `http://rust-helper:8080`).
- `RUST_HTTP_HELPER_TIMEOUT_MS`: optional timeout for Rust helper HTTP calls (default `2000`ms).
- `CORE_HELPER_TRANSPORT`:
  - `http`: enable Rust HTTP helper transport for mission helper kernels in `FleetService`.
  - unset/other: keep N-API-first mission helper kernels with local fallback.
- `CORE_HTTP_HELPER_TOKEN`: shared helper token sent as `x-core-helper-token` on Rust helper HTTP requests. Required when Rust HTTP helper ingress is configured with token auth.

## 5-Step Migration Matrix
| Step | Scope | Current completion status | Next milestone for full cutover |
| --- | --- | --- | --- |
| 1 | Combat simulation (`SimulateBattle`) on Rust core | Completed (Rust-first, TS fallback remains) | Add rust-only canary mode that fails closed for combat in staging before prod flip. |
| 2 | Fleet movement math (by-type N-API -> fast N-API -> gRPC -> TS) | Completed (Rust-first chain active) | Validate transport SLOs, then gate TS movement fallback behind an emergency-only flag. |
| 3 | Mission helper kernels in fleet orchestration (distribution, defense rebuild, espionage, cargo transfer, harvest) | In progress (Rust N-API/HTTP paths live; local TS fallback still active) | Run staged `CORE_HELPER_TRANSPORT=http` rollout with `RUST_HTTP_HELPER_URL` + `CORE_HTTP_HELPER_TOKEN` configured and monitored. |
| 4 | Fleet helper REST shim routes (`/api/fleet/helpers/*`) | In progress (Rust HTTP proxy-first when configured; local fallback on error) | Enforce authenticated Rust helper ingress and promote Rust proxy to default path in non-test envs. |
| 5 | Backend-wide Rust cutover posture | Pending | Set default runtime profile to Rust-first everywhere (`CORE_ENGINE=rust`, `CORE_TRANSPORT=auto|napi|grpc`, `CORE_HELPER_TRANSPORT=http`) and retire TS mission/combat fallbacks after stability window. |

## Benchmark Tooling
- Transport benchmark script: `backend/scripts/benchmarkCoreTransports.ts`
- Memory benchmark script: `backend/scripts/benchmarkCoreMemory.ts`
- PowerShell launchers (build N-API, optionally start gRPC core, then run benchmark):
  - `backend/scripts/benchCore.ps1`
  - `backend/scripts/benchCoreMemory.ps1`
- Package scripts:
  - `pnpm --dir backend run bench:core:auto`
  - `pnpm --dir backend run bench:core:grpc`
  - `pnpm --dir backend run bench:core:memory:auto`
  - `pnpm --dir backend run bench:core:memory:grpc`
- Memory benchmark runs with Node `--expose-gc` enabled to support pre/post forced-GC sampling.
- Benchmark snapshots are written to `backend/benchmarks/history/`:
  - transport: `core-bench-<timestamp>.json`
  - memory: `core-memory-bench-<timestamp>.json`

## Fallback and Resilience
- Combat simulation path:
  - if `CORE_TRANSPORT=http`: Rust HTTP `POST /api/combat/simulate`
  - then `auto` resolution (N-API if available, else gRPC)
  - then local TypeScript simulation
- Fleet movement path order:
  - by-type N-API kernel
  - fast N-API movement
  - gRPC movement
  - local TypeScript movement
- If N-API movement calls fail, backend falls back to gRPC, then TypeScript simulation path.
- If Rust gRPC call fails, backend falls back to TypeScript simulation path.
- In test environments (`NODE_ENV=test`), default simulation engine is TypeScript unless explicitly overridden.
- Helper REST shims use Rust HTTP proxy first when `RUST_HTTP_HELPER_URL` is configured; on proxy error they fall back to local `FleetHelperService` (Rust N-API first, then TypeScript).
- Espionage mission handling uses Rust N-API first for outcome computation and keeps the existing TypeScript formulas/default thresholds as the fallback path.

## Data Contract Normalization
- Rust returns snake_case protobuf fields.
- Node adapter normalizes results to backend camelCase shape:
  - `attacker_losses -> attackerLosses`
  - `defender_losses -> defenderLosses`
  - round keys normalized to `attackerShots/defenderShots/attackerDestroyed/defenderDestroyed`.

## Non-Goals (Current Phase)
- This phase does not migrate all backend logic to Rust.
- Economy ticks, fleet mission orchestration, alliance logic, and messaging remain Node-managed.

## Next Migration Candidates
- Debris and salvage computation kernels.
- Batch score recomputation kernels for leaderboards.
