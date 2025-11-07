# SpaceEmpire RPG - Production Deployment Guide

## Overview

This guide provides step-by-step instructions for deploying SpaceEmpire RPG to a production environment. The application is a full-stack MMO browser game with:

- **Backend**: Node.js + TypeScript + Express.js
- **Database**: PostgreSQL 14+ + Redis 7+
- **Frontend**: HTML5 + Vanilla JavaScript + Canvas
- **Real-time**: Socket.io
- **Deployment**: Docker + Docker Compose

---

## Prerequisites

### System Requirements
- **OS**: Linux (Ubuntu 20.04+ recommended) or macOS
- **RAM**: Minimum 2GB, recommended 4GB+
- **Storage**: Minimum 10GB free space
- **CPU**: 2+ cores recommended
- **Network**: Public IP address or domain name

### Software Requirements
- Docker 20.10+
- Docker Compose 1.29+
- Git
- SSL certificate (Let's Encrypt recommended for production)

### Accounts Needed
- Stripe account (for payment processing)
- Email service (for notifications - optional)
- Cloud hosting provider account (AWS, DigitalOcean, etc.)

---

## Step 1: Initial Setup

### 1.1 Clone Repository
```bash
git clone https://github.com/your-repo/ogame-rpg.git
cd ogame-rpg
```

### 1.2 Environment Configuration

Create `.env` file in the project root:

```bash
# Backend Configuration
NODE_ENV=production
PORT=3000

# Database Configuration
DATABASE_URL=postgresql://postgres:your_strong_password@postgres:5432/ogame_rpg
DB_HOST=postgres
DB_PORT=5432
DB_NAME=ogame_rpg
DB_USER=postgres
DB_PASSWORD=your_strong_password

# Redis Configuration
REDIS_URL=redis://redis:6379
REDIS_HOST=redis
REDIS_PORT=6379

# JWT Configuration
JWT_SECRET=your_very_strong_jwt_secret_min_32_characters
JWT_EXPIRY=7d

# Stripe Configuration (Production Keys)
STRIPE_SECRET_KEY=sk_live_your_stripe_secret_key
STRIPE_PUBLISHABLE_KEY=pk_live_your_stripe_publishable_key
STRIPE_WEBHOOK_SECRET=whsec_your_webhook_secret

# Game Configuration
GAME_SPEED=1
FLEET_SPEED=1
RESEARCH_SPEED=1
PRODUCTION_SPEED=1

# Admin Configuration
ADMIN_EMAIL=admin@yourdomain.com
```

**IMPORTANT**: Replace all placeholder values with strong, unique values.

### 1.3 Generate Strong Secrets

```bash
# Generate JWT secret (minimum 32 characters)
openssl rand -base64 32

# Generate database password
openssl rand -base64 24
```

---

## Step 2: Database Setup

### 2.1 Update Docker Compose for Production

Edit `docker-compose.yml`:

```yaml
version: '3.8'

services:
  postgres:
    image: postgres:14-alpine
    environment:
      POSTGRES_DB: ogame_rpg
      POSTGRES_USER: postgres
      POSTGRES_PASSWORD: ${DB_PASSWORD}
    volumes:
      - postgres_data:/var/lib/postgresql/data
      - ./backend/src/database/schema.sql:/docker-entrypoint-initdb.d/01-schema.sql
    ports:
      - "5432:5432"
    restart: unless-stopped
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 10s
      timeout: 5s
      retries: 5

  redis:
    image: redis:7-alpine
    command: redis-server --appendonly yes
    volumes:
      - redis_data:/data
    ports:
      - "6379:6379"
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s
      timeout: 5s
      retries: 5

  backend:
    build:
      context: ./backend
      dockerfile: Dockerfile
    depends_on:
      postgres:
        condition: service_healthy
      redis:
        condition: service_healthy
    environment:
      - NODE_ENV=production
      - DATABASE_URL=${DATABASE_URL}
      - REDIS_URL=${REDIS_URL}
      - JWT_SECRET=${JWT_SECRET}
      - STRIPE_SECRET_KEY=${STRIPE_SECRET_KEY}
      - STRIPE_WEBHOOK_SECRET=${STRIPE_WEBHOOK_SECRET}
    ports:
      - "3000:3000"
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3000/api/health"]
      interval: 30s
      timeout: 10s
      retries: 3

volumes:
  postgres_data:
  redis_data:
```

### 2.2 Apply Database Migrations

```bash
# Start database services
docker-compose up -d postgres redis

# Wait for PostgreSQL to be ready
sleep 10

# Apply migrations in order
docker-compose exec postgres psql -U postgres -d ogame_rpg -f /app/backend/src/database/migrations/001_initial_schema.sql
docker-compose exec postgres psql -U postgres -d ogame_rpg -f /app/backend/src/database/migrations/002_add_shop_tables.sql
docker-compose exec postgres psql -U postgres -d ogame_rpg -f /app/backend/src/database/migrations/003_millisecond_precision_combat.sql
docker-compose exec postgres psql -U postgres -d ogame_rpg -f /app/backend/src/database/migrations/004_admin_features.sql

# Verify migrations
docker-compose exec postgres psql -U postgres -d ogame_rpg -c "\dt"
```

### 2.3 Create Admin User

```bash
# Option 1: Via SQL
docker-compose exec postgres psql -U postgres -d ogame_rpg << EOF
UPDATE users SET is_admin = true WHERE email = 'admin@yourdomain.com';
EOF

# Option 2: Register admin via API, then promote
# 1. Register account via website
# 2. Run SQL to promote
docker-compose exec postgres psql -U postgres -d ogame_rpg -c "UPDATE users SET is_admin = true WHERE id = 1;"
```

---

## Step 3: Backend Build & Deployment

### 3.1 Build Backend

Create `backend/Dockerfile`:

```dockerfile
FROM node:18-alpine AS builder

WORKDIR /app

# Copy package files
COPY package*.json ./
RUN npm ci --only=production

# Copy source code
COPY . .

# Build TypeScript
RUN npm run build

# Production image
FROM node:18-alpine

WORKDIR /app

# Copy built application
COPY --from=builder /app/dist ./dist
COPY --from=builder /app/node_modules ./node_modules
COPY --from=builder /app/package*.json ./

# Create non-root user
RUN addgroup -g 1001 -S nodejs && \
    adduser -S nodejs -u 1001

USER nodejs

EXPOSE 3000

CMD ["node", "dist/index.js"]
```

### 3.2 Build and Start Services

```bash
# Build backend image
docker-compose build backend

# Start all services
docker-compose up -d

# Check logs
docker-compose logs -f backend

# Verify services are running
docker-compose ps
```

---

## Step 4: SSL/TLS Configuration (Nginx Reverse Proxy)

### 4.1 Install Nginx

```bash
sudo apt update
sudo apt install nginx certbot python3-certbot-nginx
```

### 4.2 Configure Nginx

Create `/etc/nginx/sites-available/spaceempire`:

```nginx
# Redirect HTTP to HTTPS
server {
    listen 80;
    server_name yourdomain.com www.yourdomain.com;
    
    location /.well-known/acme-challenge/ {
        root /var/www/html;
    }
    
    location / {
        return 301 https://$server_name$request_uri;
    }
}

# HTTPS Configuration
server {
    listen 443 ssl http2;
    server_name yourdomain.com www.yourdomain.com;

    # SSL Configuration
    ssl_certificate /etc/letsencrypt/live/yourdomain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/yourdomain.com/privkey.pem;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;
    ssl_prefer_server_ciphers on;

    # Security Headers
    add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-XSS-Protection "1; mode=block" always;

    # Proxy to backend
    location / {
        proxy_pass http://localhost:3000;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_cache_bypass $http_upgrade;
    }

    # WebSocket support
    location /socket.io/ {
        proxy_pass http://localhost:3000;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    # Static file caching
    location ~* \.(jpg|jpeg|png|gif|ico|css|js)$ {
        proxy_pass http://localhost:3000;
        expires 30d;
        add_header Cache-Control "public, immutable";
    }
}
```

### 4.3 Enable Site and Get SSL Certificate

```bash
# Enable site
sudo ln -s /etc/nginx/sites-available/spaceempire /etc/nginx/sites-enabled/

# Test configuration
sudo nginx -t

# Get SSL certificate
sudo certbot --nginx -d yourdomain.com -d www.yourdomain.com

# Reload nginx
sudo systemctl reload nginx

# Enable auto-renewal
sudo systemctl enable certbot.timer
```

---

## Step 5: Stripe Webhook Configuration

### 5.1 Create Webhook Endpoint in Stripe Dashboard

1. Go to https://dashboard.stripe.com/webhooks
2. Click "Add endpoint"
3. Enter URL: `https://yourdomain.com/api/shop/webhook`
4. Select events:
   - `payment_intent.succeeded`
   - `payment_intent.payment_failed`
   - `charge.succeeded`
5. Copy webhook signing secret
6. Update `.env` with `STRIPE_WEBHOOK_SECRET`

### 5.2 Test Webhook

```bash
# Use Stripe CLI to test
stripe listen --forward-to localhost:3000/api/shop/webhook

# Trigger test event
stripe trigger payment_intent.succeeded
```

---

## Step 6: Monitoring & Logging

### 6.1 Configure Application Logging

The application logs to stdout. Configure log collection:

```bash
# View logs
docker-compose logs -f backend

# Save logs to file
docker-compose logs backend > /var/log/spaceempire/backend.log
```

### 6.2 Set Up Monitoring (Optional)

**Option A: Prometheus + Grafana**
```bash
# Add to docker-compose.yml
prometheus:
  image: prom/prometheus
  volumes:
    - ./prometheus.yml:/etc/prometheus/prometheus.yml
  ports:
    - "9090:9090"

grafana:
  image: grafana/grafana
  ports:
    - "3001:3000"
```

**Option B: Cloud Monitoring**
- AWS CloudWatch
- Google Cloud Monitoring
- DataDog
- New Relic

### 6.3 Database Backups

```bash
# Create backup script
cat > /usr/local/bin/backup-spaceempire.sh << 'EOF'
#!/bin/bash
BACKUP_DIR="/var/backups/spaceempire"
DATE=$(date +%Y%m%d_%H%M%S)

mkdir -p $BACKUP_DIR

# Backup PostgreSQL
docker-compose exec -T postgres pg_dump -U postgres ogame_rpg | gzip > $BACKUP_DIR/db_$DATE.sql.gz

# Backup Redis
docker-compose exec -T redis redis-cli --rdb - > $BACKUP_DIR/redis_$DATE.rdb

# Keep only last 7 days
find $BACKUP_DIR -name "*.gz" -mtime +7 -delete
find $BACKUP_DIR -name "*.rdb" -mtime +7 -delete

echo "Backup completed: $DATE"
EOF

chmod +x /usr/local/bin/backup-spaceempire.sh

# Add to crontab (daily at 2 AM)
(crontab -l 2>/dev/null; echo "0 2 * * * /usr/local/bin/backup-spaceempire.sh") | crontab -
```

---

## Step 7: Performance Optimization

### 7.1 Configure PostgreSQL for Production

Edit `postgresql.conf` (inside container or mount custom config):

```conf
# Connection settings
max_connections = 200
shared_buffers = 256MB
effective_cache_size = 1GB
maintenance_work_mem = 64MB
work_mem = 4MB

# Write ahead log
wal_buffers = 16MB
checkpoint_completion_target = 0.9
```

### 7.2 Configure Redis

```bash
# In docker-compose.yml, add to redis command:
command: redis-server --maxmemory 512mb --maxmemory-policy allkeys-lru
```

### 7.3 Enable Gzip Compression in Nginx

Add to nginx configuration:

```nginx
gzip on;
gzip_vary on;
gzip_min_length 1024;
gzip_types text/plain text/css text/xml text/javascript application/x-javascript application/xml+rss application/json;
```

---

## Step 8: Security Hardening

### 8.1 Firewall Configuration

```bash
# Allow only necessary ports
sudo ufw default deny incoming
sudo ufw default allow outgoing
sudo ufw allow ssh
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp
sudo ufw enable
```

### 8.2 Rate Limiting

Add to nginx configuration:

```nginx
# Define rate limit zone
limit_req_zone $binary_remote_addr zone=api_limit:10m rate=10r/s;
limit_req_zone $binary_remote_addr zone=login_limit:10m rate=5r/m;

# Apply to locations
location /api/ {
    limit_req zone=api_limit burst=20 nodelay;
    ...
}

location /api/auth/login {
    limit_req zone=login_limit burst=5 nodelay;
    ...
}
```

### 8.3 Database Security

```bash
# Change default PostgreSQL password
docker-compose exec postgres psql -U postgres -c "ALTER USER postgres PASSWORD 'new_strong_password';"

# Update .env file with new password
```

---

## Step 9: Testing Production Deployment

### 9.1 Health Checks

```bash
# Test backend health
curl https://yourdomain.com/api/health

# Expected response:
{"status":"ok","timestamp":"2025-11-06T02:00:00.000Z"}
```

### 9.2 Create Test User

```bash
# Register via web interface
# Navigate to: https://yourdomain.com/login.html

# Or via API
curl -X POST https://yourdomain.com/api/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "username": "testuser",
    "email": "test@example.com",
    "password": "TestPassword123!"
  }'
```

### 9.3 Test Key Features

- Registration and login
- Planet creation
- Building construction
- Fleet dispatch
- Combat simulation
- Message sending
- Shop/payment (test mode)
- Leaderboard
- Admin panel (with admin account)

---

## Step 10: Go Live Checklist

### Pre-Launch
- [ ] All environment variables configured
- [ ] Database migrations applied
- [ ] Admin user created
- [ ] SSL certificate installed and auto-renewal configured
- [ ] Firewall rules configured
- [ ] Backup system configured and tested
- [ ] Monitoring/logging configured
- [ ] Rate limiting enabled
- [ ] Stripe webhooks configured (production mode)
- [ ] All services health checks passing

### Launch
- [ ] Update DNS to point to production server
- [ ] Test all critical user flows
- [ ] Verify WebSocket connections work
- [ ] Test Stripe payments (live mode)
- [ ] Monitor logs for errors
- [ ] Check database connections
- [ ] Verify Redis caching works

### Post-Launch
- [ ] Monitor server resources (CPU, RAM, Disk)
- [ ] Check database backup completion
- [ ] Review application logs daily
- [ ] Monitor user registration rate
- [ ] Track error rates and respond to issues
- [ ] Set up alerts for downtime

---

## Troubleshooting

### Common Issues

**Issue: Backend won't start**
```bash
# Check logs
docker-compose logs backend

# Common causes:
# - Database not ready: Wait 30 seconds, restart
# - Missing environment variables: Check .env file
# - Port already in use: Change PORT in .env
```

**Issue: Database connection refused**
```bash
# Verify PostgreSQL is running
docker-compose ps postgres

# Check connection string in .env
# Ensure DATABASE_URL matches docker-compose service name
```

**Issue: Websocket not connecting**
```bash
# Check nginx configuration for /socket.io/ location
# Ensure Upgrade headers are set
# Verify firewall allows WebSocket connections
```

**Issue: High memory usage**
```bash
# Check PostgreSQL connections
docker-compose exec postgres psql -U postgres -c "SELECT count(*) FROM pg_stat_activity;"

# Reduce max_connections in PostgreSQL config
# Implement connection pooling in application
```

---

## Scaling Recommendations

### Vertical Scaling (Single Server)
- Increase RAM to 8GB+
- Upgrade to 4+ CPU cores
- Use SSD storage
- Optimize database queries
- Increase Redis maxmemory

### Horizontal Scaling (Multiple Servers)
1. **Load Balancer**: Nginx or HAProxy
2. **Multiple Backend Instances**: Docker Swarm or Kubernetes
3. **Database**: Primary-replica replication
4. **Redis**: Redis Cluster or Sentinel
5. **Session Storage**: Redis for shared sessions
6. **File Storage**: S3 or similar for static assets

---

## Maintenance

### Regular Tasks

**Daily**
- Check application logs
- Monitor server resources
- Verify backups completed

**Weekly**
- Review error logs
- Check database performance
- Update Docker images (security patches)

**Monthly**
- Review and optimize database queries
- Clean up old logs
- Update application dependencies
- Security audit

---

## Support & Documentation

- **Application Logs**: `docker-compose logs backend`
- **Database Access**: `docker-compose exec postgres psql -U postgres -d ogame_rpg`
- **Redis CLI**: `docker-compose exec redis redis-cli`

For additional help, consult the project documentation or contact the development team.

---

**Deployment Version**: 1.0.0  
**Last Updated**: 2025-11-06  
**Status**: Production Ready
