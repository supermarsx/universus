/**
 * @module backend/services/accountTransferService
 *
 * Handles account ownership transfer workflow: initiation, token-based
 * verification, and completion. Performs validation, rate limiting and
 * emits security audit logs related to account transfers.
 */

// Phase 9: Account Transfer Service
// Handles account ownership transfer with verification

import { pool } from '../config/database';
import crypto from 'crypto';
import {
    AccountTransfer,
    TransferStatus,
    InitiateTransferRequest,
    SecurityEventType,
    SecurityEventSeverity
} from '../types/accountManagement';
import { AccountSecurityService } from './accountSecurityService';

export class AccountTransferService {
    private static readonly TRANSFER_EXPIRY = 24 * 60 * 60 * 1000; // 24 hours
    private static readonly MAX_TRANSFERS_PER_MONTH = 3;

    /**
     * Initiate an account transfer flow. Validates the target email, enforces
     * rate limits, creates a transfer record and sends verification emails.
     *
     * @param request - InitiateTransferRequest containing user_id, to_email, ip_address and user_agent.
     * @returns The created AccountTransfer record.
     * @throws Error when validations fail or DB insertion fails.
     */
    static async initiateTransfer(request: InitiateTransferRequest): Promise<AccountTransfer> {
        // Get current user email
        const userResult = await pool.query(
            'SELECT email, username FROM users WHERE id = $1 AND account_status = $2',
            [request.user_id, 'active']
        );

        if (userResult.rows.length === 0) {
            throw new Error('User not found or account is not active');
        }

        const user = userResult.rows[0];

        // Check if transferring to the same email
        if (user.email.toLowerCase() === request.to_email.toLowerCase()) {
            throw new Error('Cannot transfer account to the same email address');
        }

        // Check rate limiting
        const recentTransfers = await pool.query(
            `SELECT COUNT(*) FROM account_transfers 
             WHERE user_id = $1 
             AND initiated_at > NOW() - INTERVAL '30 days'`,
            [request.user_id]
        );

        const transferCount = parseInt(recentTransfers.rows[0]?.count || '0');
        if (transferCount >= this.MAX_TRANSFERS_PER_MONTH) {
            throw new Error('Maximum transfer limit reached. Please try again next month.');
        }

        // Check for existing pending transfers
        const pendingTransfer = await pool.query(
            `SELECT * FROM account_transfers 
             WHERE user_id = $1 
             AND status = $2`,
            [request.user_id, TransferStatus.PENDING]
        );

        if (pendingTransfer.rows.length > 0) {
            throw new Error('A pending transfer already exists. Please cancel it before creating a new one.');
        }

        // Check if target email already exists
        const existingEmail = await pool.query(
            'SELECT id FROM users WHERE email = $1',
            [request.to_email]
        );

        if (existingEmail.rows.length > 0) {
            throw new Error('Target email is already registered to another account');
        }

        // Generate verification token
        const verificationToken = crypto.randomBytes(32).toString('hex');
        const expiresAt = new Date(Date.now() + this.TRANSFER_EXPIRY);

        const result = await pool.query(
            `INSERT INTO account_transfers 
             (user_id, from_email, to_email, verification_token, status, expires_at, ip_address, user_agent)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             RETURNING *`,
            [
                request.user_id,
                user.email,
                request.to_email,
                verificationToken,
                TransferStatus.PENDING,
                expiresAt,
                request.ip_address,
                request.user_agent
            ]
        );

        if (result.rows.length === 0) {
            throw new Error('Failed to create transfer record');
        }
        const transfer = result.rows[0];

        // Send verification emails to both addresses
        const { EmailService } = await import('./emailService');
        await EmailService.sendAccountTransfer(user.email, request.to_email, verificationToken);

        // Log transfer initiation
        await AccountSecurityService.logSecurityEvent({
            user_id: request.user_id,
            event_type: SecurityEventType.EMAIL_CHANGE,
            event_description: `Account transfer initiated to ${request.to_email}`,
            severity: SecurityEventSeverity.HIGH,
            ip_address: request.ip_address,
            user_agent: request.user_agent,
            metadata: {
                transfer_id: transfer.id,
                from_email: user.email,
                to_email: request.to_email
            }
        });

        return transfer;
    }

    /**
     * Verify a transfer token previously issued during initiation. Marks the
     * transfer as VERIFIED and logs the verification event.
     *
     * @param verificationToken - Token string sent to the user.
     * @param ipAddress - Optional IP address performing the verification.
     * @param userAgent - Optional user agent string.
     * @returns The AccountTransfer record after verification.
     */
    static async verifyTransfer(
        verificationToken: string,
        ipAddress?: string,
        userAgent?: string
    ): Promise<AccountTransfer> {
        const client = await pool.connect();
        
        try {
            await client.query('BEGIN');

            // Find transfer record
            const transferResult = await client.query(
                `SELECT * FROM account_transfers 
                 WHERE verification_token = $1 
                 AND status = $2 
                 AND expires_at > NOW()`,
                [verificationToken, TransferStatus.PENDING]
            );

            if (transferResult.rows.length === 0) {
                throw new Error('Invalid or expired transfer token');
            }

            const transfer = transferResult.rows[0];

            // Check if new email is still available
            const existingEmail = await client.query(
                'SELECT id FROM users WHERE email = $1 AND id != $2',
                [transfer.to_email, transfer.user_id]
            );

            if (existingEmail.rows.length > 0) {
                throw new Error('Target email is no longer available');
            }

            // Update transfer status to verified
            await client.query(
                `UPDATE account_transfers 
                 SET status = $1, verified_at = NOW()
                 WHERE id = $2`,
                [TransferStatus.VERIFIED, transfer.id]
            );

            // Log verification
            await AccountSecurityService.logSecurityEvent({
                user_id: transfer.user_id,
                event_type: SecurityEventType.EMAIL_CHANGE,
                event_description: 'Account transfer verified',
                severity: SecurityEventSeverity.HIGH,
                ip_address: ipAddress,
                user_agent: userAgent,
                metadata: { transfer_id: transfer.id }
            });

            await client.query('COMMIT');

            return { ...transfer, status: TransferStatus.VERIFIED, verified_at: new Date() };
        } catch (error) {
            await client.query('ROLLBACK');
            throw error;
        } finally {
            client.release();
        }
    }

    /**
     * Complete a verified transfer by changing the user's email and terminating
     * active sessions for security. Confirmation code must match a portion of the token.
     *
     * @param transferId - Transfer record id to complete.
     * @param userId - Owner user id confirming the transfer.
     * @param confirmationCode - Short confirmation code derived from the token.
     */
    static async completeTransfer(
        transferId: number,
        userId: number,
        confirmationCode: string
    ): Promise<void> {
        const client = await pool.connect();
        
        try {
            await client.query('BEGIN');

            // Get transfer record
            const transferResult = await client.query(
                `SELECT * FROM account_transfers 
                 WHERE id = $1 
                 AND user_id = $2 
                 AND status = $3`,
                [transferId, userId, TransferStatus.VERIFIED]
            );

            if (transferResult.rows.length === 0) {
                throw new Error('Transfer not found or not verified');
            }

            const transfer = transferResult.rows[0];

            // Verify confirmation code (simple implementation - should use actual 2FA or sent code)
            if (confirmationCode !== transfer.verification_token.substring(0, 8)) {
                throw new Error('Invalid confirmation code');
            }

            // Update user email
            await client.query(
                `UPDATE users 
                 SET email = $1,
                     email_verified = FALSE,
                     updated_at = NOW()
                 WHERE id = $2`,
                [transfer.to_email, userId]
            );

            // Mark transfer as completed
            await client.query(
                `UPDATE account_transfers 
                 SET status = $1, completed_at = NOW()
                 WHERE id = $2`,
                [TransferStatus.COMPLETED, transferId]
            );

            // Terminate all existing sessions for security
            await client.query(
                `UPDATE user_sessions 
                 SET status = 'terminated'
                 WHERE user_id = $1 AND status = 'active'`,
                [userId]
            );

            // Log completion
            await AccountSecurityService.logSecurityEvent({
                user_id: userId,
                event_type: SecurityEventType.EMAIL_CHANGE,
                event_description: `Account transfer completed to ${transfer.to_email}`,
                severity: SecurityEventSeverity.CRITICAL,
                metadata: { transfer_id: transferId }
            });

            await client.query('COMMIT');

            // TODO: Send confirmation email to new address
        } catch (error) {
            await client.query('ROLLBACK');
            throw error;
        } finally {
            client.release();
        }
    }

    /**
     * Cancel a pending or verified transfer. Marks transfer as cancelled and
     * logs the cancellation event.
     *
     * @param transferId - Transfer id to cancel.
     * @param userId - Owner user id performing cancellation.
     */
    static async cancelTransfer(transferId: number, userId: number): Promise<void> {
        const result = await pool.query(
            `UPDATE account_transfers 
             SET status = $1, cancelled_at = NOW()
             WHERE id = $2 
             AND user_id = $3 
             AND status IN ($4, $5)
             RETURNING *`,
            [
                TransferStatus.CANCELLED,
                transferId,
                userId,
                TransferStatus.PENDING,
                TransferStatus.VERIFIED
            ]
        );

        if (result.rows.length > 0) {
            // Log cancellation
            await AccountSecurityService.logSecurityEvent({
                user_id: userId,
                event_type: SecurityEventType.EMAIL_CHANGE,
                event_description: 'Account transfer cancelled',
                severity: SecurityEventSeverity.MEDIUM,
                metadata: { transfer_id: transferId }
            });
        }
    }

    /**
     * Retrieve the latest pending or verified transfer for a user.
     *
     * @param userId - User id to query transfers for.
     * @returns AccountTransfer or null when none exists.
     */
    static async getTransferStatus(
        userId: number
    ): Promise<AccountTransfer | null> {
        const result = await pool.query(
            `SELECT * FROM account_transfers 
             WHERE user_id = $1 
             AND status IN ($2, $3)
             ORDER BY initiated_at DESC 
             LIMIT 1`,
            [userId, TransferStatus.PENDING, TransferStatus.VERIFIED]
        );

        return result.rows[0] || null;
    }

    /**
     * Return historical account transfer records for a user.
     *
     * @param userId - Owner user id.
     * @param limit - Maximum number of history rows to return (default 10).
     * @returns Array of AccountTransfer rows.
     */
    static async getTransferHistory(
        userId: number,
        limit: number = 10
    ): Promise<AccountTransfer[]> {
        const result = await pool.query(
            `SELECT * FROM account_transfers 
             WHERE user_id = $1 
             ORDER BY initiated_at DESC 
             LIMIT $2`,
            [userId, limit]
        );

        return result.rows;
    }

    /**
     * Mark expired pending or verified transfers as EXPIRED.
     *
     * @returns Number of transfers updated.
     */
    static async cleanupExpiredTransfers(): Promise<number> {
        const result = await pool.query(
            `UPDATE account_transfers 
             SET status = $1 
             WHERE status IN ($2, $3)
             AND expires_at < NOW()`,
            [
                TransferStatus.EXPIRED,
                TransferStatus.PENDING,
                TransferStatus.VERIFIED
            ]
        );

        return result.rowCount || 0;
    }
}
