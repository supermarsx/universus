FROM node:18-alpine AS builder

WORKDIR /app/backend

RUN npm install -g pnpm

COPY backend/pnpm-lock.yaml backend/package.json ./
RUN pnpm install --frozen-lockfile

COPY backend/ ./
RUN pnpm run build

FROM node:18-alpine

WORKDIR /app/backend

RUN npm install -g pnpm

COPY backend/pnpm-lock.yaml backend/package.json ./
RUN pnpm install --prod --frozen-lockfile

COPY --from=builder /app/backend/dist ./dist

# Optional: copy TypeScript source for debugging (not required at runtime)
COPY backend/src ./src

# Provide static assets/templates expected by the backend
COPY frontend/views /app/frontend/views
COPY frontend/css /app/frontend/css
COPY frontend/js /app/frontend/js
COPY frontend/assets /app/frontend/assets

EXPOSE 3000

CMD ["node", "dist/index.js"]
