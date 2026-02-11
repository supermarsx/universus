jest.mock('../../src/services/moonService');
jest.mock('../../src/services/fleetService');

const moonService = require('../../src/services/moonService') as jest.Mocked<any>;
const { FleetService } = require('../../src/services/fleetService') as any;
const jumpGateService = require('../../src/services/jumpGateService').default as any;

const mockedMoonService = moonService as jest.Mocked<typeof moonService>;

describe('JumpGateService', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  it('should allow jump if cooldown expired', async () => {
    mockedMoonService.getMoonById.mockResolvedValue({ jump_gate: 1, last_jump_time: null } as any);
    expect(await jumpGateService.canJump(1)).toBe(true);
  });

  it('should not allow jump if cooldown not expired', async () => {
    const now = Date.now();
    mockedMoonService.getMoonById.mockResolvedValue({ jump_gate: 1, last_jump_time: new Date(now - 1000 * 60 * 30) } as any);
    expect(await jumpGateService.canJump(1)).toBe(false);
  });

  it('should fail if no jump gate', async () => {
    mockedMoonService.getMoonById.mockResolvedValue({ jump_gate: 0 } as any);
    expect(await jumpGateService.canJump(1)).toBe(false);
  });

   it('should transfer fleets and set cooldown', async () => {
     mockedMoonService.getMoonById
       .mockResolvedValueOnce({ id: 1, user_id: 2, jump_gate: 1, last_jump_time: null } as any)
       .mockResolvedValueOnce({ id: 2, user_id: 2, jump_gate: 1, last_jump_time: null } as any);

     jest.spyOn(jumpGateService, 'canJump').mockResolvedValue(true);

     jest.spyOn(FleetService, 'moveFleetToMoon').mockResolvedValue(true);

     const result = await jumpGateService.jumpFleet(2, 1, 2, [101, 102]);
     expect(result.success).toBe(true);
     expect(FleetService.moveFleetToMoon).toHaveBeenCalledWith(2, 1, 101, 2);
     expect(FleetService.moveFleetToMoon).toHaveBeenCalledWith(2, 1, 102, 2);
   });

   it('should fail if moons not owned by same user', async () => {
     mockedMoonService.getMoonById
       .mockResolvedValueOnce({ id: 1, user_id: 2, jump_gate: 1, last_jump_time: null } as any)
       .mockResolvedValueOnce({ id: 2, user_id: 3 } as any);

     const result = await jumpGateService.jumpFleet(2, 1, 2, [101]);
     expect(result.success).toBe(false);
     expect(result.error).toBe('Destination moon not owned by user');
   });

   it('should enforce destination cooldown', async () => {
     mockedMoonService.getMoonById
       .mockResolvedValueOnce({ id: 1, user_id: 2, jump_gate: 1, last_jump_time: null } as any)
       .mockResolvedValueOnce({ id: 2, user_id: 2, jump_gate: 1, last_jump_time: null } as any);

     const canJumpSpy = jest.spyOn(jumpGateService, 'canJump');
     canJumpSpy
       .mockResolvedValueOnce(true)
       .mockResolvedValueOnce(false);

     const result = await jumpGateService.jumpFleet(2, 1, 2, [101]);
     expect(result.success).toBe(false);
     expect(result.error).toBe('Destination Jump Gate is on cooldown');
   });
});

