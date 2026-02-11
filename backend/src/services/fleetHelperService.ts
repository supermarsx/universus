import { SHIPS } from '../config/gameConfig';
import type { RustFleetMovementRequest } from '../coreAdapter/rustCoreClient';
import {
  calculateFleetMovementNapi,
  computeAttackerPostCombatDistributionNapi,
  resolveDefenseLossesNapi,
} from '../coreAdapter/rustCoreNapiClient';

export type EngineSource = 'rust-napi' | 'typescript';

export interface FleetCoordinates {
  galaxy: number;
  system: number;
  position: number;
}

export interface FleetMovementInput {
  origin: FleetCoordinates;
  target: FleetCoordinates;
  ships: Record<string, number>;
}

export interface FleetMovementOutput {
  distance: number;
  fleetSpeed: number;
  travelTimeSeconds: number;
  fuelNeeded: number;
  cargoCapacity: number;
  engine: EngineSource;
}

export interface DefenseRebuildInput {
  current: Record<string, number>;
  losses: Record<string, number>;
  rebuildRate?: number;
  seed?: string;
}

export interface DefenseRebuildOutput {
  updated: Record<string, number>;
  engine: EngineSource;
}

export interface CombatLoot {
  metal: number;
  crystal: number;
  deuterium: number;
}

export interface CombatDistributionInput {
  participants: Array<Record<string, number>>;
  totalLosses: Record<string, number>;
  loot: CombatLoot;
  winner: 'attacker' | 'defender' | 'draw';
}

export interface CombatDistributionOutput {
  participants: Array<{
    survivors: Record<string, number>;
    loot: CombatLoot;
  }>;
  engine: EngineSource;
}

function calculateDistance(origin: FleetCoordinates, target: FleetCoordinates): number {
  if (origin.galaxy !== target.galaxy) {
    return Math.abs(origin.galaxy - target.galaxy) * 20000;
  }
  if (origin.system !== target.system) {
    return Math.abs(origin.system - target.system) * 5 * 19 + 2700;
  }
  return Math.abs(origin.position - target.position) * 5 + 1000;
}

function calculateFleetSpeed(ships: Record<string, number>): number {
  let minSpeed = Number.POSITIVE_INFINITY;

  Object.entries(ships).forEach(([shipType, count]) => {
    if (count <= 0) return;
    const shipConfig = SHIPS[shipType];
    if (!shipConfig) return;
    minSpeed = Math.min(minSpeed, Number(shipConfig.baseSpeed) || 0);
  });

  return Number.isFinite(minSpeed) ? minSpeed : 0;
}

function buildRustMovementRequest(input: FleetMovementInput): RustFleetMovementRequest {
  const ships = Object.entries(input.ships)
    .filter(([, count]) => Number(count) > 0)
    .map(([shipType, count]) => {
      const shipConfig = SHIPS[shipType];
      return {
        ship_type: shipType,
        count: Math.max(0, Math.trunc(Number(count) || 0)),
        base_speed: Number(shipConfig?.baseSpeed || 0),
        fuel_consumption: Number(shipConfig?.fuelConsumption || 0),
        cargo: Number(shipConfig?.cargo || 0),
      };
    });

  return {
    origin_galaxy: input.origin.galaxy,
    origin_system: input.origin.system,
    origin_position: input.origin.position,
    target_galaxy: input.target.galaxy,
    target_system: input.target.system,
    target_position: input.target.position,
    ships,
  };
}

function movementFallback(input: FleetMovementInput): FleetMovementOutput {
  const distance = calculateDistance(input.origin, input.target);
  const speed = calculateFleetSpeed(input.ships);

  let fuelNeeded = 0;
  let cargoCapacity = 0;
  Object.entries(input.ships).forEach(([shipType, count]) => {
    const shipConfig = SHIPS[shipType];
    if (!shipConfig) return;
    const normalizedCount = Math.max(0, Number(count) || 0);
    fuelNeeded += Number(shipConfig.fuelConsumption || 0) * normalizedCount * (distance / 100);
    cargoCapacity += Number(shipConfig.cargo || 0) * normalizedCount;
  });

  return {
    distance,
    fleetSpeed: speed,
    travelTimeSeconds: speed > 0 ? Math.ceil((distance / speed) * 3600) : 0,
    fuelNeeded,
    cargoCapacity: cargoCapacity - fuelNeeded,
    engine: 'typescript',
  };
}

function mulberry32(seed: number): () => number {
  return function next(): number {
    let t = (seed += 0x6d2b79f5);
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function hashSeed(seed: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < seed.length; i++) {
    h ^= seed.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return h || 1;
}

function defenseRebuildFallback(input: DefenseRebuildInput): DefenseRebuildOutput {
  const rebuildRate = Math.min(1, Math.max(0, Number(input.rebuildRate ?? 0.7)));
  const rng = mulberry32(hashSeed(input.seed || 'defense-loss'));
  const updated: Record<string, number> = {};

  Object.entries(input.current || {}).forEach(([unit, currentRaw]) => {
    const current = Math.max(0, Math.trunc(Number(currentRaw) || 0));
    const loss = Math.max(0, Math.trunc(Number(input.losses?.[unit]) || 0));
    if (loss <= 0) {
      updated[unit] = current;
      return;
    }

    let rebuilt = 0;
    for (let i = 0; i < loss; i++) {
      if (rng() < rebuildRate) rebuilt++;
    }

    const effectiveLoss = Math.max(0, loss - rebuilt);
    updated[unit] = Math.max(0, current - effectiveLoss);
  });

  return {
    updated,
    engine: 'typescript',
  };
}

function allocateLosses(
  participants: Array<Record<string, number>>,
  totalLosses: Record<string, number>
): Array<Record<string, number>> {
  const allocations = participants.map(() => ({} as Record<string, number>));
  const allocated: Record<string, number> = {};
  const totals: Record<string, number> = {};

  participants.forEach((participant) => {
    Object.entries(participant || {}).forEach(([type, countRaw]) => {
      totals[type] = (totals[type] || 0) + Math.max(0, Math.trunc(Number(countRaw) || 0));
    });
  });

  participants.forEach((participant, index) => {
    Object.entries(totalLosses || {}).forEach(([type, lossRaw]) => {
      const totalLoss = Math.max(0, Math.trunc(Number(lossRaw) || 0));
      const fleetCount = Math.max(0, Math.trunc(Number(participant?.[type]) || 0));
      const totalCount = Math.max(0, Math.trunc(Number(totals[type]) || 0));

      if (!totalLoss || !fleetCount || !totalCount) {
        allocations[index][type] = 0;
        return;
      }

      if (index === participants.length - 1) {
        const remaining = totalLoss - (allocated[type] || 0);
        allocations[index][type] = Math.min(fleetCount, Math.max(remaining, 0));
        return;
      }

      const proportional = Math.round((totalLoss * fleetCount) / totalCount);
      const clamped = Math.min(fleetCount, Math.max(proportional, 0));
      allocations[index][type] = clamped;
      allocated[type] = (allocated[type] || 0) + clamped;
    });
  });

  return allocations;
}

function splitLoot(loot: CombatLoot, parts: number): CombatLoot[] {
  if (parts <= 0) return [];
  const shares = Array.from({ length: parts }, () => ({ metal: 0, crystal: 0, deuterium: 0 }));

  (['metal', 'crystal', 'deuterium'] as const).forEach((resource) => {
    let remaining = Math.max(0, Math.trunc(Number(loot[resource]) || 0));
    for (let i = 0; i < parts; i++) {
      const value = Math.floor(remaining / (parts - i));
      shares[i][resource] = value;
      remaining -= value;
    }
  });

  return shares;
}

function combatDistributionFallback(input: CombatDistributionInput): CombatDistributionOutput {
  const lossAllocations = allocateLosses(input.participants || [], input.totalLosses || {});
  const lootPool = input.winner === 'attacker' ? input.loot : { metal: 0, crystal: 0, deuterium: 0 };
  const lootShares = splitLoot(lootPool, input.participants?.length || 0);

  return {
    participants: (input.participants || []).map((participant, index) => {
      const losses = lossAllocations[index] || {};
      const survivors: Record<string, number> = {};
      Object.entries(participant || {}).forEach(([type, countRaw]) => {
        const count = Math.max(0, Math.trunc(Number(countRaw) || 0));
        const loss = Math.max(0, Math.trunc(Number(losses[type]) || 0));
        const remaining = Math.max(0, count - loss);
        if (remaining > 0) {
          survivors[type] = remaining;
        }
      });

      return {
        survivors,
        loot: lootShares[index] || { metal: 0, crystal: 0, deuterium: 0 },
      };
    }),
    engine: 'typescript',
  };
}

export class FleetHelperService {
  static async calculateMovement(input: FleetMovementInput): Promise<FleetMovementOutput> {
    try {
      const result = await calculateFleetMovementNapi(buildRustMovementRequest(input));
      return {
        distance: Number(result.distance || 0),
        fleetSpeed: Number(result.fleetSpeed || 0),
        travelTimeSeconds: Number(result.travelTimeSeconds || 0),
        fuelNeeded: Number(result.fuelNeeded || 0),
        cargoCapacity: Number(result.cargoCapacity || 0),
        engine: 'rust-napi',
      };
    } catch {
      return movementFallback(input);
    }
  }

  static async resolveDefenseRebuild(input: DefenseRebuildInput): Promise<DefenseRebuildOutput> {
    try {
      const response = await resolveDefenseLossesNapi({
        current: input.current,
        losses: input.losses,
        rebuild_rate: input.rebuildRate,
        seed: input.seed,
      });
      return {
        updated: response.updated || {},
        engine: 'rust-napi',
      };
    } catch {
      return defenseRebuildFallback(input);
    }
  }

  static async computeAttackerDistribution(
    input: CombatDistributionInput
  ): Promise<CombatDistributionOutput> {
    try {
      const response = await computeAttackerPostCombatDistributionNapi({
        participants: input.participants,
        total_losses: input.totalLosses,
        loot: input.loot,
        winner: input.winner,
      });
      return {
        participants: response.participants || [],
        engine: 'rust-napi',
      };
    } catch {
      return combatDistributionFallback(input);
    }
  }
}
