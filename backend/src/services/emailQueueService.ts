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

    constructor(queueKey: string) {
        this.queueKey = queueKey;
    }

    async enqueue(job: EmailJobPayload): Promise<void> {
        const payload = {
            ...job,
            created_at: new Date().toISOString()
        };

        await redis.rpush(this.queueKey, JSON.stringify(payload));
    }
}

export const emailQueueService = new EmailQueueService(EMAIL_QUEUE_KEY);
