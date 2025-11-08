import axios from 'axios';
import { EmailProvider } from './baseProvider';
import { EmailJob, EmailProviderConfig } from '../types';
import { logger } from '../logger';

export class MailerSendProvider implements EmailProvider {
  async send(job: EmailJob, config: EmailProviderConfig): Promise<void> {
    const apiKey = config.mailersend_api_key || process.env.MAILERSEND_API_KEY;
    if (!apiKey) {
      throw new Error('MailerSend API key missing');
    }

    await axios.post(
      'https://api.mailersend.com/v1/email',
      {
        from: {
          email: config.email_from_address,
          name: config.email_from_name
        },
        to: [
          {
            email: job.to
          }
        ],
        subject: job.subject,
        html: job.html,
        text: job.text
      },
      {
        headers: {
          Authorization: `Bearer ${apiKey}`,
          'Content-Type': 'application/json'
        }
      }
    );

    logger.debug({ to: job.to, provider: 'mailersend' }, '[EmailDispatcher] MailerSend email sent');
  }
}
