# SpaceEmpire Deployment Guide

## Quick Start

### Using Docker (Recommended)

1. **Prerequisites**:
   - Docker Engine 20.10+
   - Docker Compose 2.0+

2. **Start the application**:
   ```bash
   cd /workspace/universus-rpg
   docker-compose up --build -d
   ```

3. **Access the game**:
   Open your browser and navigate to `http://localhost:3000`

4. **Stop the application**:
   ```bash
   docker-compose down
   ```

### Manual Setup (Development)

1. **Prerequisites**:
   - Node.js 18+
   - PostgreSQL 15+
   - Redis 7+
   - pnpm

2. **Database Setup**:
   ```bash
   # Create PostgreSQL database
   createdb universus_rpg
   
   # Initialize schema
   psql -U postgres -d universus_rpg -f database/sql/schema.sql
   ```

3. **Start Redis**:
   ```bash
   redis-server
   ```

4. **Configure Environment**:
   ```bash
   cd backend
   cp .env.example .env
   # Edit .env with your settings
   ```

5. **Install Dependencies & Build**:
   ```bash
   cd backend
   pnpm install
   pnpm run build
   ```

6. **Start the Server**:
   ```bash
   # Development mode (with auto-reload)
   pnpm run dev
   
   # Production mode
   pnpm start
   ```

7. **Access the game**:
   Open `http://localhost:3000`

## Configuration

### Environment Variables

Edit `backend/.env`:

```env
# Server
NODE_ENV=production
PORT=3000

# Database
DB_HOST=localhost
DB_PORT=5432
DB_NAME=universus_rpg
DB_USER=postgres
DB_PASSWORD=your_secure_password

# Redis
REDIS_HOST=localhost
REDIS_PORT=6379

# Security
JWT_SECRET=change_this_to_a_secure_random_string
JWT_EXPIRES_IN=7d

# Game Settings
GAME_SPEED=1                      # 1-10x speed multiplier
RESOURCE_PRODUCTION_MULTIPLIER=1  # Production rate multiplier
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
docker-compose logs -f

# Specific service
docker-compose logs -f backend
docker-compose logs -f postgres
docker-compose logs -f redis
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

1. Check backend logs:
   ```bash
   docker-compose logs backend
   ```

2. Restart services:
   ```bash
   docker-compose restart backend
   ```

## Scaling

### Horizontal Scaling

To run multiple backend instances:

1. Update `docker-compose.yml`:
   ```yaml
   backend:
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
