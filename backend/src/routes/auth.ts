/**
 * @module backend/routes/auth
 *
 * Authentication and account-related routes (login, register, bot-challenges,
 * session handling). This module integrates bot protection, throttling and
 * analytics instrumentation.
 */

import express, { Request, Response } from 'express';
import { AuthService } from '../services/authService';
import { botProtectionService } from '../services/botProtectionService';
import { authThrottleService } from '../services/authThrottleService';
import { EmailVerificationService } from '../services/emailVerificationService';
import { pool } from '../config/database';
import { analyticsService } from '../services/analyticsService';
import { SmsVerificationService } from '../services/smsVerificationService';

const router = express.Router();
const enforcePhoneVerification = process.env.REQUIRE_PHONE_VALIDATION === 'true';

const getClientIp = (req: Request): string => {
  const forwarded = req.headers['x-forwarded-for'];
  if (typeof forwarded === 'string' && forwarded.length > 0) {
    return forwarded.split(',')[0].trim();
  }
  return req.ip || req.socket.remoteAddress || 'unknown';
};

const getSessionId = (req: Request): string | undefined => {
  const bodySession =
    typeof (req.body?.sessionId) === 'string' ? req.body.sessionId : undefined;
  if (bodySession) return bodySession;
  const header = req.headers['x-session-id'];
  if (typeof header === 'string') return header;
  if (Array.isArray(header)) return header[0];
  return undefined;
};

const recordAnalyticsEvent = (
  req: Request,
  eventType: string,
  userId?: number,
  properties?: Record<string, any>
) => {
  analyticsService.trackEvent({
    eventType,
    userId,
    sessionId: getSessionId(req),
    properties,
    userAgent: req.get('user-agent') || undefined,
    ipAddress: req.ip
  }).catch((error) => {
    console.warn(`Analytics event ${eventType} failed`, error);
  });
};

router.get('/bot-challenge', async (req: Request, res: Response) => {
  try {
    const force = req.query.force === '1' || req.query.force === 'true';
    const challenge = await botProtectionService.createChallenge(
      {
        ip: req.ip,
        userAgent: req.get('user-agent') || undefined,
      },
      { force }
    );
    res.json(challenge);
  } catch (error) {
    console.error('Bot challenge generation failed:', error);
    res.status(500).json({ error: 'Unable to create bot challenge' });
  }
});

router.post('/register', async (req: Request, res: Response) => {
  const clientIp = getClientIp(req);

  try {
    const { username, email, password, bot_challenge, phone_number, sms_channel } = req.body;

    if (!username || !email || !password) {
      return res.status(400).json({ error: 'Missing required fields' });
    }

    const normalizedEmail = email.trim().toLowerCase();

    const attempt = await authThrottleService.registerAttempt(clientIp);
    if (!attempt.allowed) {
      return res.status(429).json({
        error: 'Too many registration attempts. Please wait and try again.',
      });
    }

    const challengeValid = await botProtectionService.validateChallenge(bot_challenge, {
      force: attempt.requiresCaptcha,
    });

    if (!challengeValid) {
      await authThrottleService.recordFailure(clientIp);
      return res.status(400).json({ error: 'Bot verification failed', code: 'captcha_required' });
    }

    const result = await AuthService.register(username, normalizedEmail, password);
    await authThrottleService.recordSuccess(clientIp);

    let verificationSent = false;
    try {
      await EmailVerificationService.sendVerificationEmail(
        result.user.id,
        normalizedEmail,
        req.ip,
        req.get('user-agent') || undefined
      );
      verificationSent = true;
    } catch (verificationError) {
      console.error('Failed to send verification email:', verificationError);
    }

    const smsVerificationEnabled = SmsVerificationService.isEnabled();
    let smsVerificationRequired = false;
    let smsVerificationSent = false;

    if (smsVerificationEnabled && phone_number) {
      smsVerificationRequired = true;
      try {
        await SmsVerificationService.sendVerificationCode({
          userId: result.user.id,
          phoneNumber: phone_number,
          channel: sms_channel,
          ipAddress: req.ip,
          userAgent: req.get('user-agent') || undefined
        });
        smsVerificationSent = true;
      } catch (smsError) {
        console.error('Failed to send SMS verification:', smsError);
      }
    }

    res.status(201).json({
      success: true,
      message: 'Account created. Please verify your email before logging in.',
      email_verification: {
        required: true,
        sent: verificationSent
      },
      sms_verification: {
        enabled: smsVerificationEnabled,
        required: smsVerificationRequired,
        sent: smsVerificationSent
      }
    });

    recordAnalyticsEvent(req, 'registration_completed', result.user.id, {
      method: 'email'
    });
  } catch (error: any) {
    console.error('Registration error:', error);
    try {
      await authThrottleService.recordFailure(clientIp);
    } catch (throttleError) {
      console.error('Failed to record registration failure:', throttleError);
    }
    recordAnalyticsEvent(req, 'registration_failed', undefined, {
      reason: error?.code || error?.message
    });
    res.status(400).json({ error: error.message });
  }
});

router.post('/login', async (req: Request, res: Response) => {
  const clientIp = getClientIp(req);

  try {
    const { username, email, password, bot_challenge } = req.body;
    const identifier = username || email;

    if (!identifier || !password) {
      return res.status(400).json({ error: 'Missing required fields' });
    }

    const attempt = await authThrottleService.registerAttempt(clientIp);
    if (!attempt.allowed) {
      return res.status(429).json({
        error: 'Too many login attempts. Please wait and try again.',
      });
    }

    const challengeValid = await botProtectionService.validateChallenge(bot_challenge, {
      force: attempt.requiresCaptcha,
    });

    if (!challengeValid) {
      await authThrottleService.recordFailure(clientIp);
      return res.status(400).json({ error: 'Bot verification failed', code: 'captcha_required' });
    }

    const result = await AuthService.login(identifier, password);

    if (!result.user.email_verified) {
      const status = await EmailVerificationService.checkVerificationStatus(result.user.id, result.user.email);
      recordAnalyticsEvent(req, 'login_blocked_unverified', result.user.id, {
        method: 'password'
      });
      return res.status(403).json({
        error: 'Email not verified. Please verify your email to continue.',
        code: 'email_not_verified',
        pending_verification: status.pending_verification,
        can_resend: status.can_resend,
        email: result.user.email
      });
    }

    if (
      enforcePhoneVerification &&
      SmsVerificationService.isEnabled() &&
      result.user.phone_number &&
      !result.user.phone_verified
    ) {
      recordAnalyticsEvent(req, 'login_blocked_phone', result.user.id, {
        method: 'password'
      });
      return res.status(403).json({
        error: 'Phone number not verified. Please complete SMS verification.',
        code: 'phone_not_verified',
        phone_number: result.user.phone_number
      });
    }

    await authThrottleService.recordSuccess(clientIp);
    res.status(200).json(result);
    recordAnalyticsEvent(req, 'login_success', result.user.id, {
      method: 'password'
    });
  } catch (error: any) {
    console.error('Login error:', error);
    try {
      await authThrottleService.recordFailure(clientIp);
    } catch (throttleError) {
      console.error('Failed to record login failure:', throttleError);
    }
    recordAnalyticsEvent(req, 'login_failed', undefined, {
      reason: error?.message
    });
    res.status(401).json({ error: error.message });
  }
});

router.post('/resend-verification', async (req: Request, res: Response) => {
  try {
    const { email, username, bot_challenge } = req.body;
    const lookupEmail = email ? email.trim().toLowerCase() : undefined;

    if (!lookupEmail && !username) {
      return res.status(400).json({ error: 'Email is required' });
    }

    const challengeValid = await botProtectionService.validateChallenge(bot_challenge, { force: true });
    if (!challengeValid) {
      return res.status(400).json({ error: 'Bot verification failed', code: 'captcha_required' });
    }

    let userResult;
    if (lookupEmail) {
      userResult = await pool.query(
        'SELECT id, email, email_verified FROM users WHERE email = $1 LIMIT 1',
        [lookupEmail]
      );
    } else if (username) {
      userResult = await pool.query(
        'SELECT id, email, email_verified FROM users WHERE username = $1 LIMIT 1',
        [username]
      );
    } else {
      // Defensive guard: earlier we validate that either email or username exists,
      // but TypeScript may not narrow `userResult` statically. Return a 400 here
      // to ensure `userResult` is always defined below.
      return res.status(400).json({ error: 'Email or username is required' });
    }

    if (userResult.rows.length === 0) {
      return res.json({ success: true, message: 'If the account exists, a verification email will be sent.' });
    }

    const user = userResult.rows[0];
    if (user.email_verified) {
      return res.json({ success: true, message: 'Email is already verified.' });
    }

    await EmailVerificationService.resendVerification(
      user.id,
      user.email,
      req.ip,
      req.get('user-agent') || undefined
    );

    res.json({ success: true, message: 'Verification email sent.' });
  } catch (error: any) {
    console.error('Resend verification error:', error);
    res.status(400).json({ error: error.message });
  }
});

export default router;
