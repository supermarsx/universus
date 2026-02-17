# backend-core-napi Retirement Notes

Updated: 2026-02-16

## Summary
- The legacy N-API bridge crate `crates/backend-core-napi` has been retired from active backend migration paths.
- Runtime ownership for combat/fleet engine traffic is on Rust services (`backend-core` and `app-core-engine`) and domain crates.
- The workspace already excluded `backend-core-napi`; source files are now removed.

## Compatibility Context
- Historical Node paths used N-API for local bridge execution.
- Migration replaced this with Rust-native service paths and benchmark harnesses that no longer require N-API at runtime.
- Existing benchmark snapshots remain valid historical artifacts but do not require N-API for current runs.

## Operational Impact
- No compose/runtime service depends on `backend-core-napi`.
- Rust-only runtime paths remain:
  - `rust-core-engine` (`backend-core`)
  - `rust-app-core-engine` (`app-core-engine`)

## Follow-up
- If any external tooling still references N-API artifacts, migrate those calls to `app-core-engine`/`backend-core` endpoints before next release tag.
