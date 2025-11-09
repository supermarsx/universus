import Redis from 'ioredis';
import dotenv from 'dotenv';

dotenv.config();

/**
 * @module backend/config/redis
 *
 * Centralized Redis client configuration using `ioredis`.
 *
 * This module exports a single shared Redis connection (the `redis` default
 * export and the `redisClient` named export) configured from environment
 * variables with sensible defaults for local development.
 *
 * Important behavior notes:
 * - A `retryStrategy` is provided to control reconnect backoff. It receives
 *   the number of retry attempts and should return the delay (ms) before the
 *   next attempt or a non-number to stop retrying.
 * - Connection errors are logged; the library handles reconnection according
 *   to the `retryStrategy` and internal policies.
 */

/**
 * Shared Redis client instance.
 *
 * Configuration is read from environment variables with defaults:
 * - REDIS_HOST (default: 'localhost')
 * - REDIS_PORT (default: '6379')
 *
 * The client uses a small linear backoff capped at 2s to avoid tight retry
 * loops while preserving responsiveness during transient outages.
 *
 * @example
 *   import redis, { redisClient } from './config/redis';
 *   await redisClient.set('key', 'value');
 *
 * @constant {Redis}
 */
export const redis = new Redis({
  host: process.env.REDIS_HOST || 'localhost',
  port: parseInt(process.env.REDIS_PORT || '6379'),
  retryStrategy: (times) => {
    // Linear backoff: 50ms per attempt, capped to 2000ms
    const delay = Math.min(times * 50, 2000);
    return delay;
  },
});

/**
 * Log when a connection is successfully established. Useful for local
 * development logs and startup diagnostics.
 */
redis.on('connect', () => {
  console.log('Connected to Redis');
});

/**
 * Global error handler for the Redis client.
 *
 * Note: ioredis will emit `error` for connection and protocol errors. The
 * current behavior logs the error to stderr. In production you may want to
 * forward these to structured logging/monitoring systems or implement
 * additional restart policies.
 */
redis.on('error', (err) => {
  console.error('Redis error:', err);
});

/**
 * Backwards-compatible named export.
 *
 * Some modules import the client under the name `redisClient`. Keep this
 * alias to avoid breaking changes while allowing the default export usage
 * for modern ES module patterns.
 */
export const redisClient = redis;

/**
 * Default export: the shared Redis instance.
 *
 * Prefer named imports for clarity where possible.
 */
export default redis;
