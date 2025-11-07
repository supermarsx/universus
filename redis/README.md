# Universus Redis Service

This folder defines the dedicated Redis image used by the Universus stack.

## Contents

- `Dockerfile` extends `redis:7-alpine`, wires in `redis.conf`, and exposes port `6379`.
- `redis.conf` enables append-only persistence and keeps the bind/port settings consistent with the existing docker-compose volumes.

## Usage

The root `docker-compose.yml` builds this image as the `redis` service:

```bash
docker-compose up -d redis
```

Data is persisted through the `redis_data` named volume that maps to `/data` inside the container.
