/**
 * @module config/logger
 *
 * Shared backend logger. Prefer `winston` if available in the runtime; if
 * not, fall back to the console. Export a minimal logger surface used by the
 * backend (`info`, `warn`, `error`, `debug`).
 */
let logger: {
  info: (...args: any[]) => void;
  warn: (...args: any[]) => void;
  error: (...args: any[]) => void;
  debug: (...args: any[]) => void;
} = console;

try {
  // Dynamically require winston so this package doesn't hard-depend on it.
  // eslint-disable-next-line @typescript-eslint/no-var-requires
  // @ts-ignore
  const winston = require('winston');
  if (winston && typeof winston.createLogger === 'function') {
    const { createLogger, format, transports } = winston;
    const w = createLogger({
      level: process.env.NODE_ENV === 'production' ? 'info' : 'debug',
      format: format.combine(format.timestamp(), format.errors({ stack: true }), format.json()),
      transports: [new transports.Console()],
    });

    logger = {
      info: (...args: any[]) => w.info(...args),
      warn: (...args: any[]) => w.warn(...args),
      error: (...args: any[]) => w.error(...args),
      debug: (...args: any[]) => w.debug(...args),
    };
  }
} catch (e) {
  // winston not available — keep console as fallback
}

export default logger;
