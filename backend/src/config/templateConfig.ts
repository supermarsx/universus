import nunjucks from 'nunjucks';
import express from 'express';
import path from 'path';

/**
 * @module backend/config/templateConfig
 *
 * Helpers to configure the Nunjucks templating engine for Express. The main
 * exported function `configureTemplateEngine` wires up view paths, enables
 * sensible defaults for development mode, registers a set of common filters
 * (formatNumber, formatDate, formatTime, timeRemaining, abbreviate), and
 * exposes a few global variables used by server-rendered templates.
 *
 * The filters are intentionally simple and synchronous to keep rendering
 * deterministic and fast. If you need locale-specific formatting or complex
 * behaviors, consider registering custom filter implementations elsewhere
 * and composing them into the environment.
 */

/**
 * Configure and return a Nunjucks environment attached to the given Express
 * application.
 *
 * This function does the following:
 * - Sets the views path to `frontend/views` relative to the project root.
 * - Configures autoescape and caching behavior depending on NODE_ENV.
 * - Registers utility template filters: `formatNumber`, `formatDate`,
 *   `formatTime`, `timeRemaining`, and `abbreviate`.
 * - Adds globals: `APP_NAME`, `APP_VERSION`, and `currentYear`.
 *
 * @param {express.Application} app - The Express application instance to bind to.
 * @returns {nunjucks.Environment} A configured Nunjucks environment instance.
 *
 * @example
 * ```ts
 * import express from 'express';
 * import { configureTemplateEngine } from './config/templateConfig';
 * const app = express();
 * configureTemplateEngine(app);
 * ```
 *
 * @remarks
 * - In development (`NODE_ENV === 'development'`) template watching and
 *   no-cache are enabled for rapid iteration. In production these are
 *   disabled to improve performance.
 */
export function configureTemplateEngine(app: express.Application): nunjucks.Environment {
  const viewsPath = path.join(__dirname, '../../frontend/views');
  
   // Configure Nunjucks
   const env = nunjucks.configure(viewsPath, {
     autoescape: true,
     express: app,
     watch: process.env.NODE_ENV === 'development',
     noCache: process.env.NODE_ENV === 'development',
   });

  // --- i18n translation filter ---
  const fs = require('fs');
  let translations: Record<string, string> = {};
  if (process.env.NODE_ENV !== 'test' && process.env.SKIP_SERVER_START !== 'true') {
    try {
      const localesDir = path.join(__dirname, '../../frontend/locales');
      const defaultLocale = process.env.DEFAULT_LOCALE || 'en-US';
      const preferredPath = path.join(localesDir, `${defaultLocale}.json`);
      const fallbackPath = path.join(localesDir, 'en-US.json');

      let localePath = '';
      if (fs.existsSync(preferredPath)) {
        localePath = preferredPath;
      } else if (fs.existsSync(fallbackPath)) {
        localePath = fallbackPath;
      } else {
        const firstLocaleFile = (fs.readdirSync(localesDir) as string[]).find((f) => f.endsWith('.json'));
        if (firstLocaleFile) {
          localePath = path.join(localesDir, firstLocaleFile);
        }
      }

      if (localePath) {
        translations = JSON.parse(fs.readFileSync(localePath, 'utf8'));
      }
    } catch (e) {
      if (process.env.NODE_ENV !== 'test') {
        console.error('Failed to load translations:', e);
      }
    }
  }
  env.addFilter('t', (key: string) => {
    return translations[key] || key;
  });


  /**
   * formatNumber filter
   *
   * Format a number using the runtime locale's thousands separator. This is
   * useful for displaying large integers in templates (e.g. resource counts).
   *
   * @param {number} num - The number to format
   * @returns {string} Localized number string
   */
  env.addFilter('formatNumber', (num: number) => {
    return num.toLocaleString();
  });

  /**
   * formatDate filter
   *
   * Render a Date (or ISO date string) as a localized date string suitable
   * for user-facing templates.
   *
   * @param {Date|string} date - Date or ISO date string
   * @returns {string} Localized date string
   */
  env.addFilter('formatDate', (date: Date | string) => {
    const d = typeof date === 'string' ? new Date(date) : date;
    return d.toLocaleDateString();
  });

  /**
   * formatTime filter
   *
   * Render a Date (or ISO date string) as a localized time string.
   *
   * @param {Date|string} date - Date or ISO date string
   * @returns {string} Localized time string
   */
  env.addFilter('formatTime', (date: Date | string) => {
    const d = typeof date === 'string' ? new Date(date) : date;
    return d.toLocaleTimeString();
  });

  /**
   * timeRemaining filter
   *
   * Returns a human-readable remaining time string between now and the
   * provided end time. If the end time is in the past or equal to now,
   * returns the string 'Complete'.
   */
  env.addFilter('timeRemaining', (endTime: Date | string) => {
    const end = typeof endTime === 'string' ? new Date(endTime) : endTime;
    const now = new Date();
    const diff = end.getTime() - now.getTime();
    
    if (diff <= 0) return 'Complete';
    
    const hours = Math.floor(diff / 3600000);
    const minutes = Math.floor((diff % 3600000) / 60000);
    const seconds = Math.floor((diff % 60000) / 1000);
    
    return `${hours}h ${minutes}m ${seconds}s`;
  });

  /**
   * abbreviate filter
   *
   * Abbreviate large numbers for compact display in UIs. Uses 'M' for
   * millions and 'K' for thousands and falls back to the plain string for
   * smaller values.
   *
   * @param {number} num - The number to abbreviate
   * @returns {string} Abbreviated string (e.g. '1.2M', '3.4K', '12')
   */
  env.addFilter('abbreviate', (num: number) => {
    if (num >= 1000000) return `${(num / 1000000).toFixed(1)}M`;
    if (num >= 1000) return `${(num / 1000).toFixed(1)}K`;
    return num.toString();
  });

  /**
   * Template global variables
   *
   * - APP_NAME: Application display name
   * - APP_VERSION: Current app version (from env or default)
   * - currentYear: Current calendar year (useful in footers)
   */
  // Add global variables
  env.addGlobal('APP_NAME', 'Universus');
  env.addGlobal('APP_VERSION', process.env.APP_VERSION || '1.0.0');
  env.addGlobal('currentYear', new Date().getFullYear());

  return env;
}
