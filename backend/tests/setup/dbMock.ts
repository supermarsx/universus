// Test DB mock initializer
// When RUN_INTEGRATION is not true, replace the real PG pool with a lightweight mock
// to avoid requiring a real database during CI unit tests.

interface QueryResponse {
  rows?: any[];
  rowCount?: number;
  command?: string;
}

if (process.env.RUN_INTEGRATION !== 'true') {
  // Provide a minimal mock for the `pool` API used across the codebase.
  // Tests should import services and, for DB interactions, either stub `pool.query`
  // using Jest mocks or rely on higher-level mocks. This file ensures imports
  // don't throw when modules require the `pool` object at module load time.
  const noop = async () => ({ rows: [], rowCount: 0, command: '' });

  // Create a mock pool object whose `query` is a Jest mock we can configure per-test.
  const mockPool = {
    query: jest.fn(noop),
    connect: jest.fn(async () => ({
      query: jest.fn(noop),
      release: jest.fn(),
    })),
    end: jest.fn(),
  } as unknown as any;

  // Expose a small helper for tests to set default query responses.
  // Usage in a test:
  //   import { __setDefaultQueryResponse, __getMockPool } from 'tests/setup/dbMock';
  //   __setDefaultQueryResponse({ rows: [{ id: 1 }], rowCount: 1 });
  const __setDefaultQueryResponse = (resp: QueryResponse) => {
    mockPool.query.mockImplementation(async () => ({ rows: resp.rows ?? [], rowCount: resp.rowCount ?? 0, command: resp.command ?? '' }));
    // Also set the connect().query mock
    mockPool.connect.mockImplementation(async () => ({
      query: async () => ({ rows: resp.rows ?? [], rowCount: resp.rowCount ?? 0, command: resp.command ?? '' }),
      release: jest.fn(),
    }));
  };

  const __getMockPool = () => mockPool;

  // Replace the runtime module for '../../src/config/database' where many services import `pool` from.
  jest.mock('../../src/config/database', () => ({
    pool: mockPool,
    default: mockPool,
    __setDefaultQueryResponse,
    __getMockPool,
  }));

  // Also export the helpers from this module so tests can import them directly.
  // Note: Jest treats setup files as modules; we attach to `module.exports`.
  // eslint-disable-next-line @typescript-eslint/no-var-requires
  const mod = require as any;
  mod.exports.__setDefaultQueryResponse = __setDefaultQueryResponse;
  mod.exports.__getMockPool = __getMockPool;
}
