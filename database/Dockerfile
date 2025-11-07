FROM postgres:15-alpine

ENV POSTGRES_DB=universus_rpg \
    POSTGRES_USER=postgres \
    POSTGRES_PASSWORD=postgres

COPY scripts/init-db.sh /docker-entrypoint-initdb.d/00-init-db.sh
COPY sql /docker-entrypoint-initdb.d/sql

RUN chmod +x /docker-entrypoint-initdb.d/00-init-db.sh

EXPOSE 5432
