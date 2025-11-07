# Stage 1: Build static bundle
FROM node:18-alpine AS builder

WORKDIR /app/frontend

COPY package.json package-lock.json* ./
RUN npm ci

COPY . .
RUN npm run build

# Stage 2: Serve static assets using nginx
FROM nginx:1.27-alpine AS runner

COPY --from=builder /app/frontend/dist /usr/share/nginx/html

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]
