import { countRecentForContact } from './historyStore';

const WINDOW_SECONDS = parseInt(process.env.SMS_RATE_LIMIT_WINDOW_SECONDS || '300', 10);
const MAX_PER_CONTACT = parseInt(process.env.SMS_RATE_LIMIT_MAX_PER_CONTACT || '5', 10);

export function assertWithinRateLimit(contact: string): void {
    if (!MAX_PER_CONTACT || MAX_PER_CONTACT <= 0) return;
    const count = countRecentForContact(contact, WINDOW_SECONDS);
    if (count >= MAX_PER_CONTACT) {
        throw new Error('Rate limit exceeded for this contact');
    }
}
