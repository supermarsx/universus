/**
 * @module config/localeUtils
 *
 * Utilities for locating available frontend locale files in containerized
 * environments. Containers are often isolated from the frontend source tree,
 * so this module supports multiple discovery strategies in order of
 * preference:
 *
 * 1. `LOCALES_PATH` (env) - a path mounted into the backend container that
 *    contains frontend locale JSON files (recommended for Docker Compose / k8s).
 * 2. A packaged `backend/src/locales` directory - useful if you copy locale
 *    files into the backend as part of your build step.
 * 3. (Async only) `LOCALES_URL` (env) - an HTTP endpoint exposed by the
 *    frontend that returns a list of available locale codes (used by
 *    `getAvailableLocalesAsync`).
 *
 * Exports:
 * - `getAvailableLocales(): string[]` — synchronous lookup for mounted or
 *   packaged locales. Returns an array of locale codes (e.g. `['en','fr']`) or
 *   `[]` if none found.
 * - `getAvailableLocalesAsync(): Promise<string[]>` — async variant that will
 *   attempt a remote fetch from `LOCALES_URL` if no local results are present.
 *
 * Environment variables:
 * - `LOCALES_PATH` — path to a directory containing `<code>.json` locale files.
 * - `LOCALES_URL` — remote URL returning JSON with locale list (array or
 *   object with `locales` key).
 *
 * Example (sync):
 * ```ts
 * import { getAvailableLocales } from './config/localeUtils';
 * const locales = getAvailableLocales();
 * ```
 *
 * Example (async remote fallback):
 * ```ts
 * import { getAvailableLocalesAsync } from './config/localeUtils';
 * const locales = await getAvailableLocalesAsync();
 * ```
 */
import fs from 'fs';
import path from 'path';
import logger from './logger';

/**
 * Get available frontend locale codes.
 *
 * In containerized deployments the backend container will not have access
 * to the frontend source tree. To support multiple environments this
 * function checks in order:
 * 1. `process.env.LOCALES_PATH` - a path mounted into the container (recommended)
 * 2. a packaged `src/locales` directory inside the backend (if you copy
 *    locales into the backend during your build)
 *
 * If neither location exists the function returns an empty array and logs
 * a warning. This keeps services resilient in CI / container runs.
 *
 * @returns {string[]} Array of locale codes (e.g. ['en', 'fr'])
 */
export function getAvailableLocales(): string[] {
  const envPath = process.env.LOCALES_PATH;

  const candidates: string[] = [];

  if (envPath) candidates.push(envPath);

  // Allow a packaged directory in the backend (copy frontend locales here at build time)
  candidates.push(path.join(__dirname, '..', 'locales'));

  for (const localesDir of candidates) {
    try {
      if (!fs.existsSync(localesDir)) continue;
      const files = fs.readdirSync(localesDir)
        .filter((f) => f.endsWith('.json'))
        .map((f) => f.replace('.json', ''));

      if (files.length > 0) return files;
    } catch (err) {
      // Continue to next candidate; log for diagnostics
      try {
        logger.warn('Could not read locales dir', { dir: localesDir, error: err });
      } catch (e) {
        // ignore logging failures in utility
      }
    }
  }

  // No locales found in any candidate location
  try {
    logger.warn('No locales found: set LOCALES_PATH or copy frontend locales into backend/src/locales');
  } catch (e) {
    // ignore
  }

  return [];
}


/**
 * Async variant that will attempt to fetch available locales from a remote
 * frontend endpoint if no local locales are available. The frontend should
 * expose a simple JSON endpoint (e.g. `/locales/list` or `/api/locales`) that
 * returns either an array of locale codes or an object with a `locales` array.
 *
 * Behavior:
 * 1. Return sync-local results if present (mounted path or packaged locales).
 * 2. If `process.env.LOCALES_URL` is set and `fetch` is available, try fetching it.
 * 3. Return [] if nothing found or on error.
 *
 * @returns {Promise<string[]>} Promise resolving to an array of locale codes.
 */
export async function getAvailableLocalesAsync(): Promise<string[]> {
  const local = getAvailableLocales();
  if (local.length > 0) return local;

  const url = process.env.LOCALES_URL;
  if (!url) return [];

  if (typeof fetch !== 'function') {
    logger.warn('LOCALES_URL is set but `fetch` is not available in this Node runtime. Skipping remote fetch.');
    return [];
  }

  try {
    const res = await fetch(url, { method: 'GET' });
    if (!res.ok) {
      logger.warn('Failed to fetch locales from LOCALES_URL', { url, status: res.status });
      return [];
    }

    const data = await res.json();
    if (Array.isArray(data)) return data;
    if (data && typeof data === 'object') {
      if (Array.isArray((data as any).locales)) return (data as any).locales;
      return Object.keys(data as Record<string, any>);
    }

    return [];
  } catch (err) {
    logger.warn('Error fetching locales from LOCALES_URL', { url, error: err });
    return [];
  }
}
