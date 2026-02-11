import { RustHttpHelperClientService } from '../../src/services/rustHttpHelperClientService';

describe('RustHttpHelperClientService', () => {
  const envBackup = process.env.RUST_HTTP_HELPER_URL;
  const timeoutBackup = process.env.RUST_HTTP_HELPER_TIMEOUT_MS;
  const fetchBackup = global.fetch;

  beforeEach(() => {
    process.env.RUST_HTTP_HELPER_URL = 'http://rust-helper:8080';
    process.env.RUST_HTTP_HELPER_TIMEOUT_MS = '1000';
    global.fetch = jest.fn();
  });

  afterEach(() => {
    jest.clearAllMocks();
  });

  afterAll(() => {
    process.env.RUST_HTTP_HELPER_URL = envBackup;
    process.env.RUST_HTTP_HELPER_TIMEOUT_MS = timeoutBackup;
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
});
