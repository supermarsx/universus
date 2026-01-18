import request from 'supertest';
import app from '../../src/app'; // Adjust if your express app is exported elsewhere
import moonService from '../../src/services/moonService';
import jumpGateService from '../../src/services/jumpGateService';
import destroyMoonService from '../../src/services/destroyMoonService';
import phalanxService from '../../src/services/phalanxService';

jest.mock('../../src/services/moonService');
jest.mock('../../src/services/jumpGateService');
jest.mock('../../src/services/destroyMoonService');
jest.mock('../../src/services/phalanxService');

describe('Moons API', () => {
  const token = 'test.jwt.token';
  beforeEach(() => {
    jest.clearAllMocks();
  });

  describe('POST /api/moons/:moonId/phalanx', () => {
    it('should return 200 on success', async () => {
      (phalanxService.performScan as jest.Mock).mockResolvedValue({
        target: { galaxy: 1, system: 1, position: 1 },
        sensor: { level: 1, range: 0, cost: 5000 },
        fleets: { inbound: [], outbound: [] }
      });
      const res = await request(app)
        .post('/api/moons/1/phalanx')
        .set('Authorization', `Bearer ${token}`)
        .send({ targetGalaxy: 1, targetSystem: 1, targetPosition: 1 });
      expect(res.status).toBe(200);
      expect(res.body.success).toBe(true);
    });
    it('should return 400 on error', async () => {
      (phalanxService.performScan as jest.Mock).mockRejectedValue(new Error('No Sensor Phalanx'));
      const res = await request(app)
        .post('/api/moons/1/phalanx')
        .set('Authorization', `Bearer ${token}`)
        .send({ targetGalaxy: 1, targetSystem: 1, targetPosition: 1 });
      expect(res.status).toBe(400);
      expect(res.body.success).toBe(false);
    });
  });

  describe('POST /api/moons/:moonId/jump-gate', () => {
    it('should return 200 on success', async () => {
      (jumpGateService.jumpFleet as jest.Mock).mockResolvedValue({ success: true });
      const res = await request(app)
        .post('/api/moons/1/jump-gate')
        .set('Authorization', `Bearer ${token}`)
        .send({ toMoonId: 2, fleetIds: [101, 102] });
      expect(res.status).toBe(200);
      expect(res.body.success).toBe(true);
    });
    it('should return 400 on error', async () => {
      (jumpGateService.jumpFleet as jest.Mock).mockResolvedValue({ success: false, error: 'Cooldown' });
      const res = await request(app)
        .post('/api/moons/1/jump-gate')
        .set('Authorization', `Bearer ${token}`)
        .send({ toMoonId: 2, fleetIds: [101] });
      expect(res.status).toBe(400);
      expect(res.body.success).toBe(false);
    });
  });

  describe('POST /api/moons/:moonId/destroy', () => {
    it('should return 200 on success', async () => {
      (destroyMoonService.attemptDestruction as jest.Mock).mockResolvedValue({ destroyed: true, deathstarsLost: 1, chance: 50, lossChance: 10 });
      const res = await request(app)
        .post('/api/moons/1/destroy')
        .set('Authorization', `Bearer ${token}`)
        .send({ numDeathstars: 5 });
      expect(res.status).toBe(200);
      expect(res.body.success).toBe(true);
      expect(res.body.data.destroyed).toBe(true);
    });
    it('should return 400 on error', async () => {
      (destroyMoonService.attemptDestruction as jest.Mock).mockResolvedValue({ error: 'Moon not found' });
      const res = await request(app)
        .post('/api/moons/1/destroy')
        .set('Authorization', `Bearer ${token}`)
        .send({ numDeathstars: 5 });
      expect(res.status).toBe(400);
      expect(res.body.success).toBe(false);
    });
  });
});
