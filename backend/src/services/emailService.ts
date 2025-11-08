// Email Service - enqueues outbound mail for the dedicated delivery worker.

import nodemailer, { Transporter } from 'nodemailer';
import { emailQueueService, EmailJobPayload } from './emailQueueService';
import { gameConfig } from './gameConfigAdapter';
import { NotificationConfig } from '../types/configuration';

interface EmailOptions {
    to: string;
    subject: string;
    html: string;
    text?: string;
    metadata?: Record<string, any>;
    template?: string;
    context?: Record<string, any>;
}

interface TemplateDefaults {
    subject: string;
    html: string;
    text?: string;
}

export class EmailService {
    private static fallbackTransporter: Transporter | null = null;

    private static formatFrom(config?: NotificationConfig): string {
        const name = config?.email_from_name || process.env.EMAIL_FROM_NAME || 'Universus Command';
        const address = config?.email_from_address || process.env.EMAIL_FROM || 'noreply@universus.game';
        return `"${name}" <${address}>`;
    }

    private static async resolveNotificationConfig(): Promise<NotificationConfig | undefined> {
        try {
            return await gameConfig.getNotificationConfig();
        } catch (error) {
            console.warn('[EmailService] Failed to load notification config. Falling back to env.', error);
            return undefined;
        }
    }

    private static shouldUseQueue(config?: NotificationConfig): boolean {
        if (process.env.EMAIL_QUEUE_BYPASS === 'true') {
            return false;
        }
        if (config && config.queue_enabled === false) {
            return false;
        }
        return true;
    }

    static async send(options: EmailOptions, notificationConfig?: NotificationConfig): Promise<void> {
        const resolvedConfig = notificationConfig || await this.resolveNotificationConfig();
        const useQueue = this.shouldUseQueue(resolvedConfig);
        const payload: EmailJobPayload = {
            to: options.to,
            subject: options.subject,
            html: options.html,
            text: options.text || this.htmlToText(options.html),
            from: this.formatFrom(resolvedConfig),
            metadata: options.metadata,
            template: options.template,
            context: options.context
        };

        if (useQueue) {
            await emailQueueService.enqueue(payload);
            return;
        }

        await this.sendDirect(payload, resolvedConfig);
    }

    private static async sendDirect(job: EmailJobPayload, config?: NotificationConfig): Promise<void> {
        try {
            const transporter = await this.getFallbackTransporter(config);
            const info = await transporter.sendMail({
                from: job.from || this.formatFrom(config),
                to: job.to,
                subject: job.subject,
                html: job.html,
                text: job.text || this.htmlToText(job.html)
            });

            if (process.env.NODE_ENV !== 'production') {
                const previewUrl = nodemailer.getTestMessageUrl(info);
                if (previewUrl) {
                    console.log('[EmailService] Preview URL:', previewUrl);
                }
            }
        } catch (error) {
            console.error('[EmailService] Direct send failed:', error);
            throw new Error('Failed to send email');
        }
    }

    private static async getFallbackTransporter(config?: NotificationConfig): Promise<Transporter> {
        if (this.fallbackTransporter) {
            return this.fallbackTransporter;
        }

        const host = config?.smtp_host || process.env.SMTP_HOST || 'smtp.gmail.com';
        const port = parseInt(String(config?.smtp_port ?? process.env.SMTP_PORT ?? '587'), 10);
        const secure = typeof config?.smtp_secure === 'boolean'
            ? config.smtp_secure
            : process.env.SMTP_SECURE === 'true';
        const authUser = config?.smtp_username || process.env.SMTP_USER;
        const authPass = config?.smtp_password || process.env.SMTP_PASS;

        this.fallbackTransporter = nodemailer.createTransport({
            host,
            port,
            secure,
            auth: authUser && authPass ? { user: authUser, pass: authPass } : undefined
        });

        return this.fallbackTransporter;
    }

    private static replacePlaceholders(template: string, context: Record<string, any> = {}) {
        if (!template) return template;
        return template.replace(/{{\s*([\w.]+)\s*}}/g, (_, key) => {
            const value = context[key];
            return value === undefined || value === null ? '' : String(value);
        });
    }

    private static async renderTemplateContent(
        templateKey: string,
        defaults: TemplateDefaults,
        context: Record<string, any>,
        locale: string = 'en',
        notificationConfig?: NotificationConfig
    ) {
        const config = notificationConfig || await this.resolveNotificationConfig();
        const templateSet = config?.templates?.[templateKey];

        if (!templateSet) {
            return defaults;
        }

        const localeKey = locale.toLowerCase();
        const normalizedLocale = localeKey.includes('-') ? localeKey.split('-')[0] : localeKey;

        const template =
            templateSet[localeKey] ||
            templateSet[normalizedLocale] ||
            templateSet.en ||
            Object.values(templateSet)[0];

        if (!template) {
            return defaults;
        }

        const subject = this.replacePlaceholders(template.subject || defaults.subject, context);
        const html = this.replacePlaceholders(template.html || defaults.html, context);
        const textSource = template.text || defaults.text || this.htmlToText(template.html || defaults.html || '');
        const text = this.replacePlaceholders(textSource, context);

        return { subject, html, text };
    }

    // Email verification
    static async sendEmailVerification(email: string, token: string, userName?: string, locale: string = 'en'): Promise<void> {
        const verificationUrl = `${process.env.APP_URL}/account/verify-email?token=${token}`;
        const context = {
            username: userName || 'Commander',
            verification_link: verificationUrl
        };

        const defaultHtml = `
            <!DOCTYPE html>
            <html>
            <head>
                <style>
                    body { font-family: Arial, sans-serif; line-height: 1.6; color: #333; }
                    .container { max-width: 600px; margin: 0 auto; padding: 20px; }
                    .header { background: #2563eb; color: white; padding: 20px; text-align: center; }
                    .content { padding: 30px; background: #f9fafb; }
                    .button { display: inline-block; padding: 12px 30px; background: #2563eb; color: white; text-decoration: none; border-radius: 5px; margin: 20px 0; }
                    .footer { text-align: center; padding: 20px; color: #6b7280; font-size: 14px; }
                    .code { font-size: 24px; font-weight: bold; color: #2563eb; letter-spacing: 2px; }
                </style>
            </head>
            <body>
                <div class="container">
                    <div class="header">
                        <h1>Verify Your Email</h1>
                    </div>
                    <div class="content">
                        <p>Hello${userName ? ' ' + userName : ''},</p>
                        <p>Thank you for joining Universus Space Empire! Please verify your email address to complete your account setup.</p>
                        <p style="text-align: center;">
                            <a href="${verificationUrl}" class="button">Verify Email Address</a>
                        </p>
                        <p>Or copy and paste this link into your browser:</p>
                        <p style="word-break: break-all; color: #2563eb;">${verificationUrl}</p>
                        <p>This link will expire in 24 hours.</p>
                        <p>If you didn't create an account with Universus, please ignore this email.</p>
                    </div>
                    <div class="footer">
                        <p>&copy; 2025 Universus Space Empire. All rights reserved.</p>
                    </div>
                </div>
            </body>
            </html>
        `;

        const defaults = {
            subject: 'Verify Your Email - Universus Space Empire',
            html: defaultHtml,
            text: `Hi ${context.username}, verify your Universus account using ${verificationUrl}`
        };

        const notificationConfig = await this.resolveNotificationConfig();
        const rendered = await this.renderTemplateContent('verification', defaults, context, locale, notificationConfig);

        await this.send({
            to: email,
            subject: rendered.subject,
            html: rendered.html,
            text: rendered.text,
            template: 'verification',
            context
        }, notificationConfig);
    }

    // Password reset
    static async sendPasswordReset(email: string, token: string, userName?: string, locale: string = 'en'): Promise<void> {
        const resetUrl = `${process.env.APP_URL}/account/reset-password?token=${token}`;
        const context = {
            username: userName || 'Commander',
            reset_link: resetUrl
        };

        const defaultHtml = `
            <!DOCTYPE html>
            <html>
            <head>
                <style>
                    body { font-family: Arial, sans-serif; line-height: 1.6; color: #333; }
                    .container { max-width: 600px; margin: 0 auto; padding: 20px; }
                    .header { background: #2563eb; color: white; padding: 20px; text-align: center; }
                    .content { padding: 30px; background: #f9fafb; }
                    .button { display: inline-block; padding: 12px 30px; background: #2563eb; color: white; text-decoration: none; border-radius: 5px; margin: 20px 0; }
                    .footer { text-align: center; padding: 20px; color: #6b7280; font-size: 14px; }
                    .warning { background: #fef3c7; border-left: 4px solid #f59e0b; padding: 15px; margin: 15px 0; }
                </style>
            </head>
            <body>
                <div class="container">
                    <div class="header">
                        <h1>Reset Your Password</h1>
                    </div>
                    <div class="content">
                        <p>Hello${userName ? ' ' + userName : ''},</p>
                        <p>We received a request to reset your password for your Universus Space Empire account.</p>
                        <p style="text-align: center;">
                            <a href="${resetUrl}" class="button">Reset Password</a>
                        </p>
                        <p>Or copy and paste this link into your browser:</p>
                        <p style="word-break: break-all; color: #2563eb;">${resetUrl}</p>
                        <div class="warning">
                            <strong>Security Notice:</strong> This link will expire in 1 hour. If you didn't request a password reset, please ignore this email or contact support if you're concerned about your account security.
                        </div>
                    </div>
                    <div class="footer">
                        <p>&copy; 2025 Universus Space Empire. All rights reserved.</p>
                    </div>
                </div>
            </body>
            </html>
        `;

        const defaults = {
            subject: 'Reset Your Password - Universus Space Empire',
            html: defaultHtml,
            text: `Reset your Universus password using ${resetUrl}`
        };

        const notificationConfig = await this.resolveNotificationConfig();
        const rendered = await this.renderTemplateContent('password_reset', defaults, context, locale, notificationConfig);

        await this.send({
            to: email,
            subject: rendered.subject,
            html: rendered.html,
            text: rendered.text,
            template: 'password_reset',
            context
        }, notificationConfig);
    }

    // Account transfer verification
    static async sendAccountTransfer(fromEmail: string, toEmail: string, token: string): Promise<void> {
        const verifyUrl = `${process.env.APP_URL}/account/verify-transfer?token=${token}`;
        
        const htmlTo = `
            <!DOCTYPE html>
            <html>
            <head>
                <style>
                    body { font-family: Arial, sans-serif; line-height: 1.6; color: #333; }
                    .container { max-width: 600px; margin: 0 auto; padding: 20px; }
                    .header { background: #2563eb; color: white; padding: 20px; text-align: center; }
                    .content { padding: 30px; background: #f9fafb; }
                    .button { display: inline-block; padding: 12px 30px; background: #2563eb; color: white; text-decoration: none; border-radius: 5px; margin: 20px 0; }
                    .footer { text-align: center; padding: 20px; color: #6b7280; font-size: 14px; }
                    .warning { background: #fee2e2; border-left: 4px solid #dc2626; padding: 15px; margin: 15px 0; }
                </style>
            </head>
            <body>
                <div class="container">
                    <div class="header">
                        <h1>Account Transfer Request</h1>
                    </div>
                    <div class="content">
                        <p>Hello,</p>
                        <p>Someone has requested to transfer a Universus Space Empire account from <strong>${fromEmail}</strong> to this email address.</p>
                        <p>If you initiated this request, please click the button below to verify and complete the transfer:</p>
                        <p style="text-align: center;">
                            <a href="${verifyUrl}" class="button">Verify Transfer</a>
                        </p>
                        <p>Or copy and paste this link into your browser:</p>
                        <p style="word-break: break-all; color: #2563eb;">${verifyUrl}</p>
                        <div class="warning">
                            <strong>Important:</strong> This transfer will change the email associated with the account. If you didn't request this, please ignore this email.
                        </div>
                        <p>This link will expire in 24 hours.</p>
                    </div>
                    <div class="footer">
                        <p>&copy; 2025 Universus Space Empire. All rights reserved.</p>
                    </div>
                </div>
            </body>
            </html>
        `;

        const htmlFrom = `
            <!DOCTYPE html>
            <html>
            <head>
                <style>
                    body { font-family: Arial, sans-serif; line-height: 1.6; color: #333; }
                    .container { max-width: 600px; margin: 0 auto; padding: 20px; }
                    .header { background: #2563eb; color: white; padding: 20px; text-align: center; }
                    .content { padding: 30px; background: #f9fafb; }
                    .footer { text-align: center; padding: 20px; color: #6b7280; font-size: 14px; }
                    .info { background: #dbeafe; border-left: 4px solid #2563eb; padding: 15px; margin: 15px 0; }
                </style>
            </head>
            <body>
                <div class="container">
                    <div class="header">
                        <h1>Account Transfer Initiated</h1>
                    </div>
                    <div class="content">
                        <p>Hello,</p>
                        <p>You have initiated a transfer of your Universus Space Empire account to <strong>${toEmail}</strong>.</p>
                        <div class="info">
                            <strong>Next Steps:</strong> The new email address must verify the transfer request within 24 hours. You will receive a confirmation once the transfer is complete.
                        </div>
                        <p>If you didn't request this transfer, please contact support immediately.</p>
                    </div>
                    <div class="footer">
                        <p>&copy; 2025 Universus Space Empire. All rights reserved.</p>
                    </div>
                </div>
            </body>
            </html>
        `;

        const notificationConfig = await this.resolveNotificationConfig();
        const toContext = {
            from_email: fromEmail,
            verify_link: verifyUrl
        };
        const fromContext = {
            to_email: toEmail,
            verify_link: verifyUrl
        };

        const toDefaults = {
            subject: 'Verify Account Transfer - Universus Space Empire',
            html: htmlTo,
            text: `Verify the account transfer request from ${fromEmail} using ${verifyUrl}`
        };
        const fromDefaults = {
            subject: 'Account Transfer Initiated - Universus Space Empire',
            html: htmlFrom,
            text: `You initiated a transfer to ${toEmail}.`
        };

        const [renderedTo, renderedFrom] = await Promise.all([
            this.renderTemplateContent('account_transfer_request', toDefaults, toContext, 'en', notificationConfig),
            this.renderTemplateContent('account_transfer_notification', fromDefaults, fromContext, 'en', notificationConfig)
        ]);

        await Promise.all([
            this.send({
                to: toEmail,
                subject: renderedTo.subject,
                html: renderedTo.html,
                text: renderedTo.text,
                template: 'account_transfer_request',
                context: toContext
            }, notificationConfig),
            this.send({
                to: fromEmail,
                subject: renderedFrom.subject,
                html: renderedFrom.html,
                text: renderedFrom.text,
                template: 'account_transfer_notification',
                context: fromContext
            }, notificationConfig)
        ]);
    }

    // 2FA enabled notification
    static async send2FAEnabled(email: string, userName?: string, locale: string = 'en'): Promise<void> {
        const context = {
            username: userName || 'Commander'
        };

        const defaultHtml = `
            <!DOCTYPE html>
            <html>
            <head>
                <style>
                    body { font-family: Arial, sans-serif; line-height: 1.6; color: #333; }
                    .container { max-width: 600px; margin: 0 auto; padding: 20px; }
                    .header { background: #10b981; color: white; padding: 20px; text-align: center; }
                    .content { padding: 30px; background: #f9fafb; }
                    .footer { text-align: center; padding: 20px; color: #6b7280; font-size: 14px; }
                    .success { background: #dcfce7; border-left: 4px solid #10b981; padding: 15px; margin: 15px 0; }
                </style>
            </head>
            <body>
                <div class="container">
                    <div class="header">
                        <h1>Two-Factor Authentication Enabled</h1>
                    </div>
                    <div class="content">
                        <p>Hello${userName ? ' ' + userName : ''},</p>
                        <div class="success">
                            <strong>Success!</strong> Two-factor authentication has been enabled on your Universus Space Empire account.
                        </div>
                        <p>Your account is now more secure. You'll need to enter a code from your authenticator app each time you log in.</p>
                        <p>If you didn't enable this feature, please contact support immediately.</p>
                    </div>
                    <div class="footer">
                        <p>&copy; 2025 Universus Space Empire. All rights reserved.</p>
                    </div>
                </div>
            </body>
            </html>
        `;

        const defaults = {
            subject: 'Two-Factor Authentication Enabled - Universus Space Empire',
            html: defaultHtml,
            text: 'Two-factor authentication has been enabled on your account.'
        };

        const notificationConfig = await this.resolveNotificationConfig();
        const rendered = await this.renderTemplateContent('two_factor_enabled', defaults, context, locale, notificationConfig);

        await this.send({
            to: email,
            subject: rendered.subject,
            html: rendered.html,
            text: rendered.text,
            template: 'two_factor_enabled',
            context
        }, notificationConfig);
    }

    // Security alert
    static async sendSecurityAlert(email: string, alertType: string, details: string, locale: string = 'en'): Promise<void> {
        const context = {
            alert_type: alertType,
            alert_details: details
        };

        const defaultHtml = `
            <!DOCTYPE html>
            <html>
            <head>
                <style>
                    body { font-family: Arial, sans-serif; line-height: 1.6; color: #333; }
                    .container { max-width: 600px; margin: 0 auto; padding: 20px; }
                    .header { background: #dc2626; color: white; padding: 20px; text-align: center; }
                    .content { padding: 30px; background: #f9fafb; }
                    .footer { text-align: center; padding: 20px; color: #6b7280; font-size: 14px; }
                    .alert { background: #fee2e2; border-left: 4px solid #dc2626; padding: 15px; margin: 15px 0; }
                </style>
            </head>
            <body>
                <div class="container">
                    <div class="header">
                        <h1>Security Alert</h1>
                    </div>
                    <div class="content">
                        <p>Hello,</p>
                        <div class="alert">
                            <strong>${alertType}</strong><br>${details}
                        </div>
                        <p>If this was you, you can safely ignore this email. If not, please secure your account immediately by changing your password and enabling two-factor authentication.</p>
                        <p>For assistance, please contact our support team.</p>
                    </div>
                    <div class="footer">
                        <p>&copy; 2025 Universus Space Empire. All rights reserved.</p>
                    </div>
                </div>
            </body>
            </html>
        `;

        const defaults = {
            subject: `Security Alert - ${alertType}`,
            html: defaultHtml,
            text: `${alertType}: ${details}`
        };

        const notificationConfig = await this.resolveNotificationConfig();
        const rendered = await this.renderTemplateContent('security_alert', defaults, context, locale, notificationConfig);

        await this.send({
            to: email,
            subject: rendered.subject,
            html: rendered.html,
            text: rendered.text,
            template: 'security_alert',
            context
        }, notificationConfig);
    }

    // Convert HTML to plain text (basic implementation)
    private static htmlToText(html: string): string {
        return html
            .replace(/<style[^>]*>.*?<\/style>/gs, '')
            .replace(/<[^>]+>/g, '')
            .replace(/&nbsp;/g, ' ')
            .replace(/&amp;/g, '&')
            .replace(/&lt;/g, '<')
            .replace(/&gt;/g, '>')
            .replace(/\s+/g, ' ')
            .trim();
    }
}
