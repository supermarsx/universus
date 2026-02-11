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

## Runtime Controls
- `CORE_ENGINE`:
  - `rust` (default in non-test environments): Rust-first delegation with TS fallback.
  - `ts` / `typescript` / `js`: force TypeScript simulation path.
- `CORE_UNIVERSE`: universe label passed to Rust for worker routing.
- `BACKEND_CORE_ADDR`: Rust gRPC target address.

## Fallback and Resilience
- If Rust gRPC call fails, backend falls back to TypeScript simulation path and logs the failure.
- In test environments (`NODE_ENV=test`), default simulation engine is TypeScript unless explicitly overridden.

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
- Fleet travel time + fuel computation.
- Debris and salvage computation kernels.
- Batch score recomputation kernels for leaderboards.
