import { randomUUID } from 'crypto';
import { Request, Response, NextFunction } from 'express';
import { logger } from '../logger';

export const requestContext = (req: Request, res: Response, next: NextFunction) => {
    const requestId = randomUUID();
    req.requestId = requestId;
    res.setHeader('x-request-id', requestId);

    const startedAt = Date.now();
    logger.info({ requestId, method: req.method, url: req.url }, 'request:start');

    res.on('finish', () => {
        logger.info(
            {
                requestId,
                statusCode: res.statusCode,
                durationMs: Date.now() - startedAt
            },
            'request:complete'
        );
    });

    next();
};
