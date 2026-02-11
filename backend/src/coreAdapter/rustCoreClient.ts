/**
 * @module coreAdapter/rustCoreClient
 *
 * gRPC client adapter for the Rust `backend-core` service.
 *
 * This module loads the protobuf definition for the core service and exposes
 * a convenience adapter `simulateBattleRust` that accepts either the newer
 * request object shape (compatible with the Rust core's `SimulateBattle`
 * RPC) or a legacy signature `(battleId, playerIds, seed)` used elsewhere in
 * the codebase.
 *
 * Configuration:
 * - `BACKEND_CORE_ADDR` (env): host:port address of the Rust core gRPC server.
 *   Defaults to `backend-core:50051` which is convenient for Docker Compose.
 */

import * as grpc from '@grpc/grpc-js';
import * as protoLoader from '@grpc/proto-loader';
import path from 'path';

export interface RustSimulateRequest {
  battle_id: string;
  attacker_ships: Record<string, number>;
  defender_ships: Record<string, number>;
  defender_defenses: Record<string, number>;
  attacker_tech: Record<string, number>;
  defender_tech: Record<string, number>;
  planet_metal: number;
  planet_crystal: number;
  planet_deuterium: number;
  seed?: string;
  universe?: string;
}

export interface RustShipMovementSpec {
  ship_type: string;
  count: number;
  base_speed: number;
  fuel_consumption: number;
  cargo: number;
}

export interface RustFleetMovementRequest {
  origin_galaxy: number;
  origin_system: number;
  origin_position: number;
  target_galaxy: number;
  target_system: number;
  target_position: number;
  ships: RustShipMovementSpec[];
}

export interface RustFleetMovementResult {
  distance: number;
  fleetSpeed: number;
  travelTimeSeconds: number;
  fuelNeeded: number;
  cargoCapacity: number;
}

interface RustCombatResultRaw {
  winner: 'attacker' | 'defender' | 'draw' | string;
  rounds?: Array<{
    attacker_shots?: number | string;
    defender_shots?: number | string;
    attacker_destroyed?: number | string;
    defender_destroyed?: number | string;
  }>;
  attacker_losses?: Record<string, number | string>;
  defender_losses?: Record<string, number | string>;
  loot?: { metal?: number | string; crystal?: number | string; deuterium?: number | string };
  debris?: { metal?: number | string; crystal?: number | string };
}

const PROTO_PATH = path.join(__dirname, 'proto', 'core.proto');
const packageDefinition = protoLoader.loadSync(PROTO_PATH, {
  keepCase: true,
  longs: String,
  enums: String,
  defaults: true,
  oneofs: true,
});
const proto: any = grpc.loadPackageDefinition(packageDefinition).core;

const addr = process.env.BACKEND_CORE_ADDR || 'backend-core:50051';
const grpcTimeoutMs = Math.max(100, Number(process.env.BACKEND_CORE_TIMEOUT_MS || 2000));
const client = new proto.GameLoop(addr, grpc.credentials.createInsecure(), {
  // Keep channels warm to reduce handshake churn under bursty traffic.
  'grpc.keepalive_time_ms': 20_000,
  'grpc.keepalive_timeout_ms': 5_000,
  'grpc.max_receive_message_length': 8 * 1024 * 1024,
});

const toInt = (value: number | string | undefined): number => {
  if (typeof value === 'number') return Math.trunc(value);
  if (typeof value === 'string') {
    const parsed = parseInt(value, 10);
    return Number.isFinite(parsed) ? parsed : 0;
  }
  return 0;
};

const normalizeNumericMap = (value: Record<string, number | string> | undefined): Record<string, number> => {
  const output: Record<string, number> = {};
  for (const [key, raw] of Object.entries(value || {})) {
    output[key] = toInt(raw);
  }
  return output;
};

const normalizeCombatResult = (raw: RustCombatResultRaw): any => {
  return {
    winner: (raw.winner || 'draw') as 'attacker' | 'defender' | 'draw',
    rounds: (raw.rounds || []).map((round) => ({
      attackerShots: toInt(round.attacker_shots),
      defenderShots: toInt(round.defender_shots),
      attackerDestroyed: toInt(round.attacker_destroyed),
      defenderDestroyed: toInt(round.defender_destroyed),
    })),
    attackerLosses: normalizeNumericMap(raw.attacker_losses),
    defenderLosses: normalizeNumericMap(raw.defender_losses),
    loot: {
      metal: toInt(raw.loot?.metal),
      crystal: toInt(raw.loot?.crystal),
      deuterium: toInt(raw.loot?.deuterium),
    },
    debris: {
      metal: toInt(raw.debris?.metal),
      crystal: toInt(raw.debris?.crystal),
    },
  };
};

const toFloat = (value: number | string | undefined): number => {
  if (typeof value === 'number') return Number.isFinite(value) ? value : 0;
  if (typeof value === 'string') {
    const parsed = parseFloat(value);
    return Number.isFinite(parsed) ? parsed : 0;
  }
  return 0;
};

const getDeadline = (): Date => new Date(Date.now() + grpcTimeoutMs);

/**
 * Simulate a battle using the Rust core service.
 *
 * This function is a flexible adapter that accepts either:
 * - A full request object that matches the gRPC `SimulateBattle` request
 *   schema (recommended), OR
 * - The legacy signature: `(battleId: string, playerIds: string[], seed?: string)`.
 *
 * The adapter normalizes input into a request object and calls the `SimulateBattle`
 * RPC. The RPC response is returned as-is when it appears to be a structured
 * result; if the response contains a legacy `json_result` field we attempt to
 * parse it as JSON and fall back to the raw string on parse errors.
 *
 * @async
 * @param {object|string} arg1 - Either the full request object for `SimulateBattle`,
 *   or the legacy `battleId` string when using the legacy signature.
 * @param {string[]} [arg2] - If using the legacy signature, the array of `playerIds`.
 * @param {string} [arg3] - If using the legacy signature, an optional `seed` value.
 * @returns {Promise<any>} Resolves with the simulation result (structured object
 *   when available, otherwise the parsed `json_result` or raw response).
 * @throws {Error} If the gRPC call returns an error.
 * @example
 * // New style: pass a request object compatible with the proto
 * await simulateBattleRust({ battle_id: 'b123', player_ids: ['p1','p2'], seed: 'xyz' });
 *
 * @example
 * // Legacy style: (battleId, playerIds, seed)
 * await simulateBattleRust('b123', ['p1','p2'], 'xyz');
 */
export function simulateBattleRust(arg1: RustSimulateRequest | string, arg2?: any, arg3?: any): Promise<any> {
  return new Promise((resolve, reject) => {
    let req: RustSimulateRequest;
    if (typeof arg1 === 'string') {
      // legacy: (battleId, playerIds, seed)
      req = {
        battle_id: String(arg1 || ''),
        attacker_ships: {},
        defender_ships: {},
        defender_defenses: {},
        attacker_tech: {},
        defender_tech: {},
        planet_metal: 0,
        planet_crystal: 0,
        planet_deuterium: 0,
        seed: arg3 ? String(arg3) : '',
      };
    } else {
      req = {
        battle_id: arg1.battle_id || 'local',
        attacker_ships: arg1.attacker_ships || {},
        defender_ships: arg1.defender_ships || {},
        defender_defenses: arg1.defender_defenses || {},
        attacker_tech: arg1.attacker_tech || {},
        defender_tech: arg1.defender_tech || {},
        planet_metal: toInt(arg1.planet_metal),
        planet_crystal: toInt(arg1.planet_crystal),
        planet_deuterium: toInt(arg1.planet_deuterium),
        seed: arg1.seed || '',
        universe: arg1.universe || 'default',
      };
    }

    client.SimulateBattle(req, { deadline: getDeadline() }, (err: any, res: any) => {
      if (err) return reject(err);
      if (res && (res.winner || res.winner === 'draw')) {
        return resolve(normalizeCombatResult(res as RustCombatResultRaw));
      }
      // Fallback: try parsing legacy json_result field
      if (res && res.json_result) {
        try {
          const parsed = JSON.parse(res.json_result) as RustCombatResultRaw;
          return resolve(normalizeCombatResult(parsed));
        } catch (error) {
          return resolve(res.json_result);
        }
      }
      return resolve(res);
    });
  });
}

export function calculateFleetMovementRust(
  request: RustFleetMovementRequest
): Promise<RustFleetMovementResult> {
  return new Promise((resolve, reject) => {
    client.CalculateFleetMovement(request, { deadline: getDeadline() }, (err: any, res: any) => {
      if (err) return reject(err);
      return resolve({
        distance: toInt(res?.distance),
        fleetSpeed: toFloat(res?.fleet_speed),
        travelTimeSeconds: toInt(res?.travel_time_seconds),
        fuelNeeded: toFloat(res?.fuel_needed),
        cargoCapacity: toFloat(res?.cargo_capacity),
      });
    });
  });
}
