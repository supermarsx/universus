import phalanxService from '../../src/services/phalanxService';
import moonService from '../../src/services/moonService';

jest.mock('../../src/services/moonService');

describe('PhalanxService', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  it('should throw error if moon not found', async () => {
    (moonService.getMoonById as jest.Mock).mockResolvedValue(null);
    await expect(phalanxService.performScan({ userId: 1, moonId: 1, targetGalaxy: 1, targetSystem: 1, targetPosition: 1 })).rejects.toThrow('Moon not found or access denied');
  });

  it('should throw error if no sensor phalanx', async () => {
    (moonService.getMoonById as jest.Mock).mockResolvedValue({ sensor_phalanx: 0 });
    await expect(phalanxService.performScan({ userId: 1, moonId: 1, targetGalaxy: 1, targetSystem: 1, targetPosition: 1 })).rejects.toThrow('Sensor Phalanx required on this moon');
  });

  it('should throw error on cooldown', async () => {
    const now = Date.now();
    (moonService.getMoonById as jest.Mock).mockResolvedValue({
      sensor_phalanx: 1,
      last_scan_time: new Date(now - 2000).toISOString(),
    });
    await expect(phalanxService.performScan({ userId: 1, moonId: 1, targetGalaxy: 1, targetSystem: 1, targetPosition: 1 })).rejects.toThrow('Phalanx scan cooldown active (3 seconds)');
  });

  it('should throw error on daily cap', async () => {
    (moonService.getMoonById as jest.Mock).mockResolvedValue({
      sensor_phalanx: 1,
      last_scan_time: null,
      daily_scan_count: 100,
      last_reset_day: new Date().toISOString().split('T')[0],
    });
    await expect(phalanxService.performScan({ userId: 1, moonId: 1, targetGalaxy: 1, targetSystem: 1, targetPosition: 1 })).rejects.toThrow('Daily phalanx scan limit reached for this moon');
  });

  // For filtering and jitter, need integration tests since it queries DB directly
  // These unit tests cover the basic validation logic
});