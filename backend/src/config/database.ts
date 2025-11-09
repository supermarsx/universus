import { Pool } from 'pg';
import dotenv from 'dotenv';

dotenv.config();

/**
 * Database module
 *
 * @module backend/config/database
 *
 * Provides a shared PostgreSQL connection pool and a small helper wrapper for
 * executing queries. Configuration is read from environment variables with
 * sensible defaults to make local development simple.
 *
 * Environment variables used (defaults shown):
 * - DB_HOST: 'localhost'
 * - DB_PORT: '5432'
 * - DB_NAME: 'universus_rpg'
 * - DB_USER: 'postgres'
 * - DB_PASSWORD: 'postgres'
 *
 * @remarks
 * This module exposes a single Pool instance to encourage reuse and avoid
 * connection proliferation. Errors emitted by the pool are logged and the
 * process currently exits on unexpected idle-client errors to avoid running
 * in a degraded state; adjust this behavior for your deployment if required.
 *
 * @example
 * ```ts
 * import { query, pool } from './config/database';
 * const res = await query('SELECT * FROM users WHERE id = $1', [id]);
 * console.log(res.rows);
 * ```
 */

/**
 * Shared PostgreSQL connection pool
 *
 * The pool is configured to support moderate concurrency and sensible timeouts
 * for web applications. Consumers should reuse this exported `pool` rather than
 * creating new Pool instances to avoid resource exhaustion.
 *
 * Configuration details (defaults shown):
 * - host: process.env.DB_HOST || 'localhost'
 * - port: parseInt(process.env.DB_PORT || '5432')
 * - database: process.env.DB_NAME || 'universus_rpg'
 * - user: process.env.DB_USER || 'postgres'
 * - password: process.env.DB_PASSWORD || 'postgres'
 * - max: 20 (maximum number of clients in the pool)
 * - idleTimeoutMillis: 30000 (how long a client is allowed to remain idle)
 * - connectionTimeoutMillis: 2000 (how long to wait when connecting a new client)
 *
 * @constant {Pool}
 */
export const pool = new Pool({
  host: process.env.DB_HOST || 'localhost',
  port: parseInt(process.env.DB_PORT || '5432'),
  database: process.env.DB_NAME || 'universus_rpg',
  user: process.env.DB_USER || 'postgres',
  password: process.env.DB_PASSWORD || 'postgres',
  max: 20,
  idleTimeoutMillis: 30000,
  connectionTimeoutMillis: 2000,
});

/**
 * Default export maintained for backwards compatibility
 *
 * Some older modules may import the default export; keep it available while
 * encouraging named imports (`pool`, `query`) in new code.
 *
 * @deprecated Prefer named `pool` import: `import { pool } from './config/database'`.
 */
export default pool;

/**
 * Pool connect event
 *
 * Emits when a new client connection is established. This log is useful for
 * diagnosing connection lifecycle during development and in environments
 * where the pool frequently creates new clients.
 */
pool.on('connect', () => {
  console.log('Connected to PostgreSQL database');
});

/**
 * Pool error handler
 *
 * Handles unexpected errors emitted by idle clients. In production you may
 * prefer to integrate with a monitoring/alerting system instead of exiting
 * the process immediately. The current behavior exits with a non-zero code
 * to ensure the process doesn't continue in a degraded state.
 *
 * @param {Error} err - The error emitted by the pool/client
 */
pool.on('error', (err: Error) => {
  console.error('Unexpected error on idle client', err);
  process.exit(-1);
});

/**
 * query helper
 *
 * Convenience wrapper around `pool.query` that logs execution duration and
 * returns the underlying result. This centralizes query timing/logging so
 * call sites don't need to perform repetitive instrumentation.
 *
 * @param {string} text - SQL query text with optional parameter placeholders ($1, $2, ...)
 * @param {any[]} [params] - Optional array of parameter values for the query
 * @returns {Promise<import('pg').QueryResult>} Resolves with the PG query result object
 *
 * @example
 *   const res = await query('SELECT * FROM users WHERE id = $1', [id]);
 *   console.log(res.rows);
 *
 * Notes:
 * - This helper rethrows any error from `pool.query`, so callers should
 *   handle/rethrow as appropriate. The timing/logging occurs only for
 *   successful queries in the current implementation.
 */
/**
 * query helper
 *
 * Convenience wrapper around `pool.query` that logs execution duration and
 * returns the underlying result. This centralizes query timing/logging so
 * call sites don't need to perform repetitive instrumentation.
 *
 * @param {string} text - SQL query text with optional parameter placeholders ($1, $2, ...)
 * @param {any[]} [params] - Optional array of parameter values for the query
 * @returns {Promise<import('pg').QueryResult>} Resolves with the PG query result object
 *
 * @throws {Error} Rethrows errors produced by `pool.query` (e.g., connection or SQL errors).
 *
 * @example
 * ```ts
 * const res = await query('SELECT * FROM users WHERE id = $1', [id]);
 * console.log(res.rows);
 * ```
 *
 * @remarks
 * The helper logs only successful query durations. If you need guaranteed
 * logging for failures, wrap calls and handle errors explicitly.
 */
export const query = async (text: string, params?: any[]) => {
  const start = Date.now();
  const res = await pool.query(text, params);
  const duration = Date.now() - start;
  console.log('Executed query', { text, duration, rows: res.rowCount });
  return res;
};
