import destroyMoonService from '../../src/services/destroyMoonService';
import moonService from '../../src/services/moonService';

jest.mock('../../src/services/moonService');

describe('DestroyMoonService', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  it('should error if moon not found', async () => {
    (moonService.getMoonById as jest.Mock).mockResolvedValue(null);
    const result = await destroyMoonService.attemptDestruction(1, 999, 5);
    expect(result.error).toBe('Moon not found');
  });

  it('should error if no Deathstars', async () => {
    (moonService.getMoonById as jest.Mock).mockResolvedValue({ diameter: 5000 });
    const result = await destroyMoonService.attemptDestruction(1, 1, 0);
    expect(result.error).toBe('No Deathstars sent');
  });

  it('should return destroyed true or false', async () => {
    (moonService.getMoonById as jest.Mock).mockResolvedValue({ diameter: 5000 });
    const result = await destroyMoonService.attemptDestruction(1, 1, 10);
    expect(typeof result.destroyed).toBe('boolean');
    expect(typeof result.deathstarsLost).toBe('number');
    expect(typeof result.chance).toBe('number');
    expect(typeof result.lossChance).toBe('number');
  });
});
