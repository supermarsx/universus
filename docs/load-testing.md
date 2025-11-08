## Load Testing Playbook

The server ships with a lightweight load-testing harness so we can verify horizontal-scaling assumptions from the spec.

### Prerequisites

1. Start the backend (`npm run dev` or the production build) and ensure Redis/Postgres are up.
2. Generate a bearer token (login via `/api/auth/login`) and export it:

```bash
export LOADTEST_TOKEN="eyJhbGciOiJI..."   # optional, but required for authenticated routes
```

### Running the scenarios

```
cd backend
LOADTEST_BASE_URL="http://localhost:3000" \
LOADTEST_CONNECTIONS=60 \
LOADTEST_DURATION=45 \
npm run load:test
```

The script (see `backend/scripts/loadTest.ts`) will hammer:

- `GET /api/galaxy` – emulates concurrent scans
- `GET /api/leaderboard` – public leaderboard refreshes
- `GET /api/admin/monitoring/scaling` – admin scaling dashboard

Each scenario prints average req/s, p95 latency, and error counts so you can track regressions over time or under different cluster sizes.
