import 'dotenv/config';
import Redis from 'ioredis';
import { logger } from './logger';
import { EmailConfigLoader } from './configLoader';
import { EmailDispatcher } from './emailDispatcher';
import { EmailQueueWorker } from './queueWorker';

function createRedisClient(name: string) {
  const host = process.env.REDIS_HOST || '127.0.0.1';
  const port = parseInt(process.env.REDIS_PORT || '6379', 10);
  const url = process.env.REDIS_URL;

  const client = url ? new Redis(url) : new Redis({ host, port });
  client.on('error', (error) => logger.error({ error }, `[Redis:${name}] connection error`));
  client.on('connect', () => logger.info(`[Redis:${name}] connected`));
  return client;
}

async function start() {
  const redis = createRedisClient('config');
  const queueRedis = createRedisClient('queue');

  const configLoader = new EmailConfigLoader(redis);
  await configLoader.init();

  const dispatcher = new EmailDispatcher(configLoader);
  const worker = new EmailQueueWorker(queueRedis, dispatcher);

  const shutdown = async () => {
    logger.info('[EmailService] Shutting down gracefully...');
    await worker.stop();
    await redis.quit();
    await queueRedis.quit();
    process.exit(0);
  };

  process.on('SIGINT', shutdown);
  process.on('SIGTERM', shutdown);

  await worker.start();
}

start().catch((error) => {
  logger.error({ error }, '[EmailService] Fatal error');
  process.exit(1);
});
