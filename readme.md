# Universus (Browser MMO Strategy)

This repository now hosts the fully Rust-based Universus platform. All runtime services run through the `crates/` workspace, the new Rust frontend, and the Docker Compose stacks that bring up `rust-*` containers. The previous TypeScript services (`backend`, `backend-admin-service`, `backend-bot-service`, `backend-sms-service`, `email-delivery-service`, `frontend`) have been removed in favor of the Rust equivalents.

## Repository layout
- `crates/`: every Rust binary, adapter, domain, and shared platform crate (API gateway, realtime gateway, admin/bot/SMS gateways, workers, domain logic, adapters).
- `database/`: PostgreSQL schema, migrations, seeds.
- `scripts/`: deployment helpers, Rust cutover validation, benchmarks, and test orchestrators.
- `docs/`: live deployment/verification guides that point to the Rust stack.
- `docker-compose.yml`: orchestrates the Rust services plus PostgreSQL, Redis, RabbitMQ.

## Runtime architecture
- `rust-api-gateway` (`crates/app-api-gateway`): replaces the old `backend` REST surface.
- `rust-realtime-gateway` (`crates/app-realtime-gateway`): websocket/realtime endpoints, chat moderation.
- `rust-web-frontend` (`crates/app-web-frontend`): server-rendered UI listening on port `8080`.
- `rust-admin-api`, `rust-bot-api`, `rust-sms-api`: scoped admin, bot, and SMS APIs.
- Worker fleet (`rust-scheduler-worker`, `rust-sharding-worker`, `rust-analytics-worker`, `rust-email-worker`, `rust-bot-worker`): asynchronous schedulers, analytics, email, and bot loops.
- Shared platform crates (`platform-*`, `game-*`, `adapter-*`): persistence, domain logic, adapter integration.

## Quick start — Docker
1. Build the workspace (optional):
   ```
   cargo build --workspace
   ```
2. Start the hard-cut services:
   ```
   docker compose up -d rust-api-gateway rust-realtime-gateway rust-web-frontend rust-admin-api rust-bot-api rust-sms-api rust-core-engine rust-scheduler-worker rust-sharding-worker rust-analytics-worker rust-email-worker rust-bot-worker database-migrate redis rabbitmq
   ```
3. Open the services:
   - UI: http://localhost:8080
   - API gateway: http://localhost:3300
   - Admin API: http://localhost:4302
   - Bot API: http://localhost:4301
4. Tear down:
   ```
   docker compose down
   ```

## Local development & validation
- Run `scripts/rust/run-cutover-validation.ps1` (workspace `cargo check`, parity tests, scheduler/shard stress tests).
- For focused verification:
  - `cargo test -p app-api-gateway --test routes -- --nocapture`
  - `cargo test -p app-sms-api`
- Logs:
  ```
  docker compose logs -f rust-api-gateway
  docker compose logs -f rust-web-frontend
  ```

## Environment
- Copy `.env.example` (root or service-specific) and set:
  ```
  DATABASE_URL=postgres://postgres:<url-encoded-password>@localhost:5432/universus_rpg
  POSTGRES_PASSWORD=<long-random-password>
  DATABASE_URL_INTERNAL=postgres://postgres:<url-encoded-password>@database:5432/universus_rpg
  REDIS_URL=redis://localhost:6379
  RUST_LOG=info
  API_PORT=3300
  WEB_PORT=8080
  ```
- Initialize the database:
  ```
  createdb universus_rpg
  PGDATABASE=universus_rpg database/scripts/migrate-db.sh
  ```
  The canonical runner applies numeric migration order, records SHA-256
  checksums, serializes concurrent runners, and safely skips completed steps.
  Compose runs the same command in the one-shot `database-migrate` service on
  every startup, including when `postgres_data` already exists.

## Testing & observability
- `cargo check --workspace`
- `cargo test --workspace` (standard)
- `scripts/rust/run-cutover-validation.ps1`
- `database/scripts/test-migrations.sh` (PostgreSQL 16 migration durability)

Telemetry/logging:
```
docker compose logs -f rust-api-gateway
docker compose logs -f postgres
docker compose logs -f redis
docker compose logs -f rust-realtime-gateway
```

## Notes
- Legacy Node/TypeScript sources have been deleted; any residual docs referencing Node should be considered historical artifacts.
