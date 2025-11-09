import jumpGateService from '../../src/services/jumpGateService';
import moonService from '../../src/services/moonService';
import fleetService from '../../src/services/fleetService';

jest.mock('../../src/services/moonService');
jest.mock('../../src/services/fleetService');

describe('JumpGateService', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  it('should allow jump if cooldown expired', async () => {
    (moonService.getMoonById as jest.Mock).mockResolvedValue({ jump_gate: 1, last_jump_time: null });
    expect(await jumpGateService.canJump(1)).toBe(true);
  });

  it('should not allow jump if cooldown not expired', async () => {
    const now = Date.now();
    (moonService.getMoonById as jest.Mock).mockResolvedValue({ jump_gate: 1, last_jump_time: new Date(now - 1000 * 60 * 30) });
    expect(await jumpGateService.canJump(1)).toBe(false);
  });

  it('should fail if no jump gate', async () => {
    (moonService.getMoonById as jest.Mock).mockResolvedValue({ jump_gate: 0 });
    expect(await jumpGateService.canJump(1)).toBe(false);
  });

  it('should transfer fleets and set cooldown', async () => {
    (moonService.getMoonById as jest.Mock)
      .mockResolvedValueOnce({ id: 1, user_id: 2, jump_gate: 1, last_jump_time: null })
      .mockResolvedValueOnce({ id: 2, user_id: 2, jump_gate: 1, last_jump_time: null });
    (jumpGateService.canJump as jest.Mock).mockResolvedValue(true);
    (fleetService.moveFleetToMoon as jest.Mock).mockResolvedValue(true);
    const result = await jumpGateService.jumpFleet(2, 1, 2, [101, 102]);
    expect(result.success).toBe(true);
  });
});
