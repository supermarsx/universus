# Build stage
FROM node:18-alpine AS builder

WORKDIR /app

# Install bot-service dependencies
COPY bot-service/package.json bot-service/package-lock.json* ./bot-service/
RUN cd bot-service && npm install

# Copy source code
COPY backend ./backend
COPY bot-service ./bot-service

# Build bot-service (compiles shared modules as part of the graph)
RUN cd bot-service && npm run build

# Production image
FROM node:18-alpine

ENV NODE_ENV=production \
    BOT_WORKER_INTERVAL_MS=60000 \
    BOT_WORKER_MAX_BOTS=25

WORKDIR /app/bot-service

COPY --from=builder /app/bot-service/dist ./dist
COPY --from=builder /app/bot-service/package.json ./package.json
COPY --from=builder /app/bot-service/node_modules ./node_modules

CMD ["node", "dist/bot-service/src/index.js"]
