export function createRedisMock(overrides: Partial<any> = {}) {
  const base: any = {
    exists: jest.fn().mockResolvedValue(0),
    zrevrange: jest.fn().mockResolvedValue([]),
    zrevrank: jest.fn().mockResolvedValue(null),
    zadd: jest.fn().mockResolvedValue(0),
    zrange: jest.fn().mockResolvedValue([]),
    hmget: jest.fn().mockResolvedValue([]),
    hget: jest.fn().mockResolvedValue(Date.now().toString()),
    hgetall: jest.fn().mockResolvedValue({}),
    hset: jest.fn().mockResolvedValue(0),
    hmset: jest.fn().mockResolvedValue('OK'),
    ttl: jest.fn().mockResolvedValue(-1),
    del: jest.fn().mockResolvedValue(0),
    expire: jest.fn().mockResolvedValue(0),
    pipeline: jest.fn(() => ({
      del: jest.fn().mockReturnThis(),
      zadd: jest.fn().mockReturnThis(),
      expire: jest.fn().mockReturnThis(),
      hset: jest.fn().mockReturnThis(),
      exec: jest.fn().mockResolvedValue([]),
    })),
  };

  return Object.assign({}, base, overrides);
}
