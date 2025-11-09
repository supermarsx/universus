# Universus Internationalization (i18n) & Accessibility (a11y) Specification

## Overview
This document specifies the requirements and implementation standards for robust internationalization (i18n) and accessibility (a11y) in the Universus frontend. The goal is to ensure the application is usable, understandable, and accessible to users of all languages and abilities, and that these standards are enforced in development and CI.

---

## 1. Internationalization (i18n) Requirements

### 1.1. Language Support
- The application must support multiple languages, starting with English (`en`).
- Additional languages (e.g., Spanish `es`, French `fr`, German `de`) must be easy to add by providing new translation files.
- All user-facing strings, including UI text, error messages, tooltips, and notifications, must be translatable.
- Date, time, number, and currency formatting must be locale-aware.

### 1.2. Technical Implementation
- Use a translation file structure: `/frontend/locales/{lang}.json` (e.g., `en.json`, `es.json`).
- Nunjucks templates must use a translation filter/tag (e.g., `{% trans %}` or `{{ 'key' | t }}`) for all strings.
- Frontend JS must use a translation function (e.g., `t('key')`).
- Language switching must be available in the UI and persist user preference (cookie/localStorage).
- Fallback to English if a translation is missing.
- Translation workflow must be documented for developers and translators.

---

## 2. Accessibility (a11y) Requirements

### 2.1. Standards & Guidelines
- The application must comply with [WCAG 2.1 AA](https://www.w3.org/WAI/standards-guidelines/wcag/) standards.
- All interactive elements must be keyboard accessible (tab order, skip links, focus management).
- All images must have meaningful `alt` text or be marked as decorative.
- Use ARIA roles, labels, and properties where appropriate.
- Ensure sufficient color contrast for all text and UI elements.
- Provide visible focus indicators for all interactive elements.
- All forms must have associated labels and accessible error messages.
- Modals and popups must be accessible (focus trap, ARIA, keyboard close, etc.).
- Dynamic content must use `aria-live` or similar for screen reader updates.

### 2.2. Testing & Enforcement
- Automated accessibility tests (e.g., `jest-axe`, `pa11y`) must run in CI and fail the build on violations.
- Manual accessibility testing must be performed for major UI flows (screen reader, keyboard-only, color blindness tools).
- Accessibility best practices must be documented for developers.

---

## 3. Integration & Advanced Features
- i18n and a11y must be integrated (e.g., translated ARIA labels, alt text).
- The language switcher must be accessible (keyboard, screen reader).
- Support for right-to-left (RTL) languages must be considered.
- Pluralization, gender, and context-sensitive translations must be supported.
- Custom widgets/components must be accessible and translatable.
- Visual regression testing should cover different locales and accessibility states.

---

## 4. Documentation & Maintenance
- All i18n and a11y features and workflows must be documented in the project.
- Onboarding/training must be provided for developers and translators.
- Regular audits must be performed to ensure ongoing compliance.
- Community/user feedback should be solicited for continuous improvement.

---

## 5. CI/CD Integration
- All i18n and a11y tests must run automatically in CI on every push and pull request.
- The build must fail if any accessibility or translation test fails.
- Documentation must be kept up to date with any changes to i18n/a11y workflows.

---

## References
- [WCAG 2.1 AA](https://www.w3.org/WAI/standards-guidelines/wcag/)
- [i18next](https://www.i18next.com/)
- [jest-axe](https://github.com/nickcolley/jest-axe)
- [pa11y](https://pa11y.org/)
- [Nunjucks i18n](https://www.npmjs.com/package/nunjucks-i18n)
