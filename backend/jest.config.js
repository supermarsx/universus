module.exports = (() => {
  // Integration tests that require a real database are placed under `tests/integration`.
  // By default (CI without DB), we skip integration tests. Set `RUN_INTEGRATION=true`
  // to include them (e.g., in deployment pipelines or when a test DB is available).
  const includeIntegration = process.env.RUN_INTEGRATION === 'true';

  return {
    preset: 'ts-jest',
    testEnvironment: 'node',
    roots: ['<rootDir>/src', '<rootDir>/tests'],
    testMatch: ['**/__tests__/**/*.ts', '**/?(*.)+(spec|test).ts'],
    transform: {
      '^.+\\.ts$': 'ts-jest',
    },
    collectCoverageFrom: [
      'src/**/*.ts',
      '!src/**/*.d.ts',
      '!src/index.ts',
    ],
    coverageDirectory: 'coverage',
    coverageReporters: ['text', 'lcov', 'html'],
    coverageThreshold: {
      global: {
        branches: 70,
        functions: 70,
        lines: 70,
        statements: 70,
      },
    },
    moduleFileExtensions: ['ts', 'js', 'json'],
    verbose: true,
    testTimeout: 10000,
    setupFiles: ['<rootDir>/tests/setup-jest-env.ts'],
    // When integration tests are not included, ignore the integration test folder
    testPathIgnorePatterns: includeIntegration ? [] : ['/tests/integration/'],
  };
})();

