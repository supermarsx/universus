const mockQuery = jest.fn();

jest.mock('../../src/config/database', () => ({
  pool: {
    connect: jest.fn(async () => ({
      query: mockQuery,
      release: jest.fn(),
    })),
    query: mockQuery,
  },
}));

jest.mock('../../src/services/moonService', () => ({
  __esModule: true,
  default: {
    getMoonById: jest.fn(),
  },
}));

import destroyMoonService from '../../src/services/destroyMoonService';
import moonService from '../../src/services/moonService';

describe('DestroyMoonService', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    mockQuery.mockResolvedValue({ rows: [] });
  });

  it('should error if moon not found (legacy direct resolution path)', async () => {
    (moonService.getMoonById as jest.Mock).mockResolvedValue(null);
    const result = await destroyMoonService.attemptDestruction(1, 999, 5);
    expect(result.error).toBe('Moon not found');
  });

  it('should schedule moon destruction attack', async () => {
    mockQuery
      .mockResolvedValueOnce(undefined) // BEGIN
      .mockResolvedValueOnce({
        rows: [{ id: 1, user_id: 7, deathstar: 20, galaxy: 1, system: 10, position: 5 }],
      }) // source
      .mockResolvedValueOnce({
        rows: [{ id: 99, user_id: 9, diameter: 5000, galaxy: 1, system: 20, position: 8 }],
      }) // target
      .mockResolvedValueOnce(undefined) // deduct source DS
      .mockResolvedValueOnce({
        rows: [{ id: 123, scheduled_for: new Date().toISOString() }],
      }) // insert rip_attack
      .mockResolvedValueOnce(undefined); // COMMIT

    const result = await destroyMoonService.scheduleDestruction(7, 1, 99, 5, 100);

    expect(result.attackId).toBe(123);
    expect(result.travelSeconds).toBeGreaterThanOrEqual(1800);
    expect(result.chancePreview).toBeGreaterThanOrEqual(0);
  });

  it('should block attacks against own moon', async () => {
    mockQuery
      .mockResolvedValueOnce(undefined) // BEGIN
      .mockResolvedValueOnce({
        rows: [{ id: 1, user_id: 7, deathstar: 20, galaxy: 1, system: 10, position: 5 }],
      }) // source
      .mockResolvedValueOnce({
        rows: [{ id: 99, user_id: 7, diameter: 5000, galaxy: 1, system: 20, position: 8 }],
      }) // target owned by attacker
      .mockResolvedValueOnce(undefined); // ROLLBACK

    await expect(destroyMoonService.scheduleDestruction(7, 1, 99, 5, 100)).rejects.toThrow(
      'Cannot destroy your own moon'
    );
  });
});
