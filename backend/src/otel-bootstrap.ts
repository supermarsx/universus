// OpenTelemetry bootstrap for Universus backend
// Usage: import './otel-bootstrap' at the top of your entrypoint if OTEL_ENABLED is true
import { NodeSDK } from '@opentelemetry/sdk-node';
import { getNodeAutoInstrumentations } from '@opentelemetry/auto-instrumentations-node';

if (process.env.OTEL_ENABLED === 'true') {
  const sdk = new NodeSDK({
    instrumentations: [getNodeAutoInstrumentations()],
    serviceName: process.env.OTEL_SERVICE_NAME || 'universus-backend',
    otlpExporterConfig: {
      url: process.env.OTEL_EXPORTER_OTLP_ENDPOINT || 'http://otel-collector:4318/v1/traces',
    },
  });
  sdk.start()
    .then(() => {
      console.log('OpenTelemetry tracing initialized');
    })
    .catch((error) => {
      console.error('Error initializing OpenTelemetry', error);
    });
}
