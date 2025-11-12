# Integration Guide: backend-core

This document explains how to integrate the Rust `backend-core` service with the existing Node backend.

Service contract
----------------
- Proto: `backend-core/proto/core.proto`
- Service: `GameLoop`
  - `StartBattle(BattleRequest) -> BattleState`
  - `StepBattle(StepRequest) -> BattleState`
  - `StreamBattle(StepRequest) -> stream BattleState`

Node client (example)
---------------------
Install dependencies for Node gRPC client generation:

- Option A: Use `grpc-tools` and `@grpc/grpc-js` (pure JS)
- Option B: Use `ts-proto` to generate TypeScript client types and stubs

Minimal example using `@grpc/grpc-js` + `@grpc/proto-loader`:

```js
const grpc = require('@grpc/grpc-js');
const protoLoader = require('@grpc/proto-loader');
const packageDef = protoLoader.loadSync('backend-core/proto/core.proto', {});
const grpcObj = grpc.loadPackageDefinition(packageDef).core;
const client = new grpcObj.GameLoop('backend-core:50051', grpc.credentials.createInsecure());

client.StartBattle({ battle_id: 'b1', player_ids: ['p1','p2'] }, (err, res) => {
  if (err) throw err;
  console.log('started', res);
});

// Stream example
const call = client.StreamBattle({ battle_id: 'b1' });
call.on('data', (state) => console.log('tick', state));
call.on('end', () => console.log('stream ended'));
```

Adapter pattern
---------------
Create a `core-adapter` module inside your Node backend that:
- Converts your internal battle models into `BattleRequest`/`StepRequest`.
- Converts `BattleState` proto into your internal model.
- Handles connection pooling, retries, and health-checks.

Security
--------
- Attach an auth token in gRPC metadata for each call.
- Alternatively, run the Rust service behind your existing internal network and rely on network ACLs.
- Consider mTLS for production.

Deployment
----------
- Add `backend-core` to your `docker-compose.yml` and expose port `50051`.
- Configure `CORE_BIND_ADDR` if you need to bind to a different address.

Observability
-------------
- The Rust service uses `tracing`. Wire up logs to stdout and collect with your logging stack.
- Consider adding Prometheus metrics from Rust (e.g., `prometheus` crate + exporter) for tick rates and queue sizes.

Versioning
----------
Keep `proto/core.proto` stable or add `v1` namespace. Use semantic changes carefully.
