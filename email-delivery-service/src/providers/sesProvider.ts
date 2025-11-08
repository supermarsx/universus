import { SESClient, SendEmailCommand } from '@aws-sdk/client-ses';
import { EmailProvider } from './baseProvider';
import { EmailJob, EmailProviderConfig } from '../types';
import { logger } from '../logger';

export class SESProvider implements EmailProvider {
  private client?: SESClient;

  private getClient(config: EmailProviderConfig) {
    if (this.client) return this.client;

    const accessKey = config.ses_access_key || process.env.SES_ACCESS_KEY;
    const secretKey = config.ses_secret_key || process.env.SES_SECRET_KEY;
    const region = config.ses_region || process.env.SES_REGION || 'us-east-1';

    if (!accessKey || !secretKey) {
      throw new Error('SES credentials missing');
    }

    this.client = new SESClient({
      region,
      credentials: {
        accessKeyId: accessKey,
        secretAccessKey: secretKey
      }
    });
    return this.client;
  }

  async send(job: EmailJob, config: EmailProviderConfig): Promise<void> {
    const client = this.getClient(config);
    const command = new SendEmailCommand({
      Destination: { ToAddresses: [job.to] },
      Message: {
        Body: {
          Html: { Data: job.html },
          Text: job.text ? { Data: job.text } : undefined
        },
        Subject: { Data: job.subject }
      },
      Source: `${config.email_from_name} <${config.email_from_address}>`
    });

    await client.send(command);
    logger.debug({ to: job.to, provider: 'ses' }, '[EmailDispatcher] SES email sent');
  }
}
