// Phase 9: Password Recovery Service
// Handles password reset requests and token validation

import { pool } from '../config/database';
import crypto from 'crypto';
import bcrypt from 'bcryptjs';
import {
    PasswordReset,
    PasswordResetWithEmail,
    ResetStatus,
    InitiatePasswordResetRequest,
    CompletePasswordResetRequest,
    SecurityEventType,
    SecurityEventSeverity
} from '../types/accountManagement';
import { AccountSecurityService } from './accountSecurityService';

export class PasswordRecoveryService {
    private static readonly TOKEN_EXPIRY = 60 * 60 * 1000; // 1 hour
    private static readonly MAX_RESETS_PER_DAY = 5;

    // =====================================================
    // INITIATE PASSWORD RESET
    // =====================================================

    static async initiatePasswordReset(
        request: InitiatePasswordResetRequest
    ): Promise<PasswordReset> {
        // Find user by email
        const userResult = await pool.query(
            'SELECT id, email FROM users WHERE email = $1 AND account_status = $2',
            [request.email, 'active']
        );

        if (userResult.rows.length === 0) {
            // Don't reveal if email exists for security
            throw new Error('If this email exists, a reset link has been sent');
        }

        const user = userResult.rows[0];

        // Check rate limiting
        const recentResets = await pool.query(
            `SELECT COUNT(*) FROM password_resets 
             WHERE user_id = $1 
             AND initiated_at > NOW() - INTERVAL '24 hours'`,
            [user.id]
        );

        const resetCount = parseInt(recentResets.rows[0].count);
        if (resetCount >= this.MAX_RESETS_PER_DAY) {
            throw new Error('Too many reset requests. Please try again later.');
        }

        // Expire any pending resets
        await pool.query(
            `UPDATE password_resets 
             SET status = $1 
             WHERE user_id = $2 
             AND status = $3`,
            [ResetStatus.EXPIRED, user.id, ResetStatus.PENDING]
        );

        // Generate reset token
        const resetToken = crypto.randomBytes(32).toString('hex');
        const expiresAt = new Date(Date.now() + this.TOKEN_EXPIRY);

        const result = await pool.query(
            `INSERT INTO password_resets 
             (user_id, reset_token, status, expires_at, ip_address, user_agent)
             VALUES ($1, $2, $3, $4, $5, $6)
             RETURNING *`,
            [
                user.id,
                resetToken,
                ResetStatus.PENDING,
                expiresAt,
                request.ip_address,
                request.user_agent
            ]
        );

        const passwordReset = result.rows[0];

        // Send password reset email
        const { EmailService } = await import('./emailService');
        await EmailService.sendPasswordReset(user.email, resetToken);

        // Log event
        await AccountSecurityService.logSecurityEvent({
            user_id: user.id,
            event_type: SecurityEventType.PASSWORD_RESET,
            event_description: 'Password reset initiated',
            severity: SecurityEventSeverity.MEDIUM,
            ip_address: request.ip_address,
            user_agent: request.user_agent,
            metadata: { reset_id: passwordReset.id }
        });

        return passwordReset;
    }

    // =====================================================
    // VALIDATE RESET TOKEN
    // =====================================================

    static async validateResetToken(resetToken: string): Promise<PasswordResetWithEmail> {
        const result = await pool.query(
            `SELECT pr.*, u.email 
             FROM password_resets pr
             JOIN users u ON u.id = pr.user_id
             WHERE pr.reset_token = $1 
             AND pr.status = $2 
             AND pr.expires_at > NOW()`,
            [resetToken, ResetStatus.PENDING]
        );

        if (result.rows.length === 0) {
            throw new Error('Invalid or expired reset token');
        }

        const reset = result.rows[0];

        // Mark as validated
        await pool.query(
            `UPDATE password_resets 
             SET status = $1, validated_at = NOW()
             WHERE id = $2`,
            [ResetStatus.VALIDATED, reset.id]
        );

        return reset;
    }

    // =====================================================
    // COMPLETE PASSWORD RESET
    // =====================================================

    static async completePasswordReset(
        request: CompletePasswordResetRequest
    ): Promise<void> {
        const client = await pool.connect();
        
        try {
            await client.query('BEGIN');

            // Verify token
            const resetResult = await client.query(
                `SELECT * FROM password_resets 
                 WHERE reset_token = $1 
                 AND status IN ($2, $3)
                 AND expires_at > NOW()`,
                [request.reset_token, ResetStatus.PENDING, ResetStatus.VALIDATED]
            );

            if (resetResult.rows.length === 0) {
                throw new Error('Invalid or expired reset token');
            }

            const reset = resetResult.rows[0];

            // Hash new password
            const hashedPassword = await bcrypt.hash(request.new_password, 10);

            // Update user password
            await client.query(
                `UPDATE users 
                 SET password = $1,
                     failed_login_attempts = 0,
                     locked_until = NULL,
                     updated_at = NOW()
                 WHERE id = $2`,
                [hashedPassword, reset.user_id]
            );

            // Complete reset
            await client.query(
                `UPDATE password_resets 
                 SET status = $1,
                     completed_at = NOW(),
                     ip_address = $2,
                     user_agent = $3
                 WHERE id = $4`,
                [
                    ResetStatus.COMPLETED,
                    request.ip_address,
                    request.user_agent,
                    reset.id
                ]
            );

            // Invalidate all existing sessions for security
            await client.query(
                `UPDATE user_sessions 
                 SET status = 'terminated'
                 WHERE user_id = $1 AND status = 'active'`,
                [reset.user_id]
            );

            // Log event
            await AccountSecurityService.logSecurityEvent({
                user_id: reset.user_id,
                event_type: SecurityEventType.PASSWORD_CHANGE,
                event_description: 'Password successfully reset',
                severity: SecurityEventSeverity.HIGH,
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
    // CANCEL PASSWORD RESET
    // =====================================================

    static async cancelPasswordReset(resetToken: string): Promise<void> {
        const result = await pool.query(
            `UPDATE password_resets 
             SET status = $1 
             WHERE reset_token = $2 
             AND status IN ($3, $4)
             RETURNING user_id`,
            [
                ResetStatus.CANCELLED,
                resetToken,
                ResetStatus.PENDING,
                ResetStatus.VALIDATED
            ]
        );

        if (result.rows.length > 0) {
            // Log cancellation
            await AccountSecurityService.logSecurityEvent({
                user_id: result.rows[0].user_id,
                event_type: SecurityEventType.PASSWORD_RESET,
                event_description: 'Password reset cancelled',
                severity: SecurityEventSeverity.LOW
            });
        }
    }

    // =====================================================
    // CLEANUP EXPIRED RESETS
    // =====================================================

    static async cleanupExpiredResets(): Promise<number> {
        const result = await pool.query(
            `UPDATE password_resets 
             SET status = $1 
             WHERE status IN ($2, $3)
             AND expires_at < NOW()`,
            [ResetStatus.EXPIRED, ResetStatus.PENDING, ResetStatus.VALIDATED]
        );

        return result.rowCount || 0;
    }

    // =====================================================
    // GET RESET HISTORY
    // =====================================================

    static async getResetHistory(
        userId: number,
        limit: number = 10
    ): Promise<PasswordReset[]> {
        const result = await pool.query(
            `SELECT * FROM password_resets 
             WHERE user_id = $1 
             ORDER BY initiated_at DESC 
             LIMIT $2`,
            [userId, limit]
        );

        return result.rows;
    }
}
