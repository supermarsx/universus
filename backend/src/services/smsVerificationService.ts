import fetch from 'node-fetch';
import { pool } from '../config/database';
import {
    SmsVerification,
    VerificationStatus,
    SecurityEventType,
    SecurityEventSeverity,
    SmsVerificationChannel
} from '../types/accountManagement';
import { AccountSecurityService } from './accountSecurityService';
import { SmsServiceConfigService, SmsServiceConfig } from './smsServiceConfigService';
import { canonicalizeSmsChannel, SUPPORTED_SMS_CHANNELS } from '../constants/smsChannels';

interface SendVerificationOptions {
    userId: number;
    phoneNumber: string;
    channel?: SmsVerificationChannel;
    ipAddress?: string;
    userAgent?: string;
}

interface VerifyCodeOptions {
    userId: number;
    code: string;
    ipAddress?: string;
    userAgent?: string;
}

export class SmsVerificationService {
    private static readonly CODE_EXPIRY_MS = (parseInt(process.env.SMS_VERIFICATION_TTL_SECONDS || '300', 10)) * 1000;
    private static readonly MAX_ATTEMPTS = parseInt(process.env.SMS_VERIFICATION_MAX_ATTEMPTS || '5', 10);
    private static readonly RESEND_COOLDOWN_MS = parseInt(process.env.SMS_VERIFICATION_RESEND_COOLDOWN_MS || '60000', 10);

    private static featureEnabled(): boolean {
        return (process.env.SMS_VERIFICATION_ENABLED || 'true').toLowerCase() !== 'false';
    }

    static isEnabled(): boolean {
        return this.featureEnabled();
    }

    private static assertEnabled(): void {
        if (!this.featureEnabled()) {
            throw new Error('SMS verification is disabled');
        }
    }

    private static buildChannelSequence(
        config: SmsServiceConfig,
        override?: SmsVerificationChannel
    ): SmsVerificationChannel[] {
        const sequence: SmsVerificationChannel[] = [];
        const append = (value?: string | SmsVerificationChannel) => {
            if (!value) return;
            const canonical = canonicalizeSmsChannel(value.toString());
            if (!sequence.includes(canonical)) {
                sequence.push(canonical);
            }
        };

        if (override) {
            append(override);
        } else {
            append(config.default_channel);
        }

        if (Array.isArray(config.fallback_channels)) {
            for (const channel of config.fallback_channels) {
                append(channel);
            }
        }

        if (sequence.length === 0) {
            append(SUPPORTED_SMS_CHANNELS[0]);
        }

        return sequence;
    }

    private static generateCode(): string {
        return Math.floor(100000 + Math.random() * 900000).toString();
    }

    private static buildMessage(code: string): string {
        const template = process.env.SMS_VERIFICATION_TEMPLATE || 'Your Universus verification code is {{code}}';
        return template.replace(/{{\s*code\s*}}/gi, code);
    }

    private static async enforceCooldown(userId: number): Promise<void> {
        const result = await pool.query(
            `SELECT sent_at FROM sms_verifications 
             WHERE user_id = $1 
             ORDER BY sent_at DESC 
             LIMIT 1`,
            [userId]
        );

        if (result.rows.length === 0) return;

        const lastSent = new Date(result.rows[0].sent_at).getTime();
        const now = Date.now();

        if ((now - lastSent) < this.RESEND_COOLDOWN_MS) {
            throw new Error('Please wait before requesting another verification code');
        }
    }

    private static async dispatchThroughSmsService(
        config: SmsServiceConfig,
        payload: {
            contact: string;
            channels: SmsVerificationChannel[];
            message: string;
            metadata?: Record<string, any>;
        }
    ): Promise<{ channel: string; destination: string }> {
        const baseUrl = config.service_url || process.env.SMS_SERVICE_URL || 'http://localhost:4700';
        const endpoint = `${baseUrl.replace(/\/$/, '')}/api/send`;
        const headers: Record<string, string> = {
            'Content-Type': 'application/json'
        };
        const apiKey = config.api_key || process.env.SMS_SERVICE_API_KEY;
        if (apiKey) {
            headers['x-api-key'] = apiKey;
        }

        const response = await fetch(endpoint, {
            method: 'POST',
            headers,
            body: JSON.stringify(payload)
        });

        let data: any;
        try {
            data = await response.json();
        } catch (error) {
            data = null;
        }

        if (!response.ok) {
            const message = data?.error || `SMS service error (${response.status})`;
            throw new Error(message);
        }

        if (!data?.success) {
            throw new Error(data?.error || 'SMS service failed to send message');
        }

        if (!data?.channel || !data?.destination) {
            throw new Error('SMS service response missing channel or destination');
        }

        return { channel: data.channel, destination: data.destination };
    }

    static async sendVerificationCode(options: SendVerificationOptions): Promise<SmsVerification> {
        this.assertEnabled();
        await this.enforceCooldown(options.userId);

        const config = await SmsServiceConfigService.getConfig();
        const overrideChannel = options.channel ? canonicalizeSmsChannel(options.channel) : undefined;
        const channelSequence = this.buildChannelSequence(config, overrideChannel);
        const verificationCode = this.generateCode();
        const expiresAt = new Date(Date.now() + this.CODE_EXPIRY_MS);
        const message = this.buildMessage(verificationCode);

        const dispatchResult = await this.dispatchThroughSmsService(config, {
            contact: options.phoneNumber,
            channels: channelSequence,
            message,
            metadata: {
                userId: options.userId
            }
        });

        const result = await pool.query(
            `INSERT INTO sms_verifications 
             (user_id, phone_number, channel, verification_code, status, expires_at, ip_address, user_agent)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             RETURNING *`,
            [
                options.userId,
                dispatchResult.destination,
                dispatchResult.channel,
                verificationCode,
                VerificationStatus.PENDING,
                expiresAt,
                options.ipAddress,
                options.userAgent
            ]
        );

        if (result.rows.length === 0) {
            throw new Error('Failed to create SMS verification record');
        }

        const verification = result.rows[0] as SmsVerification;

        // Update user phone metadata
        await pool.query(
            `UPDATE users 
             SET phone_number = $1, phone_verified = FALSE, updated_at = NOW()
             WHERE id = $2`,
            [dispatchResult.destination, options.userId]
        );

        await AccountSecurityService.logSecurityEvent({
            user_id: options.userId,
            event_type: SecurityEventType.PHONE_VERIFICATION_SENT,
            event_description: 'SMS verification code sent',
            severity: SecurityEventSeverity.INFO,
            ip_address: options.ipAddress,
            user_agent: options.userAgent,
            metadata: { phone_number: dispatchResult.destination, channel: dispatchResult.channel }
        });

        return verification;
    }

    static async verifyCode(options: VerifyCodeOptions): Promise<void> {
        this.assertEnabled();
        const client = await pool.connect();
        try {
            const trimmedCode = options.code.trim();

            await client.query('BEGIN');

            const verificationResult = await client.query(
                `SELECT * FROM sms_verifications 
                 WHERE user_id = $1 
                 AND verification_code = $2 
                 AND status = $3 
                 AND expires_at > NOW()
                 ORDER BY sent_at DESC 
                 LIMIT 1`,
                [options.userId, trimmedCode, VerificationStatus.PENDING]
            );

            if (verificationResult.rows.length === 0) {
                throw new Error('Invalid or expired verification code');
            }

            const verification = verificationResult.rows[0];

            if (verification.attempts >= this.MAX_ATTEMPTS) {
                throw new Error('Maximum verification attempts exceeded');
            }

            await client.query(
                `UPDATE sms_verifications 
                 SET status = $1, verified_at = NOW(), attempts = attempts + 1, ip_address = $2, user_agent = $3
                 WHERE id = $4`,
                [
                    VerificationStatus.VERIFIED,
                    options.ipAddress,
                    options.userAgent,
                    verification.id
                ]
            );

            await client.query(
                `UPDATE users 
                 SET phone_number = $1,
                     phone_verified = TRUE,
                     phone_verified_at = NOW(),
                     updated_at = NOW()
                 WHERE id = $2`,
                [verification.phone_number, options.userId]
            );

            await AccountSecurityService.logSecurityEvent({
                user_id: options.userId,
                event_type: SecurityEventType.PHONE_VERIFIED,
                event_description: 'Phone number verified via SMS',
                severity: SecurityEventSeverity.INFO,
                ip_address: options.ipAddress,
                user_agent: options.userAgent,
                metadata: { phone_number: verification.phone_number }
            });

            await client.query('COMMIT');
        } catch (error) {
            await client.query('ROLLBACK');
            throw error;
        } finally {
            client.release();
        }
    }

    static async resendVerification(options: SendVerificationOptions): Promise<SmsVerification> {
        this.assertEnabled();

        await pool.query(
            `UPDATE sms_verifications 
             SET status = $1 
             WHERE user_id = $2 
             AND status = $3`,
            [
                VerificationStatus.EXPIRED,
                options.userId,
                VerificationStatus.PENDING
            ]
        );

        return this.sendVerificationCode(options);
    }

    static async checkStatus(userId: number): Promise<{
        is_verified: boolean;
        pending_verification: boolean;
        can_resend: boolean;
        phone_number?: string;
        channel?: SmsVerificationChannel;
    }> {
        const userResult = await pool.query(
            `SELECT phone_number, phone_verified 
             FROM users 
             WHERE id = $1`,
            [userId]
        );

        if (userResult.rows.length === 0) {
            throw new Error('User not found');
        }

        const user = userResult.rows[0];

        const pendingResult = await pool.query(
            `SELECT channel, sent_at 
             FROM sms_verifications 
             WHERE user_id = $1 
             AND status = $2 
             AND expires_at > NOW()
             ORDER BY sent_at DESC 
             LIMIT 1`,
            [userId, VerificationStatus.PENDING]
        );

        const hasPending = pendingResult.rows.length > 0;
        let canResend = true;
        let channel: SmsVerificationChannel | undefined;

        if (hasPending) {
            const lastSent = new Date(pendingResult.rows[0].sent_at).getTime();
            const now = Date.now();
            canResend = (now - lastSent) > this.RESEND_COOLDOWN_MS;
            channel = pendingResult.rows[0].channel;
        }

        return {
            is_verified: Boolean(user.phone_verified),
            pending_verification: hasPending,
            can_resend: canResend,
            phone_number: user.phone_number || undefined,
            channel
        };
    }
}
