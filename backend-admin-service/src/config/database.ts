
/**
 * Database configuration and connection pool setup for PostgreSQL using 'pg'.
 * Loads environment variables from a .env file using dotenv.
 * @module config/database
 */
import { Pool } from 'pg';
import dotenv from 'dotenv';
import logger from './logger';

dotenv.config();


/**
 * PostgreSQL connection pool instance.
 *
 * @type {Pool}
 * @see {@link https://node-postgres.com/api/pool}
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
 * Default export for backwards compatibility.
 * @type {Pool}
 */
export default pool;


/**
 * Event listener for successful PostgreSQL connection.
 * Logs a message when the pool connects to the database.
 */
pool.on('connect', () => {
  logger.info('Connected to PostgreSQL database');
});

/**
 * Event listener for pool errors.
 * Logs unexpected errors and exits the process.
 * @param {Error} err - The error object.
 */
pool.on('error', (err: Error) => {
  logger.error('Unexpected error on idle client', { error: err });
  process.exit(-1);
});


/**
 * Executes a SQL query using the connection pool and logs execution time and row count.
 *
 * @param {string} text - The SQL query text to execute.
 * @param {any[]} [params] - Optional array of query parameters.
 * @returns {Promise<import('pg').QueryResult>} The result of the query.
 */
export const query = async (text: string, params?: any[]) => {
  const start = Date.now();
  const res = await pool.query(text, params);
  const duration = Date.now() - start;
  logger.debug('Executed query', { text, duration, rows: res.rowCount });
  return res;
};
