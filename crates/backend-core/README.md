# backend-core

High-performance Rust service containing the deterministic game loop and
combat simulation engine used by Universus. The core is implemented in Rust
using `tonic` (gRPC) + `prost` (protobuf) and is designed for correctness,
determinism and high throughput. The service exposes a small gRPC API
(`proto/core.proto`) that the Node backend and other services call to run
simulations and progress live battles.

## Contents

- `proto/core.proto` — protobuf service and message definitions (source of
	truth for interop).
- `src/main.rs` — gRPC server, worker manager, and IPC bridge to worker
	processes.
- `src/sim.rs` / `src/ships.rs` — core deterministic simulation logic.
- `src/ipc_local.rs` — local-socket IPC helper used by the worker mode.

## Design goals

- Deterministic simulation: given the same inputs and seed the simulation
	produces identical results.
- Low-latency per-simulation execution; worker processes can be spawned to
	handle parallel simulation workloads per "universe".
- Flexible IPC: supports both TCP and platform-local sockets for worker
	communication (configurable via env).

## Quickstart (local build)

Build and run the server locally (binds to `0.0.0.0:50051` by default):

```bash
cd backend-core
cargo build --release
CORE_BIND_ADDR=0.0.0.0:50051 cargo run --release
```

## Docker

Build the container image:

```bash
docker build -t universus/backend-core:latest .
```

Run the container (example):

```bash
docker run --rm -p 50051:50051 \ 
	-e CORE_BIND_ADDR=0.0.0.0:50051 \ 
	universus/backend-core:latest
```

## Protobuf / gRPC interface

The protobuf file is `proto/core.proto`. Key message and RPC summaries follow
— use the proto as the authoritative specification when generating client
stubs or types.

### Service: `GameLoop`

- `StartBattle(BattleRequest) -> BattleState`
	- Create and initialize a live battle identified by `battle_id` in a
		named `universe`. The server stores a lightweight `BattleState` and
		returns the initial state.

- `StepBattle(StepRequest) -> BattleState`
	- Advance the in-memory battle state by one tick and return the updated
		`BattleState`.

- `StreamBattle(StepRequest) -> stream BattleState`
	- Server-side streaming helper that emits battle state updates (useful
		for websocket-like streaming or real-time UIs).

- `SimulateBattle(SimulateRequest) -> CombatResult`
	- Run an isolated combat simulation based on explicit inputs (ship
		compositions, defenses, tech levels, planet resources, and an optional
		`seed` for deterministic RNG). Returns a `CombatResult` describing the
		outcome.

### Messages (high level)

- `SimulateRequest` — fields include `battle_id`, ship maps for attacker
	and defender, defense maps, technology maps, planet resource counts,
	`seed` and `universe`.
- `CombatResult` — `winner` (`attacker` | `defender` | `draw`), array of
	`RoundResult` objects, losses per side, optional `loot` and `debris`.

See `proto/core.proto` for exact field names and types. The service is
backwards-compatible-friendly — older integrations may return a `json_result`
string in some IPC responses; callers should prefer the structured
`CombatResult` when present.

## Environment configuration

The server is configurable via environment variables:

- `CORE_BIND_ADDR` — gRPC listen address (default `0.0.0.0:50051`).
- `CORE_MAX_WORKERS_PER_UNIVERSE` — maximum worker processes spawned per
	universe (default `4`).
- `CORE_SPAWN_BACKOFF_SECS` — seconds to wait between spawn attempts on
	backoff (default `2`).
- `CORE_WORKER_CONNECT_TIMEOUT_MS` — timeout for connecting to spawned
	worker IPC endpoints (default `500`).
- `CORE_IPC` — IPC mode for spawned workers: `tcp` (default) or `local`.
	`local` uses platform local sockets (Unix domain sockets / named pipes).

## Worker model and IPC

The core uses a manager that maintains a pool of worker processes per
`universe`. Workers can be spawned on demand (up to `CORE_MAX_WORKERS_PER_UNIVERSE`)
and the server picks the least-loaded worker for each simulation request.

Worker processes can run in two modes:

- `--local` — bind a platform-specific local socket and print a handshake
	line `SOCKET:<path>` to stdout. The manager reads this line to perform
	IPC.
- `--tcp` — bind a TCP ephemeral port and print `PORT:<ip:port>` to stdout.

The parent process communicates with workers by serializing an `IPCSimulateRequest`
JSON object and reading a single-line JSON `IPCCombatResult` response. Two
special `cmd` values are supported in the IPC protocol:

- `prewarm` — instructs a worker to pre-load ship/tech data for a universe.
- `drain` — instructs the worker to exit after completing current work.

## Worker CLI

The same binary supports a worker mode. Example:

```bash
# Start a local worker for universe `default`
./backend-core --worker --universe default --local

# Start a TCP worker
./backend-core --worker --universe default --tcp

# Prewarm (via parent IPC or by sending a JSON with `cmd: "prewarm"`)
```

## Determinism and seeds

Simulations are deterministic when driven by the same input and `seed`.
If you require reproducible combat logs for debugging or replay, include a
`seed` in `SimulateRequest`. When `seed` is omitted the server or worker
may use a default deterministic strategy; prefer explicit seeds for
reproducibility.

## Client integration examples

### Node (using existing adapter in this repo):

```ts
import { simulateBattleRust } from '../backend/src/coreAdapter/rustCoreClient';

// New-style request object
const result = await simulateBattleRust({
	battle_id: 'b123',
	attacker_ships: { cruiser: 10 },
	defender_ships: { fighter: 20 },
	seed: 'rando-1',
	universe: 'default',
});

// Legacy signature (kept for compatibility)
const legacy = await simulateBattleRust('b123', ['p1','p2'], 'rando-1');
```

### Direct gRPC invocation (grpcurl):

```bash
grpcurl -plaintext -d '{"battle_id":"b123","attacker_ships":{"cruiser":10},"defender_ships":{},"seed":"s1","universe":"default"}' localhost:50051 core.GameLoop/SimulateBattle
```

## TypeScript / Protobuf generation

You can generate TypeScript clients in several ways:

- `@grpc/proto-loader` + `@grpc/grpc-js` (runtime proto loading) — used by
	some Node adapters in this repo.
- `ts-proto` — generates idiomatic TypeScript files from `.proto` files and
	is recommended if you prefer static types and compile-time validation.

To generate with `ts-proto` (example):

```bash
npx protoc --plugin=protoc-gen-ts_proto --ts_proto_out=./generated --ts_proto_opt=outputServices=grpc-js,esModuleInterop=true -I proto proto/core.proto
```

## Health, logging and observability

- Logging: the service uses `tracing_subscriber::fmt::init()`; configure
	`RUST_LOG` to control verbosity (e.g., `RUST_LOG=info`). Logs are written
	to stdout for container-friendly consumption.
- Metrics: there is no built-in Prometheus exporter in this version. For
	production observability consider adding `opentelemetry` or a metrics
	crate and exporting runtime and worker metrics (latency, queue depth,
	worker count).

## Testing and benchmarks

Unit and integration tests are located under `tests/` and `src/` as
appropriate. To run tests:

```bash
cd backend-core
cargo test --release
```

For microbenchmarks you can add `criterion`-based benches or use the
existing worker harness to run heavy-load simulations for profiling.

## Error handling and status codes

- gRPC errors use `tonic::Status` to convey failures to clients. Common
	status codes used are `internal` (worker spawn/connect failures) and
	`not_found` (missing battle state for `StepBattle`/`StreamBattle`).

## Security

- By default the server binds with plaintext (insecure) credentials. For
	production you should enable TLS on the gRPC server and use mTLS or
	token-based authentication via gRPC metadata.

## Operational guidance

- Worker sizing: tune `CORE_MAX_WORKERS_PER_UNIVERSE` based on CPU and
	observed simulation latency. Workers are independent OS processes to
	reduce GC/heap contention and allow CPU affinity.
- Prewarm workers for busy universes using the `prewarm` IPC command to
	load ships/lookup tables into memory before traffic spikes.
- When performing rolling upgrades, use the `drain` IPC command to stop
	accepting new work on a worker and exit after in-flight tasks finish.

## Compatibility and versioning

- The protobuf file (`proto/core.proto`) is the compatibility contract. When
	extending messages prefer additive changes (new optional fields) and avoid
	renaming existing fields.

## Contact / Contributing

If you work on the simulation logic (in `src/sim.rs` / `src/ships.rs`),
add tests that validate deterministic outputs for a given `seed` and
input fixture. When changing proto messages, coordinate with consumers in
the Node backend to update client stubs.

---

For any questions about deployment or interface changes, open an issue or
contact the maintainers in the project.
