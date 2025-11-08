import 'dotenv/config';
import { analyticsQueue } from '../services/analyticsQueue';
import { analyticsService } from '../services/analyticsService';

async function startWorker() {
  if (!analyticsQueue.isEnabled()) {
    console.log('[AnalyticsWorker] Analytics queue disabled or RABBITMQ_URL missing');
    return;
  }

  await analyticsQueue.consume(async (event) => {
    await analyticsService.persistEvent(event);
  });

  console.log('[AnalyticsWorker] Listening for analytics events');
}

startWorker().catch((error) => {
  console.error('[AnalyticsWorker] Fatal error', error);
  process.exit(1);
});
