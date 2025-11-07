# Bot worker build stage: reuse backend build artifacts
FROM node:18-alpine AS backend-build

WORKDIR /app/backend

COPY backend/package*.json ./
RUN npm install

COPY backend/ ./
RUN npm run build

# Production bot worker image
FROM node:18-alpine

WORKDIR /app

# Install pnpm once for production dependency install
RUN npm install -g pnpm

# Copy dependency manifests
COPY backend/package*.json backend/pnpm-lock.yaml* ./backend/

WORKDIR /app/backend

# Install production dependencies
RUN pnpm install --prod

# Copy compiled backend code from build stage
COPY --from=backend-build /app/backend/dist ./dist

# Copy required runtime assets (SQL scripts for migrations/queries)
COPY backend/src/database ./src/database

# Bot worker specific configuration
ENV BOT_WORKER_INTERVAL_MS=60000 \
    BOT_WORKER_MAX_BOTS=25

CMD ["node", "dist/bot/botWorker.js"]
