# Universus Admin Service

This project hosts the administrative API endpoints that were previously bundled with the main backend. Running it as a standalone service keeps privileged operations isolated, mirroring how the bot service is deployed.

## Development

```bash
pnpm install
pnpm dev
```

Environment variables (can be provided via `.env` or docker-compose):

- `ADMIN_PORT` (default `4002`)
- `DB_HOST`, `DB_PORT`, `DB_NAME`, `DB_USER`, `DB_PASSWORD`
- `JWT_SECRET`

## Build & Run

```bash
pnpm build
pnpm start
```

The service exposes `GET /health` and all admin APIs under `/api/admin/*`.
