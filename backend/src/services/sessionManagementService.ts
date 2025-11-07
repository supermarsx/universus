// Phase 9: Session Management Service
// Handles multi-session tracking, device management, and suspicious activity detection

import { pool } from '../config/database';
import { redisClient } from '../config/redis';
import crypto from 'crypto';
import {
    UserSession,
    SessionStatus,
    CreateSessionRequest,
    SessionListResponse,
    DeviceInfo,
    SecurityEventType,
    SecurityEventSeverity,
    SuspiciousActivityAlert
} from '../types/accountManagement';
import { AccountSecurityService } from './accountSecurityService';

export class SessionManagementService {
    private static readonly SESSION_DURATION = 7 * 24 * 60 * 60 * 1000; // 7 days
    private static readonly CACHE_TTL = 3600; // 1 hour

    // =====================================================
    // SESSION CREATION
    // =====================================================

    static async createSession(request: CreateSessionRequest): Promise<UserSession> {
        const sessionToken = crypto.randomBytes(32).toString('hex');
        const expiresAt = new Date(Date.now() + this.SESSION_DURATION);

        // Parse device info
        const deviceInfo = this.parseDeviceInfo(request.user_agent);

        const result = await pool.query(
            `INSERT INTO user_sessions 
             (user_id, session_token, device_fingerprint, device_name, device_type, 
              browser, os, ip_address, location, latitude, longitude, 
              status, expires_at, last_activity)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, NOW())
             RETURNING *`,
            [
                request.user_id,
                sessionToken,
                request.device_fingerprint,
                request.device_name || deviceInfo.name,
                deviceInfo.type,
                deviceInfo.browser,
                deviceInfo.os,
                request.ip_address,
                request.location,
                request.latitude,
                request.longitude,
                SessionStatus.ACTIVE,
                expiresAt
            ]
        );

        const session = result.rows[0];

        // Check for suspicious activity
        await this.checkSuspiciousActivity(request.user_id, request.ip_address, request.location);

        // Log session creation
        await AccountSecurityService.logSecurityEvent({
            user_id: request.user_id,
            event_type: SecurityEventType.LOGIN_SUCCESS,
            event_description: 'New session created',
            severity: SecurityEventSeverity.INFO,
            ip_address: request.ip_address,
            user_agent: request.user_agent,
            metadata: {
                session_id: session.id,
                device: deviceInfo,
                location: request.location
            }
        });

        // Cache session
        await this.cacheSession(session);

        return session;
    }

    // =====================================================
    // SESSION VALIDATION
    // =====================================================

    static async validateSession(sessionToken: string): Promise<UserSession | null> {
        // Check cache first
        const cachedSession = await this.getCachedSession(sessionToken);
        if (cachedSession) {
            return cachedSession;
        }

        const result = await pool.query(
            `SELECT * FROM user_sessions 
             WHERE session_token = $1 
             AND status = $2 
             AND expires_at > NOW()`,
            [sessionToken, SessionStatus.ACTIVE]
        );

        if (result.rows.length === 0) {
            return null;
        }

        const session = result.rows[0];

        // Update last activity
        await pool.query(
            'UPDATE user_sessions SET last_activity = NOW() WHERE id = $1',
            [session.id]
        );

        session.last_activity = new Date();

        // Cache session
        await this.cacheSession(session);

        return session;
    }

    // =====================================================
    // SESSION TERMINATION
    // =====================================================

    static async terminateSession(sessionId: number, userId: number): Promise<void> {
        const result = await pool.query(
            `UPDATE user_sessions 
             SET status = $1 
             WHERE id = $2 AND user_id = $3
             RETURNING session_token`,
            [SessionStatus.TERMINATED, sessionId, userId]
        );

        if (result.rows.length > 0) {
            await this.invalidateSessionCache(result.rows[0].session_token);
        }

        // Log termination
        await AccountSecurityService.logSecurityEvent({
            user_id: userId,
            event_type: SecurityEventType.LOGOUT,
            event_description: 'Session terminated',
            severity: SecurityEventSeverity.INFO,
            metadata: { session_id: sessionId }
        });
    }

    static async terminateAllSessions(userId: number, exceptSessionId?: number): Promise<number> {
        let query = `UPDATE user_sessions 
                     SET status = $1 
                     WHERE user_id = $2 AND status = $3`;
        const params: any[] = [SessionStatus.TERMINATED, userId, SessionStatus.ACTIVE];

        if (exceptSessionId) {
            query += ' AND id != $4';
            params.push(exceptSessionId);
        }

        query += ' RETURNING session_token';

        const result = await pool.query(query, params);

        // Invalidate all terminated sessions from cache
        for (const row of result.rows) {
            await this.invalidateSessionCache(row.session_token);
        }

        // Log bulk termination
        await AccountSecurityService.logSecurityEvent({
            user_id: userId,
            event_type: SecurityEventType.LOGOUT,
            event_description: `Terminated ${result.rowCount} sessions`,
            severity: SecurityEventSeverity.MEDIUM
        });

        return result.rowCount || 0;
    }

    // =====================================================
    // SESSION QUERIES
    // =====================================================

    static async getActiveSessions(userId: number): Promise<SessionListResponse> {
        const result = await pool.query(
            `SELECT * FROM user_sessions 
             WHERE user_id = $1 
             AND status = $2 
             AND expires_at > NOW()
             ORDER BY last_activity DESC`,
            [userId, SessionStatus.ACTIVE]
        );

        return {
            sessions: result.rows,
            total: result.rows.length,
            active_count: result.rows.length
        };
    }

    static async getSessionById(sessionId: number, userId: number): Promise<UserSession | null> {
        const result = await pool.query(
            'SELECT * FROM user_sessions WHERE id = $1 AND user_id = $2',
            [sessionId, userId]
        );

        return result.rows[0] || null;
    }

    // =====================================================
    // SUSPICIOUS ACTIVITY DETECTION
    // =====================================================

    static async checkSuspiciousActivity(
        userId: number,
        ipAddress: string,
        location?: string
    ): Promise<void> {
        // Get recent sessions for comparison
        const recentSessions = await pool.query(
            `SELECT DISTINCT ip_address, location, latitude, longitude 
             FROM user_sessions 
             WHERE user_id = $1 
             AND created_at > NOW() - INTERVAL '7 days'
             ORDER BY created_at DESC 
             LIMIT 20`,
            [userId]
        );

        const alerts: SuspiciousActivityAlert[] = [];

        // Check for new IP address
        const knownIPs = recentSessions.rows.map(s => s.ip_address);
        if (!knownIPs.includes(ipAddress)) {
            alerts.push({
                user_id: userId,
                alert_type: 'new_ip',
                description: `Login from new IP address: ${ipAddress}`,
                severity: SecurityEventSeverity.MEDIUM,
                detected_at: new Date(),
                ip_address: ipAddress,
                location: location
            });
        }

        // Check for unusual location (if location data available)
        if (location && recentSessions.rows.length > 0) {
            const knownLocations = recentSessions.rows
                .map(s => s.location)
                .filter(l => l);
            
            if (!knownLocations.includes(location)) {
                alerts.push({
                    user_id: userId,
                    alert_type: 'new_location',
                    description: `Login from new location: ${location}`,
                    severity: SecurityEventSeverity.MEDIUM,
                    detected_at: new Date(),
                    ip_address: ipAddress,
                    location: location
                });
            }
        }

        // Check for rapid session creation (potential credential stuffing)
        const recentSessionCount = await pool.query(
            `SELECT COUNT(*) FROM user_sessions 
             WHERE user_id = $1 
             AND created_at > NOW() - INTERVAL '1 hour'`,
            [userId]
        );

        const sessionCount = parseInt(recentSessionCount.rows[0].count);
        if (sessionCount > 10) {
            alerts.push({
                user_id: userId,
                alert_type: 'rapid_sessions',
                description: `Unusual number of session creations: ${sessionCount} in last hour`,
                severity: SecurityEventSeverity.HIGH,
                detected_at: new Date(),
                ip_address: ipAddress
            });
        }

        // Log all alerts
        for (const alert of alerts) {
            await AccountSecurityService.logSecurityEvent({
                user_id: userId,
                event_type: SecurityEventType.SUSPICIOUS_ACTIVITY,
                event_description: alert.description,
                severity: alert.severity,
                ip_address: ipAddress,
                metadata: { alert_type: alert.alert_type, location: location }
            });
        }

        // Flag session as suspicious if high severity alerts
        if (alerts.some(a => a.severity === SecurityEventSeverity.HIGH)) {
            await pool.query(
                `UPDATE user_sessions 
                 SET status = $1 
                 WHERE user_id = $2 
                 AND ip_address = $3 
                 AND created_at > NOW() - INTERVAL '5 minutes'`,
                [SessionStatus.SUSPICIOUS, userId, ipAddress]
            );
        }
    }

    static async getSuspiciousActivities(
        userId: number,
        limit: number = 20
    ): Promise<any[]> {
        const result = await pool.query(
            `SELECT * FROM security_audit_logs 
             WHERE user_id = $1 
             AND event_type = $2 
             ORDER BY created_at DESC 
             LIMIT $3`,
            [userId, SecurityEventType.SUSPICIOUS_ACTIVITY, limit]
        );

        return result.rows;
    }

    // =====================================================
    // DEVICE TRUST MANAGEMENT
    // =====================================================

    static async updateDeviceTrust(
        sessionId: number,
        userId: number,
        isTrusted: boolean
    ): Promise<void> {
        await pool.query(
            `UPDATE user_sessions 
             SET is_trusted = $1 
             WHERE id = $2 AND user_id = $3`,
            [isTrusted, sessionId, userId]
        );

        // Log trust change
        await AccountSecurityService.logSecurityEvent({
            user_id: userId,
            event_type: SecurityEventType.SUSPICIOUS_ACTIVITY,
            event_description: `Device trust ${isTrusted ? 'granted' : 'revoked'}`,
            severity: SecurityEventSeverity.LOW,
            metadata: { session_id: sessionId }
        });
    }

    // =====================================================
    // SESSION CLEANUP
    // =====================================================

    static async cleanupExpiredSessions(): Promise<number> {
        const result = await pool.query(
            `UPDATE user_sessions 
             SET status = $1 
             WHERE status = $2 
             AND expires_at < NOW()
             RETURNING session_token`,
            [SessionStatus.EXPIRED, SessionStatus.ACTIVE]
        );

        // Invalidate expired sessions from cache
        for (const row of result.rows) {
            await this.invalidateSessionCache(row.session_token);
        }

        return result.rowCount || 0;
    }

    // =====================================================
    // DEVICE INFO PARSING
    // =====================================================

    private static parseDeviceInfo(userAgent?: string): DeviceInfo {
        if (!userAgent) {
            return {
                fingerprint: crypto.randomBytes(16).toString('hex'),
                name: 'Unknown Device',
                type: 'unknown',
                browser: 'Unknown',
                os: 'Unknown'
            };
        }

        // Simple user agent parsing (in production, use a library like ua-parser-js)
        let deviceType: 'desktop' | 'mobile' | 'tablet' | 'unknown' = 'desktop';
        if (/mobile|android|iphone/i.test(userAgent)) {
            deviceType = 'mobile';
        } else if (/tablet|ipad/i.test(userAgent)) {
            deviceType = 'tablet';
        }

        let browser = 'Unknown';
        if (/chrome/i.test(userAgent)) browser = 'Chrome';
        else if (/firefox/i.test(userAgent)) browser = 'Firefox';
        else if (/safari/i.test(userAgent)) browser = 'Safari';
        else if (/edge/i.test(userAgent)) browser = 'Edge';

        let os = 'Unknown';
        if (/windows/i.test(userAgent)) os = 'Windows';
        else if (/mac/i.test(userAgent)) os = 'macOS';
        else if (/linux/i.test(userAgent)) os = 'Linux';
        else if (/android/i.test(userAgent)) os = 'Android';
        else if (/ios|iphone|ipad/i.test(userAgent)) os = 'iOS';

        return {
            fingerprint: crypto.createHash('md5').update(userAgent).digest('hex'),
            name: `${browser} on ${os}`,
            type: deviceType,
            browser,
            os
        };
    }

    // =====================================================
    // CACHE OPERATIONS
    // =====================================================

    private static async cacheSession(session: UserSession): Promise<void> {
        try {
            const key = `session:${session.session_token}`;
            await redisClient.setex(
                key,
                this.CACHE_TTL,
                JSON.stringify(session)
            );
        } catch (error) {
            console.error('Failed to cache session:', error);
        }
    }

    private static async getCachedSession(sessionToken: string): Promise<UserSession | null> {
        try {
            const key = `session:${sessionToken}`;
            const cached = await redisClient.get(key);
            
            if (cached) {
                return JSON.parse(cached);
            }
        } catch (error) {
            console.error('Failed to get cached session:', error);
        }
        
        return null;
    }

    private static async invalidateSessionCache(sessionToken: string): Promise<void> {
        try {
            const key = `session:${sessionToken}`;
            await redisClient.del(key);
        } catch (error) {
            console.error('Failed to invalidate session cache:', error);
        }
    }
}
