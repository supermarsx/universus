// Isolated combat simulation test (no external imports)
// Verifies deterministic simulateBattle given a numeric seed

// Minimal ship/defense configs used in the test
const SHIPS: any = {
  light_fighter: { shieldPower: 10, weaponPower: 5, structurePoints: 20, cargo: 100, rapidFire: { cruiser: 5 }, cost: { metal: 300, crystal: 100 } },
  cruiser: { shieldPower: 80, weaponPower: 150, structurePoints: 400, cargo: 800, rapidFire: {}, cost: { metal: 2000, crystal: 700 } },
};

const DEFENSES: any = {};

function mulberry32(seed: number) {
  return function() {
    let t = seed += 0x6D2B79F5;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

interface CombatUnit {
  type: string;
  count: number;
  shield: number;
  weapon: number;
  hull: number;
  maxShield?: number;
  maxHull?: number;
  rapidFire?: { [key: string]: number };
}

function prepareCombatUnits(
  ships: { [key: string]: number },
  defenses: { [key: string]: number },
  tech: any
): CombatUnit[] {
  const units: CombatUnit[] = [];

  for (const [type, count] of Object.entries(ships)) {
    const cnt = count as number;
    if (cnt > 0 && SHIPS[type]) {
      const ship = SHIPS[type];
      for (let i = 0; i < cnt; i++) {
        const maxShield = ship.shieldPower * (1 + (tech?.shielding_technology || 0) * 0.1);
        const maxHull = ship.structurePoints * (1 + (tech?.armor_technology || 0) * 0.1);
        units.push({
          type,
          count: 1,
          shield: maxShield,
          weapon: ship.weaponPower * (1 + (tech?.weapons_technology || 0) * 0.1),
          hull: maxHull,
          maxShield,
          maxHull,
          rapidFire: ship.rapidFire,
        });
      }
    }
  }
  return units;
}

function removeDestroyed(units: CombatUnit[]): number {
  let destroyed = 0;
  for (let i = units.length - 1; i >= 0; i--) {
    if (units[i].hull <= 0) {
      units.splice(i, 1);
      destroyed++;
    }
  }
  return destroyed;
}

function regenerateShields(units: CombatUnit[]): void {
  for (const unit of units) {
    if (typeof unit.maxShield === 'number') {
      unit.shield = unit.maxShield;
    } else {
      const config = SHIPS[unit.type] || DEFENSES[unit.type];
      if (config) unit.shield = config.shieldPower;
    }
  }
}

function shoot(shooter: CombatUnit, target: CombatUnit, targetArray: CombatUnit[], rand: () => number) {
  let damage = shooter.weapon;
  if (damage < target.shield * 0.01) return;
  if (target.shield > 0) {
    const shieldDamage = Math.min(damage, target.shield);
    target.shield -= shieldDamage;
    damage -= shieldDamage;
  }
  if (damage > 0) {
    target.hull -= damage;
    if (target.hull <= 0) {
      target.hull = 0;
    } else if (typeof target.maxHull === 'number' && target.hull < target.maxHull * 0.7) {
      const explosionChance = 1 - (target.hull / (target.maxHull * 0.7));
      if (rand() < explosionChance) {
        target.hull = 0;
      }
    }
  }
  if (shooter.rapidFire && shooter.rapidFire[target.type]) {
    const rfChance = 1 - 1 / shooter.rapidFire[target.type];
    if (rand() < rfChance && targetArray.length > 0) {
      const newTarget = targetArray[Math.floor(rand() * targetArray.length)];
      shoot(shooter, newTarget, targetArray, rand);
    }
  }
}

function simulateRound(attackers: CombatUnit[], defenders: CombatUnit[], rand: () => number) {
  const roundData: any = { attackerShots: 0, defenderShots: 0, attackerDestroyed: 0, defenderDestroyed: 0 };
  for (const attacker of attackers) {
    if (defenders.length === 0) break;
    const target = defenders[Math.floor(rand() * defenders.length)];
    shoot(attacker, target, defenders, rand);
    roundData.attackerShots++;
  }
  roundData.defenderDestroyed = removeDestroyed(defenders);
  for (const defender of defenders) {
    if (attackers.length === 0) break;
    const target = attackers[Math.floor(rand() * attackers.length)];
    shoot(defender, target, attackers, rand);
    roundData.defenderShots++;
  }
  roundData.attackerDestroyed = removeDestroyed(attackers);
  return roundData;
}

function calculateLosses(initial: { [key: string]: number }, remaining: CombatUnit[]) {
  const losses: { [key: string]: number } = {};
  const remainingCounts: { [key: string]: number } = {};
  for (const unit of remaining) remainingCounts[unit.type] = (remainingCounts[unit.type] || 0) + 1;
  for (const [type, count] of Object.entries(initial)) {
    const lost = (count as number) - (remainingCounts[type] || 0);
    if (lost > 0) losses[type] = lost;
  }
  return losses;
}

function calculateDebris(attackerLosses: { [key: string]: number }, defenderLosses: { [key: string]: number }) {
  let metal = 0, crystal = 0;
  const addDebris = (losses: { [key: string]: number }) => {
    for (const [type, count] of Object.entries(losses)) {
      const config = SHIPS[type] || DEFENSES[type];
      if (config) {
        metal += Math.floor(config.cost.metal * (count as number) * 0.3);
        crystal += Math.floor(config.cost.crystal * (count as number) * 0.3);
      }
    }
  };
  addDebris(attackerLosses); addDebris(defenderLosses);
  return { metal, crystal };
}

function calculateLoot(planetResources: any, attackerUnits: CombatUnit[]) {
  let cargoCapacity = 0;
  for (const unit of attackerUnits) {
    const ship = SHIPS[unit.type];
    if (ship) cargoCapacity += ship.cargo;
  }
  const maxLoot = { metal: Math.floor(planetResources.metal * 0.5), crystal: Math.floor(planetResources.crystal * 0.5), deuterium: Math.floor(planetResources.deuterium * 0.5) };
  const totalAvailable = maxLoot.metal + maxLoot.crystal + maxLoot.deuterium;
  if (totalAvailable === 0 || cargoCapacity === 0) return { metal: 0, crystal: 0, deuterium: 0 };
  const capacityUsed = Math.min(cargoCapacity, totalAvailable);
  return {
    metal: Math.floor((maxLoot.metal / totalAvailable) * capacityUsed),
    crystal: Math.floor((maxLoot.crystal / totalAvailable) * capacityUsed),
    deuterium: Math.floor((maxLoot.deuterium / totalAvailable) * capacityUsed),
  };
}

async function simulateBattle(attackerShips: any, defenderShips: any, defenderDefenses: any, attackerTech: any, defenderTech: any, planetResources: any, seed?: number) {
  const rngSeed = typeof seed === 'number' ? seed : Date.now();
  const rand = mulberry32(Math.floor(rngSeed));
  const attackerUnits = prepareCombatUnits(attackerShips, {}, attackerTech);
  const defenderUnits = prepareCombatUnits(defenderShips, defenderDefenses, defenderTech);
  const rounds: any[] = [];
  const maxRounds = 6;
  for (let round = 1; round <= maxRounds; round++) {
    if (attackerUnits.length === 0 || defenderUnits.length === 0) break;
    const roundResult = simulateRound(attackerUnits, defenderUnits, rand);
    rounds.push(roundResult);
    regenerateShields(attackerUnits); regenerateShields(defenderUnits);
  }
  let winner: 'attacker' | 'defender' | 'draw';
  if (attackerUnits.length === 0 && defenderUnits.length === 0) winner = 'draw';
  else if (defenderUnits.length === 0) winner = 'attacker'; else winner = 'defender';
  const attackerLosses = calculateLosses(attackerShips, attackerUnits);
  const defenderLosses = calculateLosses({ ...defenderShips, ...defenderDefenses }, defenderUnits);
  const debris = calculateDebris(attackerLosses, defenderLosses);
  const loot = winner === 'attacker' ? calculateLoot(planetResources, attackerUnits) : { metal: 0, crystal: 0, deuterium: 0 };
  return { winner, rounds, attackerLosses, defenderLosses, loot, debris };
}

(async function main(){
  const attacker = { light_fighter: 10, cruiser: 2 };
  const defender = { light_fighter: 8, cruiser: 1 };
  const defenses = {};
  const attackerTech = { weapons_technology: 0, shielding_technology: 0, armor_technology: 0 };
  const defenderTech = { weapons_technology: 0, shielding_technology: 0, armor_technology: 0 };
  const planetResources = { metal: 10000, crystal: 5000, deuterium: 1000 };
  const seed = 42;

  const a = await simulateBattle(attacker, defender, defenses, attackerTech, defenderTech, planetResources, seed);
  const b = await simulateBattle(attacker, defender, defenses, attackerTech, defenderTech, planetResources, seed);

  const equal = JSON.stringify(a) === JSON.stringify(b);
  console.log('Result A:', JSON.stringify(a, null, 2));
  console.log('Result B:', JSON.stringify(b, null, 2));
  console.log('Deterministic equal:', equal);
  process.exit(equal ? 0 : 1);
})();
