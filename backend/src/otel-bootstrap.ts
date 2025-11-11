// OpenTelemetry bootstrap for Universus backend
// Usage: import './otel-bootstrap' at the top of your entrypoint if OTEL_ENABLED is true

// NOTE: Do not statically import optional OpenTelemetry packages here because
// type resolution/compilation during tests can fail if the optional
// dependencies are not installed. Import dynamically at runtime only when
// tracing is enabled.

if (process.env.OTEL_ENABLED === 'true') {
  (async () => {
    try {
      // Dynamically import optional OTEL modules at runtime
      // eslint-disable-next-line @typescript-eslint/no-var-requires
      const { NodeSDK } = require('@opentelemetry/sdk-node');
      // eslint-disable-next-line @typescript-eslint/no-var-requires
      const { getNodeAutoInstrumentations } = require('@opentelemetry/auto-instrumentations-node');
      // eslint-disable-next-line @typescript-eslint/no-var-requires
      const { OTLPTraceExporter } = require('@opentelemetry/exporter-trace-otlp-http');

      const sdk = new NodeSDK({
        instrumentations: [getNodeAutoInstrumentations()],
        traceExporter: new OTLPTraceExporter({
          url: process.env.OTEL_EXPORTER_OTLP_ENDPOINT || 'http://otel-collector:4318/v1/traces',
        }),
        serviceName: process.env.OTEL_SERVICE_NAME || 'universus-backend',
      });

      // Start SDK asynchronously
      await sdk.start();
      console.log('OpenTelemetry tracing initialized');
    } catch (error: unknown) {
      console.error('Error initializing OpenTelemetry', error);
    }
  })();
}

