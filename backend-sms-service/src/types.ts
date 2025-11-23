export type SmsVerificationChannel =
    | 'sms'
    | 'sms_twilio'
    | 'sms_http'
    | 'whatsapp'
    | 'whatsapp_twilio'
    | 'whatsapp_baileys'
    | 'telegram'
    | 'discord'
    | 'custom'
    | 'custom_http';

export interface SmsDispatchRequest {
    contact: string;
    message: string;
    channels?: string[];
    metadata?: Record<string, any>;
    idempotencyKey?: string;
}

export interface SmsDispatchResult {
    channel: SmsVerificationChannel;
    destination: string;
}
