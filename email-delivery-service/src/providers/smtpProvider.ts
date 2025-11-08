import nodemailer from 'nodemailer';
import { EmailProvider } from './baseProvider';
import { EmailJob, EmailProviderConfig } from '../types';
import { logger } from '../logger';

export class SMTPProvider implements EmailProvider {
  private transporter?: nodemailer.Transporter;

  async send(job: EmailJob, config: EmailProviderConfig): Promise<void> {
    const transporter = await this.getTransporter(config);
    await transporter.sendMail({
      from: job.from || formatFrom(config),
      to: job.to,
      subject: job.subject,
      html: job.html,
      text: job.text
    });
    logger.debug({ to: job.to, provider: 'smtp' }, '[EmailDispatcher] SMTP email sent');
  }

  private async getTransporter(config: EmailProviderConfig) {
    if (this.transporter) {
      return this.transporter;
    }

    const host = config.smtp_host || process.env.SMTP_HOST || 'localhost';
    const port = config.smtp_port ?? (process.env.SMTP_PORT ? parseInt(process.env.SMTP_PORT, 10) : 587);
    const secure = typeof config.smtp_secure === 'boolean'
      ? config.smtp_secure
      : process.env.SMTP_SECURE === 'true';
    const authUser = config.smtp_username || process.env.SMTP_USER;
    const authPass = config.smtp_password || process.env.SMTP_PASS;

    this.transporter = nodemailer.createTransport({
      host,
      port,
      secure,
      auth: authUser && authPass ? { user: authUser, pass: authPass } : undefined
    });

    return this.transporter;
  }
}

function formatFrom(config: EmailProviderConfig) {
  return `"${config.email_from_name}" <${config.email_from_address}>`;
}
