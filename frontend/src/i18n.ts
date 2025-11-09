import i18next from 'i18next';
import Backend from 'i18next-http-backend';

// Central i18next initializer used by all frontend scripts
i18next
  .use(Backend)
  .init({
    lng: 'en-US',
    fallbackLng: 'en-US',
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
