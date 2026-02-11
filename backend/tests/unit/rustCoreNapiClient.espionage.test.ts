describe('rustCoreNapiClient.computeEspionageOutcomeNapi', () => {
  afterEach(() => {
    jest.resetModules();
    jest.clearAllMocks();
    delete process.env.CORE_NAPI_BINDING_PATH;
  });

  it('normalizes snake_case response from compute_espionage_outcome export', async () => {
    jest.doMock(
      'backend-core-napi',
      () => ({
        compute_espionage_outcome: jest.fn((payloadJson: string) => {
          const payload = JSON.parse(payloadJson);
          expect(payload.seed).toBe('f-1:1:2:3:8');
          return JSON.stringify({
            intel_level: 'standard',
            detected: true,
            detection_chance: 0.35,
            detail_score: 7,
            defense_score: 4,
          });
        }),
      }),
      { virtual: true }
    );

    const { computeEspionageOutcomeNapi } = await import('../../src/coreAdapter/rustCoreNapiClient');
    const result = await computeEspionageOutcomeNapi({
      probes: 8,
      attacker_espionage: 4,
      defender_espionage: 4,
      seed: 'f-1:1:2:3:8',
    });

    expect(result).toEqual({
      intelLevel: 'standard',
      detected: true,
      detectionChance: 0.35,
      detailScore: 7,
      defenseScore: 4,
    });
  });

  it('normalizes camelCase response from computeEspionageOutcome export', async () => {
    jest.doMock(
      'backend-core-napi',
      () => ({
        computeEspionageOutcome: jest.fn(() =>
          JSON.stringify({
            intelLevel: 'full',
            detected: false,
            detectionChance: 0.2,
            detailScore: 11,
            defenseScore: 2,
          })
        ),
      }),
      { virtual: true }
    );

    const { computeEspionageOutcomeNapi } = await import('../../src/coreAdapter/rustCoreNapiClient');
    const result = await computeEspionageOutcomeNapi({
      probes: 32,
      attacker_espionage: 6,
      defender_espionage: 1,
      seed: 'seed-x',
    });

    expect(result).toEqual({
      intelLevel: 'full',
      detected: false,
      detectionChance: 0.2,
      detailScore: 11,
      defenseScore: 2,
    });
  });

  it('throws when espionage export is missing from the N-API binding', async () => {
    jest.doMock(
      'backend-core-napi',
      () => ({
        calculateFleetMovement: jest.fn(),
      }),
      { virtual: true }
    );

    const { computeEspionageOutcomeNapi } = await import('../../src/coreAdapter/rustCoreNapiClient');

    await expect(
      computeEspionageOutcomeNapi({
        probes: 1,
        attacker_espionage: 0,
        defender_espionage: 0,
      })
    ).rejects.toThrow('Rust N-API function computeEspionageOutcome not exported');
  });
});
