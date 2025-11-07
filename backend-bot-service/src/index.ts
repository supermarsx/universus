import express from 'express';
import cors from 'cors';
import dotenv from 'dotenv';
import adminBotRoutes from './routes/adminBots';
import { startBotWorker } from './worker';

dotenv.config();

const app = express();
const port = parseInt(process.env.BOT_SERVICE_PORT || '4001', 10);

app.use(cors());
app.use(express.json({ limit: '1mb' }));

app.get('/health', (_req, res) => {
  res.json({ status: 'ok', service: 'bot-service' });
});

app.use('/api/admin/bots', adminBotRoutes);

app.listen(port, () => {
  console.log(`[Bot Service] Listening on port ${port}`);
  startBotWorker().catch(error => {
    console.error('[Bot Service] Failed to start worker', error);
  });
});
