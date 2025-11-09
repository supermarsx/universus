import { ThemeService } from '../../src/services/themeService';

import { __setDefaultQueryResponse } from '../setup/dbMock';

const mockQuery = jest.fn();

jest.mock('../../src/config/database', () => ({
  __esModule: true,
  default: { query: jest.fn() },
  pool: { query: jest.fn() },
}));

const dbModule = require('../../src/config/database');
dbModule.default.query = mockQuery;
dbModule.pool.query = mockQuery;

// Provide a default empty response so tests don't need to set it every time.
__setDefaultQueryResponse({ rows: [], rowCount: 0 });


describe('ThemeService custom CSS handling', () => {
  beforeEach(() => {
    mockQuery.mockReset();
  });

  it('stores sanitized custom CSS', async () => {
    const sanitized = 'body.user-theme-scope .resource-bar { color: #fff; }';
    mockQuery.mockResolvedValue({
      rows: [{ custom_css: sanitized, custom_css_updated_at: new Date('2024-01-01') }],
    });

    const result = await ThemeService.updateUserCustomCSS(123, '.resource-bar { color: #fff; }  ');

    expect(mockQuery).toHaveBeenCalledTimes(1);
    const sqlArgs = mockQuery.mock.calls[0][1];
    expect(sqlArgs[0]).toBe(123);
    expect(sqlArgs[1]).toBe(sanitized);
    expect(result.custom_css).toBe(sanitized);
  });

  it('clears CSS when empty string provided', async () => {
    mockQuery.mockResolvedValue({
      rows: [{ custom_css: null, custom_css_updated_at: null }],
    });

    const result = await ThemeService.updateUserCustomCSS(55, '   ');

    expect(mockQuery).toHaveBeenCalledTimes(1);
    expect(mockQuery.mock.calls[0][1][1]).toBeNull();
    expect(result.custom_css).toBeNull();
    expect(result.custom_css_updated_at).toBeNull();
  });

  it('rejects CSS with disallowed tokens', async () => {
    await expect(ThemeService.updateUserCustomCSS(9, '@import url(https://malicious)')).rejects.toThrow(
      /disallowed/i
    );
    expect(mockQuery).not.toHaveBeenCalled();
  });
});
