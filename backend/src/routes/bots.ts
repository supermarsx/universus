/**
 * @module backend/routes/bots
 *
 * Proxy routes for the external bot-service. Requires admin privileges and
 * forwards requests to the configured BOT_SERVICE_URL.
 */

import express, { Router } from 'express';
import { authenticateToken, requireAdmin } from '../middleware/auth';

const router: Router = express.Router();
const BOT_SERVICE_URL = process.env.BOT_SERVICE_URL || 'http://bot-service:4001';

router.use(authenticateToken, requireAdmin);

router.use(async (req, res) => {
  try {
    const targetUrl = `${BOT_SERVICE_URL}${req.originalUrl}`;

    const headers: Record<string, string> = {
      authorization: req.headers.authorization || '',
    };

    if (req.headers['content-type']) {
      headers['content-type'] = req.headers['content-type'] as string;
    }

    const hasBody = !['GET', 'HEAD'].includes(req.method.toUpperCase());
    const body = hasBody ? JSON.stringify(req.body) : undefined;

    const response = await fetch(targetUrl, {
      method: req.method,
      headers,
      body,
    });

    const contentType = response.headers.get('content-type') || 'application/json';
    res.status(response.status);
    res.setHeader('content-type', contentType);

    const buffer = Buffer.from(await response.arrayBuffer());
    res.send(buffer);
  } catch (error) {
    console.error('[Backend] Failed to proxy bot-service request:', error);
    res.status(502).json({
      success: false,
      error: 'Bot service unavailable',
    });
  }
});

export default router;
