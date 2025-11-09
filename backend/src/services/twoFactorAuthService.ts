/**
 * @module backend/services/twoFactorAuthService
 *
 * Two-Factor Authentication service (TOTP & backup codes). Handles 2FA
 * setup, verification and backup code management. Integrates with QR code
 * generation and persistent storage of 2FA state for users.
 */

// Phase 9: Two-Factor Authentication Service
// Handles TOTP-based 2FA setup, verification, and backup codes

import { pool } from '../config/database';
import crypto from 'crypto';
import * as speakeasy from 'speakeasy';
import * as QRCode from 'qrcode';
import {
    TwoFactorAuth,
    TwoFactorMethod,
    Setup2FARequest,
    Setup2FAResponse,
    Verify2FARequest,
    SecurityEventType,
    SecurityEventSeverity
} from '../types/accountManagement';
import { AccountSecurityService } from './accountSecurityService';

export class TwoFactorAuthService {
    private static readonly APP_NAME = 'Universus Space Empire';
    private static readonly BACKUP_CODE_COUNT = 10;

    // =====================================================
    // SETUP 2FA
    // =====================================================

    static async setup2FA(request: Setup2FARequest): Promise<Setup2FAResponse> {
        // Check if 2FA already enabled
        const existingResult = await pool.query(
            'SELECT * FROM two_factor_auth WHERE user_id = $1',
            [request.user_id]
        );

        if (existingResult.rows.length > 0 && existingResult.rows[0].is_enabled) {
            throw new Error('2FA is already enabled for this account');
        }

        // Get user email for QR code
        const userResult = await pool.query(
            'SELECT email FROM users WHERE id = $1',
            [request.user_id]
        );

        if (userResult.rows.length === 0) {
            throw new Error('User not found');
        }

        const userEmail = userResult.rows[0].email;

        // Generate secret
        const secret = speakeasy.generateSecret({
            name: `${this.APP_NAME} (${userEmail})`,
            length: 32
        });

        // Generate backup codes
        const backupCodes = await this.generateBackupCodes();

        // Generate QR code
        const qrCode = await QRCode.toDataURL(secret.otpauth_url || '');

        // Store 2FA setup (not enabled yet, needs verification)
        if (existingResult.rows.length > 0) {
            // Update existing record
            await pool.query(
                `UPDATE two_factor_auth 
                 SET secret = $1,
                     method = $2,
                     backup_codes = $3,
                     recovery_email = $4,
                     is_enabled = FALSE,
                     updated_at = NOW()
                 WHERE user_id = $5`,
                [
                    secret.base32,
                    request.method,
                    JSON.stringify(backupCodes),
                    request.recovery_email,
                    request.user_id
                ]
            );
        } else {
            // Create new record
            await pool.query(
                `INSERT INTO two_factor_auth 
                 (user_id, secret, method, backup_codes, recovery_email, is_enabled)
                 VALUES ($1, $2, $3, $4, $5, FALSE)`,
                [
                    request.user_id,
                    secret.base32,
                    request.method,
                    JSON.stringify(backupCodes),
                    request.recovery_email
                ]
            );
        }

        // Log setup initiation
        await AccountSecurityService.logSecurityEvent({
            user_id: request.user_id,
            event_type: SecurityEventType.TWO_FACTOR_ENABLED,
            event_description: '2FA setup initiated',
            severity: SecurityEventSeverity.MEDIUM
        });

        return {
            secret: secret.base32 || '',
            qr_code: qrCode,
            backup_codes: backupCodes
        };
    }

    // =====================================================
    // VERIFY 2FA (Enable after verification)
    // =====================================================

    static async verify2FA(request: Verify2FARequest): Promise<boolean> {
        // Get 2FA record
        const result = await pool.query(
            'SELECT * FROM two_factor_auth WHERE user_id = $1',
            [request.user_id]
        );

        if (result.rows.length === 0) {
            throw new Error('2FA not set up for this account');
        }

        const twoFactorAuth = result.rows[0];

        // Verify TOTP code
        const verified = speakeasy.totp.verify({
            secret: twoFactorAuth.secret,
            encoding: 'base32',
            token: request.code,
            window: 2 // Allow 2 time steps before/after for clock drift
        });

        if (!verified) {
            // Check if it's a backup code
            const backupCodes: string[] = JSON.parse(twoFactorAuth.backup_codes || '[]');
            if (backupCodes.includes(request.code)) {
                // Remove used backup code
                const updatedCodes = backupCodes.filter(c => c !== request.code);
                
                await pool.query(
                    `UPDATE two_factor_auth 
                     SET backup_codes = $1,
                         last_used_at = NOW()
                     WHERE user_id = $2`,
                    [JSON.stringify(updatedCodes), request.user_id]
                );

                // Log backup code usage
                await AccountSecurityService.logSecurityEvent({
                    user_id: request.user_id,
                    event_type: SecurityEventType.TWO_FACTOR_ENABLED,
                    event_description: 'Backup code used for 2FA verification',
                    severity: SecurityEventSeverity.MEDIUM,
                    metadata: { backup_codes_remaining: updatedCodes.length }
                });

                return true;
            }

            return false;
        }

        // If this is the first verification, enable 2FA
        if (!twoFactorAuth.is_enabled) {
            await pool.query(
                `UPDATE two_factor_auth 
                 SET is_enabled = TRUE,
                     verified_at = NOW(),
                     last_used_at = NOW()
                 WHERE user_id = $1`,
                [request.user_id]
            );

            // Log 2FA enabled
            await AccountSecurityService.logSecurityEvent({
                user_id: request.user_id,
                event_type: SecurityEventType.TWO_FACTOR_ENABLED,
                event_description: '2FA successfully enabled',
                severity: SecurityEventSeverity.HIGH
            });
        } else {
            // Update last used
            await pool.query(
                'UPDATE two_factor_auth SET last_used_at = NOW() WHERE user_id = $1',
                [request.user_id]
            );
        }

        return true;
    }

    // =====================================================
    // DISABLE 2FA
    // =====================================================

    static async disable2FA(userId: number, verificationCode: string): Promise<void> {
        // Verify the code before disabling
        const isValid = await this.verify2FA({ user_id: userId, code: verificationCode });

        if (!isValid) {
            throw new Error('Invalid verification code');
        }

        // Disable 2FA
        await pool.query(
            'DELETE FROM two_factor_auth WHERE user_id = $1',
            [userId]
        );

        // Log 2FA disabled
        await AccountSecurityService.logSecurityEvent({
            user_id: userId,
            event_type: SecurityEventType.TWO_FACTOR_DISABLED,
            event_description: '2FA disabled',
            severity: SecurityEventSeverity.HIGH
        });
    }

    // =====================================================
    // BACKUP CODES MANAGEMENT
    // =====================================================

    static async generateBackupCodes(): Promise<string[]> {
        const codes: string[] = [];
        
        for (let i = 0; i < this.BACKUP_CODE_COUNT; i++) {
            // Generate 8-character alphanumeric code
            const code = crypto.randomBytes(4).toString('hex').toUpperCase();
            codes.push(code);
        }

        return codes;
    }

    static async regenerateBackupCodes(
        userId: number,
        verificationCode: string
    ): Promise<string[]> {
        // Verify the code before regenerating
        const isValid = await this.verify2FA({ user_id: userId, code: verificationCode });

        if (!isValid) {
            throw new Error('Invalid verification code');
        }

        // Generate new backup codes
        const newCodes = await this.generateBackupCodes();

        // Update database
        await pool.query(
            `UPDATE two_factor_auth 
             SET backup_codes = $1,
                 updated_at = NOW()
             WHERE user_id = $2`,
            [JSON.stringify(newCodes), userId]
        );

        // Log backup code regeneration
        await AccountSecurityService.logSecurityEvent({
            user_id: userId,
            event_type: SecurityEventType.TWO_FACTOR_ENABLED,
            event_description: 'Backup codes regenerated',
            severity: SecurityEventSeverity.MEDIUM
        });

        return newCodes;
    }

    static async getBackupCodes(userId: number): Promise<string[]> {
        const result = await pool.query(
            'SELECT backup_codes FROM two_factor_auth WHERE user_id = $1 AND is_enabled = TRUE',
            [userId]
        );

        if (result.rows.length === 0) {
            return [];
        }

        return JSON.parse(result.rows[0].backup_codes || '[]');
    }

    static async verifyBackupCode(userId: number, code: string): Promise<boolean> {
        const result = await pool.query(
            'SELECT backup_codes FROM two_factor_auth WHERE user_id = $1 AND is_enabled = TRUE',
            [userId]
        );

        if (result.rows.length === 0) {
            return false;
        }

        const backupCodes: string[] = JSON.parse(result.rows[0].backup_codes || '[]');
        
        if (!backupCodes.includes(code.toUpperCase())) {
            return false;
        }

        // Remove used backup code
        const updatedCodes = backupCodes.filter(c => c !== code.toUpperCase());
        
        await pool.query(
            `UPDATE two_factor_auth 
             SET backup_codes = $1,
                 last_used_at = NOW()
             WHERE user_id = $2`,
            [JSON.stringify(updatedCodes), userId]
        );

        // Log backup code usage
        await AccountSecurityService.logSecurityEvent({
            user_id: userId,
            event_type: SecurityEventType.TWO_FACTOR_ENABLED,
            event_description: 'Backup code used',
            severity: SecurityEventSeverity.MEDIUM,
            metadata: { backup_codes_remaining: updatedCodes.length }
        });

        return true;
    }

    // =====================================================
    // 2FA STATUS
    // =====================================================

    static async get2FAStatus(userId: number): Promise<{
        is_enabled: boolean;
        method?: TwoFactorMethod;
        backup_codes_remaining?: number;
        last_used?: Date;
    }> {
        const result = await pool.query(
            'SELECT * FROM two_factor_auth WHERE user_id = $1',
            [userId]
        );

        if (result.rows.length === 0) {
            return { is_enabled: false };
        }

        const twoFactorAuth = result.rows[0];
        const backupCodes: string[] = JSON.parse(twoFactorAuth.backup_codes || '[]');

        return {
            is_enabled: twoFactorAuth.is_enabled,
            method: twoFactorAuth.method,
            backup_codes_remaining: backupCodes.length,
            last_used: twoFactorAuth.last_used_at
        };
    }
}
