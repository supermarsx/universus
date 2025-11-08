import sgMail from '@sendgrid/mail';
import { EmailProvider } from './baseProvider';
import { EmailJob, EmailProviderConfig } from '../types';
import { logger } from '../logger';

export class SendGridProvider implements EmailProvider {
  private initialized = false;

  private init(apiKey?: string) {
    if (!apiKey) {
      throw new Error('SendGrid API key missing');
    }
    sgMail.setApiKey(apiKey);
    this.initialized = true;
  }

  async send(job: EmailJob, config: EmailProviderConfig): Promise<void> {
    if (!this.initialized) {
      this.init(config.sendgrid_api_key || process.env.SENDGRID_API_KEY);
    }

    await sgMail.send({
      to: job.to,
      from: {
        email: config.email_from_address,
        name: config.email_from_name
      },
      subject: job.subject,
      html: job.html,
      text: job.text
    });

    logger.debug({ to: job.to, provider: 'sendgrid' }, '[EmailDispatcher] SendGrid email sent');
  }
}
