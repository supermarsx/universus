import { CombatService } from '../../src/services/combatService';

jest.setTimeout(10000);

describe('CombatService deterministic simulation', () => {
  test('simulateBattle is deterministic given the same seed', async () => {
    const attacker = { light_fighter: 10, cruiser: 2 };
    const defender = { light_fighter: 8, cruiser: 1 };
    const defenses = {};
    const attackerTech = { weapons_technology: 0, shielding_technology: 0, armor_technology: 0 };
    const defenderTech = { weapons_technology: 0, shielding_technology: 0, armor_technology: 0 };
    const planetResources = { metal: 10000, crystal: 5000, deuterium: 1000 };

    const seed = 42;

    const resultA = await CombatService.simulateBattle(
      attacker,
      defender,
      defenses,
      attackerTech,
      defenderTech,
      planetResources,
      /* planetId */ undefined,
      seed
    );

    const resultB = await CombatService.simulateBattle(
      attacker,
      defender,
      defenses,
      attackerTech,
      defenderTech,
      planetResources,
      /* planetId */ undefined,
      seed
    );

    expect(resultA.winner).toEqual(resultB.winner);
    expect(resultA.attackerLosses).toEqual(resultB.attackerLosses);
    expect(resultA.defenderLosses).toEqual(resultB.defenderLosses);
    expect(resultA.debris).toEqual(resultB.debris);
    expect(resultA.loot).toEqual(resultB.loot);
    expect(resultA.rounds.length).toEqual(resultB.rounds.length);
  });
});
