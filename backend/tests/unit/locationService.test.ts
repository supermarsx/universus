import { resolveLocation } from '../../src/services/locationService';
import { PlanetService } from '../../src/services/planetService';

jest.mock('../../src/services/planetService', () => ({
  PlanetService: {
    updateResources: jest.fn(),
    getPlanetById: jest.fn(),
  },
}));

describe('LocationService', () => {
  afterEach(() => {
    jest.clearAllMocks();
  });

  it('resolves a planet location with refreshed resources', async () => {
    const mockPlanet = {
      id: 1,
      user_id: 9,
      metal: 1000,
      crystal: 500,
      deuterium: 250,
      robotics_factory: 3,
      shipyard: 4,
      nanite_factory: 1,
    };
    (PlanetService.updateResources as jest.Mock).mockResolvedValue(mockPlanet);

    const mockClient: any = { query: jest.fn() };
    const context = await resolveLocation(mockClient, 9, { planetId: 1 });

    expect(context.type).toBe('planet');
    expect(context.resourceTable).toBe('planets');
    expect(context.primaryId).toBe(1);
    expect(context.roboticsLevel).toBe(3);
    expect(context.shipyardLevel).toBe(4);
    expect(context.naniteLevel).toBe(1);
    expect(PlanetService.updateResources).toHaveBeenCalledWith(1);
  });

  it('throws if moon does not belong to expected planet', async () => {
    const mockClient: any = {
      query: jest.fn().mockResolvedValue({
        rows: [
          {
            id: 7,
            planet_id: 2,
            user_id: 9,
            moon_robotics_factory: 1,
            moon_shipyard: 1,
            moon_nanite_factory: 0,
          },
        ],
      }),
    };

    await expect(
      resolveLocation(mockClient, 9, {
        locationType: 'moon',
        moonId: 7,
        expectedPlanetId: 5,
      })
    ).rejects.toThrow('Moon does not belong to the specified planet');
  });
});
