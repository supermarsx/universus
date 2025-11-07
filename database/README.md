# Universus Database Project

This folder packages the Universus PostgreSQL schema and migrations as a standalone project.

## Contents

- `Dockerfile` builds a custom Postgres image with the schema automatically applied.
- `scripts/init-db.sh` runs during container startup and applies `schema.sql`, each phase schema file, and any migration scripts in order.
- `sql/` holds the schema and migration files that were previously embedded in the backend repository.

## Usage

The root `docker-compose.yml` references this project as the `database` service. To run the full stack:

```bash
docker-compose up -d database
```

Environment variables (`POSTGRES_DB`, `POSTGRES_USER`, `POSTGRES_PASSWORD`) can be overridden via .env or the compose file as needed.
