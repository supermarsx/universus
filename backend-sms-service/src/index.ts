import express, { Request, Response } from 'express';
import dotenv from 'dotenv';
import { sendMessageWithFallback } from './services/channelDispatcher';
import { SmsDispatchRequest } from './types';
import { requestContext } from './middleware/requestContext';
import { logger } from './logger';
import { metricsService } from './services/metricsService';
import { recordHistory, getHistoryStats, getRecentHistory, findHistoryByIdempotency } from './services/historyStore';
import { notifyFailure } from './services/failureNotifier';
import { assertWithinRateLimit } from './services/rateLimiter';

dotenv.config();

const app = express();
app.use(express.json());
app.use(requestContext);

const PORT = process.env.PORT || 4700;

const requireApiKey = Boolean(process.env.SMS_SERVICE_API_KEY);

const authMiddleware = (req: Request, res: Response, next: () => void) => {
    if (!requireApiKey) return next();
    const headerKey = req.headers['x-api-key'];
    if (!headerKey || headerKey !== process.env.SMS_SERVICE_API_KEY) {
        return res.status(401).json({ success: false, error: 'Unauthorized' });
    }
    return next();
};

app.get('/health', (_req, res) => {
    res.json({ status: 'ok', service: 'sms' });
});

app.get('/metrics', authMiddleware, (_req, res) => {
    res.json({
        success: true,
        metrics: metricsService.snapshot(),
        history: getHistoryStats()
    });
});

app.get('/history', authMiddleware, (req, res) => {
    const limit = Math.min(parseInt(req.query.limit as string, 10) || 50, 200);
    res.json({
        success: true,
        entries: getRecentHistory(limit)
    });
});

app.post('/api/send', authMiddleware, async (req: Request, res: Response) => {
    const payload = req.body as SmsDispatchRequest;

    if (!payload?.contact || !payload.message) {
        return res.status(400).json({ success: false, error: 'contact and message are required' });
    }

    const idempotencyHeader = typeof req.headers['idempotency-key'] === 'string' ? req.headers['idempotency-key'] : undefined;
    const idempotencyKey = (payload.idempotencyKey || idempotencyHeader || '').trim() || undefined;
    if (idempotencyKey && idempotencyKey.length > 128) {
        return res.status(400).json({ success: false, error: 'Idempotency key too long' });
    }

    if (idempotencyKey) {
        const existing = findHistoryByIdempotency(idempotencyKey);
        if (existing && existing.status === 'success') {
            logger.info({ idempotencyKey, requestId: req.requestId }, 'Returning cached idempotent response');
            return res.json({
                success: true,
                channel: existing.channel,
                destination: existing.destination,
                idempotent: true
            });
        }
    }

    try {
        assertWithinRateLimit(payload.contact);
    } catch (error: any) {
        return res.status(429).json({ success: false, error: error?.message || 'Rate limit exceeded' });
    }

    metricsService.recordRequest();
    const requestId = req.requestId || 'unknown';
    const startedAt = Date.now();

    const channels = Array.isArray(payload.channels) ? payload.channels : undefined;

    try {
        const result = await sendMessageWithFallback({
            contact: payload.contact,
            message: payload.message,
            channels,
            metadata: payload.metadata
        });

        metricsService.recordSuccess(result.channel, Date.now() - startedAt);
        recordHistory({
            requestId,
            idempotencyKey,
            contact: payload.contact,
            destination: result.destination,
            channel: result.channel,
            status: 'success',
            metadata: payload.metadata
        });

        logger.info(
            { requestId, channel: result.channel, destination: result.destination },
            'sms dispatch success'
        );

        return res.json({
            success: true,
            channel: result.channel,
            destination: result.destination
        });
    } catch (error: any) {
        const channel = error?.channel || (channels && channels[0]) || 'unknown';
        const destination = error?.destination || payload.contact;

        metricsService.recordFailure(channel);
        recordHistory({
            requestId,
            idempotencyKey,
            contact: payload.contact,
            destination,
            channel,
            status: 'failed',
            error: error?.message,
            metadata: payload.metadata
        });

        await notifyFailure({
            requestId,
            contact: payload.contact,
            message: payload.message,
            channels: channels || [],
            error: error?.message || 'SMS dispatch failed',
            idempotencyKey
        });

        logger.error({ requestId, err: error }, 'sms dispatch failed');

        return res.status(500).json({
            success: false,
            error: error?.message || 'Failed to send verification message'
        });
    }
});

app.listen(PORT, () => {
    logger.info({ port: PORT }, '[sms-service] listening');
});
