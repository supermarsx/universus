# Platform Tenant Routing

## Purpose
`platform-tenant-routing` is the glue between tenant-aware HTTP/queue traffic and the underlying sharding, consensus, and worker runtime layers. This doc catalogs the public interface, quota/backpressure semantics, and the automated tests that prove tenant isolation, routing reconfiguration, and consensus-aware failover.

## Interface

| Component | Responsibility |
| --- | --- |
| `TenantRoute::map_request` | Given an Axum/Tower request, resolve the tenant ID via headers/cookies and translate it into a `TenantDescriptor` (tenant ID, shard, consensus lease resource, queue priority). |
| `TenantRoute::acquire_lease` | Calls into `platform-consensus` to acquire a `TenantLease` for the route’s resource hint before scheduling work; leases auto-renew and are released on request completion or on explicit `RouteGuard::drop`. |
| `TenantRoute::backpressure` | Checks the `platform-worker-runtime` runtime metrics (queue depth, CPU thresholds) before allowing a request or job to run; it can reject new work with `429`/queue backpressure when the tenant’s quota is exhausted. |
| `TenantRoute::route_lock` | Exposes the route’s shard/worker affinity so schedulers can skip tenant-shard conflicts and ensure only one shard owns a tenant’s active requests at a time. |

Routes should log the resolved tenant ID, lease metadata (lease owner, expiry, resource hint), and any applied modifiers (`ResourceHint::critical`, `priority`, `quota`). `platform-tenant-routing` calls the shared `platform-observability` spans/metrics to expose each decision in dashboards and time-series.

## Quota & Backpressure Semantics

- Each tenant defines per-route quotas via `platform-tenant-routing` configuration entries (see `specification/spec-rust-crate-partition.md` for the schema). Requests incur weight based on route complexity (e.g., resource churn, migrations, cancellations).
- When a tenant hits its quota, `platform-tenant-routing` triggers `platform-worker-runtime::rate_limit::limit_tenant` which either queues the request for later or returns `429`/`Retry-After`.
- Queues (notifications, migrations, benchmarks) query `platform-tenant-routing` for the tenant’s shard/leasing state to avoid cross-tenant interference; the queue dispatcher rejects tasks referencing tenants whose leases are held by other schedulers.
- Backpressure thresholds use `platform-worker-runtime` instrumentation (queue depth, latency percentiles, GC pressure). When thresholds breach, routing emits a `tenant_backpressure` metric plus a `TenantBackpressure` event, which triggers consensus-based alerts in `platform-observability`.

## Test Harness & Scenarios

The following validation suites live under `specification/test-scenarios.md` and the future “Rust backend status” page, referencing the dedicated docs below. Each scenario is executed against the `app-*` crates plus the queue processors.

1. **HTTP tenant isolation**: Start `app-api-gateway` with a JSON-only adapter, craft concurrent requests from two tenants (one blocked by `platform-tenant-routing` quotas, the other allowed), and assert that the blocked tenant receives `429` while the other request succeeds. Log the telemetry so operator dashboards show the routed quotas plus leases.
2. **Queue routing failover**: Launch two scheduler workers, assign a tenant’s lease to Worker A, enqueue tasks from Worker B, and confirm the tasks are deferred until Worker A releases the lease. This scenario exercises `platform-consensus` lease contention alongside `platform-tenant-routing` queue checks.
3. **Lease reconfiguration**: Simulate a tenant relocation by forcing `platform-consensus` to drop Worker A’s lease; verify that `platform-tenant-routing` reroutes incoming requests to Worker B and that the tenant’s metrics report a lease handoff event plus a “tenant routed to new shard” log entry.
4. **Migration guard handshake**: When `platform-migrations` wants to run, it calls `platform-tenant-routing` to transition the tenant into a migration-only shard and acquires the migration lease. The CLI/automation must see the `TenantRoutingEvent::migration_lock` event, confirm the lease, and proceed only when the route guard is granted.

Each scenario uses `testcontainers` to boot the relevant adapters when needed, relies on the new `adapter-db/tests` parity suite for tenancy logs, and can be scripted via the existing migration-transfer CLI plus the `scripts/rust/live-rust-cutover-check.ps1` steps.

## Next steps

1. Track the above scenarios inside `specification/test-scenarios.md` (done) and expand them into automation scripts once `platform-worker-runtime` instrumentation is ready.
2. Glue `platform-tenant-routing` metrics into `platform-observability` dashboards so lease acquisitions, backpressure events, and shard handoffs are observable.
3. Once the runtime & scheduler crates are production-ready, consume this doc inside the canonical `docs/architecture.md` summary and retire any duplicate Node-era routing guidance.
