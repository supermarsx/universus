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
