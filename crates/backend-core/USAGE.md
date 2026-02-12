Backend-core usage and configuration

Overview
- `backend-core` is a gRPC service that runs the game core simulations.
- It supports per-universe ship specs under `backend-core/assets/<universe>/ships.json`.
- The service can spawn per-universe worker processes on demand, prewarm them, and autoscale.

Environment variables
- `CORE_BIND_ADDR` - bind address for the gRPC server (default `0.0.0.0:50051`).
- `CORE_MIN_WORKERS_PER_UNIVERSE` - minimum number of prewarmed workers per universe (default `1`).
- `CORE_MAX_WORKERS_PER_UNIVERSE` - maximum worker processes per universe (default `4`).
- `CORE_SPAWN_BACKOFF_SECS` - spawn backoff in seconds to avoid spawn storms (default `2`).
- `CORE_WORKER_CONNECT_TIMEOUT_MS` - timeout for connecting to a worker in milliseconds (default `500`).

Metrics
- Prometheus metrics are served on `:9090`:
  - `core_workers_total{universe}` - number of active workers per universe
  - `core_worker_load{universe,port}` - current load (in-flight requests) per worker
  - `core_in_flight_requests` - total in-flight simulate requests
  - `core_spawn_total` - total worker spawns
  - `core_request_duration_seconds` - histogram of simulation latencies

Worker control
- Worker IPC supports control commands sent as JSON over the TCP socket:
  - `{"cmd":"prewarm","universe":"<u>"}` - instruct worker to pre-load ship defs
  - `{"cmd":"drain","universe":"<u>"}` - worker will stop accepting new simulations and exit when done

How prewarmed workers are created
- The manager scans `backend-core/assets/` for universes and ensures `CORE_MIN_WORKERS_PER_UNIVERSE` prewarmed workers per universe by spawning the same binary with `--worker --universe <u> --prewarm`.

Notes
- The IPC channel uses localhost TCP and is intended for same-host operation. For multi-host or secure deployments, use unix sockets or TLS.
- Worker processes self-exit after an idle timeout (60s). The manager also prunes dead and unreachable workers.

Running locally
1. Build the crate: `cargo build -p backend-core`.
2. Run the server: `CORE_BIND_ADDR=0.0.0.0:50051 CORE_MIN_WORKERS_PER_UNIVERSE=1 cargo run -p backend-core`.
