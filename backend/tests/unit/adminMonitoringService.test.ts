import os from 'os';
import { AdminMonitoringService } from '../../src/services/adminMonitoringService';

const mockQuery = jest.fn();

jest.mock('../../src/config/database', () => ({
  __esModule: true,
  default: { query: jest.fn() },
  pool: { query: jest.fn() },
}));

const dbModule = require('../../src/config/database');
dbModule.default.query = mockQuery;
dbModule.pool.query = mockQuery;

describe('AdminMonitoringService', () => {
  beforeEach(() => {
    mockQuery.mockReset();
  });

  describe('getServerHealth', () => {
    let loadAvgSpy: jest.SpyInstance;
    let uptimeSpy: jest.SpyInstance;

    beforeAll(() => {
      loadAvgSpy = jest.spyOn(os, 'loadavg').mockReturnValue([1.2, 0.9, 0.5]);
      uptimeSpy = jest.spyOn(os, 'uptime').mockReturnValue(7200);
    });

    afterAll(() => {
      loadAvgSpy.mockRestore();
      uptimeSpy.mockRestore();
    });

    it('aggregates metrics into health snapshot', async () => {
      mockQuery.mockResolvedValue({
        rows: [
          { metric_name: 'cpu_usage', avg_value: '42', metric_unit: 'percent' },
          { metric_name: 'memory_usage', avg_value: '55', metric_unit: 'percent' },
          { metric_name: 'active_connections', avg_value: '12', metric_unit: 'count' },
          { metric_name: 'active_players', avg_value: '345', metric_unit: 'count' },
          { metric_name: 'error_rate', avg_value: '1', metric_unit: 'percent' },
        ],
      });

      const health = await AdminMonitoringService.getServerHealth();

      expect(mockQuery).toHaveBeenCalledTimes(1);
      expect(health.cpu_usage).toBe(42);
      expect(health.memory_usage).toBe(55);
      expect(health.database_connections).toBe(12);
      expect(health.active_players).toBe(345);
      expect(health.status).toBe('healthy');
      expect(health.uptime).toBe(7200);
    });
  });

  describe('getMetricsHistory', () => {
    it('returns historical metrics for the requested window', async () => {
      const fakeRows = [{ metric_value: 10, timestamp: '2024-01-01T00:00:00Z' }];
      mockQuery.mockResolvedValue({ rows: fakeRows });

      const history = await AdminMonitoringService.getMetricsHistory('cpu_usage', 2);

      expect(mockQuery).toHaveBeenCalledWith(
        expect.stringContaining('metric_name = $1'),
        ['cpu_usage']
      );
      expect(history).toEqual(fakeRows);
    });
  });

  describe('collectServerMetrics', () => {
    let loadSpy: jest.SpyInstance;
    let cpuSpy: jest.SpyInstance;
    let totalSpy: jest.SpyInstance;
    let freeSpy: jest.SpyInstance;

    beforeAll(() => {
      const cpuInfo = {
        model: 'mock',
        speed: 1200,
        times: { user: 0, nice: 0, sys: 0, idle: 0, irq: 0 },
      };
      loadSpy = jest.spyOn(os, 'loadavg').mockReturnValue([1, 0.5, 0.2]);
      cpuSpy = jest.spyOn(os, 'cpus').mockReturnValue([cpuInfo, cpuInfo, cpuInfo]);
      totalSpy = jest.spyOn(os, 'totalmem').mockReturnValue(1000);
      freeSpy = jest.spyOn(os, 'freemem').mockReturnValue(250);
    });

    afterAll(() => {
      loadSpy.mockRestore();
      cpuSpy.mockRestore();
      totalSpy.mockRestore();
      freeSpy.mockRestore();
    });

    it('writes metrics and triggers threshold checks', async () => {
      const thresholdSpy = jest
        .spyOn(AdminMonitoringService as any, 'checkThreshold')
        .mockResolvedValue(undefined);

      mockQuery
        .mockResolvedValueOnce({ rows: [{ active_connections: '7' }] })
        .mockResolvedValueOnce({ rows: [{ active_players: '21' }] })
        .mockResolvedValue({ rows: [] });

      await AdminMonitoringService.collectServerMetrics();

      expect(mockQuery).toHaveBeenCalledTimes(6);
      expect(thresholdSpy).toHaveBeenCalledTimes(4);
      const metricNames = thresholdSpy.mock.calls.map((call) => (call[0] as any).metric_name);
      expect(metricNames).toEqual([
        'cpu_usage',
        'memory_usage',
        'active_connections',
        'active_players',
      ]);

      thresholdSpy.mockRestore();
    });
  });

  describe('checkThreshold', () => {
    it('creates notification when metric exceeds threshold', async () => {
      mockQuery.mockResolvedValue({ rows: [] });
      const notificationSpy = jest
        .spyOn(AdminMonitoringService as any, 'createNotification')
        .mockResolvedValue({} as any);

      await AdminMonitoringService.checkThreshold({
        id: 99,
        metric_name: 'cpu_usage',
        metric_value: 90,
        metric_unit: 'percent',
        metric_type: 'system',
        timestamp: new Date(),
      } as any);

      expect(mockQuery).toHaveBeenCalledWith(expect.stringContaining('threshold_exceeded'), [99]);
      expect(notificationSpy).toHaveBeenCalledWith(expect.objectContaining({ notification_type: 'threshold_exceeded' }));

      notificationSpy.mockRestore();
    });

    it('ignores metrics below threshold', async () => {
      const notificationSpy = jest
        .spyOn(AdminMonitoringService as any, 'createNotification')
        .mockResolvedValue({} as any);

      await AdminMonitoringService.checkThreshold({
        id: 1,
        metric_name: 'cpu_usage',
        metric_value: 10,
        metric_unit: 'percent',
        metric_type: 'system',
        timestamp: new Date(),
      } as any);

      expect(mockQuery).not.toHaveBeenCalled();
      expect(notificationSpy).not.toHaveBeenCalled();

      notificationSpy.mockRestore();
    });
  });
});
