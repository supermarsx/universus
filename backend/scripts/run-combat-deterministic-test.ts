import { CombatService } from '../src/services/combatService';

async function run() {
  try {
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
      undefined,
      seed
    );

    const resultB = await CombatService.simulateBattle(
      attacker,
      defender,
      defenses,
      attackerTech,
      defenderTech,
      planetResources,
      undefined,
      seed
    );

    const equal =
      resultA.winner === resultB.winner &&
      JSON.stringify(resultA.attackerLosses) === JSON.stringify(resultB.attackerLosses) &&
      JSON.stringify(resultA.defenderLosses) === JSON.stringify(resultB.defenderLosses) &&
      JSON.stringify(resultA.debris) === JSON.stringify(resultB.debris) &&
      JSON.stringify(resultA.loot) === JSON.stringify(resultB.loot) &&
      resultA.rounds.length === resultB.rounds.length;

    if (equal) {
      console.log('Deterministic test PASSED');
      process.exit(0);
    } else {
      console.error('Deterministic test FAILED');
      console.log('Result A:', JSON.stringify(resultA, null, 2));
      console.log('Result B:', JSON.stringify(resultB, null, 2));
      process.exit(2);
    }
  } catch (err) {
    console.error('Error running deterministic test:', err);
    process.exit(3);
  }
}

run();
