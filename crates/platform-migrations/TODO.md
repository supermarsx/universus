# platform-migrations ToDo

- Wire migration runner to `adapter-db` so scripts execute against the chosen backend (Postgres/MySQL/JSON).
- Expose status via the Rust admin API (`app-admin-api`) and include a tenant selector.
- Add CLI/binary that can be triggered from `scripts/rust` or a Kubernetes job before deployments.
