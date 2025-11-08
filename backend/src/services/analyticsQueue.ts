import amqp, { Connection, Channel, ConsumeMessage } from 'amqplib';
import { AnalyticsEventPayload } from '../types/analytics';

const RABBIT_URL = process.env.RABBITMQ_URL;
const QUEUE_NAME = process.env.ANALYTICS_QUEUE_NAME || 'analytics_events';
const QUEUE_DISABLED = process.env.ANALYTICS_QUEUE_DISABLED === 'true';

class AnalyticsQueue {
  private connection?: Connection;
  private channel?: Channel;
  private initializing?: Promise<Channel>;

  isEnabled(): boolean {
    return !QUEUE_DISABLED && !!RABBIT_URL;
  }

  private async getChannel(): Promise<Channel> {
    if (!this.isEnabled()) {
      throw new Error('Analytics queue is not enabled');
    }

    if (this.channel) {
      return this.channel;
    }

    if (!this.initializing) {
      this.initializing = this.createChannel();
    }

    try {
      this.channel = await this.initializing;
      return this.channel;
    } finally {
      this.initializing = undefined;
    }
  }

  private async createChannel(): Promise<Channel> {
    const connection = await amqp.connect(RABBIT_URL as string);
    connection.on('close', () => {
      this.connection = undefined;
      this.channel = undefined;
    });
    this.connection = connection;
    const channel = await connection.createChannel();
    await channel.assertQueue(QUEUE_NAME, { durable: true });
    return channel;
  }

  async publish(event: AnalyticsEventPayload): Promise<boolean> {
    if (!this.isEnabled()) {
      return false;
    }

    const channel = await this.getChannel();
    const payload = Buffer.from(JSON.stringify(event));
    return channel.sendToQueue(QUEUE_NAME, payload, { persistent: true });
  }

  async consume(handler: (event: AnalyticsEventPayload) => Promise<void>): Promise<void> {
    if (!this.isEnabled()) {
      console.log('[AnalyticsQueue] Disabled; consumer not started');
      return;
    }

    const channel = await this.getChannel();
    await channel.consume(
      QUEUE_NAME,
      async (msg: ConsumeMessage | null) => {
        if (!msg) return;
        try {
          const payload = JSON.parse(msg.content.toString());
          await handler(payload as AnalyticsEventPayload);
          channel.ack(msg);
        } catch (error) {
          console.error('[AnalyticsQueue] Failed to process message', error);
          channel.nack(msg, false, false);
        }
      },
      { noAck: false }
    );

    console.log('[AnalyticsQueue] Consumer started');
  }
}

export const analyticsQueue = new AnalyticsQueue();
