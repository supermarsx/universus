import { Redis } from 'ioredis';
import { redis } from '../config/redis';
import { gameConfig } from './gameConfigAdapter';

interface ThrottleResult {
    allowed: boolean;
    requiresCaptcha: boolean;
    attemptsRemaining: number;
}

const ATTEMPT_KEY_PREFIX = 'auth:attempts';
const FAILURE_KEY_PREFIX = 'auth:failures';

class AuthThrottleService {
    private redis: Redis;

    constructor(redisClient: Redis) {
        this.redis = redisClient;
    }

    async registerAttempt(ip: string): Promise<ThrottleResult> {
        const [windowSeconds, maxAttempts, captchaThreshold] = await Promise.all([
            gameConfig.getAuthRateLimitWindowSeconds(),
            gameConfig.getAuthRateLimitMaxAttempts(),
            gameConfig.getAuthCaptchaFailureThreshold()
        ]);

        if (maxAttempts <= 0) {
            return {
                allowed: true,
                requiresCaptcha: await this.requiresCaptcha(ip, captchaThreshold),
                attemptsRemaining: Number.MAX_SAFE_INTEGER
            };
        }

        const attemptKey = this.getAttemptKey(ip);
        const current = await this.redis.incr(attemptKey);
        if (current === 1) {
            await this.redis.expire(attemptKey, windowSeconds);
        }

        const allowed = current <= maxAttempts;
        const attemptsRemaining = Math.max(0, maxAttempts - current);
        const requiresCaptcha = await this.requiresCaptcha(ip, captchaThreshold);

        return { allowed, requiresCaptcha, attemptsRemaining };
    }

    async recordFailure(ip: string): Promise<void> {
        const failureKey = this.getFailureKey(ip);
        const windowSeconds = await gameConfig.getAuthRateLimitWindowSeconds();
        const current = await this.redis.incr(failureKey);
        if (current === 1) {
            await this.redis.expire(failureKey, windowSeconds);
        }
    }

    async recordSuccess(ip: string): Promise<void> {
        await Promise.all([
            this.redis.del(this.getFailureKey(ip)),
            this.redis.del(this.getAttemptKey(ip)),
        ]);
    }

    private async requiresCaptcha(ip: string, threshold: number): Promise<boolean> {
        if (threshold <= 0) return false;
        const failureCount = await this.redis.get(this.getFailureKey(ip));
        const count = failureCount ? parseInt(failureCount, 10) : 0;
        return count >= threshold;
    }

    private getAttemptKey(ip: string): string {
        return `${ATTEMPT_KEY_PREFIX}:${ip}`;
    }

    private getFailureKey(ip: string): string {
        return `${FAILURE_KEY_PREFIX}:${ip}`;
    }
}

export const authThrottleService = new AuthThrottleService(redis);
