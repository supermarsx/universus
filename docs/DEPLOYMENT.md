# SpaceEmpire Deployment Guide

## Quick Start

### Using Docker (Recommended)

1. **Prerequisites**:
   - Docker Engine 20.10+
   - Docker Compose 2.0+

2. **Start the application**:
   ```bash
   cd /workspace/universus-rpg
   cp .env.example .env
   # Provision the Ed25519 keys and scoped service credentials described below.
   docker compose up --build -d
   ```
   PostgreSQL becomes healthy only after its final server process is running.
   The one-shot `database-migrate` service then upgrades both fresh and
   existing volumes before any database-backed Rust service can start.

3. **Access the game**:
   The Rust web frontend is available at `http://localhost:8080` (the Rust API gateway sits behind it on `http://localhost:3300`).

4. **Stop the application**:
   ```bash
   docker compose down
   ```

### Manual Setup (Development)

1. **Prerequisites**:
   - Rust toolchain (stable)
   - PostgreSQL 16+
   - Redis 7+
   - Docker Compose 2.x

2. **Database Setup**:
   ```bash
   createdb universus_rpg
   export PGDATABASE=universus_rpg
   export PGUSER=postgres
   export PGPASSWORD='<database-password>'
   database/scripts/migrate-db.sh
   ```
   Do not apply individual SQL files. The runner owns semantic numeric order,
   per-step atomic transactions, advisory locking, immutable checksums, and
   migration history.

3. **Start Redis**:
   ```bash
   redis-server
   ```

4. **Configure Environment**:
   ```bash
   cp .env.example .env
   # Edit DATABASE_URL, REDIS_URL, RUST_LOG, and other settings as needed
   # For loopback-only HTTP Compose testing, also set:
   # COOKIE_SECURE=false
   # UNIVERSUS_ALLOW_INSECURE_LOCAL_HTTP_COOKIE=true
   ```

5. **Build and run Rust services**:
   ```bash
   cargo build --workspace
   docker compose up -d rust-api-gateway rust-realtime-gateway rust-web-frontend rust-admin-api rust-bot-api rust-sms-api
   ```

6. **Access the game**:
   Visit `http://localhost:8080` for the Rust web frontend and `http://localhost:3300` for the Rust API gateway.

## Configuration

### Environment Variables

Edit the top-level `.env` (or the service-specific `.env` files `crates/app-api-gateway/.env`, etc.):

```env
# Database
DATABASE_URL=postgres://postgres:<url-encoded-password>@localhost:5432/universus_rpg
POSTGRES_DB=universus_rpg
POSTGRES_USER=postgres
POSTGRES_PASSWORD=<long-random-password>
DATABASE_URL_INTERNAL=postgres://postgres:<url-encoded-password>@database:5432/universus_rpg

# Redis
REDIS_URL=redis://localhost:6379

# Observability
RUST_LOG=info

# Web/API ports
API_PORT=3300
WEB_PORT=8080

# Browser session cookie. Production/staging defaults to true when omitted.
COOKIE_SECURE=true
```

The password represented in both URLs must be the URL-encoded form of
`POSTGRES_PASSWORD`.

The frontend rejects `COOKIE_SECURE=false` in production or staging unless
`UNIVERSUS_ALLOW_INSECURE_LOCAL_HTTP_COOKIE=true` is also set. That override is
only for an isolated loopback HTTP test of the production-mode Compose stack;
never use it on a shared network or deployment. Development and test processes
default to non-Secure cookies so direct `http://localhost` workflows continue
to work. Production traffic must terminate TLS before reaching the frontend.

### Signing keys, audiences, and service identities

Production and staging use Ed25519 (`alg=EdDSA`) JWTs. The API gateway is the
only online issuer and the only container that receives the private seed.
Frontend, admin, bot, realtime, privacy-worker delivery, and gateway request
validators receive only the public verification-key map and their own
`AUTH_EXPECTED_AUDIENCE`.
`JWT_SECRET`/HS256 is rejected in production-like environments.

Generate a key pair on a trusted provisioning host:

```bash
cargo run -p platform-auth --bin auth-keygen -- primary-2026-07
```

The command intentionally prints the private seed once. Send it directly to a
secret manager, do not paste it into tickets or logs, and do not provision it
to workers or verifier-only services. Configure:

```env
AUTH_JWT_ISSUER=https://auth.universus.internal
AUTH_JWT_SIGNING_KEY_ID=primary-2026-07
AUTH_JWT_PRIVATE_KEY_BASE64=<private-seed> # API gateway only
AUTH_JWT_VERIFICATION_KEYS=primary-2026-07:<public-key>
```

User access tokens carry all intentional user-facing audiences
(`app-api-gateway`, `app-web-frontend`, `app-admin-api`, `app-bot-api`,
`app-realtime-gateway`, and `app-privacy-worker`; the last is used by the
gateway-forwarded export delivery flow). Every verifier still requires its own
audience.
Refresh tokens have `purpose=refresh` and are accepted only by the refresh
flow; API, admin, bot, and realtime authorization reject them.

Workers do not mint tokens. Provision a distinct `role=service`,
`purpose=service` credential for each worker with one target audience and the
minimum scope. With the issuer variables loaded into the provisioning shell,
set `AUTH_TOKEN_ISSUER=true`, `AUTH_EXPECTED_AUDIENCE=app-api-gateway`, and run:

```bash
cargo run -p platform-auth --bin issue-service-token -- app-bot-worker app-bot-api bot.process 86400
cargo run -p platform-auth --bin issue-service-token -- app-bot-worker-events app-realtime-gateway realtime.publish 86400
```

Store each output directly in the corresponding secret named in
`.env.example`. Generate separate `realtime.publish` credentials for the API
gateway and the email, analytics, core-engine, notifications, chat, scheduler,
sharding, and privacy workers. A realtime publisher cannot call realtime moderation or
read recent events; the bot worker's `bot.process` credential cannot manage
bot accounts.

For zero-downtime key rotation:

1. Generate a new key with a new `kid`.
2. Deploy `AUTH_JWT_VERIFICATION_KEYS=old:<old-public>,new:<new-public>` to all verifiers.
3. Switch only the gateway's signing key ID/private seed and reissue service credentials.
4. After every access, refresh, and service token signed by the old key has expired, remove the old public key.

For direct local `cargo` development only, explicit HS256 compatibility remains
available with `UNIVERSUS_ENV=development`, `AUTH_ALLOW_LEGACY_HS256=true`, and
a local-only `JWT_SECRET`. Never reuse that secret or mode in staging or
production.

### Privacy export encryption and worker health

`rust-privacy-worker` starts only after PostgreSQL is healthy and
`database-migrate` has completed successfully. It claims the durable
`privacy_outbox` with expiring PostgreSQL leases, so another uniquely named
replica can recover jobs after a crash. Set a distinct `PRIVACY_WORKER_ID` for
every replica; reusing an ID weakens stale-owner protection.

Subject-access JSON is never stored as plaintext. The worker bounds serialized
exports, calculates a SHA-256 plaintext digest, and encrypts them with
AES-256-GCM using a random 96-bit nonce and authenticated envelope version
`v1`. Provision the active key on a trusted host:

```bash
openssl rand -base64 32
```

Store each output directly in a secret manager and provision a JSON keyring,
for example `{"v1:2026-06":"<old-key>","v1:2026-07":"<active-key>"}` through
`PRIVACY_EXPORT_KEYRING_JSON`. Set `PRIVACY_EXPORT_ACTIVE_KEY_ID=v1:2026-07`.
The active ID must exist in the keyring and every value must decode to exactly
32 bytes; missing or malformed values fail closed. Provision the identical
keyring and active ID to the API gateway for encrypted correction requests and
to the privacy worker for export encryption and delivery. During rotation, add
the new key before switching the active ID and retain old keys until every
artifact bearing the old ID has expired. The database stores the key ID, nonce,
ciphertext, digest, and size—not the encryption key or plaintext.

The production API gateway also requires an independently generated
`AUTH_SESSION_DIGEST_KEY` and `PRIVACY_REQUEST_IP_PEPPER`, each containing at
least 32 random bytes. Never reuse either value as an export or communications
key. Compose connects the gateway to the worker with
`PRIVACY_WORKER_INTERNAL_URL=http://rust-privacy-worker:3010` and does not start
the gateway until worker readiness succeeds. The worker validates forwarded
access tokens with `AUTH_EXPECTED_AUDIENCE=app-privacy-worker` and checks the
live PostgreSQL session before issuing or consuming one-time delivery grants.
The admin API validates `app-admin-api` tokens and live sessions against the
same migrated database; its container listens on port 3001 and Compose maps
`${ADMIN_PORT:-4302}` to that port.

Relevant operational settings are documented in `.env.example`. Keep both
`PRIVACY_WORKER_CLAIM_TIMEOUT_SECS` and `PRIVACY_WORKER_JOB_TIMEOUT_SECS`
below `PRIVACY_WORKER_LEASE_SECS`; Compose uses 5, 55, and 60 seconds so a
stalled claim is bounded and a timed-out handler can record its retry before
the lease expires. `PRIVACY_WORKER_RUN_ONCE=true` claims one
bounded batch, waits for those handlers, then exits. Normal shutdown stops new
claims and lets the current bounded batch finish.

The worker serves unauthenticated, non-sensitive container-local probes on
port 3010:

- `/health` reports process liveness.
- `/ready` is fail-closed until the privacy schema is verified, becomes
  unavailable after database/lease-recording failures, and rejects stale
  database-success state.

Compose probes readiness by executing
`/usr/local/bin/app-privacy-worker healthcheck` inside the container; the
health image needs no shell HTTP client. Operational realtime events contain
only job kinds, attempt numbers, stable error codes, and aggregate counts—no
user, request, tenant, token, export, or payload values.

## Production Deployment

### Cloud Deployment (AWS/GCP/Azure)

1. **Setup VM instance** with Docker installed

2. **Clone repository**:
   ```bash
   git clone <repository>
   cd universus-rpg
   ```

3. **Configure production environment**:
   ```bash
   cp .env.example .env
   # Provision Ed25519 keys, per-worker scoped service tokens, database
   # passwords, trusted realtime origins, and the remaining production values.
   ```

4. **Deploy**:
   ```bash
   docker-compose up -d
   ```

5. **Setup reverse proxy** (nginx):
   ```nginx
   server {
       listen 80;
       server_name yourdomain.com;

       location / {
           proxy_pass http://localhost:3000;
           proxy_http_version 1.1;
           proxy_set_header Upgrade $http_upgrade;
           proxy_set_header Connection 'upgrade';
           proxy_set_header Host $host;
           proxy_cache_bypass $http_upgrade;
       }
   }
   ```

6. **SSL Setup** (Let's Encrypt):
   ```bash
   certbot --nginx -d yourdomain.com
   ```

### Database Backup

```bash
# Create backup
docker exec universus_database pg_dump -U postgres -d universus_rpg > backup.sql

# Restore backup
docker exec -i universus_database psql -U postgres -d universus_rpg < backup.sql
```

## Monitoring

### View Logs

```bash
# All services
docker compose logs -f

# Rust services
docker compose logs -f rust-api-gateway
docker compose logs -f rust-web-frontend
docker compose logs -f rust-bot-api
docker compose logs -f rust-realtime-gateway
docker compose logs -f postgres
docker compose logs -f redis
```

### Check Service Status

```bash
docker-compose ps
```

## Troubleshooting

### Database Connection Issues

1. Check if PostgreSQL is running:
   ```bash
   docker-compose ps postgres
   ```

2. Verify credentials in `.env`

3. Check logs:
   ```bash
   docker-compose logs postgres
   ```

### Redis Connection Issues

1. Test Redis connection:
   ```bash
   docker exec universus_redis redis-cli ping
   ```

2. Check logs:
   ```bash
   docker-compose logs redis
   ```

### Application Errors

1. Check Rust gateway logs:
   ```bash
   docker compose logs -f rust-api-gateway
   ```

2. Restart services:
   ```bash
   docker compose restart rust-api-gateway rust-realtime-gateway
   ```

## Scaling

### Horizontal Scaling

To scale the Rust API gateway:

1. Update `docker-compose.yml`:
   ```yaml
   rust-api-gateway:
     deploy:
       replicas: 3
   ```

2. Add load balancer (nginx/HAProxy)

3. Redis adapter handles WebSocket session sharing

### Database Optimization

1. **Connection Pooling**: Already configured (max: 20 connections)

2. **Indexes**: All critical queries indexed in schema

3. **Read Replicas**: For high read loads

## Security Checklist

- [ ] Change default passwords
- [ ] Keep the Ed25519 private seed only on the API gateway issuer
- [ ] Give every worker a distinct, short-lived, audience-bound service token
- [ ] Keep old and new public `kid` entries during key rotation
- [ ] Enable HTTPS
- [ ] Set up firewall rules
- [ ] Regular database backups
- [ ] Keep dependencies updated
- [ ] Monitor logs for suspicious activity

## Performance Tuning

1. **Game Speed**: Adjust `GAME_SPEED` in .env (1-10)

2. **Resource Production**: Modify `RESOURCE_PRODUCTION_MULTIPLIER`

3. **Database**: Tune PostgreSQL settings based on load

4. **Redis**: Configure maxmemory and eviction policy

## Support

For issues or questions, please refer to the README.md or check the game logs.
