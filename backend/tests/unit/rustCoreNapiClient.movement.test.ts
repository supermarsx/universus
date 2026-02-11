describe('rustCoreNapiClient.calculateFleetMovementByTypeNapi', () => {
  afterEach(() => {
    jest.resetModules();
    jest.clearAllMocks();
    delete process.env.CORE_NAPI_BINDING_PATH;
  });

  it('normalizes by-type movement response and sends deterministic ship map', async () => {
    jest.doMock(
      'backend-core-napi',
      () => ({
        calculate_fleet_movement_by_type: jest.fn((payload: any) => {
          expect(payload).toEqual({
            originGalaxy: 1,
            originSystem: 10,
            originPosition: 5,
            targetGalaxy: 1,
            targetSystem: 11,
            targetPosition: 7,
            ships: {
              cruiser: 2,
              small_cargo: 5,
            },
          });
          return {
            distance: 2795,
            fleet_speed: 5000,
            travel_time_seconds: 2013,
            fuel_needed: 1300,
            cargo_capacity: 22700,
          };
        }),
      }),
      { virtual: true }
    );

    const { calculateFleetMovementByTypeNapi } = await import('../../src/coreAdapter/rustCoreNapiClient');
    const result = await calculateFleetMovementByTypeNapi({
      origin_galaxy: 1,
      origin_system: 10,
      origin_position: 5,
      target_galaxy: 1,
      target_system: 11,
      target_position: 7,
      ships: [
        { ship_type: 'small_cargo', count: 4, base_speed: 5000, fuel_consumption: 10, cargo: 5000 },
        { ship_type: 'cruiser', count: 2, base_speed: 15000, fuel_consumption: 300, cargo: 800 },
        { ship_type: 'small_cargo', count: 1, base_speed: 5000, fuel_consumption: 10, cargo: 5000 },
      ],
    });

    expect(result).toEqual({
      distance: 2795,
      fleetSpeed: 5000,
      travelTimeSeconds: 2013,
      fuelNeeded: 1300,
      cargoCapacity: 22700,
    });
  });

  it('throws when by-type movement export is missing from binding', async () => {
    jest.doMock(
      'backend-core-napi',
      () => ({
        calculateFleetMovementFast: jest.fn(),
      }),
      { virtual: true }
    );

    const { calculateFleetMovementByTypeNapi } = await import('../../src/coreAdapter/rustCoreNapiClient');

    await expect(
      calculateFleetMovementByTypeNapi({
        origin_galaxy: 1,
        origin_system: 1,
        origin_position: 1,
        target_galaxy: 1,
        target_system: 2,
        target_position: 3,
        ships: [{ ship_type: 'small_cargo', count: 1, base_speed: 5000, fuel_consumption: 10, cargo: 5000 }],
      })
    ).rejects.toThrow('Rust N-API function calculateFleetMovementByType not exported');
  });
});
