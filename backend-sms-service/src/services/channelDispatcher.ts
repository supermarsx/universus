import { SmsProviderFactory } from './smsProvider';
import { SmsDispatchRequest, SmsDispatchResult, SmsVerificationChannel } from '../types';
import { logger } from '../logger';
import { isChannelAvailable, recordChannelFailure, recordChannelSuccess } from './circuitBreaker';

const SUPPORTED_CHANNELS: SmsVerificationChannel[] = [
    'sms_twilio',
    'sms_http',
    'whatsapp_twilio',
    'whatsapp_baileys',
    'telegram',
    'discord',
    'custom_http'
];

const LEGACY_CHANNEL_MAP: Record<string, SmsVerificationChannel> = {
    sms: 'sms_twilio',
    whatsapp: 'whatsapp_twilio',
    custom: 'custom_http'
};

const PHONE_BASED_CHANNELS = new Set<SmsVerificationChannel>([
    'sms_twilio',
    'sms_http',
    'whatsapp_twilio',
    'whatsapp_baileys'
]);

const DEFAULT_CHANNEL = process.env.SMS_DEFAULT_CHANNEL || 'sms_twilio';

const fallbackEntries = (process.env.SMS_FALLBACK_CHANNELS || '')
    .split(',')
    .map((entry) => entry.trim())
    .filter(Boolean);

const DEFAULT_SEQUENCE = buildSequence([DEFAULT_CHANNEL, ...fallbackEntries]);

function canonicalizeChannel(value: string): SmsVerificationChannel {
    const normalized = value.toLowerCase();
    const canonical = (LEGACY_CHANNEL_MAP[normalized] || normalized) as SmsVerificationChannel;
    if (!SUPPORTED_CHANNELS.includes(canonical)) {
        throw new Error(`Unsupported SMS verification channel: ${value}`);
    }
    return canonical;
}

function buildSequence(channels: string[]): SmsVerificationChannel[] {
    const sequence: SmsVerificationChannel[] = [];
    for (const entry of channels) {
        if (!entry) continue;
        const canonical = canonicalizeChannel(entry);
        if (!sequence.includes(canonical)) {
            sequence.push(canonical);
        }
    }
    return sequence.length > 0 ? sequence : [canonicalizeChannel(DEFAULT_CHANNEL)];
}

function normalizePhoneNumber(phone: string): string {
    const sanitized = phone.replace(/[^0-9+]/g, '');
    if (!sanitized) {
        throw new Error('Invalid phone number');
    }
    if (sanitized.startsWith('+')) {
        return sanitized;
    }

    const defaultCountry = process.env.SMS_DEFAULT_COUNTRY_CODE;
    if (!defaultCountry) {
        throw new Error('SMS_DEFAULT_COUNTRY_CODE is required when sending to bare phone numbers');
    }

    const trimmedCode = defaultCountry.replace(/^\+/, '');
    return `+${trimmedCode}${sanitized}`;
}

function normalizeDestination(channel: SmsVerificationChannel, contact: string): string {
    if (!contact || contact.trim().length === 0) {
        throw new Error('Contact value is required');
    }

    if (PHONE_BASED_CHANNELS.has(channel)) {
        return normalizePhoneNumber(contact);
    }

    return contact.trim();
}

export async function sendMessageWithFallback(request: SmsDispatchRequest): Promise<SmsDispatchResult> {
    const sequence = request.channels && request.channels.length > 0
        ? buildSequence(request.channels)
        : DEFAULT_SEQUENCE;

    let lastError: any = null;
    for (const channel of sequence) {
        if (!isChannelAvailable(channel)) {
            logger.warn({ channel }, 'Skipping channel due to open circuit breaker');
            continue;
        }
        const destination = normalizeDestination(channel, request.contact);
        try {
            const provider = SmsProviderFactory.create(channel);
            await provider.sendMessage(destination, request.message);
            recordChannelSuccess(channel);
            return { channel, destination };
        } catch (error: any) {
            const wrappedError = error instanceof Error ? error : new Error(String(error));
            (wrappedError as any).channel = channel;
            (wrappedError as any).destination = destination;
            lastError = wrappedError;
            recordChannelFailure(channel);
            logger.warn(
                { channel, destination, error: wrappedError?.message },
                'Channel failed'
            );
        }
    }

    throw lastError || new Error('All SMS channels failed');
}
