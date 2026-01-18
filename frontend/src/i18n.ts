import i18next from 'i18next';
import Backend from 'i18next-http-backend';

// Central i18next initializer used by all frontend scripts
const SUPPORTED_LANGUAGES = ['en-US', 'en-UK', 'fr-FR', 'de-DE', 'es-ES', 'pt-PT'];
const DEFAULT_LANGUAGE = 'en-US';

const normalizeLanguage = (value?: string | null) => {
  if (!value) return null;
  const trimmed = value.trim();
  if (!trimmed) return null;
  const match = SUPPORTED_LANGUAGES.find(
    (lang) => lang.toLowerCase() === trimmed.toLowerCase()
  );
  return match || null;
};

const resolvePreferredLanguage = () => {
  try {
    const stored = normalizeLanguage(localStorage.getItem('preferredLanguage'));
    if (stored) return stored;
  } catch (error) {
    // ignore storage access errors
  }

  if (typeof window !== 'undefined') {
    const winLang = normalizeLanguage((window as any).__preferredLanguage);
    if (winLang) return winLang;
  }

  if (typeof navigator !== 'undefined') {
    const browserLang = normalizeLanguage(navigator.language);
    if (browserLang) return browserLang;
    const shortLang = normalizeLanguage((navigator.language || '').split('-')[0]);
    if (shortLang) return shortLang;
  }

  return DEFAULT_LANGUAGE;
};

const detectedLanguage = resolvePreferredLanguage();

try {
  if (typeof document !== 'undefined') {
    document.documentElement.lang = detectedLanguage;
  }
  if (typeof window !== 'undefined') {
    (window as any).__locale = detectedLanguage;
  }
} catch (error) {
  // ignore document access errors
}

i18next
  .use(Backend)
  .init({
    lng: detectedLanguage,
    fallbackLng: DEFAULT_LANGUAGE,
    debug: false,
    backend: {
      loadPath: '/locales/{{lng}}.json'
    },
  // Treat dotted keys as literal keys (we are using flat keys like "overview.welcome")
    keySeparator: false,
    nsSeparator: false,
    interpolation: {
      escapeValue: false
    }
  });

export default i18next;
