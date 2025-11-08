import { EmailConfigLoader } from './configLoader';
import { EmailJob, EmailProviderConfig } from './types';
import { SMTPProvider } from './providers/smtpProvider';
import { SendGridProvider } from './providers/sendgridProvider';
import { SESProvider } from './providers/sesProvider';
import { MailerSendProvider } from './providers/mailersendProvider';
import { EmailProvider } from './providers/baseProvider';
import { logger } from './logger';
import { htmlToText } from 'html-to-text';

export class EmailDispatcher {
  private configLoader: EmailConfigLoader;
  private smtp = new SMTPProvider();
  private sendgrid = new SendGridProvider();
  private ses = new SESProvider();
  private mailersend = new MailerSendProvider();

  constructor(configLoader: EmailConfigLoader) {
    this.configLoader = configLoader;
  }

  async send(job: EmailJob): Promise<void> {
    const config = this.configLoader.getConfig();
    const provider = this.resolveProvider(config);

    const normalizedJob: EmailJob = {
      ...job,
      text: job.text || htmlToText(job.html)
    };

    try {
      await provider.send(normalizedJob, config);
    } catch (error) {
      logger.error({ error, to: job.to }, '[EmailDispatcher] Failed to send email');
      throw { error, job: normalizedJob };
    }
  }

  private resolveProvider(config: EmailProviderConfig): EmailProvider {
    switch ((config.email_provider || 'smtp').toLowerCase()) {
      case 'sendgrid':
        return this.sendgrid;
      case 'ses':
      case 'amazon_ses':
        return this.ses;
      case 'mailersend':
        return this.mailersend;
      case 'smtp':
      default:
        return this.smtp;
    }
  }
}
