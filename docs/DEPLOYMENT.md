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

3. **Access the game**:
   The Rust web frontend is available at `http://localhost:8080` (the Rust API gateway sits behind it on `http://localhost:3300`).

4. **Stop the application**:
   ```bash
   docker compose down
   ```

### Manual Setup (Development)

1. **Prerequisites**:
   - Rust toolchain (stable)
   - PostgreSQL 15+
   - Redis 7+
   - Docker Compose 2.x

2. **Database Setup**:
   ```bash
   createdb universus_rpg
   psql -U postgres -d universus_rpg -f database/sql/schema.sql
   ```

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
DATABASE_URL=postgres://postgres:postgres@localhost:5432/universus_rpg

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

The frontend rejects `COOKIE_SECURE=false` in production or staging unless
`UNIVERSUS_ALLOW_INSECURE_LOCAL_HTTP_COOKIE=true` is also set. That override is
only for an isolated loopback HTTP test of the production-mode Compose stack;
never use it on a shared network or deployment. Development and test processes
default to non-Secure cookies so direct `http://localhost` workflows continue
to work. Production traffic must terminate TLS before reaching the frontend.

### Signing keys, audiences, and service identities

Production and staging use Ed25519 (`alg=EdDSA`) JWTs. The API gateway is the
only online issuer and the only container that receives the private seed.
Frontend, admin, bot, realtime, and gateway request validation receives only
the public verification-key map and its own `AUTH_EXPECTED_AUDIENCE`.
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
(`app-api-gateway`, `app-web-frontend`, `app-admin-api`, `app-bot-api`, and
`app-realtime-gateway`). Every verifier still requires its own audience.
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
and sharding workers. A realtime publisher cannot call realtime moderation or
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
docker exec universus_postgres pg_dump -U postgres universus_rpg > backup.sql

# Restore backup
docker exec -i universus_postgres psql -U postgres universus_rpg < backup.sql
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
