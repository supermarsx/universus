import { ShipyardService } from '../../src/services/shipyardService';
import { resolveLocation } from '../../src/services/locationService';

jest.mock('../../src/config/database', () => ({
  pool: {
    query: jest.fn(),
    connect: jest.fn(),
  },
}));

jest.mock('../../src/services/locationService', () => ({
  resolveLocation: jest.fn(),
}));

const dbModule = require('../../src/config/database');
const pool = dbModule.pool;

describe('ShipyardService', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  it('enqueues moon production with location metadata', async () => {
    const mockClient = {
      query: jest.fn().mockResolvedValue({ rows: [] }),
      release: jest.fn(),
    };
    pool.connect.mockResolvedValue(mockClient);

    (resolveLocation as jest.Mock).mockResolvedValue({
      type: 'moon',
      planetId: 1,
      moonId: 7,
      resourceTable: 'moons',
      primaryId: 7,
      record: { metal: 100000, crystal: 100000, deuterium: 100000 },
      shipyardLevel: 2,
      roboticsLevel: 1,
      naniteLevel: 0,
    });

    mockClient.query
      .mockResolvedValueOnce({}) // BEGIN
      .mockResolvedValueOnce({}) // UPDATE moons resources
      .mockResolvedValueOnce({
        rows: [
          {
            id: 5,
            location_type: 'moon',
            moon_id: 7,
            unit_type: 'rocket_launcher',
          },
        ],
      }) // INSERT RETURNING *
      .mockResolvedValue({}); // COMMIT

    const result = await ShipyardService.startProduction(
      99,
      'rocket_launcher',
      5,
      { planetId: 1, moonId: 7, locationType: 'moon' }
    );

    expect(resolveLocation).toHaveBeenCalledWith(expect.anything(), 99, {
      planetId: 1,
      moonId: 7,
      locationType: 'moon',
      expectedPlanetId: 1,
    });
    expect(mockClient.query).toHaveBeenCalledWith(expect.stringContaining('INSERT INTO shipyard_queue'), expect.arrayContaining(['moon', 'rocket_launcher', 5]));
    expect(result.location_type).toBe('moon');
  });

  it('updates moon unit counts when completing queues', async () => {
    const queueRow = {
      id: 12,
      location_type: 'moon',
      moon_id: 42,
      planet_id: 1,
      unit_type: 'rocket_launcher',
      quantity: 8,
    };

    const mockQuery = pool.query as jest.Mock;
    mockQuery.mockImplementation((sql: string) => {
      if (sql.includes('SELECT * FROM shipyard_queue')) {
        return Promise.resolve({ rows: [queueRow] });
      }
      return Promise.resolve({ rows: [] });
    });

    const completed = await ShipyardService.completeFinishedJobs();

    expect(completed).toBe(1);
    expect(mockQuery).toHaveBeenCalledWith(
      expect.stringContaining('UPDATE moons SET rocket_launcher'),
      [queueRow.quantity, queueRow.moon_id]
    );
    expect(mockQuery).toHaveBeenCalledWith('DELETE FROM shipyard_queue WHERE id = $1', [queueRow.id]);
  });
});
