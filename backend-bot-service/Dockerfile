# Build stage
FROM node:18-alpine AS builder

WORKDIR /app

# Install bot-service dependencies
COPY backend-bot-service/package.json backend-bot-service/package-lock.json* ./backend-bot-service/
RUN cd backend-bot-service && npm install

# Copy source code
COPY backend ./backend
COPY backend-bot-service ./backend-bot-service

# Build bot-service (compiles shared modules as part of the graph)
RUN cd backend-bot-service && npm run build

# Production image
FROM node:18-alpine

ENV NODE_ENV=production \
    BOT_WORKER_INTERVAL_MS=60000 \
    BOT_WORKER_MAX_BOTS=25

WORKDIR /app/backend-bot-service

COPY --from=builder /app/backend-bot-service/dist ./dist
COPY --from=builder /app/backend-bot-service/package.json ./package.json
COPY --from=builder /app/backend-bot-service/node_modules ./node_modules

CMD ["node", "dist/backend-bot-service/src/index.js"]
