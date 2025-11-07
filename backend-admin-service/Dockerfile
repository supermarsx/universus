FROM node:18-alpine AS builder

WORKDIR /app/backend-admin-service

RUN npm install -g pnpm

COPY backend-admin-service/package.json backend-admin-service/pnpm-lock.yaml* ./
RUN pnpm install --frozen-lockfile || pnpm install

COPY backend-admin-service/tsconfig.json ./
COPY backend-admin-service/src ./src
RUN pnpm run build

FROM node:18-alpine

WORKDIR /app/backend-admin-service

RUN npm install -g pnpm

COPY backend-admin-service/package.json backend-admin-service/pnpm-lock.yaml* ./
RUN pnpm install --prod --frozen-lockfile || pnpm install --prod

COPY --from=builder /app/backend-admin-service/dist ./dist

EXPOSE 4002

CMD ["node", "dist/index.js"]
