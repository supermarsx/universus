import twilio, { Twilio } from 'twilio';
import fetch from 'node-fetch';
import path from 'path';
import { promises as fs } from 'fs';
import makeWASocket, {
    DisconnectReason,
    useMultiFileAuthState,
    WASocket
} from '@whiskeysockets/baileys';
import pino from 'pino';
import { SmsVerificationChannel } from '../types';

export interface SmsDeliveryProvider {
    sendMessage(to: string, message: string): Promise<void>;
}

class TwilioSmsProvider implements SmsDeliveryProvider {
    constructor(private readonly client: Twilio) {}

    async sendMessage(to: string, message: string): Promise<void> {
        const from = process.env.TWILIO_SMS_FROM;
        if (!from) {
            throw new Error('TWILIO_SMS_FROM is not configured');
        }

        await this.client.messages.create({
            body: message,
            to,
            from
        });
    }
}

class TwilioWhatsAppProvider implements SmsDeliveryProvider {
    constructor(private readonly client: Twilio) {}

    async sendMessage(to: string, message: string): Promise<void> {
        const from = process.env.TWILIO_WHATSAPP_FROM;
        if (!from) {
            throw new Error('TWILIO_WHATSAPP_FROM is not configured');
        }

        const formattedFrom = from.startsWith('whatsapp:') ? from : `whatsapp:${from}`;
        const formattedTo = to.startsWith('whatsapp:') ? to : `whatsapp:${to}`;

        await this.client.messages.create({
            body: message,
            to: formattedTo,
            from: formattedFrom
        });
    }
}

class CustomHttpProvider implements SmsDeliveryProvider {
    async sendMessage(to: string, message: string): Promise<void> {
        const endpoint = process.env.SMS_CUSTOM_API_URL;
        if (!endpoint) {
            throw new Error('SMS_CUSTOM_API_URL is not configured');
        }

        const method = (process.env.SMS_CUSTOM_API_METHOD || 'POST').toUpperCase();
        const headers: Record<string, string> = {
            'Content-Type': 'application/json'
        };

        const apiKey = process.env.SMS_CUSTOM_API_KEY;
        if (apiKey) {
            const headerName = process.env.SMS_CUSTOM_API_HEADER || 'Authorization';
            const prefix = process.env.SMS_CUSTOM_API_PREFIX || 'Bearer';
            headers[headerName] = prefix ? `${prefix} ${apiKey}`.trim() : apiKey;
        }

        const response = await fetch(endpoint, {
            method,
            headers,
            body: JSON.stringify({
                to,
                message,
                transport: 'sms'
            })
        });

        if (!response.ok) {
            const text = await response.text();
            throw new Error(`Custom SMS API responded with ${response.status}: ${text}`);
        }
    }
}

class TelegramBotProvider implements SmsDeliveryProvider {
    async sendMessage(to: string, message: string): Promise<void> {
        const token = process.env.TELEGRAM_BOT_TOKEN;
        if (!token) {
            throw new Error('TELEGRAM_BOT_TOKEN is not configured');
        }

        const chatId = (to && to.trim().length > 0)
            ? to.trim()
            : process.env.TELEGRAM_DEFAULT_CHAT_ID;

        if (!chatId) {
            throw new Error('Telegram chat id is required (pass via contact or TELEGRAM_DEFAULT_CHAT_ID)');
        }

        const response = await fetch(`https://api.telegram.org/bot${token}/sendMessage`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json'
            },
            body: JSON.stringify({
                chat_id: chatId,
                text: message
            })
        });

        if (!response.ok) {
            const text = await response.text();
            throw new Error(`Telegram API responded with ${response.status}: ${text}`);
        }
    }
}

class DiscordBotProvider implements SmsDeliveryProvider {
    private static readonly API_BASE = 'https://discord.com/api/v10';

    async sendMessage(to: string, message: string): Promise<void> {
        const token = process.env.DISCORD_BOT_TOKEN;
        if (!token) {
            throw new Error('DISCORD_BOT_TOKEN is not configured');
        }

        const targetUser = (to && to.trim().length > 0)
            ? to.trim()
            : process.env.DISCORD_DEFAULT_USER_ID;

        if (!targetUser) {
            throw new Error('Discord user id is required (pass via contact or DISCORD_DEFAULT_USER_ID)');
        }

        const dmResponse = await fetch(`${DiscordBotProvider.API_BASE}/users/@me/channels`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                Authorization: `Bot ${token}`
            },
            body: JSON.stringify({
                recipient_id: targetUser
            })
        });

        if (!dmResponse.ok) {
            const text = await dmResponse.text();
            throw new Error(`Discord DM creation failed (${dmResponse.status}): ${text}`);
        }

        const channel = await dmResponse.json();
        const messageResp = await fetch(`${DiscordBotProvider.API_BASE}/channels/${channel.id}/messages`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                Authorization: `Bot ${token}`
            },
            body: JSON.stringify({ content: message })
        });

        if (!messageResp.ok) {
            const text = await messageResp.text();
            throw new Error(`Discord message failed (${messageResp.status}): ${text}`);
        }
    }
}

class BaileysWhatsAppProvider implements SmsDeliveryProvider {
    private static socketPromise: Promise<WASocket> | null = null;

    private static async createSocket(): Promise<WASocket> {
        const authFolder = path.resolve(process.env.BAILEYS_AUTH_FOLDER || '.baileys_auth');
        await fs.mkdir(authFolder, { recursive: true });

        const { state, saveCreds } = await useMultiFileAuthState(authFolder);
        const sock = makeWASocket({
            auth: state,
            logger: pino({ level: process.env.BAILEYS_LOG_LEVEL || 'error' }),
            printQRInTerminal: process.env.BAILEYS_PRINT_QR === 'true'
        });

        sock.ev.on('creds.update', saveCreds);
        sock.ev.on('connection.update', (update) => {
            const { connection, lastDisconnect } = update;
            if (connection === 'close') {
                const statusCode = (lastDisconnect?.error as any)?.output?.statusCode;
                if (statusCode !== DisconnectReason.loggedOut) {
                    BaileysWhatsAppProvider.socketPromise = BaileysWhatsAppProvider.createSocket();
                    console.warn('Baileys connection lost. Attempting to reconnect...');
                } else {
                    BaileysWhatsAppProvider.socketPromise = null;
                    console.error('Baileys logged out. Please re-scan the QR code.');
                }
            }
        });

        return sock;
    }

    private static async getSocket(): Promise<WASocket> {
        if (!this.socketPromise) {
            this.socketPromise = this.createSocket().catch((error) => {
                this.socketPromise = null;
                throw error;
            });
        }
        return this.socketPromise;
    }

    private static formatJid(phone: string): string {
        if (phone.includes('@s.whatsapp.net')) {
            return phone;
        }

        const digits = phone.replace(/[^0-9]/g, '');
        if (!digits) {
            throw new Error('WhatsApp destination must include a phone number');
        }

        return `${digits}@s.whatsapp.net`;
    }

    async sendMessage(to: string, message: string): Promise<void> {
        const socket = await BaileysWhatsAppProvider.getSocket();
        const jid = BaileysWhatsAppProvider.formatJid(to);
        await socket.sendMessage(jid, { text: message });
    }
}

const providerCache = new Map<string, SmsDeliveryProvider>();

export class SmsProviderFactory {
    private static twilioClient: Twilio | null = null;

    private static getTwilioClient(): Twilio {
        if (!this.twilioClient) {
            const accountSid = process.env.TWILIO_ACCOUNT_SID;
            const authToken = process.env.TWILIO_AUTH_TOKEN;

            if (!accountSid || !authToken) {
                throw new Error('Twilio credentials are not configured');
            }

            this.twilioClient = twilio(accountSid, authToken);
        }

        return this.twilioClient;
    }

    private static buildProvider(channel: SmsVerificationChannel): SmsDeliveryProvider {
        switch (channel) {
            case 'sms':
            case 'sms_twilio':
                return new TwilioSmsProvider(this.getTwilioClient());
            case 'whatsapp':
            case 'whatsapp_twilio':
                return new TwilioWhatsAppProvider(this.getTwilioClient());
            case 'sms_http':
            case 'custom':
            case 'custom_http':
                return new CustomHttpProvider();
            case 'telegram':
                return new TelegramBotProvider();
            case 'discord':
                return new DiscordBotProvider();
            case 'whatsapp_baileys':
                return new BaileysWhatsAppProvider();
            default:
                throw new Error(`Unsupported SMS verification channel: ${channel}`);
        }
    }

    static create(channel: SmsVerificationChannel): SmsDeliveryProvider {
        if (!providerCache.has(channel)) {
            providerCache.set(channel, this.buildProvider(channel));
        }
        return providerCache.get(channel)!;
    }
}
