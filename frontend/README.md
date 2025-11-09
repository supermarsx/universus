# Frontend Assets & Templates

This directory hosts all frontend resources for Universus. Static assets live in the assets/, css/, and js/ folders, while server-rendered Nunjucks templates reside under frontend/views/. Update tooling to reference frontend/views/pages instead of the retired standalone views directory.

## Accessibility & Internationalization Standards

- Accessibility target: [WCAG 2.1 AA](https://www.w3.org/WAI/standards-guidelines/wcag/)
- Initial supported locale: en (English)
- Plan for future: Add more locales (es, fr, de, etc.)

## Accessibility Testing & CI Integration

Automated accessibility tests are set up using [jest-axe](https://github.com/nickcolley/jest-axe) and run with Jest. Tests live in `/frontend/__tests__/`.

- To run locally: `pnpm test` (from the `/frontend` directory)
- These tests are automatically run in CI on every push and pull request via the GitHub Actions workflow.
- The test script is configured for CI best practices (`jest --ci --runInBand`).
- Add more tests for rendered Nunjucks templates or components as needed.

See `/frontend/__tests__/a11y-basic.test.ts` for a sample.
