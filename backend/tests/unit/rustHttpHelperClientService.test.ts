import { RustHttpHelperClientService } from '../../src/services/rustHttpHelperClientService';

describe('RustHttpHelperClientService', () => {
  const envBackup = process.env.RUST_HTTP_HELPER_URL;
  const timeoutBackup = process.env.RUST_HTTP_HELPER_TIMEOUT_MS;
  const coreHelperTokenBackup = process.env.CORE_HTTP_HELPER_TOKEN;
  const fetchBackup = global.fetch;

  beforeEach(() => {
    process.env.RUST_HTTP_HELPER_URL = 'http://rust-helper:8080';
    process.env.RUST_HTTP_HELPER_TIMEOUT_MS = '1000';
    delete process.env.CORE_HTTP_HELPER_TOKEN;
    global.fetch = jest.fn();
  });

  afterEach(() => {
    jest.clearAllMocks();
  });

  afterAll(() => {
    process.env.RUST_HTTP_HELPER_URL = envBackup;
    process.env.RUST_HTTP_HELPER_TIMEOUT_MS = timeoutBackup;
    process.env.CORE_HTTP_HELPER_TOKEN = coreHelperTokenBackup;
    global.fetch = fetchBackup;
  });

  test('detects whether Rust HTTP helper is configured', () => {
    process.env.RUST_HTTP_HELPER_URL = '   ';
    expect(RustHttpHelperClientService.isConfigured()).toBe(false);

    process.env.RUST_HTTP_HELPER_URL = 'http://rust-helper:8080';
    expect(RustHttpHelperClientService.isConfigured()).toBe(true);
  });

  test('unwraps success envelope payload', async () => {
    (global.fetch as jest.Mock).mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => ({
        success: true,
        data: {
          distance: 1100,
          fleetSpeed: 5000,
          travelTimeSeconds: 792,
          fuelNeeded: 110,
          cargoCapacity: 4890,
          engine: 'rust-http',
        },
      }),
    } as any);

    const result = await RustHttpHelperClientService.calculateMovement({
      origin: { galaxy: 1, system: 1, position: 1 },
      target: { galaxy: 1, system: 1, position: 21 },
      ships: { small_cargo: 10 },
    });

    expect(result.distance).toBe(1100);
    expect((global.fetch as jest.Mock).mock.calls[0][0]).toBe('http://rust-helper:8080/api/fleet/helpers/movement');
    expect((global.fetch as jest.Mock).mock.calls[0][1].headers).toEqual({
      'content-type': 'application/json',
    });
  });

  test('sends x-core-helper-token header when CORE_HTTP_HELPER_TOKEN is set', async () => {
    process.env.CORE_HTTP_HELPER_TOKEN = 'helper-token-value';
    (global.fetch as jest.Mock).mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => ({
        success: true,
        data: {
          distance: 1100,
          fleetSpeed: 5000,
          travelTimeSeconds: 792,
          fuelNeeded: 110,
          cargoCapacity: 4890,
          engine: 'rust-http',
        },
      }),
    } as any);

    await RustHttpHelperClientService.calculateMovement({
      origin: { galaxy: 1, system: 1, position: 1 },
      target: { galaxy: 1, system: 1, position: 21 },
      ships: { small_cargo: 10 },
    });

    expect((global.fetch as jest.Mock).mock.calls[0][1].headers).toEqual({
      'content-type': 'application/json',
      'x-core-helper-token': 'helper-token-value',
    });
  });

  test('throws on non-2xx status', async () => {
    (global.fetch as jest.Mock).mockResolvedValue({
      ok: false,
      status: 502,
      json: async () => ({}),
    } as any);

    await expect(
      RustHttpHelperClientService.resolveDefenseRebuild({
        current: { rocket_launcher: 10 },
        losses: { rocket_launcher: 4 },
      })
    ).rejects.toThrow('Rust helper request failed');
  });

  test('calls espionage outcome endpoint path', async () => {
    (global.fetch as jest.Mock).mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => ({
        success: true,
        data: {
          intelLevel: 'minimal',
          detected: true,
          detectionChance: 0.5,
          detailScore: 1.2,
          defenseScore: 2.3,
          engine: 'rust-http',
        },
      }),
    } as any);

    await RustHttpHelperClientService.computeEspionageOutcome({
      probes: 3,
      attackerEspionage: 1,
      defenderEspionage: 2,
      seed: 'fleet-1',
    });

    expect((global.fetch as jest.Mock).mock.calls[0][0]).toBe(
      'http://rust-helper:8080/api/fleet/helpers/espionage-outcome'
    );
  });
});
