# Universus Backend

## Real-time Multiplayer & Scaling

This backend uses [Socket.IO](https://socket.io/) with the [Redis adapter](https://socket.io/docs/v4/redis-adapter/) for scalable real-time multiplayer support.

### Running with Redis for Horizontal Scaling

1. **Start a Redis server** (standalone or cluster):
   - Default: `localhost:6379`
   - Configure via `.env`:
     - `REDIS_HOST`, `REDIS_PORT`, `REDIS_PASSWORD` (if needed)
     - For advanced: `REDIS_CLUSTER_NODES`, `REDIS_SHARDED_PUBSUB`

2. **Start multiple backend nodes** (in separate terminals or with a process manager):
   ```sh
   PORT=3000 node dist/index.js
   PORT=3001 node dist/index.js
   # ...
   ```
   Or use [PM2](https://pm2.keymetrics.io/) or Docker Compose for clustering.

3. **Configure your load balancer for sticky sessions** (required for HTTP long-polling):
   - See [Socket.IO docs](https://socket.io/docs/v4/using-multiple-nodes/#enabling-sticky-session) for Traefik, Nginx, etc.

4. **Test cluster-wide events:**
   - Connect a client and emit a `ping` event:
     ```js
     socket.emit('ping', { test: 123 });
     socket.on('pong', (data) => console.log(data));
     ```
   - The response will include the server ID or process PID, verifying cross-node delivery.

### Environment Variables
See `.env.example` for all options.

### SMS / Messaging Verification
Outbound messaging runs through the dedicated `backend-sms-service`, letting the main API remain agnostic of transport details.

- Set `SMS_SERVICE_URL`/`SMS_SERVICE_API_KEY` so the backend can reach the detached service.
- Choose preferred channels via `SMS_VERIFICATION_CHANNEL` and `SMS_VERIFICATION_FALLBACK_CHANNELS`; toggle the feature with `SMS_VERIFICATION_ENABLED=false`.
- The SMS service handles Twilio SMS/WhatsApp, Baileys WhatsApp pairing, Telegram bots, Discord bots, and custom HTTP gateways (configure credentials inside the `backend-sms-service`).
- Use `/api/account/phone/verify/*` endpoints on the main backend to send, verify, resend, and query verification states. The backend forwards the request details to the SMS service and stores the normalized destination returned from it.
- Admins with `notifications:sms:read|write` permissions can manage the SMS connection details via `/api/admin/sms-service/config`, so changes no longer require redeploying environment variables.

### Notes
- Uses `ioredis` for robust Redis connections.
- Redis adapter is required for multi-node real-time scaling.
- For production, secure your Redis and use a managed cluster if possible.

---
For more, see [Socket.IO scaling docs](https://socket.io/docs/v4/using-multiple-nodes/) and [Redis adapter](https://socket.io/docs/v4/redis-adapter/).
