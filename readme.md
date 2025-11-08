# Universus Browser MMO RPG

A complete browser-based multiplayer strategy game inspired by Universus, featuring real-time gameplay, planet management, fleet warfare, technology research, and alliance systems.

## Features

- **Real-time Multiplayer Gameplay**: WebSocket-based real-time communication
- **Planet Management**: Build and upgrade structures on multiple planets
- **Resource Production**: Metal, Crystal, Deuterium, and Energy systems
- **Building Construction**: Mines, power plants, shipyards, research labs, and more
- **Technology Research**: Comprehensive tech tree with prerequisites
- **Fleet System**: Ship construction and fleet missions (in progress)
- **Combat System**: 6-round battle simulation with complex mechanics (in progress)
- **Alliance System**: Create and manage alliances with chat (in progress)
- **Leaderboards**: Player rankings by score
- **Responsive UI**: Modern, space-themed interface
- **Analytics Tracking**: Built-in event tracking and admin usage reporting

## Technology Stack

### Backend

- **Node.js** + **TypeScript** + **Express.js**
- **PostgreSQL** (primary database)
- **Redis** (caching & pub/sub)
- **Socket.io** (WebSocket communication)
- **JWT** (authentication)
- **bcrypt** (password hashing)

### Frontend

- **HTML5** + **CSS3** + **Vanilla JavaScript**
- **Canvas** (for graphics - planned)
- **Socket.io Client** (real-time updates)

### Deployment

- **Docker** + **Docker Compose**
- Multi-container architecture
- Health checks and auto-restart

## Prerequisites

- **Node.js** 18+ and pnpm
- **PostgreSQL** 15+
- **Redis** 7+
- **Docker** and **Docker Compose** (for containerized deployment)

## Installation & Setup

### Option 1: Docker Deployment (Recommended)

1. Clone the repository:
   
   ```bash
   cd universus-rpg
   ```

2. Build and start all services (API, bot worker, Redis, PostgreSQL, standalone frontend):
   
   ```bash
   docker-compose up -d
   ```

3. The game will be available at `http://localhost:3000` (served by the backend) and the static bundle at `http://localhost:8080` (served by the dedicated frontend container)

4. To stop the services:
   
   ```bash
   docker-compose down
   ```

### Bot Service Worker

- Dedicated container (`universus_bot_service`) handles scheduled bot AI processing outside the main API service
- Shares the same PostgreSQL and Redis instances via environment variables
- Backend API proxies all admin bot endpoints to the worker via `BOT_SERVICE_URL` (defaults to `http://bot-service:4001`)
- Configure cadence with `BOT_WORKER_INTERVAL_MS` and `BOT_WORKER_MAX_BOTS` (see `docker-compose.yml`)
- For local development run the services:
  - `pnpm dev` inside `backend`
  - `npm run dev` inside `backend-bot-service`
  - `npm run build` (or desired workflow) inside `frontend`
- PostgreSQL now lives in the dedicated `database` project (`universus_database` container). Build-time schema initialization is handled via `database/Dockerfile`.

### Admin Service

- Lives under `backend-admin-service/` and exposes all privileged admin APIs (dashboards, user moderation, monitoring, game configuration)
- Runs as its own container (`universus_admin_service`) so the public backend never handles admin-only routes
- Shares the same PostgreSQL instance and JWT secret with the main backend; communicates over REST
- Local development mirrors the backend workflow:
  - `pnpm install` then `pnpm dev` inside `backend-admin-service`
  - Environment variables: `ADMIN_PORT` (defaults to `4002`), database credentials, shared `JWT_SECRET`
- Docker deployments build the image via `backend-admin-service/Dockerfile` and expose port `4002`

### Option 2: Local Development Setup

1. Install dependencies:
   
   ```bash
   # Backend
   cd backend
   pnpm install

   # Bot service
   cd ../backend-bot-service
   npm install

   # Admin service
   cd ../backend-admin-service
   pnpm install

   # Frontend
   cd ../frontend
   npm install
   ```

2. Set up PostgreSQL database:
```bash
# Create database
createdb universus_rpg

# Initialize schema
psql -U postgres -d universus_rpg -f database/sql/schema.sql
```

3. Start Redis (or rely on `redis/` docker image with `docker-compose up redis`):
   
   ```bash
   redis-server
   ```

4. Configure environment variables:
   
   ```bash
   cd backend
   cp .env.example .env
   # Edit .env with your database credentials

   cd ../backend-bot-service
   cp .env.example .env
   ```

5. Start the backend server, bot service, admin service, and optionally rebuild the frontend bundle:
   
   ```bash
   cd backend
   pnpm run dev

   # In a new terminal
   cd ../backend-bot-service
   npm run dev

   # In a third terminal
   cd ../backend-admin-service
   pnpm run dev

   # In another terminal (when you need to refresh static assets)
   cd ../frontend
   npm run build
   ```

6. Open your browser and navigate to `http://localhost:3000` (backend-rendered UI) or `http://localhost:8080` (static bundle)

## Environment Variables

Create a `.env` file in the `backend/` directory:

```env
NODE_ENV=development
PORT=3000

# PostgreSQL
DB_HOST=localhost
DB_PORT=5432
DB_NAME=universus_rpg
DB_USER=postgres
DB_PASSWORD=your_password

# Redis
REDIS_HOST=localhost
REDIS_PORT=6379

# JWT
JWT_SECRET=your_super_secret_jwt_key_change_in_production
JWT_EXPIRES_IN=7d

# Bot service endpoint
BOT_SERVICE_URL=http://localhost:4001

# Admin service endpoint (if the backend ever needs to call it)
ADMIN_SERVICE_URL=http://localhost:4002

# RabbitMQ (for analytics queue)
RABBITMQ_URL=amqp://guest:guest@localhost:5672
ANALYTICS_QUEUE_NAME=analytics_events
# Set to true to bypass RabbitMQ and write analytics directly
ANALYTICS_QUEUE_DISABLED=false

# Game Configuration
GAME_SPEED=1
RESOURCE_PRODUCTION_MULTIPLIER=1
```

Create a `.env` file in the `backend-bot-service/` directory:

```env
BOT_SERVICE_PORT=4001

# PostgreSQL
DB_HOST=localhost
DB_PORT=5432
DB_NAME=universus_rpg
DB_USER=postgres
DB_PASSWORD=your_password

# Redis
REDIS_HOST=localhost
REDIS_PORT=6379

# JWT shared with backend
JWT_SECRET=your_super_secret_jwt_key_change_in_production

# Bot worker cadence
BOT_WORKER_INTERVAL_MS=60000
BOT_WORKER_MAX_BOTS=25
```

Create a `.env` file in the `backend-admin-service/` directory:

```env
ADMIN_PORT=4002

# PostgreSQL
DB_HOST=localhost
DB_PORT=5432
DB_NAME=universus_rpg
DB_USER=postgres
DB_PASSWORD=your_password

# JWT shared with backend/backend-bot-service
JWT_SECRET=your_super_secret_jwt_key_change_in_production
```

## Game Mechanics

### Resources

- **Metal**: Primary construction resource
- **Crystal**: Advanced technology and ships
- **Deuterium**: Fuel for ships and advanced buildings
- **Energy**: Powers mines and production facilities

### Buildings

- **Metal Mine**: Produces metal
- **Crystal Mine**: Produces crystal
- **Deuterium Synthesizer**: Produces deuterium
- **Solar Plant / Fusion Reactor**: Generates energy
- **Robotics Factory**: Reduces construction time
- **Shipyard**: Enables ship construction
- **Research Lab**: Enables technology research
- **Storage Facilities**: Increases resource caps

### Game Loop

- Resource production runs continuously
- Building construction has time-based completion
- Automatic event processing every 10 seconds
- Real-time updates via WebSocket

## API Endpoints

### Authentication

- `POST /api/auth/register` - Register new account
- `POST /api/auth/login` - Login

### Planets

- `GET /api/planets` - Get all user planets
- `GET /api/planets/:id` - Get planet details
- `POST /api/planets/:id/build` - Start building construction
- `DELETE /api/planets/construction/:id` - Cancel construction

### Users

- `GET /api/users/me` - Get current user data
- `GET /api/users/leaderboard` - Get top players

## Project Structure

```
universus-rpg/
├── backend/                 # Express + TypeScript API
│   ├── src/
│   │   ├── config/
│   │   ├── middleware/
│   │   ├── routes/
│   │   ├── services/
│   │   ├── socket/
│   │   └── types/
│   ├── package.json
│   ├── pnpm-lock.yaml
│   └── Dockerfile
├── backend-bot-service/     # Dedicated bot processing worker/API
│   ├── src/
│   └── package.json
├── backend-admin-service/   # Standalone admin API service
│   ├── src/
│   └── package.json
├── frontend/                # Static assets and Nunjucks templates
│   ├── assets/
│   ├── css/
│   ├── js/
│   ├── views/
│   └── package.json
├── database/                # PostgreSQL schema and custom image
│   ├── sql/
│   ├── scripts/
│   └── Dockerfile
├── docker-compose.yml       # Orchestrates all services
└── README.md
```


## Development

### Backend Development

```bash
cd backend
pnpm run dev  # Starts with nodemon for auto-reload
```

### Building for Production

```bash
cd backend
pnpm run build  # Compiles TypeScript to dist/
pnpm start      # Runs compiled JavaScript
```

### Database Management

```bash
# Reinitialize database
psql -U postgres -d universus_rpg -f database/sql/schema.sql

# Or use npm script
cd backend
pnpm run db:init
```

## Game Features Status

### Completed

- [x] User authentication (registration, login)
- [x] Planet management
- [x] Resource production system
- [x] Building construction with time-based completion
- [x] Construction queue management
- [x] Real-time WebSocket updates
- [x] Responsive UI
- [x] Game loop for event processing

### In Progress / Planned

- [ ] Technology research system (backend complete, UI needed)
- [ ] Fleet construction and management
- [ ] Fleet missions (attack, transport, deploy, etc.)
- [ ] Combat simulation
- [ ] Galaxy view
- [ ] Alliance system
- [ ] In-game messaging
- [ ] Leaderboards UI
- [ ] Admin panel
- [ ] Premium currency shop
- [ ] Mobile responsive improvements

## Performance Considerations

- **Resource Calculation**: Lazy evaluation prevents constant database updates
- **Event Scheduling**: Efficient game loop checks every 10 seconds
- **Caching**: Redis used for sessions and frequently accessed data
- **WebSocket**: Selective room-based updates minimize bandwidth
- **Database Indexing**: Optimized queries with proper indexes

## Security

- **Password Hashing**: bcrypt with salt rounds
- **JWT Authentication**: Secure token-based auth
- **Input Validation**: Server-side validation for all inputs
- **SQL Injection Prevention**: Parameterized queries
- **XSS Protection**: Proper HTML escaping
- **Authoritative Server**: All game logic runs server-side

## Scaling

- **Horizontal Scaling**: Stateless backend can run multiple instances
- **Redis Pub/Sub**: Socket.io adapter for multi-node support
- **Database**: PostgreSQL supports read replicas and sharding
- **Load Balancing**: Docker Swarm or Kubernetes ready

## Email Delivery Service

- Outbound transactional emails are enqueued by the main backend and processed by the worker in `email-delivery-service/`.
- Configure providers (SMTP, SendGrid, Amazon SES, MailerSend) and sender details under **Admin → Configuration → Notifications**.
- Start the worker locally with `pnpm dev` (after installing dependencies inside `email-delivery-service`) so verification, password reset, and alert emails are dispatched automatically.

## Analytics & Usage Tracking

- The frontend emits lightweight page-view events through `/api/analytics/events`, storing them in `analytics_events`.
- Custom events can be sent by calling `window.UniversusAnalytics.track('event_name', { ...properties })` anywhere in the UI.
- Admins can retrieve aggregate stats via `GET /api/analytics/usage?days=7` (requires admin token) for quick dashboards or exports.
- For high-volume deployments, set `RABBITMQ_URL` and start the queue worker (`pnpm run analytics:worker`) so events are published to RabbitMQ and processed asynchronously. When RabbitMQ is unavailable, the backend automatically falls back to direct writes.

## Contributing

This project was created as a demonstration of full-stack game development. Contributions are welcome!

## License

MIT License

## Author

MiniMax Agent

## Acknowledgments

Inspired by the classic browser game Universus, recreated with modern web technologies.
