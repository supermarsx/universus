import { LeaderboardService } from '../../src/services/leaderboardService';
import { Pool } from 'pg';
import Redis from 'ioredis';

// Mock dependencies
jest.mock('pg');
jest.mock('ioredis');

describe('LeaderboardService', () => {
  let leaderboardService: LeaderboardService;
  let mockPool: any;
  let mockRedis: any;

  beforeEach(() => {
    // Create mock pool
    mockPool = {
      query: jest.fn(),
      connect: jest.fn(),
    };

    // Create mock redis
    mockRedis = {
      exists: jest.fn(),
      zrevrange: jest.fn(),
      zadd: jest.fn(),
      del: jest.fn(),
      expire: jest.fn(),
      pipeline: jest.fn(() => ({
        del: jest.fn().mockReturnThis(),
        zadd: jest.fn().mockReturnThis(),
        expire: jest.fn().mockReturnThis(),
        exec: jest.fn().mockResolvedValue([]),
      })),
      zrevrank: jest.fn(),
    };

    leaderboardService = new LeaderboardService(mockPool, mockRedis);
  });

  afterEach(() => {
    jest.clearAllMocks();
  });

  describe('calculatePlayerScore', () => {
    it('should calculate total player score correctly', async () => {
      // Mock user query
      mockPool.query
        .mockResolvedValueOnce({
          rows: [{ id: 1, username: 'testuser', alliance_tag: 'TEST' }],
        })
        // Building score query
        .mockResolvedValueOnce({ rows: [{ building_score: 10000 }] })
        // Research query
        .mockResolvedValueOnce({
          rows: [
            { technology: 'energy_technology', level: 5 },
            { technology: 'laser_technology', level: 3 },
          ],
        })
        // Fleet query
        .mockResolvedValueOnce({
          rows: [
            {
              small_cargo: 10,
              large_cargo: 5,
              light_fighter: 20,
              heavy_fighter: 0,
              cruiser: 0,
              battleship: 0,
              colony_ship: 0,
              recycler: 0,
              espionage_probe: 0,
              bomber: 0,
              destroyer: 0,
              deathstar: 0,
            },
          ],
        })
        // Defense query
        .mockResolvedValueOnce({
          rows: [
            {
              rocket_launcher: 50,
              light_laser: 20,
              heavy_laser: 0,
              gauss_cannon: 0,
              ion_cannon: 0,
              plasma_turret: 0,
              small_shield_dome: 0,
              large_shield_dome: 0,
            },
          ],
        });

      const result = await leaderboardService.calculatePlayerScore(1);

      expect(result).toHaveProperty('userId', 1);
      expect(result).toHaveProperty('username', 'testuser');
      expect(result).toHaveProperty('totalScore');
      expect(result).toHaveProperty('buildingScore');
      expect(result).toHaveProperty('researchScore');
      expect(result).toHaveProperty('fleetScore');
      expect(result).toHaveProperty('defenseScore');
      expect(result.totalScore).toBeGreaterThan(0);
    });

    it('should throw error if user not found', async () => {
      mockPool.query.mockResolvedValueOnce({ rows: [] });

      await expect(leaderboardService.calculatePlayerScore(999)).rejects.toThrow(
        'User 999 not found'
      );
    });

    it('should handle users with no alliance', async () => {
      mockPool.query
        .mockResolvedValueOnce({
          rows: [{ id: 1, username: 'testuser', alliance_tag: null }],
        })
        .mockResolvedValueOnce({ rows: [{ building_score: 5000 }] })
        .mockResolvedValueOnce({ rows: [] })
        .mockResolvedValueOnce({ rows: [{ small_cargo: 0 }] })
        .mockResolvedValueOnce({ rows: [{ rocket_launcher: 0 }] });

      const result = await leaderboardService.calculatePlayerScore(1);

      expect(result.allianceTag).toBeUndefined();
    });
  });

  describe('updatePlayerLeaderboard', () => {
    it('should update leaderboard for all active users', async () => {
      mockPool.query.mockResolvedValueOnce({
        rows: [{ id: 1 }, { id: 2 }, { id: 3 }],
      });

      // Mock calculatePlayerScore calls
      jest.spyOn(leaderboardService, 'calculatePlayerScore').mockResolvedValue({
        userId: 1,
        username: 'user1',
        totalScore: 10000,
        buildingScore: 5000,
        researchScore: 3000,
        fleetScore: 1500,
        defenseScore: 500,
        rank: 0,
      });

      const count = await leaderboardService.updatePlayerLeaderboard();

      expect(count).toBe(3);
      expect(mockRedis.pipeline).toHaveBeenCalled();
    });

    it('should handle errors gracefully and continue', async () => {
      mockPool.query.mockResolvedValueOnce({
        rows: [{ id: 1 }, { id: 2 }],
      });

      const calculateSpy = jest
        .spyOn(leaderboardService, 'calculatePlayerScore')
        .mockRejectedValueOnce(new Error('Database error'))
        .mockResolvedValueOnce({
          userId: 2,
          username: 'user2',
          totalScore: 5000,
          buildingScore: 3000,
          researchScore: 1500,
          fleetScore: 500,
          defenseScore: 0,
          rank: 0,
        });

      const count = await leaderboardService.updatePlayerLeaderboard();

      expect(count).toBe(2);
      expect(calculateSpy).toHaveBeenCalledTimes(2);
    });
  });

  describe('getTopPlayers', () => {
    it('should return top players from cache', async () => {
      mockRedis.exists.mockResolvedValue(1);
      mockRedis.zrevrange.mockResolvedValue([
        JSON.stringify({
          userId: 1,
          username: 'player1',
          totalScore: 100000,
          allianceTag: 'TOP',
        }),
        '100000',
        JSON.stringify({
          userId: 2,
          username: 'player2',
          totalScore: 90000,
        }),
        '90000',
      ]);

      const result = await leaderboardService.getTopPlayers(10, 0);

      expect(result).toHaveLength(2);
      expect(result[0]).toHaveProperty('rank', 1);
      expect(result[0]).toHaveProperty('username', 'player1');
      expect(result[0]).toHaveProperty('totalScore', 100000);
      expect(result[1]).toHaveProperty('rank', 2);
    });

    it('should update leaderboard if cache is empty', async () => {
      mockRedis.exists.mockResolvedValue(0);
      mockRedis.zrevrange.mockResolvedValue([]);

      jest.spyOn(leaderboardService, 'updatePlayerLeaderboard').mockResolvedValue(5);

      await leaderboardService.getTopPlayers(10, 0);

      expect(leaderboardService.updatePlayerLeaderboard).toHaveBeenCalled();
    });

    it('should handle pagination correctly', async () => {
      mockRedis.exists.mockResolvedValue(1);
      mockRedis.zrevrange.mockResolvedValue([
        JSON.stringify({ userId: 11, username: 'player11', totalScore: 50000 }),
        '50000',
      ]);

      const result = await leaderboardService.getTopPlayers(10, 10);

      expect(result[0]).toHaveProperty('rank', 11);
      expect(mockRedis.zrevrange).toHaveBeenCalledWith(
        'leaderboard:players',
        10,
        19,
        'WITHSCORES'
      );
    });
  });

  describe('getPlayerRank', () => {
    it('should return player rank with neighbors', async () => {
      jest.spyOn(leaderboardService, 'calculatePlayerScore').mockResolvedValue({
        userId: 1,
        username: 'testuser',
        totalScore: 50000,
        buildingScore: 25000,
        researchScore: 15000,
        fleetScore: 8000,
        defenseScore: 2000,
        rank: 0,
      });

      mockRedis.zrevrank.mockResolvedValue(41); // 42nd rank (0-indexed)

      jest.spyOn(leaderboardService, 'getTopPlayers').mockResolvedValue([
        {
          userId: 1,
          username: 'testuser',
          totalScore: 50000,
          buildingScore: 0,
          researchScore: 0,
          fleetScore: 0,
          defenseScore: 0,
          rank: 42,
        },
      ]);

      const result = await leaderboardService.getPlayerRank(1, 5);

      expect(result.player).toHaveProperty('rank', 42);
      expect(result.neighbors).toBeDefined();
    });
  });

  describe('updateAllianceLeaderboard', () => {
    it('should calculate and update alliance scores', async () => {
      mockPool.query
        .mockResolvedValueOnce({
          rows: [
            { id: 1, name: 'Alliance1', tag: 'A1', member_count: 10 },
            { id: 2, name: 'Alliance2', tag: 'A2', member_count: 5 },
          ],
        })
        .mockResolvedValueOnce({
          rows: [{ id: 1 }, { id: 2 }, { id: 3 }],
        })
        .mockResolvedValueOnce({
          rows: [{ id: 4 }, { id: 5 }],
        });

      jest.spyOn(leaderboardService, 'calculatePlayerScore').mockResolvedValue({
        userId: 1,
        username: 'user',
        totalScore: 10000,
        buildingScore: 5000,
        researchScore: 3000,
        fleetScore: 1500,
        defenseScore: 500,
        rank: 0,
      });

      const count = await leaderboardService.updateAllianceLeaderboard();

      expect(count).toBe(2);
      expect(mockRedis.pipeline).toHaveBeenCalled();
    });
  });

  describe('getTopAlliances', () => {
    it('should return top alliances from cache', async () => {
      mockRedis.exists.mockResolvedValue(1);
      mockRedis.zrevrange.mockResolvedValue([
        JSON.stringify({
          allianceId: 1,
          allianceName: 'Top Alliance',
          allianceTag: 'TOP',
          totalScore: 1000000,
          memberCount: 20,
          averageScore: 50000,
        }),
        '1000000',
      ]);

      const result = await leaderboardService.getTopAlliances(10, 0);

      expect(result).toHaveLength(1);
      expect(result[0]).toHaveProperty('rank', 1);
      expect(result[0]).toHaveProperty('allianceName', 'Top Alliance');
      expect(result[0]).toHaveProperty('totalScore', 1000000);
    });
  });
});
