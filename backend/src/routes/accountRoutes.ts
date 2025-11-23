// Phase 9: Account Management Routes
// REST API endpoints for all account management features

import express, { Request, Response } from 'express';
import { AuthRequest } from '../types';
import { authenticateToken } from '../middleware/auth';
import { AccountSecurityService } from '../services/accountSecurityService';
import { SessionManagementService } from '../services/sessionManagementService';
import { EmailVerificationService } from '../services/emailVerificationService';
import { PasswordRecoveryService } from '../services/passwordRecoveryService';
import { TwoFactorAuthService } from '../services/twoFactorAuthService';
import { GDPRComplianceService } from '../services/gdprComplianceService';
import { AccountTransferService } from '../services/accountTransferService';
import { SmsVerificationService } from '../services/smsVerificationService';
import { getUserId } from '../utils/authHelpers';
import {
    SuspensionReason,
    GDPRRequestType,
    BlockType,
    ActivityType,
    TwoFactorMethod,
    SmsVerificationChannel
} from '../types/accountManagement';

const router = express.Router();

// =====================================================
// ACCOUNT SECURITY ENDPOINTS
// =====================================================

// Suspend account (admin only)
router.post('/security/suspend', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const { user_id, reason, expires_at, notes } = req.body;
        const adminId = getUserId(req);
        if (adminId === null) return res.status(401).json({ error: 'Unauthorized' });

        const suspension = await AccountSecurityService.suspendAccount(
            user_id,
            reason as SuspensionReason,
            adminId,
            expires_at ? new Date(expires_at) : undefined,
            notes
        );

        res.json({ success: true, suspension });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// Lift suspension (admin only)
router.post('/security/unsuspend', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const { user_id } = req.body;
        const adminId = getUserId(req);
        if (adminId === null) return res.status(401).json({ error: 'Unauthorized' });

        await AccountSecurityService.liftSuspension(user_id, adminId);

        res.json({ success: true, message: 'Suspension lifted successfully' });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// Delete account
router.delete('/security/delete', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ error: 'Unauthorized' });
        const { reason, soft = true } = req.body;

        await AccountSecurityService.deleteAccount(userId, reason, soft);

        res.json({ success: true, message: 'Account deleted successfully' });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// Restore deleted account
router.post('/security/restore', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const { user_id } = req.body;

        await AccountSecurityService.restoreAccount(user_id);

        res.json({ success: true, message: 'Account restored successfully' });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// Lock account
router.post('/security/lock', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const { user_id, reason, duration } = req.body;

        await AccountSecurityService.lockAccount(user_id, reason, duration);

        res.json({ success: true, message: 'Account locked successfully' });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// Unlock account
router.post('/security/unlock', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const { user_id } = req.body;

        await AccountSecurityService.unlockAccount(user_id);

        res.json({ success: true, message: 'Account unlocked successfully' });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// Get security summary
router.get('/security/summary', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ error: 'Unauthorized' });

        const summary = await AccountSecurityService.getSecuritySummary(userId);

        res.json({ success: true, summary });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// Get security logs
router.get('/security/logs', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ error: 'Unauthorized' });
        const limit = parseInt(req.query.limit as string) || 50;
        const offset = parseInt(req.query.offset as string) || 0;

        const result = await AccountSecurityService.getSecurityLogs(userId, limit, offset);

        res.json({ success: true, ...result });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// =====================================================
// SESSION MANAGEMENT ENDPOINTS
// =====================================================

// Get active sessions
router.get('/sessions', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ error: 'Unauthorized' });

        const sessions = await SessionManagementService.getActiveSessions(userId);

        res.json({ success: true, ...sessions });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// Validate session
router.post('/sessions/validate', async (req: Request, res: Response) => {
    try {
        const { session_token } = req.body;

        const session = await SessionManagementService.validateSession(session_token);

        if (session) {
            res.json({ success: true, valid: true, session });
        } else {
            res.json({ success: true, valid: false });
        }
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// Terminate specific session
router.delete('/sessions/:sessionId', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ error: 'Unauthorized' });
        const sessionId = parseInt(req.params.sessionId);

        await SessionManagementService.terminateSession(sessionId, userId);

        res.json({ success: true, message: 'Session terminated successfully' });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// Terminate all sessions
router.delete('/sessions', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ error: 'Unauthorized' });
        const exceptSessionId = req.body.except_session_id;

        const count = await SessionManagementService.terminateAllSessions(userId, exceptSessionId);

        res.json({ success: true, message: `${count} sessions terminated` });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// Get suspicious activities
router.get('/sessions/suspicious', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ error: 'Unauthorized' });
        const limit = parseInt(req.query.limit as string) || 20;

        const activities = await SessionManagementService.getSuspiciousActivities(userId, limit);

        res.json({ success: true, activities });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// Update device trust
router.patch('/sessions/:sessionId/trust', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ error: 'Unauthorized' });
        const sessionId = parseInt(req.params.sessionId);
        const { is_trusted } = req.body;

        await SessionManagementService.updateDeviceTrust(sessionId, userId, is_trusted);

        res.json({ success: true, message: 'Device trust updated' });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// =====================================================
// EMAIL VERIFICATION ENDPOINTS
// =====================================================

// Send verification email
router.post('/email/verify/send', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ error: 'Unauthorized' });
        const { email } = req.body;
        const ipAddress = req.ip;
        const userAgent = req.headers['user-agent'];

        const verification = await EmailVerificationService.sendVerificationEmail(
            userId,
            email,
            ipAddress,
            userAgent
        );

        res.json({ success: true, verification: { id: verification.id, expires_at: verification.expires_at } });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// Verify email with token
router.post('/email/verify', async (req: Request, res: Response) => {
    try {
        const { token } = req.body;
        const ipAddress = req.ip;
        const userAgent = req.headers['user-agent'];

        await EmailVerificationService.verifyEmail({
            verification_token: token,
            ip_address: ipAddress,
            user_agent: userAgent
        });

        res.json({ success: true, message: 'Email verified successfully' });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// Resend verification email
router.post('/email/verify/resend', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ error: 'Unauthorized' });
        const { email } = req.body;
        const ipAddress = req.ip;
        const userAgent = req.headers['user-agent'];

        const verification = await EmailVerificationService.resendVerification(
            userId,
            email,
            ipAddress,
            userAgent
        );

        res.json({ success: true, verification: { id: verification.id, expires_at: verification.expires_at } });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// Check verification status
router.get('/email/verify/status', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ error: 'Unauthorized' });
        const email = req.query.email as string;

        const status = await EmailVerificationService.checkVerificationStatus(userId, email);

        res.json({ success: true, ...status });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// =====================================================
// PHONE / SMS VERIFICATION ENDPOINTS
// =====================================================

router.post('/phone/verify/send', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ error: 'Unauthorized' });
        if (!SmsVerificationService.isEnabled()) {
            return res.status(503).json({ error: 'SMS verification is disabled' });
        }
        const { phone_number, channel } = req.body;

        if (!phone_number) {
            return res.status(400).json({ error: 'Phone number is required' });
        }

        const verification = await SmsVerificationService.sendVerificationCode({
            userId,
            phoneNumber: phone_number,
            channel: channel as SmsVerificationChannel | undefined,
            ipAddress: req.ip,
            userAgent: req.headers['user-agent']
        });

        res.json({
            success: true,
            verification: {
                id: verification.id,
                channel: verification.channel,
                expires_at: verification.expires_at
            }
        });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

router.post('/phone/verify', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ error: 'Unauthorized' });
        if (!SmsVerificationService.isEnabled()) {
            return res.status(503).json({ error: 'SMS verification is disabled' });
        }
        const { code } = req.body;

        if (!code) {
            return res.status(400).json({ error: 'Verification code is required' });
        }

        await SmsVerificationService.verifyCode({
            userId,
            code,
            ipAddress: req.ip,
            userAgent: req.headers['user-agent']
        });

        res.json({ success: true, message: 'Phone verified successfully' });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

router.post('/phone/verify/resend', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ error: 'Unauthorized' });
        if (!SmsVerificationService.isEnabled()) {
            return res.status(503).json({ error: 'SMS verification is disabled' });
        }
        const { phone_number, channel } = req.body;

        if (!phone_number) {
            return res.status(400).json({ error: 'Phone number is required' });
        }

        const verification = await SmsVerificationService.resendVerification({
            userId,
            phoneNumber: phone_number,
            channel: channel as SmsVerificationChannel | undefined,
            ipAddress: req.ip,
            userAgent: req.headers['user-agent']
        });

        res.json({
            success: true,
            verification: {
                id: verification.id,
                channel: verification.channel,
                expires_at: verification.expires_at
            }
        });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

router.get('/phone/verify/status', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ error: 'Unauthorized' });
        const disabled = !SmsVerificationService.isEnabled();

        const status = await SmsVerificationService.checkStatus(userId);

        res.json({ success: true, disabled, ...status });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// =====================================================
// PASSWORD RECOVERY ENDPOINTS
// =====================================================

// Initiate password reset
router.post('/password/reset/initiate', async (req: Request, res: Response) => {
    try {
        const { email } = req.body;
        const ipAddress = req.ip;
        const userAgent = req.headers['user-agent'];

        await PasswordRecoveryService.initiatePasswordReset({
            email,
            ip_address: ipAddress,
            user_agent: userAgent
        });

        res.json({ success: true, message: 'If this email exists, a reset link has been sent' });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// Validate reset token
router.post('/password/reset/validate', async (req: Request, res: Response) => {
    try {
        const { token } = req.body;

        const reset = await PasswordRecoveryService.validateResetToken(token);

        res.json({ success: true, valid: true, email: reset.email });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// Complete password reset
router.post('/password/reset/complete', async (req: Request, res: Response) => {
    try {
        const { token, new_password } = req.body;
        const ipAddress = req.ip;
        const userAgent = req.headers['user-agent'];

        await PasswordRecoveryService.completePasswordReset({
            reset_token: token,
            new_password,
            ip_address: ipAddress,
            user_agent: userAgent
        });

        res.json({ success: true, message: 'Password reset successfully' });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// Cancel password reset
router.post('/password/reset/cancel', async (req: Request, res: Response) => {
    try {
        const { token } = req.body;

        await PasswordRecoveryService.cancelPasswordReset(token);

        res.json({ success: true, message: 'Password reset cancelled' });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// =====================================================
// TWO-FACTOR AUTHENTICATION ENDPOINTS
// =====================================================

// Setup 2FA
router.post('/2fa/setup', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ error: 'Unauthorized' });
        const { method = TwoFactorMethod.TOTP, recovery_email } = req.body;

        const setup = await TwoFactorAuthService.setup2FA({
            user_id: userId,
            method,
            recovery_email
        });

        res.json({ success: true, ...setup });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// Verify 2FA code (to enable 2FA)
router.post('/2fa/verify', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ error: 'Unauthorized' });
        const { code } = req.body;

        const verified = await TwoFactorAuthService.verify2FA({
            user_id: userId,
            code
        });

        if (verified) {
            res.json({ success: true, message: '2FA verified successfully' });
        } else {
            res.status(400).json({ error: 'Invalid verification code' });
        }
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// Disable 2FA
router.post('/2fa/disable', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ error: 'Unauthorized' });
        const { code } = req.body;

        await TwoFactorAuthService.disable2FA(userId, code);

        res.json({ success: true, message: '2FA disabled successfully' });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// Get 2FA status
router.get('/2fa/status', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ error: 'Unauthorized' });

        const status = await TwoFactorAuthService.get2FAStatus(userId);

        res.json({ success: true, ...status });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// Get backup codes
router.get('/2fa/backup-codes', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ error: 'Unauthorized' });

        const codes = await TwoFactorAuthService.getBackupCodes(userId);

        res.json({ success: true, backup_codes: codes });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// Regenerate backup codes
router.post('/2fa/backup-codes/regenerate', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ error: 'Unauthorized' });
        const { code } = req.body;

        const newCodes = await TwoFactorAuthService.regenerateBackupCodes(userId, code);

        res.json({ success: true, backup_codes: newCodes });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// =====================================================
// GDPR COMPLIANCE ENDPOINTS
// =====================================================

// Create GDPR request
router.post('/gdpr/request', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ error: 'Unauthorized' });
        const { request_type, notes } = req.body;

        const gdprRequest = await GDPRComplianceService.createGDPRRequest({
            user_id: userId,
            request_type: request_type as GDPRRequestType,
            notes
        });

        res.json({ success: true, request: gdprRequest });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// Get GDPR requests
router.get('/gdpr/requests', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ error: 'Unauthorized' });
        const limit = parseInt(req.query.limit as string) || 20;
        const offset = parseInt(req.query.offset as string) || 0;

        const result = await GDPRComplianceService.getGDPRRequests(userId, limit, offset);

        res.json({ success: true, ...result });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// Download exported data
router.get('/gdpr/download/:code', async (req: Request, res: Response) => {
    try {
        const { code } = req.params;

        const data = await GDPRComplianceService.downloadExportedData(code);

        res.json({ success: true, data });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// Cancel GDPR request
router.delete('/gdpr/request/:requestId', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ error: 'Unauthorized' });
        const requestId = parseInt(req.params.requestId);

        await GDPRComplianceService.cancelGDPRRequest(requestId, userId);

        res.json({ success: true, message: 'GDPR request cancelled' });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// =====================================================
// ACCOUNT TRANSFER ENDPOINTS
// =====================================================

// Initiate account transfer
router.post('/transfer/initiate', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ error: 'Unauthorized' });
        const { to_email } = req.body;
        const ipAddress = req.ip;
        const userAgent = req.headers['user-agent'];

        const transfer = await AccountTransferService.initiateTransfer({
            user_id: userId,
            to_email,
            ip_address: ipAddress,
            user_agent: userAgent
        });

        res.json({ success: true, transfer: { id: transfer.id, status: transfer.status, expires_at: transfer.expires_at } });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// Verify transfer
router.post('/transfer/verify', async (req: Request, res: Response) => {
    try {
        const { token } = req.body;
        const ipAddress = req.ip;
        const userAgent = req.headers['user-agent'];

        const transfer = await AccountTransferService.verifyTransfer(token, ipAddress, userAgent);

        res.json({ success: true, transfer });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// Complete transfer
router.post('/transfer/complete', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ error: 'Unauthorized' });
        const { transfer_id, confirmation_code } = req.body;

        await AccountTransferService.completeTransfer(transfer_id, userId, confirmation_code);

        res.json({ success: true, message: 'Account transfer completed successfully' });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// Cancel transfer
router.delete('/transfer/:transferId', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ error: 'Unauthorized' });
        const transferId = parseInt(req.params.transferId);

        await AccountTransferService.cancelTransfer(transferId, userId);

        res.json({ success: true, message: 'Transfer cancelled successfully' });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

// Get transfer status
router.get('/transfer/status', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ error: 'Unauthorized' });

        const transfer = await AccountTransferService.getTransferStatus(userId);

        res.json({ success: true, transfer });
    } catch (error: any) {
        res.status(400).json({ error: error.message });
    }
});

export default router;
