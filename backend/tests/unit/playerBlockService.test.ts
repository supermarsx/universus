import playerBlockService from '../../src/services/playerBlockService';

const mockQuery = jest.fn();

jest.mock('../../src/config/database', () => ({
  __esModule: true,
  pool: { query: jest.fn() },
  default: { query: jest.fn() },
}));

const dbModule = require('../../src/config/database');
dbModule.pool.query = mockQuery;
dbModule.default.query = mockQuery;

describe('PlayerBlockService', () => {
  beforeEach(() => {
    mockQuery.mockReset();
  });

  it('blocks a player with upsert semantics', async () => {
    const row = { id: 1, user_id: 1, blocked_user_id: 2, block_scope: 'all' };
    mockQuery.mockResolvedValue({ rows: [row] });

    const result = await playerBlockService.blockUser(1, 2, 'chat', 'spamming');

    expect(mockQuery).toHaveBeenCalledWith(
      expect.stringContaining('INSERT INTO player_blocks'),
      [1, 2, 'chat', 'spamming', null]
    );
    expect(result).toEqual(row);
  });

  it('detects bilateral blocks', async () => {
    mockQuery
      .mockResolvedValueOnce({ rows: [{ id: 1 }] })
      .mockResolvedValueOnce({ rows: [] });

    const blocked = await playerBlockService.isBlockedEither(1, 2, 'messages');
    expect(blocked).toBe(true);
  });

  it('unblocks a player by scope', async () => {
    mockQuery.mockResolvedValue({ rowCount: 1 });
    const removed = await playerBlockService.unblockUser(1, 2, 'chat');
    expect(removed).toBe(true);
    expect(mockQuery).toHaveBeenCalledWith(
      expect.stringContaining('DELETE FROM player_blocks'),
      [1, 2, 'chat']
    );
  });
});
