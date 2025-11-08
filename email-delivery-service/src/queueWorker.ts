import { Redis } from 'ioredis';
import { EmailDispatcher } from './emailDispatcher';
import { EmailJob } from './types';
import { logger } from './logger';

const QUEUE_KEY = process.env.EMAIL_QUEUE_KEY || 'email:queue';
const DEAD_LETTER_KEY = process.env.EMAIL_DEAD_LETTER_KEY || 'email:dead-letter';

export class EmailQueueWorker {
  private redis: Redis;
  private dispatcher: EmailDispatcher;
  private running = false;

  constructor(redis: Redis, dispatcher: EmailDispatcher) {
    this.redis = redis;
    this.dispatcher = dispatcher;
  }

  async start(): Promise<void> {
    this.running = true;
    logger.info({ queue: QUEUE_KEY }, '[EmailQueueWorker] Listening for messages');
    while (this.running) {
      try {
        const result = await this.redis.brpop(QUEUE_KEY, 0);
        if (!result || result.length < 2) {
          continue;
        }
        const payload = result[1];
        const job: EmailJob = JSON.parse(payload);
        await this.dispatcher.send(job);
      } catch (error: any) {
        logger.error({ error }, '[EmailQueueWorker] Failed to process email job');
        const failedPayload = JSON.stringify({
          job: error?.job,
          error: error?.message || 'Unknown error',
          failedAt: new Date().toISOString()
        });
        await this.redis.lpush(DEAD_LETTER_KEY, failedPayload);
      }
    }
  }

  async stop(): Promise<void> {
    this.running = false;
  }
}
