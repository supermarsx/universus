# Universus (Browser MMO Strategy)

This repository contains a modular, full‑stack browser-based strategy game inspired by Universus. The project is split into multiple services so pieces can be developed, tested, and deployed independently.

This README is a concise developer-facing guide describing what is implemented today, where to find services, and how to run the project locally or with Docker.

**Repository layout**
- `backend/` — Main public API and authoritative game logic (Node.js + TypeScript + Express).
- `backend-bot-service/` — Dedicated bot worker and bot APIs (Node.js + TypeScript).
- `backend-admin-service/` — Admin-only API service (dashboards, moderation, configuration).
- `email-delivery-service/` — Outbound email worker for transactional emails.
- `frontend/` — Static assets, Nunjucks templates, JS and CSS.
- `database/` — PostgreSQL schema and custom Docker image.
- `observability-service/` — Prometheus/Grafana/OTel configs and helpers.
- `docker-compose.yml` — Development / local orchestration for all services.

What’s implemented (current feature set)
- User accounts: registration, login, password hashing, JWT authentication.
- Planet management: create and list planets, planet details and buildings.
- Resource model: Metal / Crystal / Deuterium / Energy with server-side production calculations.
- Building construction: time-based builds, construction queue, cancelation endpoints.
- Real‑time updates: Socket.IO powered real-time events; Redis + socket.io-redis adapter present for horizontal scaling.
- Bot processing: background bot worker service for AI players and scheduled tasks.
- Admin service: separate admin API for privileged operations and configuration.
- Email & analytics: email worker and analytics queue worker to offload background tasks.
- Observability: OpenTelemetry + Prometheus + Grafana provisioning files included.

Planned / in progress (visible in repository)
- Fleet & missions: higher-level fleet management and mission handling (partial or planned).
- Combat simulation: complex battle resolution engine (work in progress in code/docs).
- UI features: richer galaxy map, alliances UI, and leaderboard views are partially implemented or awaiting UI work.

Technology stack
- Backend services: Node.js (v18+), TypeScript, Express
- Real-time: Socket.IO with Redis adapter
- Database: PostgreSQL (primary), SQL schema in `database/sql/`
- Cache / pubsub: Redis
- Queueing: RabbitMQ is optional (analytics queue); code falls back to direct writes
- Email: Nodemailer + provider adapters (config driven)
- Observability: OpenTelemetry, Prometheus, Grafana provisioning
- Frontend: Nunjucks templates, vanilla JS, CSS; build scripts in `frontend/`
- Containerization: Docker & Docker Compose

Quick Start — Docker (recommended)
1. Copy environment files if you want to customize: `cp backend/.env.example backend/.env` and similarly for other services.
2. Start the full stack locally:

   docker-compose up -d --build

3. Open the services in your browser:
- Public backend / UI: http://localhost:3000
- Admin service (if exposed): http://localhost:4002
- Bot service (API): http://localhost:4001

4. To stop and remove containers:

   docker-compose down

Local development (service by service)
- Backend
  - Install: `cd backend && pnpm install`
  - Dev: `pnpm run dev` (uses `ts-node` + `nodemon`)
  - Build: `pnpm run build` → `pnpm start`
- Bot service
  - Install: `cd backend-bot-service && npm install`
  - Dev: `npm run dev` (ts-node-dev)
- Admin service
  - Install: `cd backend-admin-service && pnpm install`
  - Dev: `pnpm run dev`
- Frontend
  - Install & build: `cd frontend && npm install && npm run build`

Environment variables
- Each service has a `.env.example`; copy it into `.env` and provide DB/Redis credentials.
- Important vars:
  - `PORT` / `BOT_SERVICE_PORT` / `ADMIN_PORT`
  - `DB_HOST`, `DB_PORT`, `DB_NAME`, `DB_USER`, `DB_PASSWORD`
  - `REDIS_HOST`, `REDIS_PORT`
  - `JWT_SECRET` (shared across services)
  - `RABBITMQ_URL` (optional for analytics queue)

Database
- The SQL schema and seed scripts are under `database/sql/`.
- Quick local init:

  createdb universus_rpg
  psql -U postgres -d universus_rpg -f database/sql/schema.sql

API overview (examples)
- Authentication
  - `POST /api/auth/register` — register a user
  - `POST /api/auth/login` — login, returns JWT
- Planets & buildings
  - `GET /api/planets` — list player planets
  - `GET /api/planets/:id` — planet details (resources, buildings, queues)
  - `POST /api/planets/:id/build` — queue a building
  - `DELETE /api/planets/construction/:id` — cancel construction
- Users
  - `GET /api/users/me` — current user data
  - `GET /api/users/leaderboard` — leaderboard data
- Admin & bot endpoints live in their respective services (`backend-admin-service/`, `backend-bot-service/`)

Testing & quality
- Backend has Jest tests and lint/type checks. Useful scripts (in `backend/package.json`):
  - `pnpm run test` (runs Jest)
  - `pnpm run lint` / `pnpm run type-check`
- Frontend has Jest + accessibility tests configured.

Operational notes
- Socket.IO adapter uses Redis for multi-instance deployments.
- Analytics events can be published to RabbitMQ by setting `RABBITMQ_URL`; otherwise events are persisted directly to the DB.
- Email delivery is handled by the `email-delivery-service` worker; provider configuration is admin-configurable.
- Observability configurations are included under `observability-service/` for Prometheus/Grafana and OTel collection.

Contributing
- This project is structured to allow independent changes per service. Open a PR per logical change and run the relevant service tests.
- Follow TypeScript + ESLint rules configured across the monorepo.

License
- MIT

If you want, I can also:
- Trim or expand specific sections (API, deployment, developer workflow).
- Add quick curl examples for auth and planet operations.


