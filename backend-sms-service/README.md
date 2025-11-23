# Backend SMS Service

Dedicated microservice that delivers Universus verification codes over SMS, WhatsApp, Telegram, Discord or any custom HTTP gateway. The main backend calls this service over HTTP (`/api/send`) and the service handles transport-specific logic plus fallback sequencing.

## Features

- Supports Twilio SMS, Twilio WhatsApp, Baileys (unofficial) WhatsApp pairing, Telegram bots, Discord bots and custom HTTP gateways.
- Accepts a preferred channel order per request and automatically falls back to subsequent transports when a gateway fails.
- Normalizes phone/contact data per channel and returns the final destination back to the caller.
- Optional API key authentication via the `X-API-Key` header.

## Running Locally

```bash
cd backend-sms-service
pnpm install    # or npm install
pnpm dev        # ts-node-dev src/index.ts
```

The service listens on `PORT` (default `4700`). Configure the backend to point `SMS_SERVICE_URL` at this port.

## Environment Variables

| Variable | Description | Default |
| --- | --- | --- |
| `PORT` | HTTP port | `4700` |
| `SMS_HISTORY_DB_PATH` | SQLite file for delivery history | `sms-history.db` |
| `SMS_FAILURE_WEBHOOK_URL` | Optional webhook invoked when every channel fails | – |
| `SMS_DEFAULT_CHANNEL` | Primary channel when the caller omits a preference | `sms_twilio` |
| `SMS_FALLBACK_CHANNELS` | Comma-separated fallback list | _(empty)_ |
| `SMS_DEFAULT_COUNTRY_CODE` | Used when normalizing bare phone numbers | _(required for phone channels)_ |
| `SMS_SERVICE_API_KEY` | Shared secret required in `X-API-Key` header | _(disabled when blank)_ |
| `SMS_CHANNEL_FAILURE_THRESHOLD` | Failures before a channel is paused | `3` |
| `SMS_CHANNEL_COOLDOWN_MS` | Cooldown duration after pausing a channel | `60000` |
| `SMS_RATE_LIMIT_MAX_PER_CONTACT` | Max sends per contact within the window | `5` |
| `SMS_RATE_LIMIT_WINDOW_SECONDS` | Window for the rate limit | `300` |
| `SMS_CUSTOM_API_URL` | Destination for the custom HTTP gateway | – |
| `SMS_CUSTOM_API_METHOD` | HTTP method for custom gateway | `POST` |
| `SMS_CUSTOM_API_KEY` | API key/token for the custom gateway | – |
| `SMS_CUSTOM_API_HEADER` | Header name for the custom gateway | `Authorization` |
| `SMS_CUSTOM_API_PREFIX` | Prefix (e.g. `Bearer`) for the custom gateway | `Bearer` |
| `TWILIO_ACCOUNT_SID` | Twilio Account SID (SMS/WhatsApp) | – |
| `TWILIO_AUTH_TOKEN` | Twilio auth token | – |
| `TWILIO_SMS_FROM` | Default Twilio SMS number | – |
| `TWILIO_WHATSAPP_FROM` | Default Twilio WhatsApp number (prefixed with `whatsapp:`) | – |
| `BAILEYS_AUTH_FOLDER` | Directory storing Baileys auth state | `.baileys_auth` |
| `BAILEYS_PRINT_QR` | Print QR in terminal for Baileys pairing | `false` |
| `BAILEYS_LOG_LEVEL` | Baileys logger level | `error` |
| `TELEGRAM_BOT_TOKEN` | Telegram bot token | – |
| `TELEGRAM_DEFAULT_CHAT_ID` | Default Telegram chat/channel | – |
| `DISCORD_BOT_TOKEN` | Discord bot token | – |
| `DISCORD_DEFAULT_USER_ID` | Default Discord user for DMs | – |

## API

`POST /api/send`

```json
{
  "contact": "+15551234567",
  "message": "Your Universus code is 123456",
  "channels": ["sms_twilio", "telegram"],
  "metadata": { "userId": 42 }
}
```

Response:

```json
{
  "success": true,
  "channel": "sms_twilio",
  "destination": "+15551234567"
}
```

If all channels fail the response status is `500` with `{ "success": false, "error": "..." }`.

- Include `Idempotency-Key` header (or `idempotencyKey` in the body) to make requests idempotent; the service caches successful responses and replays them when the same key is re-used.

### Observability Endpoints

- `GET /metrics` – Returns aggregate counters (total requests, per-channel success/failure) plus persisted history stats.
- `GET /history?limit=100` – Retrieves the most recent delivery attempts from the SQLite store.
- `GET /health` – Lightweight health probe.
