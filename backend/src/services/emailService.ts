// Email Service - Handles all email communications
// Uses Nodemailer for sending emails

import nodemailer, { Transporter } from 'nodemailer';

interface EmailOptions {
    to: string;
    subject: string;
    html: string;
    text?: string;
}

export class EmailService {
    private static transporter: Transporter | null = null;

    // Initialize email transporter
    private static getTransporter(): Transporter {
        if (this.transporter) {
            return this.transporter;
        }

        // Configure based on environment
        if (process.env.EMAIL_SERVICE === 'smtp') {
            // SMTP Configuration
            this.transporter = nodemailer.createTransport({
                host: process.env.SMTP_HOST || 'smtp.gmail.com',
                port: parseInt(process.env.SMTP_PORT || '587'),
                secure: process.env.SMTP_SECURE === 'true',
                auth: {
                    user: process.env.SMTP_USER,
                    pass: process.env.SMTP_PASS
                }
            });
        } else if (process.env.EMAIL_SERVICE === 'sendgrid') {
            // SendGrid Configuration
            this.transporter = nodemailer.createTransport({
                host: 'smtp.sendgrid.net',
                port: 587,
                auth: {
                    user: 'apikey',
                    pass: process.env.SENDGRID_API_KEY
                }
            });
        } else {
            // Development mode - use ethereal for testing
            console.warn('No email service configured. Using development mode.');
            // Note: In production, this should throw an error
            // For now, create a test account
            nodemailer.createTestAccount().then(testAccount => {
                this.transporter = nodemailer.createTransport({
                    host: 'smtp.ethereal.email',
                    port: 587,
                    secure: false,
                    auth: {
                        user: testAccount.user,
                        pass: testAccount.pass
                    }
                });
            });
        }

        return this.transporter as Transporter;
    }

    // Send email
    static async send(options: EmailOptions): Promise<void> {
        try {
            const transporter = this.getTransporter();
            
            const mailOptions = {
                from: process.env.EMAIL_FROM || '"Universus Space Empire" <noreply@universus.game>',
                to: options.to,
                subject: options.subject,
                html: options.html,
                text: options.text || this.htmlToText(options.html)
            };

            const info = await transporter.sendMail(mailOptions);
            
            console.log('Email sent:', {
                messageId: info.messageId,
                to: options.to,
                subject: options.subject
            });

            // Log preview URL in development
            if (process.env.NODE_ENV !== 'production') {
                const previewUrl = nodemailer.getTestMessageUrl(info);
                if (previewUrl) {
                    console.log('Preview URL:', previewUrl);
                }
            }
        } catch (error) {
            console.error('Email send error:', error);
            throw new Error('Failed to send email');
        }
    }

    // Email verification
    static async sendEmailVerification(email: string, token: string, userName?: string): Promise<void> {
        const verificationUrl = `${process.env.APP_URL}/account/verify-email?token=${token}`;
        
        const html = `
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

        await this.send({
            to: email,
            subject: 'Verify Your Email - Universus Space Empire',
            html
        });
    }

    // Password reset
    static async sendPasswordReset(email: string, token: string, userName?: string): Promise<void> {
        const resetUrl = `${process.env.APP_URL}/account/reset-password?token=${token}`;
        
        const html = `
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

        await this.send({
            to: email,
            subject: 'Reset Your Password - Universus Space Empire',
            html
        });
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

        // Send to both emails
        await Promise.all([
            this.send({
                to: toEmail,
                subject: 'Verify Account Transfer - Universus Space Empire',
                html: htmlTo
            }),
            this.send({
                to: fromEmail,
                subject: 'Account Transfer Initiated - Universus Space Empire',
                html: htmlFrom
            })
        ]);
    }

    // 2FA enabled notification
    static async send2FAEnabled(email: string, userName?: string): Promise<void> {
        const html = `
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

        await this.send({
            to: email,
            subject: 'Two-Factor Authentication Enabled - Universus Space Empire',
            html
        });
    }

    // Security alert
    static async sendSecurityAlert(email: string, alertType: string, details: string): Promise<void> {
        const html = `
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

        await this.send({
            to: email,
            subject: `Security Alert - ${alertType}`,
            html
        });
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
