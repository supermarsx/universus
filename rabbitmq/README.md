# Universus RabbitMQ Service

This directory packages the RabbitMQ broker with management UI support so messaging can be managed independently of other services.

## Contents

- `Dockerfile` extends `rabbitmq:3.13-management`, adds the local configuration, and exposes ports `5672` (AMQP) and `15672` (management UI).
- `rabbitmq.conf` disables the guest loopback restriction and points RabbitMQ to `definitions.json` for optional preloaded resources.
- `definitions.json` starts empty but can include exchanges/queues/bindings if the project ever needs predefined topology.

## Usage

The root `docker-compose.yml` builds this image as the `rabbitmq` service:

```bash
docker-compose up -d rabbitmq
```

Credentials are provided through compose environment variables (`RABBITMQ_DEFAULT_USER` and `RABBITMQ_DEFAULT_PASS`). Data persists via the `rabbitmq_data` named volume.
