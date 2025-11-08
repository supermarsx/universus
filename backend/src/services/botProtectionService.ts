import crypto from 'crypto';
import { Pool } from 'pg';
import { Redis } from 'ioredis';

interface BotChallengeRecord {
    answer: number;
    createdAt: number;
    ip?: string;
    userAgent?: string;
}

interface BotChallengeResponse {
    enabled: boolean;
    token?: string;
    operands?: number[];
    operator?: string;
    expiresIn?: number;
}

export interface BotChallengePayload {
    token?: string;
    response?: number | string;
}

const CHALLENGE_TTL_SECONDS = 120;

export class BotProtectionService {
    private redis: Redis;
    private pool: Pool;
    private enabledCache?: { value: boolean; expires: number };

    constructor(pool: Pool, redis: Redis) {
        this.pool = pool;
        this.redis = redis;
    }

    private async fetchEnabledFlag(): Promise<boolean> {
        let enabled = false;
        try {
            const cached = await this.redis.get('config:gameplay.bot_protection_enabled');
            if (cached !== null) {
                enabled = cached === 'true' || cached === '1';
            } else {
                const result = await this.pool.query(
                    'SELECT current_value FROM config_parameters WHERE parameter_key = $1 LIMIT 1',
                    ['gameplay.bot_protection_enabled']
                );
                if (result.rows.length > 0) {
                    enabled = result.rows[0].current_value === 'true';
                }
            }
        } catch (error) {
            console.error('Failed to load bot protection flag:', error);
        }

        this.enabledCache = {
            value: enabled,
            expires: Date.now() + 30000 // 30s cache
        };

        return enabled;
    }

    async isEnabled(): Promise<boolean> {
        if (this.enabledCache && this.enabledCache.expires > Date.now()) {
            return this.enabledCache.value;
        }

        return this.fetchEnabledFlag();
    }

    async createChallenge(meta: { ip?: string; userAgent?: string }): Promise<BotChallengeResponse> {
        if (!(await this.isEnabled())) {
            return { enabled: false };
        }

        const operands = [this.randomOperand(), this.randomOperand()];
        const answer = operands[0] + operands[1];
        const token = crypto.randomUUID ? crypto.randomUUID() : crypto.randomBytes(16).toString('hex');
        const record: BotChallengeRecord = {
            answer,
            createdAt: Date.now(),
            ip: meta.ip,
            userAgent: meta.userAgent
        };

        const key = this.getChallengeKey(token);
        await this.redis.set(key, JSON.stringify(record), 'EX', CHALLENGE_TTL_SECONDS);

        return {
            enabled: true,
            token,
            operands,
            operator: '+',
            expiresIn: CHALLENGE_TTL_SECONDS
        };
    }

    async validateChallenge(payload?: BotChallengePayload): Promise<boolean> {
        if (!(await this.isEnabled())) {
            return true;
        }

        if (!payload?.token || typeof payload.response === 'undefined' || payload.response === null) {
            return false;
        }

        const key = this.getChallengeKey(payload.token);
        const stored = await this.redis.get(key);

        if (!stored) {
            return false;
        }

        const record: BotChallengeRecord = JSON.parse(stored);
        const numericResponse = typeof payload.response === 'number'
            ? payload.response
            : parseInt(payload.response, 10);

        if (Number.isNaN(numericResponse) || numericResponse !== record.answer) {
            return false;
        }

        await this.redis.del(key);
        return true;
    }

    private getChallengeKey(token: string): string {
        return `bot_challenge:${token}`;
    }

    private randomOperand(): number {
        return 10 + Math.floor(Math.random() * 40); // 10-49 to keep sums reasonable
    }
}

// Singleton instance reused across routes
import { pool } from '../config/database';
import { redis } from '../config/redis';

export const botProtectionService = new BotProtectionService(pool, redis);
