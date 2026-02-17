# SpaceEmpire Deployment Guide

## Quick Start

### Using Docker (Recommended)

1. **Prerequisites**:
   - Docker Engine 20.10+
   - Docker Compose 2.0+

2. **Start the application**:
   ```bash
   cd /workspace/universus-rpg
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
```

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
   # Edit docker-compose.yml with production settings
   # Update JWT_SECRET, database passwords, etc.
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
- [ ] Use strong JWT secret
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
