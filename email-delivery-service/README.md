# Email Delivery Service

Dedicated worker that delivers outbound Universus emails from the Redis queue.

## Features

- Consumes jobs from `email:queue` (configurable via `EMAIL_QUEUE_KEY`)
- Uses notification configuration from the shared `config:game_snapshot`
- Supports SMTP, SendGrid, Amazon SES, and MailerSend providers
- Publishes dead-letter jobs to `email:dead-letter` on failures

## Running locally

```bash
cd email-delivery-service
pnpm install    # or npm install
pnpm dev        # runs ts-node src/index.ts
```

### Environment variables

| Variable | Description | Default |
| --- | --- | --- |
| `REDIS_URL` | Redis connection string (overrides host/port) | – |
| `REDIS_HOST` | Redis host | `127.0.0.1` |
| `REDIS_PORT` | Redis port | `6379` |
| `EMAIL_QUEUE_KEY` | Redis list for queued jobs | `email:queue` |
| `EMAIL_DEAD_LETTER_KEY` | Redis list for failed jobs | `email:dead-letter` |
| `CONFIG_SNAPSHOT_KEY` | Redis key for game config snapshot | `config:game_snapshot` |
| `EMAIL_PROVIDER` | Fallback provider when config snapshot unavailable | `smtp` |
| `EMAIL_FROM` | Fallback from address | `noreply@universus.game` |
| `EMAIL_FROM_NAME` | Fallback from display name | `Universus Command` |

Provider-specific secrets (SMTP, SendGrid, SES, MailerSend) can also be supplied through environment variables; admin edits to the Notifications category override them automatically.

## Queue format

The backend enqueues JSON objects containing:

```json
{
  "to": "user@example.com",
  "subject": "Subject line",
  "html": "<p>Body</p>",
  "text": "Body",
  "from": "\"Universus\" <noreply@universus.game>",
  "metadata": {},
  "template": "verification",
  "context": { "token": "123" },
  "created_at": "2024-03-22T10:00:00.000Z"
}
```

The worker enriches the payload with the configured provider settings and sends it through the selected transport.
