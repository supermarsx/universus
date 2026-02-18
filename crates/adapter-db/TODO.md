# adapter-db TODO

- Postgres wiring currently establishes `tokio_postgres` clients per entry but lacks a connection pool, query abstraction, or diagnostics tied to `AdapterRegistry`. Flesh out a dedicated `PostgresAdapter` module with pool/serde helpers, lifecycle cleanup hooks, and tenant-aware logging.
- The MySQL branch today reuses the Postgres connector, so there is no real `mysql_async`/connection-pool adapter yet. Replace that stub with a proper `MysqlAdapter`, add driver-specific tests, and expose MySQL health info through the registry.
- The JSON adapter only reads the file into a `Mutex<String>` once; it needs real CRUD helpers that respect a JSON schema (table dumps, tenant-specific docs) plus a watch/reload strategy that keeps the registry’s metadata current.
- Document the runtime JSON schema for `AdapterEntry` (driver, tenant, url/path, optional lease/diagnostics hints) so `platform-adapter` and dashboards know how to interpret the config.
- Eventually surface a transaction trait or executor-bound interface that callers can depend on regardless of backend (Postgres/MySQL/JSON) and use it for migrations/tests.
