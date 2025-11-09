// Phase 9: Account Security Service
// Handles account suspension, deletion, locking, and security operations

import { pool } from '../config/database';
import { redisClient } from '../config/redis';
import {
    AccountStatus,
    SuspensionReason,
    SecurityEventType,
    SecurityEventSeverity,
    AccountSuspension,
    SuspendAccountRequest,
    LogSecurityEventRequest,
    SecuritySummaryResponse,
    AccountAccessCheck
} from '../types/accountManagement';

export class AccountSecurityService {
    /**
     * Account security operations: suspensions, deletions, locking and
     * security audit logging. Methods here are high-privilege and will
     * update user status and emit security audit events.
     */
    
    /**
     * Suspend a user's account. Deactivates any existing suspensions, creates
     * a new suspension record, updates the user's account status and logs a security event.
     *
     * @param userId - Target user id to suspend.
     * @param reason - Suspension reason enum.
     * @param adminId - Admin user id performing the suspension.
     * @param expiresAt - Optional expiration date for the suspension.
     * @param notes - Optional admin notes describing the suspension.
     * @returns The created AccountSuspension record.
     */
    static async suspendAccount(
        userId: number,
        reason: SuspensionReason,
        adminId: number,
        expiresAt?: Date,
        notes?: string
    ): Promise<AccountSuspension> {
        const client = await pool.connect();
        
        try {
            await client.query('BEGIN');

            // Deactivate any existing active suspensions
            await client.query(
                `UPDATE account_suspensions 
                 SET is_active = FALSE 
                 WHERE user_id = $1 AND is_active = TRUE`,
                [userId]
            );

            // Create new suspension
            const result = await client.query(
                `INSERT INTO account_suspensions 
                 (user_id, reason, suspended_by, expires_at, notes, is_active)
                 VALUES ($1, $2, $3, $4, $5, TRUE)
                 RETURNING *`,
                [userId, reason, adminId, expiresAt, notes]
            );

            // Update user status
            await client.query(
                `UPDATE users 
                 SET account_status = $1, updated_at = NOW()
                 WHERE id = $2`,
                [AccountStatus.SUSPENDED, userId]
            );

            // Log security event
            await this.logSecurityEvent({
                user_id: userId,
                event_type: SecurityEventType.ACCOUNT_SUSPENDED,
                event_description: `Account suspended: ${reason}`,
                severity: SecurityEventSeverity.HIGH,
                metadata: { admin_id: adminId, expires_at: expiresAt }
            });

            await client.query('COMMIT');

            // Invalidate cache
            await this.invalidateUserCache(userId);

            if (result.rows.length === 0) {
                throw new Error('Suspension record creation failed');
            }
            return result.rows[0];
        } catch (error) {
            await client.query('ROLLBACK');
            throw error;
        } finally {
            client.release();
        }
    }

    /**
     * Lift an active suspension for a user and restore their account status.
     * Logs the action and invalidates caches.
     *
     * @param userId - Target user id.
     * @param adminId - Admin id performing the lift.
     */
    static async liftSuspension(
        userId: number,
        adminId: number
    ): Promise<void> {
        const client = await pool.connect();
        
        try {
            await client.query('BEGIN');

            // Update suspension
            await client.query(
                `UPDATE account_suspensions 
                 SET is_active = FALSE, lifted_at = NOW(), lifted_by = $1
                 WHERE user_id = $2 AND is_active = TRUE`,
                [adminId, userId]
            );

            // Restore user status
            await client.query(
                `UPDATE users 
                 SET account_status = $1, updated_at = NOW()
                 WHERE id = $2`,
                [AccountStatus.ACTIVE, userId]
            );

            // Log security event
            await this.logSecurityEvent({
                user_id: userId,
                event_type: SecurityEventType.ACCOUNT_SUSPENDED,
                event_description: 'Suspension lifted',
                severity: SecurityEventSeverity.MEDIUM,
                metadata: { admin_id: adminId }
            });

            await client.query('COMMIT');

            // Invalidate cache
            await this.invalidateUserCache(userId);
        } catch (error) {
            await client.query('ROLLBACK');
            throw error;
        } finally {
            client.release();
        }
    }

    /**
     * Delete a user's account. Supports soft-delete (mark deleted) and
     * hard-delete (anonymize and remove sensitive related data).
     * Logs a critical security event and invalidates caches.
     *
     * @param userId - Target user id.
     * @param reason - Reason string for deletion.
     * @param soft - When true perform a soft-delete; otherwise remove personal data.
     */
    static async deleteAccount(
        userId: number,
        reason: string,
        soft: boolean = true
    ): Promise<void> {
        const client = await pool.connect();
        
        try {
            await client.query('BEGIN');

            if (soft) {
                // Soft delete: mark as deleted but keep data
                await client.query(
                    `UPDATE users 
                     SET account_status = $1, 
                         deleted_at = NOW(),
                         deletion_reason = $2,
                         updated_at = NOW()
                     WHERE id = $3`,
                    [AccountStatus.DELETED, reason, userId]
                );
            } else {
                // Hard delete: anonymize and remove personal data
                await client.query(
                    `UPDATE users 
                     SET username = 'deleted_' || id,
                         email = 'deleted_' || id || '@deleted.com',
                         password = '',
                         account_status = $1,
                         deleted_at = NOW(),
                         deletion_reason = $2,
                         updated_at = NOW()
                     WHERE id = $3`,
                    [AccountStatus.DELETED, reason, userId]
                );

                // Delete personal data from related tables
                await client.query('DELETE FROM email_verifications WHERE user_id = $1', [userId]);
                await client.query('DELETE FROM password_resets WHERE user_id = $1', [userId]);
                await client.query('DELETE FROM two_factor_auth WHERE user_id = $1', [userId]);
                await client.query('DELETE FROM user_sessions WHERE user_id = $1', [userId]);
            }

            // Log security event
            await this.logSecurityEvent({
                user_id: userId,
                event_type: SecurityEventType.ACCOUNT_DELETED,
                event_description: `Account ${soft ? 'soft' : 'hard'} deleted: ${reason}`,
                severity: SecurityEventSeverity.CRITICAL
            });

            await client.query('COMMIT');

            // Invalidate cache
            await this.invalidateUserCache(userId);
        } catch (error) {
            await client.query('ROLLBACK');
            throw error;
        } finally {
            client.release();
        }
    }

    /**
     * Restore a previously soft-deleted account back to active status.
     * Logs a security event and invalidates caches.
     *
     * @param userId - Target user id to restore.
     */
    static async restoreAccount(userId: number): Promise<void> {
        const client = await pool.connect();
        
        try {
            await client.query('BEGIN');

            await client.query(
                `UPDATE users 
                 SET account_status = $1, 
                     deleted_at = NULL,
                     deletion_reason = NULL,
                     updated_at = NOW()
                 WHERE id = $2 AND account_status = $3`,
                [AccountStatus.ACTIVE, userId, AccountStatus.DELETED]
            );

            // Log security event
            await this.logSecurityEvent({
                user_id: userId,
                event_type: SecurityEventType.ACCOUNT_DELETED,
                event_description: 'Account restored from deletion',
                severity: SecurityEventSeverity.HIGH
            });

            await client.query('COMMIT');

            // Invalidate cache
            await this.invalidateUserCache(userId);
        } catch (error) {
            await client.query('ROLLBACK');
            throw error;
        } finally {
            client.release();
        }
    }

    /**
     * Lock a user's account for a duration (minutes) or indefinitely.
     * Sets locked flags on the user row, logs a high-severity security event
     * and invalidates caches.
     *
     * @param userId - Target user id.
     * @param reason - Reason for locking.
     * @param duration - Optional duration in minutes to auto-unlock.
     */
    static async lockAccount(
        userId: number,
        reason: string,
        duration?: number // minutes
    ): Promise<void> {
        const lockedUntil = duration 
            ? new Date(Date.now() + duration * 60000)
            : null;

        await pool.query(
            `UPDATE users 
             SET is_locked = TRUE,
                 locked_at = NOW(),
                 locked_reason = $1,
                 locked_until = $2,
                 updated_at = NOW()
             WHERE id = $3`,
            [reason, lockedUntil, userId]
        );

        // Log security event
        await this.logSecurityEvent({
            user_id: userId,
            event_type: SecurityEventType.ACCOUNT_LOCKED,
            event_description: `Account locked: ${reason}`,
            severity: SecurityEventSeverity.HIGH,
            metadata: { locked_until: lockedUntil }
        });

        // Invalidate cache
        await this.invalidateUserCache(userId);
    }

    /**
     * Unlock a previously locked account and reset failed login counters.
     * Logs a medium-severity security event and invalidates caches.
     *
     * @param userId - Target user id.
     */
    static async unlockAccount(userId: number): Promise<void> {
        await pool.query(
            `UPDATE users 
             SET is_locked = FALSE,
                 locked_at = NULL,
                 locked_reason = NULL,
                 locked_until = NULL,
                 failed_login_attempts = 0,
                 updated_at = NOW()
             WHERE id = $1`,
            [userId]
        );

        // Log security event
        await this.logSecurityEvent({
            user_id: userId,
            event_type: SecurityEventType.ACCOUNT_LOCKED,
            event_description: 'Account unlocked',
            severity: SecurityEventSeverity.MEDIUM
        });

        // Invalidate cache
        await this.invalidateUserCache(userId);
    }

    /**
     * Run a database check function that verifies whether the account may access
     * game services (not suspended, not banned, etc.). Returns a structure with
     * boolean and reason fields.
     *
     * @param userId - User id to check.
     */
    static async checkAccountAccess(userId: number): Promise<AccountAccessCheck> {
        const result = await pool.query(
            'SELECT * FROM check_account_access($1)',
            [userId]
        );

        if (result.rows.length === 0) {
            return {
                can_access: false,
                reason: 'Account not found'
            };
        }

        return result.rows[0];
    }

    /**
     * Increment the failed login counter for a user and auto-lock the account
     * when thresholds are exceeded.
     *
     * @param userId - User id whose counter will be incremented.
     */
    static async incrementFailedLoginAttempts(userId: number): Promise<void> {
        const result = await pool.query(
            `UPDATE users 
             SET failed_login_attempts = failed_login_attempts + 1,
                 updated_at = NOW()
             WHERE id = $1
             RETURNING failed_login_attempts`,
            [userId]
        );

        const attempts = result.rows[0]?.failed_login_attempts || 0;

        // Auto-lock after 5 failed attempts
        if (attempts >= 5) {
            await this.lockAccount(
                userId,
                'Too many failed login attempts',
                30 // Lock for 30 minutes
            );
        }
    }

    /**
     * Reset the failed login attempts counter for a user.
     *
     * @param userId - User id to reset.
     */
    static async resetFailedLoginAttempts(userId: number): Promise<void> {
        await pool.query(
            `UPDATE users 
             SET failed_login_attempts = 0,
                 updated_at = NOW()
             WHERE id = $1`,
            [userId]
        );
    }

    /**
     * Append a security audit log entry into the security_audit_logs table.
     *
     * @param request - LogSecurityEventRequest containing user_id, event_type and metadata.
     * @returns The id of the created audit log row.
     * @throws Error when insertion fails.
     */
    static async logSecurityEvent(request: LogSecurityEventRequest): Promise<number> {
        const result = await pool.query(
            `INSERT INTO security_audit_logs 
             (user_id, event_type, event_description, severity, ip_address, user_agent, metadata)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             RETURNING id`,
            [
                request.user_id,
                request.event_type,
                request.event_description,
                request.severity,
                request.ip_address,
                request.user_agent,
                request.metadata ? JSON.stringify(request.metadata) : null
            ]
        );

        if (result.rows.length === 0) {
            throw new Error('Failed to create security log entry');
        }
        return result.rows[0].id;
    }

    /**
     * Retrieve security audit logs for a user with pagination.
     *
     * @param userId - User id whose logs will be returned.
     * @param limit - Maximum number of logs to fetch.
     * @param offset - Pagination offset.
     * @returns An object containing logs array and total count.
     */
    static async getSecurityLogs(
        userId: number,
        limit: number = 50,
        offset: number = 0
    ): Promise<{ logs: any[]; total: number }> {
        const [logsResult, countResult] = await Promise.all([
            pool.query(
                `SELECT * FROM security_audit_logs 
                 WHERE user_id = $1 
                 ORDER BY created_at DESC 
                 LIMIT $2 OFFSET $3`,
                [userId, limit, offset]
            ),
            pool.query(
                'SELECT COUNT(*) FROM security_audit_logs WHERE user_id = $1',
                [userId]
            )
        ]);

        return {
            logs: logsResult.rows,
            total: parseInt(countResult.rows[0]?.count || '0')
        };
    }

    // =====================================================
    // SECURITY SUMMARY
    // =====================================================

    static async getSecuritySummary(userId: number): Promise<SecuritySummaryResponse> {
        const result = await pool.query(
            `SELECT 
                u.id as user_id,
                u.account_status,
                u.is_locked,
                u.email_verified,
                COALESCE(tfa.is_enabled, FALSE) as has_2fa,
                (SELECT COUNT(*) FROM user_sessions 
                 WHERE user_id = u.id AND status = 'active' AND expires_at > NOW()) as active_sessions,
                (SELECT COUNT(*) FROM security_audit_logs 
                 WHERE user_id = u.id AND severity IN ('high', 'critical') 
                 AND created_at > NOW() - INTERVAL '30 days') as recent_security_events,
                u.last_login_at,
                u.last_login_ip
             FROM users u
             LEFT JOIN two_factor_auth tfa ON tfa.user_id = u.id
             WHERE u.id = $1`,
            [userId]
        );

        if (result.rows.length === 0) {
            throw new Error('User not found');
        }

        const data = result.rows[0];

        // Calculate risk level
        let riskLevel: 'low' | 'medium' | 'high' = 'low';
        if (data.recent_security_events > 5 || !data.has_2fa) {
            riskLevel = 'high';
        } else if (data.recent_security_events > 2 || data.active_sessions > 5) {
            riskLevel = 'medium';
        }

        return {
            user_id: data.user_id,
            account_status: data.account_status,
            is_locked: data.is_locked,
            email_verified: data.email_verified,
            has_2fa: data.has_2fa,
            active_sessions: parseInt(data.active_sessions),
            recent_security_events: parseInt(data.recent_security_events),
            risk_level: riskLevel,
            last_login: data.last_login_at,
            last_login_ip: data.last_login_ip
        };
    }

    // =====================================================
    // CACHE MANAGEMENT
    // =====================================================

    private static async invalidateUserCache(userId: number): Promise<void> {
        try {
            await redisClient.del(`user:${userId}`);
            await redisClient.del(`user:${userId}:security`);
            await redisClient.del(`user:${userId}:sessions`);
        } catch (error) {
            console.error('Redis cache invalidation failed:', error);
        }
    }
}
