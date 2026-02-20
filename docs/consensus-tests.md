# Platform Consensus Tests

## Purpose
`platform-consensus` coordinates leases, routing, and migrations, so the Rust backend needs concrete validation for contention, failover, and the route/queue guard integration. This document captures the scenarios that keep the tenancy guarantees alive and the instrumentation that makes them observable.

## Required instrumentation

- **Lease lifecycle telemetry**: Every `TenantLease` acquisition, renewal, release, or failure should emit `platform-observability` metrics that include `tenant`, `resource_hint`, `owner`, `expiry`, and `state`. The tests below assert those dimensions.
- **Routing guard hooks**: `platform-tenant-routing` must expose hooks (`RouteGuard::acquire`, `RouteGuard::fail`, `RouteGuard::release`) so tests can await the correct path and assert the emitted `TenantRoutingEvent`.
- **Queue/shard context**: Workers need to tag their logs/metrics with `shard` and `queue` metadata plus the `TenantLease` ID so the tests can confirm tasks only run where a lease is held.
- **Migration guard handshake**: `platform-migrations` must accept a proxy lease from `platform-tenant-routing` before running migrations; the test ensures `TenantRoutingEvent::migration_lock` fires and the migration waits until the lease arrives.

## High-level scenarios

| Scenario | Objective | Key assertions |
| --- | --- | --- |
| **Lease contention (HTTP)** | Verify `platform-tenant-routing` rejects overlapping requests for the same tenant when only one lease is available. | Tenant A’s first request acquires a lease; a concurrent request receives `429` with a `Retry-After` hint while a `tenant_lease_contention` metric fires. |
| **Lease failover (worker)** | Confirm a worker can hand off a tenant’s lease to another worker without data loss. | Worker A holds the lease, crashes (simulated drop), Worker B acquires the lease automatically after `platform-consensus` detects the expiry, and queued tasks resume with new `tenant` / `lease_owner` tags. |
| **Consensus lease under load** | Ensure the system renews leases under heavy queue pressure and revokes them when a worker exceeds `platform-worker-runtime` thresholds. | While running benchmark actions or queue bursts, `platform-consensus` sends renewal requests every `lease_renewal_interval`; tests assert renewals succeed and no duplicate ownership metrics are emitted. |
| **Migration guard** | Validate `platform-migrations` requests a specialized migration lease before altering schema. | The migration runner waits for `TenantRoutingEvent::migration_lock`, obtains the lease, runs migrations, then releases the lease; the test verifies a migration metric (`migration_lease_acquired`) plus the existing queue/HTTP flows are blocked during the change.

## Testing strategy

1. **Simulated cluster**: Use `testcontainers` or local JSON adapters to spin up an instance of `platform-consensus`, `platform-tenant-routing`, and two `platform-worker-runtime` nodes. Drive HTTP requests and queue jobs via the API gateway, and observe `platform-observability` logs.
2. **Telemetry validation**: Use the `logs` or `metrics` output from `platform-observability` to assert that each scenario emits the expected `lease`/`routing` events. The tests may run with `-- --nocapture` and parse JSON logs if necessary.
3. **Failure injection**: Introduce simulated worker crashes by sending `SIGTERM` (or aborting the async task) to `platform-worker-runtime`; ensure `platform-consensus` rebalances leases immediately and `platform-tenant-routing` routes new requests to the new owner.
4. **Benchmark (1M action)**: Extend the existing `crates/benchmark-actions` scenario to capture lease renewals and backpressure metrics when tenants issue millions of actions. Compare the `lease_owner` tags before/after the run to spot contention or duplicates.
5. **Runbook**: Use `scripts/rust/run-consensus-worker-validation.ps1` as the CLI harness that executes the consensus lease tests, scheduler routing/failover suites, worker runtime threshold suites, and optional adapter-db SQL parity flows (when Docker is available). Pass `-NoDocker` to skip the SQL parity portion on machines that cannot reach Docker.

## Observability & docs

- Update `docs/tenant-routing.md` and `docs/architecture.md` with references to these scenarios once automation scripts exist.
- Once the tests are implemented, register their commands in `specification/test-scenarios.md` and `docs/spec-gap-analysis.md` so operators know how to execute them.
- Add these scenarios to the canonical status page (future doc) to show `platform-consensus` is fully guarded.
