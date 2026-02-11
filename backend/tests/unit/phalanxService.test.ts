const mockQuery = jest.fn();
const mockRelease = jest.fn();

jest.mock('../../src/config/database', () => ({
  pool: {
    connect: jest.fn(async () => ({
      query: mockQuery,
      release: mockRelease,
    })),
  },
}));

jest.mock('../../src/services/moonService', () => ({
  __esModule: true,
  default: {
    getMoonById: jest.fn(),
    deductResources: jest.fn(),
  },
}));

import phalanxService from '../../src/services/phalanxService';
import moonService from '../../src/services/moonService';

describe('PhalanxService', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    mockQuery.mockResolvedValue({ rows: [] });
  });

  it('should throw error if moon not found', async () => {
    (moonService.getMoonById as jest.Mock).mockResolvedValue(null);

    await expect(
      phalanxService.performScan({
        userId: 1,
        moonId: 1,
        targetGalaxy: 1,
        targetSystem: 1,
        targetPosition: 1,
      })
    ).rejects.toThrow('Moon not found or access denied');
  });

  it('should throw error if no sensor phalanx', async () => {
    (moonService.getMoonById as jest.Mock).mockResolvedValue({
      id: 1,
      user_id: 1,
      sensor_phalanx: 0,
      last_reset_day: new Date().toISOString().split('T')[0],
      daily_scan_count: 0,
      deuterium: 100000,
    });

    await expect(
      phalanxService.performScan({
        userId: 1,
        moonId: 1,
        targetGalaxy: 1,
        targetSystem: 1,
        targetPosition: 1,
      })
    ).rejects.toThrow('Sensor Phalanx required on this moon');
  });

  it('should throw error on cooldown', async () => {
    const now = Date.now();
    (moonService.getMoonById as jest.Mock).mockResolvedValue({
      id: 1,
      user_id: 1,
      sensor_phalanx: 1,
      last_scan_time: new Date(now - 2000).toISOString(),
      last_reset_day: new Date().toISOString().split('T')[0],
      daily_scan_count: 1,
      deuterium: 100000,
    });

    await expect(
      phalanxService.performScan({
        userId: 1,
        moonId: 1,
        targetGalaxy: 1,
        targetSystem: 1,
        targetPosition: 1,
      })
    ).rejects.toThrow('Phalanx scan cooldown active (3 seconds)');
  });

  it('should throw error on daily cap', async () => {
    (moonService.getMoonById as jest.Mock).mockResolvedValue({
      id: 1,
      user_id: 1,
      sensor_phalanx: 1,
      last_scan_time: null,
      daily_scan_count: 100,
      last_reset_day: new Date().toISOString().split('T')[0],
      deuterium: 100000,
    });

    await expect(
      phalanxService.performScan({
        userId: 1,
        moonId: 1,
        targetGalaxy: 1,
        targetSystem: 1,
        targetPosition: 1,
      })
    ).rejects.toThrow('Daily phalanx scan limit reached for this moon');
  });
});
