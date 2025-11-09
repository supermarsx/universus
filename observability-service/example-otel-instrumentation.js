// Example: Node.js OpenTelemetry tracing instrumentation
// Install: npm install @opentelemetry/api @opentelemetry/sdk-node @opentelemetry/auto-instrumentations-node
const { NodeSDK } = require('@opentelemetry/sdk-node');
const { getNodeAutoInstrumentations } = require('@opentelemetry/auto-instrumentations-node');

const sdk = new NodeSDK({
  instrumentations: [getNodeAutoInstrumentations()],
  serviceName: 'universus-backend',
  otlpExporterConfig: {
    url: 'http://localhost:4318/v1/traces', // OTLP HTTP endpoint
  },
});

sdk.start()
  .then(() => {
    console.log('OpenTelemetry tracing initialized');
    // Start your app here (e.g., require('./index'))
  })
  .catch((error) => {
    console.error('Error initializing OpenTelemetry', error);
  });
