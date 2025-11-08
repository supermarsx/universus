import { Redis } from 'ioredis';
import { EmailProviderConfig } from './types';
import { logger } from './logger';

const SNAPSHOT_KEY = process.env.CONFIG_SNAPSHOT_KEY || 'config:game_snapshot';
const CONFIG_CHANNEL = 'config:changed';

export class EmailConfigLoader {
  private redis: Redis;
  private cache?: EmailProviderConfig;

  constructor(redis: Redis) {
    this.redis = redis;
  }

  async init(): Promise<void> {
    await this.refresh();
    await this.subscribeToChanges();
  }

  getConfig(): EmailProviderConfig {
    if (!this.cache) {
      throw new Error('Email configuration not loaded');
    }
    return this.cache;
  }

  private async subscribeToChanges() {
    const sub = this.redis.duplicate();
    await sub.subscribe(CONFIG_CHANNEL);
    sub.on('message', async (channel, message) => {
      if (channel !== CONFIG_CHANNEL) return;
      try {
        const payload = JSON.parse(message);
        if (payload?.key?.startsWith('notifications.')) {
          logger.info({ key: payload.key }, '[EmailConfig] Notifications config changed, refreshing snapshot');
          await this.refresh();
        }
      } catch (error) {
        logger.warn({ error }, '[EmailConfig] Failed to process config change event');
      }
    });
  }

  private async refresh(): Promise<void> {
    try {
      const snapshotRaw = await this.redis.get(SNAPSHOT_KEY);
      if (snapshotRaw) {
        const snapshot = JSON.parse(snapshotRaw);
        if (snapshot?.notifications) {
          this.cache = snapshot.notifications as EmailProviderConfig;
          return;
        }
      }
      logger.warn('[EmailConfig] Notifications snapshot missing, falling back to environment variables');
      this.cache = this.buildFromEnv();
    } catch (error) {
      logger.error({ error }, '[EmailConfig] Failed to load snapshot, using env defaults');
      this.cache = this.buildFromEnv();
    }
  }

  private buildFromEnv(): EmailProviderConfig {
    return {
      email_provider: process.env.EMAIL_PROVIDER || 'smtp',
      email_from_address: process.env.EMAIL_FROM || 'noreply@universus.game',
      email_from_name: process.env.EMAIL_FROM_NAME || 'Universus Command',
      smtp_host: process.env.SMTP_HOST,
      smtp_port: process.env.SMTP_PORT ? parseInt(process.env.SMTP_PORT, 10) : undefined,
      smtp_secure: process.env.SMTP_SECURE === 'true',
      smtp_username: process.env.SMTP_USER,
      smtp_password: process.env.SMTP_PASS,
      sendgrid_api_key: process.env.SENDGRID_API_KEY,
      ses_access_key: process.env.SES_ACCESS_KEY,
      ses_secret_key: process.env.SES_SECRET_KEY,
      ses_region: process.env.SES_REGION,
      mailersend_api_key: process.env.MAILERSEND_API_KEY,
      queue_enabled: true
    };
  }
}
