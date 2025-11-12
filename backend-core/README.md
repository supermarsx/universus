# backend-core

High-performance Rust service containing the core deterministic game loop and combat simulation for Universus.

This service exposes a gRPC API defined in `proto/core.proto` and is implemented with `tonic` + `prost`.

Quickstart
---------

Build locally:

```
cd backend-core
cargo build --release
```

Run (default binds to `0.0.0.0:50051`):

```
CORE_BIND_ADDR=0.0.0.0:50051 cargo run --release
```

Docker
------

```
docker build -t universus/backend-core:latest .
```

Integration
-----------

- The Node backend should call the gRPC endpoints in `proto/core.proto`.
- Use `@grpc/grpc-js` or generated TypeScript stubs (e.g., via `ts-proto`).
- Ensure proper authentication via gRPC metadata or TLS.
