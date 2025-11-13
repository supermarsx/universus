
// RENAMED: see readme.md

this directory contains the custom redis image and configuration for the universus stack.

## contents

- `dockerfile`: builds from `redis:7-alpine`, copies in `redis.conf`, exposes port 6379, and sets the default command.
- `redis.conf`: main redis configuration file (see below for details).


## build instructions

to build the custom redis image defined in this directory:

# Universus Redis Service

This directory contains the custom Redis image and configuration for the Universus stack.

## Build Instructions

To build the custom Redis image defined in this directory:

```sh
docker-compose build redis
```

This will use the `Dockerfile` and `redis.conf` in this folder to create the `universus_redis` image.

## Usage

The Redis service is managed via Docker Compose. To start Redis:

```sh
docker-compose up -d redis
```

Data is persisted in the `redis_data` named volume, mapped to `/data` inside the container.

## Configuration

The `redis.conf` file provides the following settings:

- `bind 0.0.0.0` — Listen on all interfaces (for container networking)
- `protected-mode yes` — Enables protected mode for security
- `port 6379` — Default Redis port
- `dir /data` — Data directory (mapped to Docker volume)
- `appendonly yes` — Enables append-only file persistence
- `save 900 1`, `save 300 10`, `save 60 10000` — Snapshotting rules for RDB persistence

You can further customize Redis by editing `redis.conf` and rebuilding the image:

```sh
docker-compose build redis
```

Then restart the service:

```sh
docker-compose up -d redis
```

## Environment Variables

The Redis service does not require environment variables by default, but you can override configuration by mounting a custom `redis.conf` or using Redis command-line arguments in the `docker-compose.yml` if needed.

## Connecting to Redis

From other services in the stack, connect to Redis at:

- Host: `redis`
- Port: `6379`

Example (using redis-cli):

```sh
docker exec -it universus_redis redis-cli
```

## Security Notes

- By default, no password is set. For production, consider enabling `requirepass` in `redis.conf` and updating dependent services.
- Only expose Redis to trusted networks. The default config binds to all interfaces for container networking, but you should not expose Redis directly to the public internet.

## References

- [Redis Official Documentation](https://redis.io/docs/)
- [Docker Redis Image](https://hub.docker.com/_/redis)
