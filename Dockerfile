# Node.js backend
FROM node:18-alpine AS backend-build

WORKDIR /app/backend

COPY backend/package*.json ./
RUN npm install

COPY backend/ ./
RUN npm run build

# Production image
FROM node:18-alpine

WORKDIR /app

# Install pnpm
RUN npm install -g pnpm

# Copy backend
COPY backend/package*.json backend/pnpm-lock.yaml* ./backend/
WORKDIR /app/backend
RUN pnpm install --prod

COPY --from=backend-build /app/backend/dist ./dist
COPY backend/src/database ./src/database

# Copy frontend
WORKDIR /app
COPY frontend/ ./frontend/

WORKDIR /app/backend

EXPOSE 3000

CMD ["node", "dist/index.js"]
