import { pool } from '../config/database';
import { canonicalizeSmsChannel, normalizeChannelList } from '../constants/smsChannels';

export interface SmsServiceConfig {
    id: number;
    service_url: string;
    api_key?: string | null;
    default_channel: string;
    fallback_channels: string[];
    updated_by?: number | null;
    updated_at: Date;
}

export interface UpdateSmsServiceConfigRequest {
    service_url?: string;
    api_key?: string | null;
    default_channel?: string;
    fallback_channels?: string[];
}

export class SmsServiceConfigService {
    private static cache: SmsServiceConfig | null = null;
    private static cacheExpiry = 0;
    private static readonly CACHE_TTL = 60 * 1000;

    private static mapRow(row: any): SmsServiceConfig {
        return {
            id: row.id,
            service_url: row.service_url,
            api_key: row.api_key,
            default_channel: row.default_channel,
            fallback_channels: row.fallback_channels || [],
            updated_by: row.updated_by,
            updated_at: row.updated_at
        };
    }

    static async getConfig(force = false): Promise<SmsServiceConfig> {
        if (!force && this.cache && Date.now() < this.cacheExpiry) {
            return this.cache;
        }

        const result = await pool.query('SELECT * FROM sms_service_settings ORDER BY id ASC LIMIT 1');

        if (result.rows.length === 0) {
            throw new Error('SMS service settings not found');
        }

        const config = this.mapRow(result.rows[0]);
        this.cache = config;
        this.cacheExpiry = Date.now() + this.CACHE_TTL;
        return config;
    }

    static async updateConfig(update: UpdateSmsServiceConfigRequest, userId: number): Promise<SmsServiceConfig> {
        const fields: string[] = [];
        const values: any[] = [];

        if (typeof update.service_url === 'string') {
            values.push(update.service_url.trim());
            fields.push(`service_url = $${values.length}`);
        }

        if (update.api_key !== undefined) {
            values.push(update.api_key ? update.api_key : null);
            fields.push(`api_key = $${values.length}`);
        }

        if (typeof update.default_channel === 'string') {
            const canonical = canonicalizeSmsChannel(update.default_channel.trim());
            values.push(canonical);
            fields.push(`default_channel = $${values.length}`);
        }

        if (Array.isArray(update.fallback_channels)) {
            const normalized = normalizeChannelList(update.fallback_channels);
            values.push(normalized);
            fields.push(`fallback_channels = $${values.length}`);
        }

        values.push(userId);
        fields.push(`updated_by = $${values.length}`);
        fields.push(`updated_at = NOW()`);

        const query = `
            UPDATE sms_service_settings
            SET ${fields.join(', ')}
            WHERE id = (SELECT id FROM sms_service_settings ORDER BY id ASC LIMIT 1)
            RETURNING *
        `;

        const result = await pool.query(query, values);
        if (result.rows.length === 0) {
            throw new Error('Failed to update SMS service configuration');
        }

        const config = this.mapRow(result.rows[0]);
        this.cache = config;
        this.cacheExpiry = Date.now() + this.CACHE_TTL;
        return config;
    }
}
