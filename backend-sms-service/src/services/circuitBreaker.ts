import { logger } from '../logger';

interface ChannelState {
    failures: number;
    openUntil: number;
}

const states: Record<string, ChannelState> = {};
const FAILURE_THRESHOLD = parseInt(process.env.SMS_CHANNEL_FAILURE_THRESHOLD || '3', 10);
const COOLDOWN_MS = parseInt(process.env.SMS_CHANNEL_COOLDOWN_MS || '60000', 10);

export function isChannelAvailable(channel: string): boolean {
    const state = states[channel];
    if (!state) return true;
    if (state.openUntil === 0) return true;
    if (Date.now() >= state.openUntil) {
        states[channel] = { failures: 0, openUntil: 0 };
        logger.info({ channel }, 'Circuit breaker reset for channel');
        return true;
    }
    return false;
}

export function recordChannelSuccess(channel: string): void {
    states[channel] = { failures: 0, openUntil: 0 };
}

export function recordChannelFailure(channel: string): void {
    const state = states[channel] || { failures: 0, openUntil: 0 };
    state.failures += 1;
    if (state.failures >= FAILURE_THRESHOLD) {
        state.openUntil = Date.now() + COOLDOWN_MS;
        state.failures = 0;
        logger.warn({ channel, cooldownMs: COOLDOWN_MS }, 'Circuit breaker open for channel');
    }
    states[channel] = state;
}
