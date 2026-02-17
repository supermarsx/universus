# platform-tenancy TODO

- Define middleware to extract tenant from headers or queue metadata.
- Add tenant-aware request tracing (correlate with `tenant_id` in logs).
- Provide utilities to route work items to tenant-specific queues.
- Integrate with `platform-consensus` for tenant-scoped leader election.
