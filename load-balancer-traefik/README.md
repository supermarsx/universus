# Universus Traefik Load Balancer

This folder contains a production-ready Traefik configuration for the Universus monorepo. It provides:
- Centralized HTTP/HTTPS routing for all services (frontend, backend, admin, bots, email, observability, etc.)
- Automatic service discovery via Docker labels
- Health checks, sticky sessions, and security best practices
- Built-in dashboard for monitoring

## Folder Contents
- `traefik.yml` — Main static config (entrypoints, providers, dashboard, logging, ACME)
- `dynamic.yml` — Dynamic config (middlewares, advanced routing, can be extended)
- `.gitignore` — Ignores ACME cert storage
- `docker-compose.yml` — Example for local dev (see below)

## How It Works
- Traefik runs as a container, listening on ports 80/443.
- All other services are started as Docker containers with proper labels.
- Traefik auto-discovers services and routes traffic based on labels.
- The dashboard is available at `/dashboard` (see below).

## Example docker-compose.yml

```
version: '3.8'

services:
  traefik:
    image: traefik:v3.0
    command:
      - --configFile=/etc/traefik/traefik.yml
    ports:
      - "80:80"
      - "443:443"
      - "8080:8080" # Dashboard (dev only)
    volumes:
      - ./traefik.yml:/etc/traefik/traefik.yml:ro
      - ./dynamic.yml:/etc/traefik/dynamic.yml:ro
      - ./acme.json:/acme.json
      - /var/run/docker.sock:/var/run/docker.sock:ro
    restart: unless-stopped
    networks:
      - universus

  frontend:
    build: ../frontend
    labels:
      - "traefik.enable=true"
      - "traefik.http.routers.frontend.rule=Host(`localhost`) || Host(`yourdomain.com`)"
      - "traefik.http.services.frontend.loadbalancer.server.port=80"
    networks:
      - universus

  backend:
    build: ../backend
    labels:
      - "traefik.enable=true"
      - "traefik.http.routers.backend.rule=Host(`api.localhost`) || Host(`api.yourdomain.com`)"
      - "traefik.http.services.backend.loadbalancer.server.port=3000"
    networks:
      - universus

  backend-admin-service:
    build: ../backend-admin-service
    labels:
      - "traefik.enable=true"
      - "traefik.http.routers.admin.rule=Host(`admin.localhost`) || Host(`admin.yourdomain.com`)"
      - "traefik.http.services.admin.loadbalancer.server.port=3001"
    networks:
      - universus

  backend-bot-service:
    build: ../backend-bot-service
    labels:
      - "traefik.enable=true"
      - "traefik.http.routers.bot.rule=Host(`bot.localhost`) || Host(`bot.yourdomain.com`)"
      - "traefik.http.services.bot.loadbalancer.server.port=3002"
    networks:
      - universus

  email-delivery-service:
    build: ../email-delivery-service
    labels:
      - "traefik.enable=true"
      - "traefik.http.routers.email.rule=Host(`email.localhost`) || Host(`email.yourdomain.com`)"
      - "traefik.http.services.email.loadbalancer.server.port=3003"
    networks:
      - universus

  observability-service:
    build: ../observability-service
    labels:
      - "traefik.enable=true"
      - "traefik.http.routers.observability.rule=Host(`obs.localhost`) || Host(`obs.yourdomain.com`)"
      - "traefik.http.services.observability.loadbalancer.server.port=3004"
    networks:
      - universus

  # Add other services (db, redis, rabbitmq) as needed, but usually not exposed via Traefik

networks:
  universus:
    driver: bridge
```

## Dashboard (Admin-Only Access)
- The Traefik dashboard is now **protected by HTTP Basic Auth** and only accessible to platform admins.
- Access at `http://traefik.localhost:8080/dashboard/` (or your internal admin domain).
- By default, the dashboard is **not exposed to the public internet**.
- Credentials are set in `dynamic.yml` under the `dashboard-auth` middleware. **Replace the password hash with your own!**
- To generate a password hash, use:
  ```
  htpasswd -nb admin yourpassword
  ```
  and copy the output to replace the example hash in `dynamic.yml`.
- You may also restrict access by IP or use a more advanced auth provider (see Traefik docs).
- **Do not expose port 8080 to the public.**

## Adding/Modifying Services
- Add new services to `docker-compose.yml` with proper `traefik.http.routers` and `traefik.http.services` labels.
- See [Traefik docs](https://doc.traefik.io/traefik/) for advanced routing, middlewares, and security.

## HTTPS/ACME (Automatic Certificates)
- **Automatic Let's Encrypt is fully integrated!**
- Traefik will request and renew certificates for your real domains automatically.
- To enable:
  1. Set your real email in `traefik.yml` under `certificatesResolvers.letsencrypt.acme.email`.
  2. Set your real domain(s) in the `Host(...)` rules in `docker-compose.yml` labels for each service.
  3. Make sure ports 80 and 443 are open and forwarded to your server.
  4. Run `./init-acme.sh` once to create `acme.json` with secure permissions.
  5. Start Traefik with `docker-compose up`.
- Traefik will handle all certificate requests, renewals, and storage in `acme.json`.
- For local dev, use self-signed or disable HTTPS (see Traefik docs).

### Example: Setting up HTTPS for your domain
- In `traefik.yml`, set:
  ```yaml
  certificatesResolvers:
    letsencrypt:
      acme:
        email: "your@email.com"
        storage: acme.json
        httpChallenge:
          entryPoint: web
  ```
- In `docker-compose.yml`, for your service:
  ```yaml
  labels:
    - "traefik.enable=true"
    - "traefik.http.routers.frontend.rule=Host(`yourdomain.com`)"
    - "traefik.http.routers.frontend.entrypoints=web,websecure"
    - "traefik.http.routers.frontend.tls.certresolver=letsencrypt"
    - "traefik.http.services.frontend.loadbalancer.server.port=80"
  ```
- Traefik will automatically provision and renew the cert for `yourdomain.com`.

### First-time setup script
- Run `./init-acme.sh` in this folder before starting Traefik for the first time.
- This creates `acme.json` with the correct permissions (required for cert storage).

### Troubleshooting
- Check Traefik logs for ACME/cert errors.
- Make sure your domain’s DNS points to your server and ports 80/443 are open.
- See [Traefik ACME docs](https://doc.traefik.io/traefik/https/acme/) for advanced options (DNS challenge, wildcard certs, etc).

## Troubleshooting
- Check Traefik logs (`docker logs traefik`)
- Visit the dashboard for live status and errors
- See [Traefik Community](https://community.traefik.io/) for help

## Security
- Do **not** expose the dashboard or `insecure: true` in production.
- Use strong passwords and firewalls for admin endpoints.

---

**This setup is ready for local dev and can be extended for production.**
