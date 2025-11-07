// Phase 9: Email Verification Service
// Handles email verification tokens and verification flow

import { pool } from '../config/database';
import crypto from 'crypto';
import {
    EmailVerification,
    VerificationStatus,
    VerifyEmailRequest,
    SecurityEventType,
    SecurityEventSeverity
} from '../types/accountManagement';
import { AccountSecurityService } from './accountSecurityService';

export class EmailVerificationService {
    private static readonly TOKEN_EXPIRY = 24 * 60 * 60 * 1000; // 24 hours
    private static readonly MAX_ATTEMPTS = 5;
    private static readonly RESEND_COOLDOWN = 60 * 1000; // 1 minute

    // =====================================================
    // SEND VERIFICATION EMAIL
    // =====================================================

    static async sendVerificationEmail(
        userId: number,
        email: string,
        ipAddress?: string,
        userAgent?: string
    ): Promise<EmailVerification> {
        // Check rate limiting
        const recentVerification = await pool.query(
            `SELECT * FROM email_verifications 
             WHERE user_id = $1 
             AND email = $2 
             AND sent_at > NOW() - INTERVAL '1 minute'
             ORDER BY sent_at DESC 
             LIMIT 1`,
            [userId, email]
        );

        if (recentVerification.rows.length > 0) {
            throw new Error('Please wait before requesting another verification email');
        }

        // Generate verification token
        const verificationToken = crypto.randomBytes(32).toString('hex');
        const expiresAt = new Date(Date.now() + this.TOKEN_EXPIRY);

        const result = await pool.query(
            `INSERT INTO email_verifications 
             (user_id, email, verification_token, status, expires_at, ip_address, user_agent)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             RETURNING *`,
            [
                userId,
                email,
                verificationToken,
                VerificationStatus.PENDING,
                expiresAt,
                ipAddress,
                userAgent
            ]
        );

        if (result.rows.length === 0) {
            throw new Error('Failed to create verification record');
        }
        const verification = result.rows[0];

        // Send verification email
        const { EmailService } = await import('./emailService');
        await EmailService.sendEmailVerification(email, verificationToken);

        // Log event
        await AccountSecurityService.logSecurityEvent({
            user_id: userId,
            event_type: SecurityEventType.EMAIL_VERIFIED,
            event_description: 'Verification email sent',
            severity: SecurityEventSeverity.INFO,
            ip_address: ipAddress,
            user_agent: userAgent,
            metadata: { email, verification_id: verification.id }
        });

        return verification;
    }

    // =====================================================
    // VERIFY EMAIL
    // =====================================================

    static async verifyEmail(request: VerifyEmailRequest): Promise<void> {
        const client = await pool.connect();
        
        try {
            await client.query('BEGIN');

            // Find verification record
            const verificationResult = await client.query(
                `SELECT * FROM email_verifications 
                 WHERE verification_token = $1 
                 AND status = $2 
                 AND expires_at > NOW()`,
                [request.verification_token, VerificationStatus.PENDING]
            );

            if (verificationResult.rows.length === 0) {
                throw new Error('Invalid or expired verification token');
            }

            const verification = verificationResult.rows[0];

            // Check attempts
            if (verification.attempts >= this.MAX_ATTEMPTS) {
                throw new Error('Maximum verification attempts exceeded');
            }

            // Update verification status
            await client.query(
                `UPDATE email_verifications 
                 SET status = $1, 
                     verified_at = NOW(),
                     attempts = attempts + 1,
                     ip_address = $2,
                     user_agent = $3
                 WHERE id = $4`,
                [
                    VerificationStatus.VERIFIED,
                    request.ip_address,
                    request.user_agent,
                    verification.id
                ]
            );

            // Update user email verification status
            await client.query(
                `UPDATE users 
                 SET email_verified = TRUE,
                     email_verified_at = NOW(),
                     updated_at = NOW()
                 WHERE id = $1`,
                [verification.user_id]
            );

            // Log event
            await AccountSecurityService.logSecurityEvent({
                user_id: verification.user_id,
                event_type: SecurityEventType.EMAIL_VERIFIED,
                event_description: 'Email successfully verified',
                severity: SecurityEventSeverity.INFO,
                ip_address: request.ip_address,
                user_agent: request.user_agent
            });

            await client.query('COMMIT');
        } catch (error) {
            await client.query('ROLLBACK');
            throw error;
        } finally {
            client.release();
        }
    }

    // =====================================================
    // RESEND VERIFICATION
    // =====================================================

    static async resendVerification(
        userId: number,
        email: string,
        ipAddress?: string,
        userAgent?: string
    ): Promise<EmailVerification> {
        // Expire old pending verifications
        await pool.query(
            `UPDATE email_verifications 
             SET status = $1 
             WHERE user_id = $2 
             AND email = $3 
             AND status = $4`,
            [VerificationStatus.EXPIRED, userId, email, VerificationStatus.PENDING]
        );

        // Send new verification
        return await this.sendVerificationEmail(userId, email, ipAddress, userAgent);
    }

    // =====================================================
    // CHECK VERIFICATION STATUS
    // =====================================================

    static async checkVerificationStatus(userId: number, email: string): Promise<{
        is_verified: boolean;
        pending_verification: boolean;
        can_resend: boolean;
    }> {
        // Check if email is verified
        const userResult = await pool.query(
            'SELECT email_verified FROM users WHERE id = $1',
            [userId]
        );

        if (userResult.rows.length === 0) {
            throw new Error('User not found');
        }

        const isVerified = userResult.rows[0].email_verified;

        // Check for pending verification
        const pendingResult = await pool.query(
            `SELECT * FROM email_verifications 
             WHERE user_id = $1 
             AND email = $2 
             AND status = $3 
             AND expires_at > NOW()
             ORDER BY sent_at DESC 
             LIMIT 1`,
            [userId, email, VerificationStatus.PENDING]
        );

        const hasPending = pendingResult.rows.length > 0;

        // Check if can resend (cooldown period)
        let canResend = true;
        if (hasPending) {
            const lastSent = new Date(pendingResult.rows[0].sent_at).getTime();
            const now = Date.now();
            canResend = (now - lastSent) > this.RESEND_COOLDOWN;
        }

        return {
            is_verified: isVerified,
            pending_verification: hasPending,
            can_resend: canResend
        };
    }

    // =====================================================
    // CLEANUP EXPIRED VERIFICATIONS
    // =====================================================

    static async cleanupExpiredVerifications(): Promise<number> {
        const result = await pool.query(
            `UPDATE email_verifications 
             SET status = $1 
             WHERE status = $2 
             AND expires_at < NOW()`,
            [VerificationStatus.EXPIRED, VerificationStatus.PENDING]
        );

        return result.rowCount || 0;
    }
}
