# Observability Service

This folder contains configuration and setup for observability/monitoring for the Universus project.

## Components
- **Prometheus**: Metrics collection and querying
- **Grafana**: Dashboards and visualization
- **Alertmanager**: Alerting
- **Node Exporter**: Host metrics
- **Blackbox Exporter**: Endpoint probing
- **OpenTelemetry Collector**: Tracing/metrics/logs pipeline

## Quick Start

1. Add the services to your `docker-compose.yml` (see below for example).
2. Run `docker-compose up -d prometheus grafana alertmanager node-exporter blackbox-exporter otel-collector`
3. Access Prometheus at http://localhost:9090 and Grafana at http://localhost:3000

## Prometheus
- Config: `prometheus.yml`
- Dockerfile: `Dockerfile.prometheus`

## Grafana
- Config: `Dockerfile.grafana`
- Provisioning: `grafana-provisioning/`
- Example dashboard: `grafana-provisioning/dashboards/universus-overview.json`

## Alertmanager
- Config: `alertmanager.yml`

## Exporters
- Node Exporter: for host metrics
- Blackbox Exporter: for endpoint checks

## OpenTelemetry Collector
- Config: `otel-collector-config.yaml`

## Example docker-compose.yml snippet

```
  prometheus:
    build:
      context: ./observability-service
      dockerfile: Dockerfile.prometheus
    container_name: universus_prometheus
    ports:
      - "9090:9090"
    volumes:
      - ./observability-service/prometheus.yml:/etc/prometheus/prometheus.yml
    depends_on:
      - backend
      - node-exporter
      - blackbox-exporter

  grafana:
    build:
      context: ./observability-service
      dockerfile: Dockerfile.grafana
    container_name: universus_grafana
    ports:
      - "3000:3000"
    depends_on:
      - prometheus
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin

  alertmanager:
    image: prom/alertmanager:latest
    container_name: universus_alertmanager
    ports:
      - "9093:9093"
    volumes:
      - ./observability-service/alertmanager.yml:/etc/alertmanager/alertmanager.yml

  node-exporter:
    image: prom/node-exporter:latest
    container_name: universus_node_exporter
    ports:
      - "9100:9100"

  blackbox-exporter:
    image: prom/blackbox-exporter:latest
    container_name: universus_blackbox_exporter
    ports:
      - "9115:9115"
    volumes:
      - ./observability-service/blackbox.yml:/etc/blackbox_exporter/config.yml

  otel-collector:
    image: otel/opentelemetry-collector:latest
    container_name: universus_otel_collector
    command: ["--config=/etc/otel-collector-config.yaml"]
    volumes:
      - ./observability-service/otel-collector-config.yaml:/etc/otel-collector-config.yaml
    ports:
      - "4317:4317"
      - "4318:4318"
      - "8889:8889"
```

---

## Adding Metrics to Your Backend

- **Prometheus:**
  - Use the [prom-client](https://github.com/siimon/prom-client) library for Node.js.
  - See `example-prometheus-instrumentation.js` for a ready-to-use Express middleware.
  - Expose `/metrics` endpoint in your backend and add it to `prometheus.yml` scrape configs.

- **OpenTelemetry:**
  - Use the [@opentelemetry/sdk-node](https://opentelemetry.io/docs/instrumentation/js/getting-started/nodejs/) for distributed tracing.
  - See `example-otel-instrumentation.js` for a basic setup.
  - Configure the OTLP exporter to point to `otel-collector:4318`.

## Adding Dashboards to Grafana

- Place dashboard JSON files in `grafana-provisioning/dashboards/`.
- Edit `grafana-provisioning/provisioning/dashboards.yaml` to add new providers if needed.
- On startup, Grafana will auto-import dashboards.

## Viewing Metrics and Traces

- **Prometheus UI:** [http://localhost:9090](http://localhost:9090)
- **Grafana UI:** [http://localhost:3000](http://localhost:3000) (default password: `admin`)
- **Alertmanager:** [http://localhost:9093](http://localhost:9093)
- **OpenTelemetry Collector:** Exposes Prometheus metrics at `:8889` and receives traces at `:4317`/`:4318`.

## Example Instrumentation Files

- `example-prometheus-instrumentation.js`: Node.js/Express Prometheus metrics example
- `example-otel-instrumentation.js`: Node.js OpenTelemetry tracing example

---

Add your own metrics endpoints to Prometheus by editing `prometheus.yml`.

See each subcomponent's README for more details.
