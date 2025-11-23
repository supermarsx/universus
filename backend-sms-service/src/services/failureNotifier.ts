import fetch from 'node-fetch';
import { logger } from '../logger';

const webhookUrl = process.env.SMS_FAILURE_WEBHOOK_URL;

export interface FailurePayload {
    requestId: string;
    contact: string;
    message: string;
    channels: string[];
    error: string;
    idempotencyKey?: string;
}

export async function notifyFailure(payload: FailurePayload): Promise<void> {
    if (!webhookUrl) {
        return;
    }

    try {
        await fetch(webhookUrl, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json'
            },
            body: JSON.stringify({
                ...payload,
                timestamp: new Date().toISOString()
            })
        });
    } catch (error) {
        logger.warn({ error }, 'Failed to send failure webhook');
    }
}
