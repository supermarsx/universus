/**
 * @module backend/services/emailQueueService
 *
 * Lightweight queue abstraction for sending email tasks. Jobs are pushed to
 * Redis for worker consumption. This module exposes an `EmailQueueService`
 * instance which provides a small API to enqueue email payloads.
 */

import { redis } from '../config/redis';

export interface EmailJobPayload {
    to: string;
    subject: string;
    html: string;
    text?: string;
    from?: string;
    metadata?: Record<string, any>;
    template?: string;
    context?: Record<string, any>;
}

const EMAIL_QUEUE_KEY = process.env.EMAIL_QUEUE_KEY || 'email:queue';

class EmailQueueService {
    private queueKey: string;
    /**
     * Create a new EmailQueueService instance.
     *
     * @param queueKey - Redis list key used for the email queue.
     */
    constructor(queueKey: string) {
        this.queueKey = queueKey;
    }

    /**
     * Enqueue an email job payload onto the Redis-backed queue for worker consumption.
     * The job payload will be JSON-serialized and appended to the configured list.
     *
     * @param job - EmailJobPayload describing recipient, subject, body and optional template/context.
     */
    async enqueue(job: EmailJobPayload): Promise<void> {
        const payload = {
            ...job,
            created_at: new Date().toISOString()
        };

        await redis.rpush(this.queueKey, JSON.stringify(payload));
    }
}

export const emailQueueService = new EmailQueueService(EMAIL_QUEUE_KEY);
