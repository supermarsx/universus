export const moonConfig = {
  MOONCHANCE_UNIT: 100000,
  MOONCHANCE_CAP: 20,
  BASE_DIAMETER: 2000,
  SIZE_PER_PERCENT: 250,
  SIZE_JITTER: 200,
  DIAMETER_MIN: 2000,
  DIAMETER_MAX: 12000,
  BASE_FIELDS: 1,
  FIELDS_PER_LUNAR_BASE: 3,
  PHALANX_SCAN_COST: 5000,
};

export function getMoonChanceFromDebris(debrisMetal: number, debrisCrystal: number): number {
  const total = debrisMetal + debrisCrystal;
  if (total <= 0) return 0;
  const chance = Math.floor(total / moonConfig.MOONCHANCE_UNIT);
  return Math.min(chance, moonConfig.MOONCHANCE_CAP);
}

export function rollForMoon(chancePercent: number): boolean {
  if (chancePercent <= 0) return false;
  return Math.random() * 100 < chancePercent;
}

export function calculateMoonDiameter(chancePercent: number): number {
  const base =
    moonConfig.BASE_DIAMETER +
    moonConfig.SIZE_PER_PERCENT * chancePercent +
    (Math.random() * 2 - 1) * moonConfig.SIZE_JITTER;
  return Math.max(
    moonConfig.DIAMETER_MIN,
    Math.min(moonConfig.DIAMETER_MAX, Math.round(base))
  );
}
