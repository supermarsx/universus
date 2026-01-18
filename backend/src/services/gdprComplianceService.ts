// Phase 9: GDPR Compliance Service
// Handles GDPR requests including data export, deletion, and user rights management

import { pool } from '../config/database';
import { redisClient } from '../config/redis';
import crypto from 'crypto';
import {
    GDPRRequest,
    GDPRRequestType,
    GDPRRequestStatus,
    CreateGDPRRequestRequest,
    GDPRDataExportResponse,
    SecurityEventType,
    SecurityEventSeverity
} from '../types/accountManagement';
import { AccountSecurityService } from './accountSecurityService';

export class GDPRComplianceService {
    private static readonly EXPORT_EXPIRY = 7 * 24 * 60 * 60 * 1000; // 7 days

    // =====================================================
    // CREATE GDPR REQUEST
    // =====================================================

    static async createGDPRRequest(
        request: CreateGDPRRequestRequest
    ): Promise<GDPRRequest> {
        // Check for existing pending requests
        const existingRequest = await pool.query(
            `SELECT * FROM gdpr_requests 
             WHERE user_id = $1 
             AND request_type = $2 
             AND status = $3`,
            [request.user_id, request.request_type, GDPRRequestStatus.PENDING]
        );

        if (existingRequest.rows.length > 0) {
            throw new Error('A pending request of this type already exists');
        }

        const result = await pool.query(
            `INSERT INTO gdpr_requests 
             (user_id, request_type, status, notes)
             VALUES ($1, $2, $3, $4)
             RETURNING *`,
            [
                request.user_id,
                request.request_type,
                GDPRRequestStatus.PENDING,
                request.notes
            ]
        );

        const gdprRequest = result.rows[0];

        // Log GDPR request
        await AccountSecurityService.logSecurityEvent({
            user_id: request.user_id,
            event_type: SecurityEventType.GDPR_REQUEST,
            event_description: `GDPR request created: ${request.request_type}`,
            severity: SecurityEventSeverity.HIGH,
            metadata: { request_id: gdprRequest.id, request_type: request.request_type }
        });

        // Process request based on type
        if (request.request_type === GDPRRequestType.EXPORT_DATA) {
            // Start async data export
            this.processDataExport(gdprRequest.id, request.user_id).catch(console.error);
        }

        return gdprRequest;
    }

    // =====================================================
    // PROCESS DATA EXPORT
    // =====================================================

    static async processDataExport(requestId: number, userId: number): Promise<void> {
        try {
            // Update status to processing
            await pool.query(
                `UPDATE gdpr_requests 
                 SET status = $1, processed_at = NOW()
                 WHERE id = $2`,
                [GDPRRequestStatus.PROCESSING, requestId]
            );

            // Export user data
            const userData = await this.exportUserData(userId);

            // Store export data
            const dataSize = JSON.stringify(userData).length;
            const verificationCode = crypto.randomBytes(16).toString('hex');

            // Create backup record
            const backupResult = await pool.query(
                `INSERT INTO account_data_backups 
                 (user_id, backup_data, backup_size, expires_at)
                 VALUES ($1, $2, $3, NOW() + INTERVAL '7 days')
                 RETURNING id`,
                [userId, JSON.stringify(userData), dataSize]
            );

            const backupId = backupResult.rows[0].id;

            // Generate download URL/token
            const downloadUrl = `/api/account/gdpr/download/${verificationCode}`;

            // Create verification code
            await pool.query(
                `INSERT INTO backup_verification_codes 
                 (backup_id, verification_code, expires_at)
                 VALUES ($1, $2, NOW() + INTERVAL '7 days')`,
                [backupId, verificationCode]
            );

            // Update GDPR request with download URL
            await pool.query(
                `UPDATE gdpr_requests 
                 SET status = $1,
                     completed_at = NOW(),
                     data_url = $2,
                     expires_at = NOW() + INTERVAL '7 days'
                 WHERE id = $3`,
                [GDPRRequestStatus.COMPLETED, downloadUrl, requestId]
            );

            // Log completion
            await AccountSecurityService.logSecurityEvent({
                user_id: userId,
                event_type: SecurityEventType.DATA_EXPORT,
                event_description: 'Data export completed',
                severity: SecurityEventSeverity.MEDIUM,
                metadata: { request_id: requestId, data_size: dataSize }
            });

            try {
                const userResult = await pool.query(
                    'SELECT email, username FROM users WHERE id = $1',
                    [userId]
                );
                const user = userResult.rows[0];
                if (user?.email) {
                    const { EmailService } = await import('./emailService');
                    const expiresAt = new Date(Date.now() + this.EXPORT_EXPIRY);
                    await EmailService.sendGdprExportReady(
                        user.email,
                        downloadUrl,
                        expiresAt,
                        user.username
                    );
                }
            } catch (error) {
                console.error('Failed to send GDPR export email:', error);
            }
        } catch (error) {
            // Update status to failed
            await pool.query(
                `UPDATE gdpr_requests 
                 SET status = $1
                 WHERE id = $2`,
                [GDPRRequestStatus.FAILED, requestId]
            );

            console.error('Data export failed:', error);
        }
    }

    // =====================================================
    // EXPORT USER DATA
    // =====================================================

    static async exportUserData(userId: number): Promise<GDPRDataExportResponse> {
        const client = await pool.connect();
        
        try {
            // Collect all user data from various tables
            const userData: Record<string, any> = {};

            // User profile
            const userResult = await client.query(
                'SELECT id, username, email, created_at, updated_at, last_login_at FROM users WHERE id = $1',
                [userId]
            );
            userData.profile = userResult.rows[0];

            // Planets
            const planetsResult = await client.query(
                'SELECT * FROM planets WHERE user_id = $1',
                [userId]
            );
            userData.planets = planetsResult.rows;

            // Buildings
            const buildingsResult = await client.query(
                `SELECT b.* FROM buildings b
                 JOIN planets p ON p.id = b.planet_id
                 WHERE p.user_id = $1`,
                [userId]
            );
            userData.buildings = buildingsResult.rows;

            // Fleets
            const fleetsResult = await client.query(
                'SELECT * FROM fleets WHERE user_id = $1',
                [userId]
            );
            userData.fleets = fleetsResult.rows;

            // Messages
            const messagesResult = await client.query(
                'SELECT * FROM messages WHERE sender_id = $1 OR receiver_id = $1',
                [userId]
            );
            userData.messages = messagesResult.rows;

            // Security logs
            const securityLogsResult = await client.query(
                'SELECT * FROM security_audit_logs WHERE user_id = $1 ORDER BY created_at DESC LIMIT 1000',
                [userId]
            );
            userData.security_logs = securityLogsResult.rows;

            // Sessions
            const sessionsResult = await client.query(
                'SELECT * FROM user_sessions WHERE user_id = $1 ORDER BY created_at DESC',
                [userId]
            );
            userData.sessions = sessionsResult.rows;

            // Activity logs
            const activityResult = await client.query(
                'SELECT * FROM user_activity_logs WHERE user_id = $1 ORDER BY created_at DESC LIMIT 1000',
                [userId]
            );
            userData.activity_logs = activityResult.rows;

            return {
                user_data: userData,
                export_date: new Date(),
                data_size: JSON.stringify(userData).length
            };
        } finally {
            client.release();
        }
    }

    // =====================================================
    // DOWNLOAD EXPORTED DATA
    // =====================================================

    static async downloadExportedData(verificationCode: string): Promise<GDPRDataExportResponse> {
        // Verify code
        const codeResult = await pool.query(
            `SELECT bvc.*, adb.user_id, adb.backup_data, adb.backup_size 
             FROM backup_verification_codes bvc
             JOIN account_data_backups adb ON adb.id = bvc.backup_id
             WHERE bvc.verification_code = $1 
             AND bvc.is_used = FALSE 
             AND bvc.expires_at > NOW()`,
            [verificationCode]
        );

        if (codeResult.rows.length === 0) {
            throw new Error('Invalid or expired verification code');
        }

        const record = codeResult.rows[0];

        // Mark code as used
        await pool.query(
            `UPDATE backup_verification_codes 
             SET is_used = TRUE, used_at = NOW()
             WHERE verification_code = $1`,
            [verificationCode]
        );

        // Log download
        await AccountSecurityService.logSecurityEvent({
            user_id: record.user_id,
            event_type: SecurityEventType.DATA_EXPORT,
            event_description: 'Data export downloaded',
            severity: SecurityEventSeverity.MEDIUM
        });

        return {
            user_data: JSON.parse(record.backup_data),
            export_date: new Date(),
            data_size: record.backup_size
        };
    }

    // =====================================================
    // PROCESS DATA DELETION
    // =====================================================

    static async processDataDeletion(requestId: number, userId: number): Promise<void> {
        try {
            // Update status
            await pool.query(
                `UPDATE gdpr_requests 
                 SET status = $1, processed_at = NOW()
                 WHERE id = $2`,
                [GDPRRequestStatus.PROCESSING, requestId]
            );

            // Perform soft delete using AccountSecurityService
            await AccountSecurityService.deleteAccount(
                userId,
                'GDPR data deletion request',
                false // Hard delete to remove personal data
            );

            // Update request status
            await pool.query(
                `UPDATE gdpr_requests 
                 SET status = $1, completed_at = NOW()
                 WHERE id = $2`,
                [GDPRRequestStatus.COMPLETED, requestId]
            );

            // Log completion
            await AccountSecurityService.logSecurityEvent({
                user_id: userId,
                event_type: SecurityEventType.DATA_DELETE,
                event_description: 'GDPR data deletion completed',
                severity: SecurityEventSeverity.CRITICAL,
                metadata: { request_id: requestId }
            });
        } catch (error) {
            await pool.query(
                `UPDATE gdpr_requests 
                 SET status = $1
                 WHERE id = $2`,
                [GDPRRequestStatus.FAILED, requestId]
            );

            console.error('Data deletion failed:', error);
        }
    }

    // =====================================================
    // GET GDPR REQUESTS
    // =====================================================

    static async getGDPRRequests(
        userId: number,
        limit: number = 20,
        offset: number = 0
    ): Promise<{ requests: GDPRRequest[]; total: number }> {
        const [requestsResult, countResult] = await Promise.all([
            pool.query(
                `SELECT * FROM gdpr_requests 
                 WHERE user_id = $1 
                 ORDER BY requested_at DESC 
                 LIMIT $2 OFFSET $3`,
                [userId, limit, offset]
            ),
            pool.query(
                'SELECT COUNT(*) FROM gdpr_requests WHERE user_id = $1',
                [userId]
            )
        ]);

        return {
            requests: requestsResult.rows,
            total: parseInt(countResult.rows[0].count)
        };
    }

    // =====================================================
    // CANCEL GDPR REQUEST
    // =====================================================

    static async cancelGDPRRequest(requestId: number, userId: number): Promise<void> {
        const result = await pool.query(
            `UPDATE gdpr_requests 
             SET status = $1 
             WHERE id = $2 AND user_id = $3 AND status = $4
             RETURNING *`,
            [
                GDPRRequestStatus.CANCELLED,
                requestId,
                userId,
                GDPRRequestStatus.PENDING
            ]
        );

        if (result.rows.length > 0) {
            await AccountSecurityService.logSecurityEvent({
                user_id: userId,
                event_type: SecurityEventType.GDPR_REQUEST,
                event_description: 'GDPR request cancelled',
                severity: SecurityEventSeverity.LOW,
                metadata: { request_id: requestId }
            });
        }
    }

    // =====================================================
    // CLEANUP EXPIRED EXPORTS
    // =====================================================

    static async cleanupExpiredExports(): Promise<number> {
        // Delete expired backup data
        const result = await pool.query(
            `DELETE FROM account_data_backups 
             WHERE expires_at < NOW() 
             RETURNING id`
        );

        return result.rowCount || 0;
    }
}
