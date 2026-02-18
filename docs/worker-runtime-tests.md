# Worker Runtime Leak and Performance Tests

## Purpose
`platform-worker-runtime` provides a shared executor, leak detection, and runtime instrumentation for all worker/process crates. These tests verify that tenants cannot overwhelm the runtime, that memory/CPU caps hold, and that the instrumentation surface is reliable.

## Key instrumentation

- **Heap/CPU gauges**: `platform-worker-runtime` must expose per-worker gauges for heap usage, CPU percent, GC pressure, and queue depth; tests poll those metrics to validate thresholds.
- **Lease-aware execution**: Each worker should register the tenant context/lease ID before processing a job; the tests assert every processed job logs the `tenant`, `shard`, and `lease_owner`.
- **Leak detection hooks**: The runtime should expose counters for pending futures, timed-out tasks, and parked threads; tests monitor those counters before/after running synthetic loads.

## Test scenarios

1. **Leak detection baseline**: Start the runtime and run a controlled synthetic workload (e.g., spawn 1000 no-op futures); ensure the `pending_futures` counter returns to zero within a grace period and no `leak_detected` metric fires.  
2. **Backpressure enforcement**: Flood a tenant’s queue with tasks while the worker’s queue depth surpasses thresholds; expect `platform-tenant-routing` to emit `tenant_backpressure`, the runtime to throttle new work, and `queue_depth_limit` metrics to cross their guard.  
3. **CPU/heap cap compliance**: Run CPU-intensive or GC-heavy tasks via the benchmark runner; assert that once caps are reached, the runtime logs `runtime_cap_reached`, reduces concurrency, and eventually drains the queue without crashing.  
4. **Lease-aware scheduling**: Run tasks for two tenants with conflicting leases and ensure only the tenant whose lease is active receives worker time; the runtime should log `lease_idle` events for the blocked tenant while the other continues.

## Execution guidance

- Implement these tests as integration suites under `crates/platform-worker-runtime/tests` or as dedicated scripts referencing `platform-worker-runtime::tests`.  
- Use `tokio::runtime::Builder` to configure the worker runtime with deterministic metrics/queue thresholds for tests.  
- Leverage existing benchmarking infrastructure (`crates/benchmark-actions`) to run heavy workloads while capturing the leak/performance metrics.  
- Collect logs via `-- --nocapture` and parse the JSON metrics if necessary to assert the desired thresholds.

## Documentation sync

- Link these scenarios from `docs/spec-gap-analysis.md`, `specification/test-scenarios.md`, and `docs/architecture.md` so the status tracker highlights the runtime coverage.  
- Once automation scripts exist, publish the command list in `specification/validation-reports/worker-runtime-2026-02-XX.md` (or merge into the canonical status page).  
- Note any required instrumentation additions in `TODO.md` under the Runtime & sharding section so the work stays tracked.
