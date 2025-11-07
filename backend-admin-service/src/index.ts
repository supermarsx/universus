import express from 'express';
import cors from 'cors';
import dotenv from 'dotenv';

import adminRoutes from './routes/admin';
import adminApiRoutes from './routes/adminRoutes';
import { AdminMonitoringService } from './services/adminMonitoringService';

dotenv.config();

const app = express();
const PORT = process.env.ADMIN_PORT || 4002;

app.use(cors());
app.use(express.json());

app.get('/health', (_, res) => {
  res.json({ status: 'ok', service: 'admin', timestamp: new Date().toISOString() });
});

app.use('/api/admin', adminRoutes);
app.use('/api/admin', adminApiRoutes);

const server = app.listen(PORT, () => {
  console.log(`Admin service listening on port ${PORT}`);
});

// Collect server metrics periodically for admin dashboards
setInterval(() => {
  AdminMonitoringService.collectServerMetrics().catch((err) => {
    console.error('Failed to collect server metrics:', err);
  });
}, 300000);

process.on('SIGTERM', () => {
  console.log('Admin service shutting down gracefully');
  server.close(() => process.exit(0));
});
