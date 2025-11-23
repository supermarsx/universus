import { SmsVerificationChannel } from '../types/accountManagement';

export const SUPPORTED_SMS_CHANNELS: SmsVerificationChannel[] = [
    'sms_twilio',
    'sms_http',
    'whatsapp_twilio',
    'whatsapp_baileys',
    'telegram',
    'discord',
    'custom_http'
];

export const LEGACY_SMS_CHANNEL_MAP: Record<string, SmsVerificationChannel> = {
    sms: 'sms_twilio',
    whatsapp: 'whatsapp_twilio',
    custom: 'custom_http'
};

export function canonicalizeSmsChannel(value: string): SmsVerificationChannel {
    const normalized = value.toLowerCase();
    const canonical = (LEGACY_SMS_CHANNEL_MAP[normalized] || normalized) as SmsVerificationChannel;
    if (!SUPPORTED_SMS_CHANNELS.includes(canonical)) {
        throw new Error(`Unsupported SMS verification channel: ${value}`);
    }
    return canonical;
}

export function normalizeChannelList(channels: Array<string | SmsVerificationChannel>): SmsVerificationChannel[] {
    const normalized: SmsVerificationChannel[] = [];
    for (const entry of channels) {
        if (!entry) continue;
        const canonical = canonicalizeSmsChannel(entry.toString());
        if (!normalized.includes(canonical)) {
            normalized.push(canonical);
        }
    }
    return normalized;
}
