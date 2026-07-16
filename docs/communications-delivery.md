# Durable communications delivery

Email and SMS delivery use the PostgreSQL communication outbox introduced by
migration 54. Redis queues, process-local SMS history, arbitrary message bodies,
and caller-supplied destinations are not part of the production contract.

## Security and privacy contract

- Enqueue requests identify a tenant, user, strict category, registered template,
  authoritative event reference, and idempotency key. They never contain a raw
  destination, subject, body, or arbitrary template variables.
- A contact-verification service records keyed-HMAC and masked evidence for the
  current account contact. Raw contacts remain in `users` and are resolved only
  while an active dispatch lease is held.
- Every dispatch renews its lease, rechecks tenant/provider policy, re-resolves
  durable verification evidence, and calls the canonical GDPR communication
  policy immediately before the provider HTTP request.
- Provider requests carry the durable idempotency key. Completion, retry,
  suppression, and failure require the same unexpired lease; a stale worker
  cannot finalize a reclaimed job.
- Delivery and control audit rows are append-only and contain no destination or
  message. Retention removes bounded evidence unless the subject has an active
  GDPR legal hold.

Service JWTs use exact scopes:

- `communications.enqueue`
- `communications.dispatch`
- `communications.audit.read`
- `communications.policy.write`
- `communications.contact.verify`
- `communications.retention`

Cross-tenant credentials additionally require the explicitly provisioned
`communications.global` scope. Human access tokens and shared API keys are not
accepted. Long-running workers reread and verify the token file for every claim,
job, and finalization operation so rotation and expiry fail closed.

## Required runtime configuration

All communication processes require `DATABASE_URL`, the platform-auth verifier
configuration, and `COMMUNICATION_EVIDENCE_HMAC_KEY_BASE64` containing at least
32 bytes after base64 decoding. `UNIVERSUS_ENV` follows the platform values
`production`, `staging`, `development`, or `test` (including established short
aliases). Unknown values fail closed; production and staging require HTTPS.

Email provider and worker:

- `EMAIL_PROVIDER_URL`
- `EMAIL_PROVIDER_BEARER_TOKEN`
- `EMAIL_PROVIDER_KEY` (default `email_http`)
- `EMAIL_PROVIDER_TIMEOUT_SECONDS` (default 15, maximum 120)
- `EMAIL_WORKER_UNIVERSE_ID`
- `EMAIL_WORKER_ID` (default `email-worker-1`)
- `EMAIL_WORKER_CLAIM_LIMIT` (default 20, maximum 100)
- `EMAIL_WORKER_LEASE_SECONDS` (default 90, maximum 900)
- `EMAIL_WORKER_POLL_MILLIS` (default 1000)
- `EMAIL_WORKER_RETRY_BASE_SECONDS` (default 15)
- `EMAIL_WORKER_HEALTH_PORT` (default 3002; container-internal by default)
- `EMAIL_READINESS_MAX_STALENESS_SECONDS` (default 30)
- `COMMUNICATION_SERVICE_TOKEN_FILE`

SMS provider, API, and autonomous dispatcher:

- `SMS_PROVIDER_URL`
- `SMS_PROVIDER_BEARER_TOKEN`
- `SMS_PROVIDER_KEY` (default `sms_http`)
- `SMS_PROVIDER_TIMEOUT_SECONDS` (default 15, maximum 120)
- `SMS_WORKER_UNIVERSE_IDS` (comma-separated; singular
  `SMS_WORKER_UNIVERSE_ID` is accepted)
- `SMS_WORKER_CLAIM_LIMIT` (default 20, maximum 100)
- `SMS_WORKER_POLL_MILLIS` (default 1000)
- `SMS_DISPATCH_WORKER_ID` (default `sms-api-dispatcher-1`)
- `SMS_DISPATCH_LEASE_SECONDS` (default 90, maximum 900)
- `SMS_READINESS_MAX_STALENESS_SECONDS` (default 30)
- `COMMUNICATION_SERVICE_TOKEN_FILE`
- `PORT` (default 3003)

The dispatch lease must exceed the provider timeout by at least five seconds.
Startup rejects an unsafe combination. Plain HTTP is accepted only for an
explicit development/test environment and a loopback host. Provider URLs with
credentials, query parameters, or fragments are rejected.

The email worker exposes `GET /health` on its dedicated health port. It reports
ready only while PostgreSQL and the durable communication schema are reachable,
the dispatch loop is running, and its last successful database claim or health
ping is within the configured staleness window.

## SMS API

`POST /api/send` requires `communications.enqueue` and accepts only:

```json
{
  "universeId": 1,
  "userId": 42,
  "category": "security",
  "templateKey": "password_reset",
  "payloadIdentity": "security_event:dead-beef",
  "idempotencyKey": "password-reset:dead-beef",
  "maxAttempts": 5
}
```

Unknown fields are rejected. The response is `202 Accepted` with only the job
identifier, durable state, and idempotent-replay flag.

The service drains SMS jobs automatically. `POST /api/dispatch` remains an
optional scoped operations control. `GET /api/status` and `GET /api/audit`
require `communications.audit.read` and return aggregate or pseudonymized
evidence only. `GET /health` is ready only while PostgreSQL is reachable, the
durable schema is present, the background dispatcher is running, and the last
successful database operation is fresh.

## Recovery behavior

A process crash leaves the job leased in PostgreSQL. After `lease_until`, another
worker atomically reclaims it with `FOR UPDATE SKIP LOCKED`. Provider timeouts are
retryable, and the repeated request carries the same durable idempotency key.
Jobs transition through `pending`, `leased`, `retry`, and one terminal state:
`sent`, `dead`, or `suppressed`. Operators should use aggregate status/audit
endpoints rather than querying or logging contact data.
