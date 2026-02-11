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
- Protocol: gRPC
- Contract source: `backend/src/coreAdapter/proto/core.proto`
- Rust service endpoint: `BACKEND_CORE_ADDR` (default `backend-core:50051`)

## Current Rust-Delegated Flows
- Combat simulation (`SimulateBattle`) via:
  - Node adapter: `backend/src/coreAdapter/rustCoreClient.ts`
  - Node caller: `backend/src/services/combatService.ts`
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
- Fleet/combat helper REST shims (N-API-first):
  - Node routes: `backend/src/routes/fleet.ts`
  - Node caller: `backend/src/services/fleetHelperService.ts`
  - Endpoints:
    - `POST /api/fleet/helpers/movement`
    - `POST /api/fleet/helpers/combat/defense-rebuild`
    - `POST /api/fleet/helpers/combat/attacker-distribution`
  - Scope: expose low-risk calculator kernels to clients/admin tooling while keeping DB mutation out of the shim paths.

## Runtime Controls
- `CORE_ENGINE`:
  - `rust` (default in non-test environments): Rust-first delegation with TS fallback.
  - `ts` / `typescript` / `js`: force TypeScript simulation path.
- `CORE_TRANSPORT`:
  - `auto` (default): prefer N-API when available, otherwise gRPC.
  - `grpc`: Node -> Rust core gRPC path.
  - `napi`: Node -> in-process Rust addon path (`backend-core-napi`) with fallback to gRPC, then TS.
- `CORE_UNIVERSE`: universe label passed to Rust for worker routing.
- `BACKEND_CORE_ADDR`: Rust gRPC target address.
- `CORE_NAPI_BINDING_PATH`: optional absolute path to compiled N-API `.node` module.

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
- Fleet movement path order:
  - by-type N-API kernel
  - fast N-API movement
  - gRPC movement
  - local TypeScript movement
- If N-API movement calls fail, backend falls back to gRPC, then TypeScript simulation path.
- If Rust gRPC call fails, backend falls back to TypeScript simulation path.
- In test environments (`NODE_ENV=test`), default simulation engine is TypeScript unless explicitly overridden.
- Helper REST shims use Rust N-API first and immediately fall back to TypeScript local calculators when the binding is unavailable or returns an error.
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
