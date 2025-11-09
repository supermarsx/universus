// Example: Node.js Prometheus metrics instrumentation
// Install prom-client: npm install prom-client
const client = require('prom-client');
const express = require('express');
const app = express();

// Create a Registry to register the metrics
const register = new client.Registry();

// Create a counter metric
const httpRequestCounter = new client.Counter({
  name: 'http_requests_total',
  help: 'Total number of HTTP requests',
  labelNames: ['method', 'route', 'status']
});
register.registerMetric(httpRequestCounter);

// Create a histogram metric
const httpRequestDuration = new client.Histogram({
  name: 'http_request_duration_seconds',
  help: 'Duration of HTTP requests in seconds',
  labelNames: ['method', 'route', 'status'],
  buckets: [0.1, 0.5, 1, 2, 5]
});
register.registerMetric(httpRequestDuration);

// Middleware to instrument requests
app.use((req, res, next) => {
  const end = httpRequestDuration.startTimer({ method: req.method, route: req.path });
  res.on('finish', () => {
    httpRequestCounter.inc({ method: req.method, route: req.path, status: res.statusCode });
    end({ status: res.statusCode });
  });
  next();
});

// Expose /metrics endpoint
app.get('/metrics', async (req, res) => {
  res.set('Content-Type', register.contentType);
  res.end(await register.metrics());
});

// ... your other routes ...

app.listen(3000, () => {
  console.log('Server running on port 3000');
});
