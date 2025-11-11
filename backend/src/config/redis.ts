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
 * For unit tests we avoid creating a real network connection by exporting a
 * lightweight no-op client when running under the test environment. This
 * prevents connection attempts and open handles during Jest runs.
 */

const isTest = process.env.NODE_ENV === 'test' || !!process.env.JEST_WORKER_ID;

let client: Redis;

if (isTest) {
  // Minimal no-op client for tests. Provide a broader surface of async
  // methods used across the codebase so imports that call `subscribe`,
  // `duplicate`, `publish`, etc. will not throw. Methods return harmless
  // resolved promises or sensible defaults. Duplicate returns another
  // no-op client so code calling `redis.duplicate()` continues to operate.
  const noopAsync = async (..._args: any[]) => null;
  const noop = (..._args: any[]) => null;

  const noOpClient: any = {
    get: noopAsync,
    set: async (_k: string, _v: any) => 'OK',
    del: noopAsync,
    // Pub/Sub
    subscribe: async (_channel: string | string[]) => 0,
    psubscribe: async (_pattern: string | string[]) => 0,
    unsubscribe: async (_channel?: string) => 0,
    punsubscribe: async (_pattern?: string) => 0,
    publish: async (_channel: string, _message: string) => 0,
    // Key helpers
    keys: async (_pattern: string) => [],
    setex: async (_key: string, _ttl: number, _val: any) => 'OK',
    exists: async (_key: string) => 0,
    expire: async (_key: string, _ttl: number) => 0,
    // Client utility
    duplicate: () => noOpClient,
    on: noop,
    off: noop,
    once: noop,
    quit: async () => null,
    disconnect: () => null,
  };

  client = noOpClient as Redis;
} else {
  client = new Redis({
    host: process.env.REDIS_HOST || 'localhost',
    port: parseInt(process.env.REDIS_PORT || '6379'),
    retryStrategy: (times) => Math.min(times * 50, 2000),
  });

  client.on('connect', () => {
    console.log('Connected to Redis');
  });

  client.on('error', (err) => {
    console.error('Redis error:', err);
  });
}

export const redisClient = client;
export const redis = client;
export default client;

