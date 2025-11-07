FROM node:18-alpine AS builder

WORKDIR /app/admin-service

RUN npm install -g pnpm

COPY admin-service/package.json admin-service/pnpm-lock.yaml* ./
RUN pnpm install --frozen-lockfile || pnpm install

COPY admin-service/tsconfig.json ./
COPY admin-service/src ./src
RUN pnpm run build

FROM node:18-alpine

WORKDIR /app/admin-service

RUN npm install -g pnpm

COPY admin-service/package.json admin-service/pnpm-lock.yaml* ./
RUN pnpm install --prod --frozen-lockfile || pnpm install --prod

COPY --from=builder /app/admin-service/dist ./dist

EXPOSE 4002

CMD ["node", "dist/index.js"]
