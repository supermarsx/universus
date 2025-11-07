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
   cd ogame-rpg
   ```

2. Build and start all services (API, bot worker, Redis, PostgreSQL):
   
   ```bash
   docker-compose up -d
   ```

3. The game will be available at `http://localhost:3000`

4. To stop the services:
   
   ```bash
   docker-compose down
   ```

### Bot Service Worker

- Dedicated container (`ogame_bot_service`) handles scheduled bot AI processing outside the main API service
- Shares the same PostgreSQL and Redis instances via environment variables
- Backend API proxies all admin bot endpoints to the worker via `BOT_SERVICE_URL` (defaults to `http://bot-service:4001`)
- Configure cadence with `BOT_WORKER_INTERVAL_MS` and `BOT_WORKER_MAX_BOTS` (see `docker-compose.yml`)
- For local development run both services:
  - `pnpm dev` inside `backend`
  - `npm run dev` inside `bot-service`

### Option 2: Local Development Setup

1. Install dependencies:
   
   ```bash
   # Backend
   cd backend
   pnpm install

   # Bot service
   cd ../bot-service
   npm install
   ```

# No frontend dependencies needed (vanilla JS)

```
2. Set up PostgreSQL database:
```bash
# Create database
createdb ogame_rpg

# Initialize schema
psql -U postgres -d ogame_rpg -f backend/src/database/schema.sql
```

3. Start Redis:
   
   ```bash
   redis-server
   ```

4. Configure environment variables:
   
   ```bash
   cd backend
   cp .env.example .env
   # Edit .env with your database credentials

   cd ../bot-service
   cp .env.example .env
   ```

5. Start the backend server and bot service:
   
   ```bash
   cd backend
   pnpm run dev

   # In a new terminal
   cd ../bot-service
   npm run dev
   ```

6. Open your browser and navigate to `http://localhost:3000`

## Environment Variables

Create a `.env` file in the `backend/` directory:

```env
NODE_ENV=development
PORT=3000

# PostgreSQL
DB_HOST=localhost
DB_PORT=5432
DB_NAME=ogame_rpg
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

# Game Configuration
GAME_SPEED=1
RESOURCE_PRODUCTION_MULTIPLIER=1
```

Create a `.env` file in the `bot-service/` directory:

```env
BOT_SERVICE_PORT=4001

# PostgreSQL
DB_HOST=localhost
DB_PORT=5432
DB_NAME=ogame_rpg
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
ogame-rpg/
├── backend/
│   ├── src/
│   │   ├── config/          # Database, Redis, game config
│   │   ├── services/        # Business logic
│   │   ├── routes/          # API routes
│   │   ├── middleware/      # Auth middleware
│   │   ├── socket/          # WebSocket handlers
│   │   ├── types/           # TypeScript types
│   │   ├── database/        # SQL schema
│   │   └── index.ts         # Server entry point
│   ├── package.json
│   └── tsconfig.json
├── frontend/
│   ├── css/                 # Stylesheets
│   ├── js/                  # JavaScript files
│   ├── index.html           # Login page
│   ├── overview.html        # Game overview
│   └── buildings.html       # Buildings page
├── docker-compose.yml
├── Dockerfile
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
psql -U postgres -d ogame_rpg -f backend/src/database/schema.sql

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

## Contributing

This project was created as a demonstration of full-stack game development. Contributions are welcome!

## License

MIT License

## Author

MiniMax Agent

## Acknowledgments

Inspired by the classic browser game Universus, recreated with modern web technologies.
