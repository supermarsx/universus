jest.mock('../../src/config/database', () => ({
  pool: { query: jest.fn() },
  default: { query: jest.fn() },
}));

jest.mock('../../src/config/redis', () => ({
  redis: {
    get: jest.fn(),
    set: jest.fn(),
    status: 'mock',
  },
  default: {
    get: jest.fn(),
    set: jest.fn(),
    status: 'mock',
  },
}));

jest.mock('../../src/services/planetService', () => ({
  PlanetService: {
    getPlanetById: jest.fn(),
  },
}));

jest.mock('../../src/services/researchService', () => ({
  ResearchService: {
    getUserResearch: jest.fn().mockResolvedValue({ espionage_technology: 0 }),
  },
}));

jest.mock('../../src/services/gameConfigAdapter', () => ({
  GameConfigAdapter: {
    getInstance: () => ({
      getGalaxyCount: jest.fn().mockResolvedValue(9),
      getSystemsPerGalaxy: jest.fn().mockResolvedValue(499),
      getPositionsPerSystem: jest.fn().mockResolvedValue(15),
    }),
  },
}));

const GalaxyService = require('../../src/services/galaxyService').default;
const svc = GalaxyService as any;

describe('GalaxyService intel helpers', () => {
  const defaultPlanetRow = {
    id: 42,
    name: 'Outpost',
    planet_type: 'terran',
    temperature: 10,
    galaxy: 1,
    system: 5,
    position: 7,
    user_id: 2,
    username: 'opponent',
    alliance_id: 99,
    alliance_name: 'Nova',
    alliance_tag: 'NOVA',
    last_seen: new Date().toISOString(),
  };

  it('classifies intel quality based on sensor range', () => {
    const origin = { galaxy: 1, system: 5, sensor_phalanx: 0, sensor_array: 0 };

    expect(svc.determineIntelQuality(defaultPlanetRow, origin, 2)).toBe('full');
    expect(svc.determineIntelQuality({ ...defaultPlanetRow, system: 8 }, origin, 2)).toBe('partial');
    expect(svc.determineIntelQuality({ ...defaultPlanetRow, system: 10 }, origin, 2)).toBe('minimal');
  });

  it('marks self, ally, and neutral relations correctly', () => {
    const selfOwner = svc.decorateOwner(defaultPlanetRow, 2, null);
    expect(selfOwner.relation).toBe('self');

    const allyOwner = svc.decorateOwner(defaultPlanetRow, 3, 99);
    expect(allyOwner.relation).toBe('ally');

    const neutralOwner = svc.decorateOwner(defaultPlanetRow, 3, 10);
    expect(neutralOwner.relation).toBe('neutral');
  });

  it('strips planet intel for minimal scans', () => {
    const rawData = {
      planets: [{ ...defaultPlanetRow, name: 'Hidden', user_id: 5 }],
      debris: [],
    };

    const slots = svc.buildSlots({
      rawData,
      positionsPerSystem: 15,
      originPlanet: { galaxy: 2, system: 1 },
      sensorRange: 1,
      userId: 1,
      requesterAllianceId: null,
    });

    const slot = slots.find((s: any) => s.position === defaultPlanetRow.position);
    expect(slot?.intelQuality).toBe('minimal');
    expect(slot?.planet?.name).toBeNull();
    expect(slot?.owner).toBeUndefined();
  });

  it('calculateSensorRange stacks espionage and sensor arrays', () => {
    expect(svc.calculateSensorRange(0, null)).toBe(1);
    expect(svc.calculateSensorRange(5, { sensor_array: 4 })).toBe(5);
    expect(svc.calculateSensorRange(3, { sensor_phalanx: 3 })).toBe(3);
  });
});
