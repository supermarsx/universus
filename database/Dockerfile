FROM postgres:16-alpine

ENV POSTGRES_DB=universus_rpg \
    POSTGRES_USER=postgres

COPY scripts/init-db.sh /docker-entrypoint-initdb.d/00-init-db.sh
COPY sql /docker-entrypoint-initdb.d/sql
COPY scripts /opt/universus/database/scripts
COPY sql /opt/universus/database/sql
COPY tests /opt/universus/database/tests

RUN chmod +x /docker-entrypoint-initdb.d/00-init-db.sh \
    /opt/universus/database/scripts/*.sh

EXPOSE 5432
