import path from 'path';
import type {
  RustFleetMovementRequest,
  RustFleetMovementResult,
  RustSimulateRequest,
} from './rustCoreClient';

type RustNapiBinding = {
  simulateBattle?: (payload: string) => string;
  simulate_battle?: (payload: string) => string;
  calculateFleetMovement?: (payload: string) => string;
  calculate_fleet_movement?: (payload: string) => string;
  resolveDefenseLosses?: (payload: string) => string;
  resolve_defense_losses?: (payload: string) => string;
  calculateFleetMovementFast?: (payload: RustFleetMovementRequest) => {
    distance: number;
    fleet_speed?: number;
    fleetSpeed?: number;
    travel_time_seconds?: number;
    travelTimeSeconds?: number;
    fuel_needed?: number;
    fuelNeeded?: number;
    cargo_capacity?: number;
    cargoCapacity?: number;
  };
  calculate_fleet_movement_fast?: (payload: RustFleetMovementRequest) => {
    distance: number;
    fleet_speed?: number;
    fleetSpeed?: number;
    travel_time_seconds?: number;
    travelTimeSeconds?: number;
    fuel_needed?: number;
    fuelNeeded?: number;
    cargo_capacity?: number;
    cargoCapacity?: number;
  };
  calculateFleetMovementBatch?: (payload: RustFleetMovementRequest[]) => Array<{
    distance: number;
    fleet_speed?: number;
    fleetSpeed?: number;
    travel_time_seconds?: number;
    travelTimeSeconds?: number;
    fuel_needed?: number;
    fuelNeeded?: number;
    cargo_capacity?: number;
    cargoCapacity?: number;
  }>;
  calculate_fleet_movement_batch?: (payload: RustFleetMovementRequest[]) => Array<{
    distance: number;
    fleet_speed?: number;
    fleetSpeed?: number;
    travel_time_seconds?: number;
    travelTimeSeconds?: number;
    fuel_needed?: number;
    fuelNeeded?: number;
    cargo_capacity?: number;
    cargoCapacity?: number;
  }>;
};

let bindingState: RustNapiBinding | null | undefined;

const tryLoad = (modulePath: string): RustNapiBinding | null => {
  try {
    // eslint-disable-next-line @typescript-eslint/no-var-requires
    return require(modulePath) as RustNapiBinding;
  } catch {
    return null;
  }
};

const getBinding = (): RustNapiBinding => {
  if (bindingState !== undefined) {
    if (bindingState) return bindingState;
    throw new Error('Rust N-API binding not available');
  }

  const candidates = [
    process.env.CORE_NAPI_BINDING_PATH,
    path.join(process.cwd(), 'backend-core-napi', 'index.node'),
    path.join(__dirname, '..', '..', '..', 'backend-core-napi', 'index.node'),
    'backend-core-napi',
  ].filter((entry): entry is string => Boolean(entry));

  for (const candidate of candidates) {
    const loaded = tryLoad(candidate);
    if (loaded) {
      bindingState = loaded;
      return loaded;
    }
  }

  bindingState = null;
  throw new Error('Rust N-API binding not found; set CORE_NAPI_BINDING_PATH or build backend-core-napi');
};

export function isNapiAvailable(): boolean {
  try {
    getBinding();
    return true;
  } catch {
    return false;
  }
}

const parseJson = <T>(raw: string): T => {
  return JSON.parse(raw) as T;
};

export async function simulateBattleNapi(request: RustSimulateRequest): Promise<any> {
  const binding = getBinding();
  const fn = binding.simulateBattle || binding.simulate_battle;
  if (!fn) {
    throw new Error('Rust N-API function simulateBattle not exported');
  }
  const raw = fn(JSON.stringify(request));
  return parseJson<any>(raw);
}

export async function calculateFleetMovementNapi(
  request: RustFleetMovementRequest
): Promise<RustFleetMovementResult> {
  const binding = getBinding();
  const fastFn = binding.calculateFleetMovementFast || binding.calculate_fleet_movement_fast;
  if (fastFn) {
    const fastRequest = {
      originGalaxy: request.origin_galaxy,
      originSystem: request.origin_system,
      originPosition: request.origin_position,
      targetGalaxy: request.target_galaxy,
      targetSystem: request.target_system,
      targetPosition: request.target_position,
      ships: request.ships.map((ship) => ({
        count: ship.count,
        baseSpeed: ship.base_speed,
        fuelConsumption: ship.fuel_consumption,
        cargo: ship.cargo,
      })),
    };
    const result = fastFn(fastRequest as any);
    return {
      distance: Number(result.distance || 0),
      fleetSpeed: Number(result.fleetSpeed ?? result.fleet_speed ?? 0),
      travelTimeSeconds: Number(result.travelTimeSeconds ?? result.travel_time_seconds ?? 0),
      fuelNeeded: Number(result.fuelNeeded ?? result.fuel_needed ?? 0),
      cargoCapacity: Number(result.cargoCapacity ?? result.cargo_capacity ?? 0),
    };
  }

  const fn = binding.calculateFleetMovement || binding.calculate_fleet_movement;
  if (!fn) {
    throw new Error('Rust N-API function calculateFleetMovement not exported');
  }

  const raw = fn(JSON.stringify(request));
  const result = parseJson<{
    distance: number;
    fleetSpeed?: number;
    fleet_speed?: number;
    travelTimeSeconds?: number;
    travel_time_seconds?: number;
    fuelNeeded?: number;
    fuel_needed?: number;
    cargoCapacity?: number;
    cargo_capacity?: number;
  }>(raw);

  return {
    distance: Number(result.distance || 0),
    fleetSpeed: Number(result.fleetSpeed ?? result.fleet_speed ?? 0),
    travelTimeSeconds: Number(result.travelTimeSeconds ?? result.travel_time_seconds ?? 0),
    fuelNeeded: Number(result.fuelNeeded ?? result.fuel_needed ?? 0),
    cargoCapacity: Number(result.cargoCapacity ?? result.cargo_capacity ?? 0),
  };
}

export async function calculateFleetMovementNapiLegacyJson(
  request: RustFleetMovementRequest
): Promise<RustFleetMovementResult> {
  const binding = getBinding();
  const fn = binding.calculateFleetMovement || binding.calculate_fleet_movement;
  if (!fn) {
    throw new Error('Rust N-API function calculateFleetMovement not exported');
  }

  const raw = fn(JSON.stringify(request));
  const result = parseJson<{
    distance: number;
    fleetSpeed?: number;
    fleet_speed?: number;
    travelTimeSeconds?: number;
    travel_time_seconds?: number;
    fuelNeeded?: number;
    fuel_needed?: number;
    cargoCapacity?: number;
    cargo_capacity?: number;
  }>(raw);

  return {
    distance: Number(result.distance || 0),
    fleetSpeed: Number(result.fleetSpeed ?? result.fleet_speed ?? 0),
    travelTimeSeconds: Number(result.travelTimeSeconds ?? result.travel_time_seconds ?? 0),
    fuelNeeded: Number(result.fuelNeeded ?? result.fuel_needed ?? 0),
    cargoCapacity: Number(result.cargoCapacity ?? result.cargo_capacity ?? 0),
  };
}

export async function calculateFleetMovementBatchNapi(
  requests: RustFleetMovementRequest[]
): Promise<RustFleetMovementResult[]> {
  const binding = getBinding();
  const fn = binding.calculateFleetMovementBatch || binding.calculate_fleet_movement_batch;
  if (!fn) {
    return Promise.all(requests.map((request) => calculateFleetMovementNapi(request)));
  }

  const fastRequests = requests.map((request) => ({
    originGalaxy: request.origin_galaxy,
    originSystem: request.origin_system,
    originPosition: request.origin_position,
    targetGalaxy: request.target_galaxy,
    targetSystem: request.target_system,
    targetPosition: request.target_position,
    ships: request.ships.map((ship) => ({
      count: ship.count,
      baseSpeed: ship.base_speed,
      fuelConsumption: ship.fuel_consumption,
      cargo: ship.cargo,
    })),
  }));

  const rows = fn(fastRequests as any);
  return rows.map((result) => ({
    distance: Number(result.distance || 0),
    fleetSpeed: Number(result.fleetSpeed ?? result.fleet_speed ?? 0),
    travelTimeSeconds: Number(result.travelTimeSeconds ?? result.travel_time_seconds ?? 0),
    fuelNeeded: Number(result.fuelNeeded ?? result.fuel_needed ?? 0),
    cargoCapacity: Number(result.cargoCapacity ?? result.cargo_capacity ?? 0),
  }));
}

export async function resolveDefenseLossesNapi(payload: {
  current: Record<string, number>;
  losses: Record<string, number>;
  rebuild_rate?: number;
  seed?: string;
}): Promise<{ updated: Record<string, number> }> {
  const binding = getBinding();
  const fn = binding.resolveDefenseLosses || binding.resolve_defense_losses;
  if (!fn) {
    throw new Error('Rust N-API function resolveDefenseLosses not exported');
  }
  const raw = fn(JSON.stringify(payload));
  const parsed = parseJson<{ updated?: Record<string, number> }>(raw);
  return {
    updated: parsed.updated || {},
  };
}
