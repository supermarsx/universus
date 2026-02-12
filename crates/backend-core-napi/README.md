# backend-core-napi

Native Node addon wrapper for Rust simulation kernels.

## Exports
- `simulateBattle(payloadJson: string): string`
- `calculateFleetMovement(payloadJson: string): string`

Both functions accept JSON strings and return JSON strings for low marshalling overhead.

## Build
From repo root:

```bash
cargo build -p backend-core-napi --release
```

Then point backend to the built module:

```bash
CORE_TRANSPORT=napi
CORE_NAPI_BINDING_PATH=<absolute-path-to-.node-file>
```

If the binding is unavailable, backend falls back to gRPC and then TypeScript logic.
