import fs from 'fs';
import path from 'path';

export function getAvailableLocales(): string[] {
  const localesDir = path.join(__dirname, '../../../frontend/locales');
  return fs.readdirSync(localesDir)
    .filter(f => f.endsWith('.json'))
    .map(f => f.replace('.json', ''));
}
