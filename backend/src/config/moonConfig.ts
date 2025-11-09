/**
 * @module backend/config/moonConfig
 *
 * Moon generation configuration and helper functions. This module contains the
 * numeric tunables for moon chance, sizing and related gameplay costs, plus a
 * few small helper functions used by game logic to derive a moon spawn
 * probability, perform a roll, and to compute an appropriate moon diameter.
 *
 * All values are expressed in integer units unless noted otherwise. The
 * helpers intentionally return primitive values (numbers/booleans) suitable
 * for direct storage or immediate use in game flows.
 */

/**
 * moonConfig
 *
 * Central configuration object for moon-related constants.
 *
 * Keys:
 * - MOONCHANCE_UNIT: base debris amount that yields 1% moon chance
 * - MOONCHANCE_CAP: maximum percentage chance for moon creation
 * - BASE_DIAMETER: baseline moon diameter (units)
 * - SIZE_PER_PERCENT: additional diameter per percent chance
 * - SIZE_JITTER: random jitter applied to diameter (±)
 * - DIAMETER_MIN / DIAMETER_MAX: clamping bounds for diameter
 * - BASE_FIELDS: base number of fields on a moon
 * - FIELDS_PER_LUNAR_BASE: additional fields per lunar base
 * - PHALANX_SCAN_COST: cost to perform a phalanx scan on lunar bodies
 *
 * @constant {Object}
 */
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

/**
 * getMoonChanceFromDebris
 *
 * Calculate the percent chance (integer percent) to form a moon from debris
 * based on the total metal+crystal present in debris fields.
 *
 * Formula:
 *   chance = floor((metal + crystal) / MOONCHANCE_UNIT)
 *   chance is clamped to MOONCHANCE_CAP
 *
 * @param {number} debrisMetal - Amount of metal in debris
 * @param {number} debrisCrystal - Amount of crystal in debris
 * @returns {number} Integer chance percent (0..MOONCHANCE_CAP)
 *
 * @example
 *   getMoonChanceFromDebris(150000, 50000) // returns 2 (percent)
 */
export function getMoonChanceFromDebris(debrisMetal: number, debrisCrystal: number): number {
  const total = debrisMetal + debrisCrystal;
  if (total <= 0) return 0;
  const chance = Math.floor(total / moonConfig.MOONCHANCE_UNIT);
  return Math.min(chance, moonConfig.MOONCHANCE_CAP);
}

/**
 * rollForMoon
 *
 * Perform a single probabilistic roll to determine whether a moon is created
 * given a chance expressed in percent.
 *
 * @param {number} chancePercent - Chance in percent (0-100). Values above
 *   100 are treated as 100% by the calling logic; function treats values >0
 *   as probabilistic.
 * @returns {boolean} True if the roll succeeded (moon spawned), false otherwise
 *
 * Notes:
 * - Uses Math.random(), so results are non-deterministic. For deterministic
 *   testing stub Math.random.
 */
export function rollForMoon(chancePercent: number): boolean {
  if (chancePercent <= 0) return false;
  return Math.random() * 100 < chancePercent;
}

/**
 * calculateMoonDiameter
 *
 * Compute a candidate moon diameter using the configured base diameter,
 * an additive term proportional to the chancePercent, and a small random
 * jitter. The result is clamped between `DIAMETER_MIN` and `DIAMETER_MAX`.
 *
 * @param {number} chancePercent - The percent chance previously computed
 *   (used to bias the diameter; higher chance => larger moon)
 * @returns {number} Clamped moon diameter (integer)
 *
 * Example:
 *   calculateMoonDiameter(5) // might return 3250
 */
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
