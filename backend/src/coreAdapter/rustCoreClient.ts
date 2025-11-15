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

import grpc from '@grpc/grpc-js';
import protoLoader from '@grpc/proto-loader';
import path from 'path';

const PROTO_PATH = path.join(__dirname, '../../backend-core/proto/core.proto');
const packageDefinition = protoLoader.loadSync(PROTO_PATH, { keepCase: true, longs: String, enums: String, defaults: true, oneofs: true });
const proto: any = grpc.loadPackageDefinition(packageDefinition).core;

const addr = process.env.BACKEND_CORE_ADDR || 'backend-core:50051';
const client = new proto.GameLoop(addr, grpc.credentials.createInsecure());

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
export function simulateBattleRust(arg1: any, arg2?: any, arg3?: any): Promise<any> {
  return new Promise((resolve, reject) => {
    let req: any = {};
    if (typeof arg1 === 'object' && arg1 !== null && !Array.isArray(arg1)) {
      req = arg1;
    } else {
      // legacy: (battleId, playerIds, seed)
      req = { battle_id: String(arg1 || ''), player_ids: Array.isArray(arg2) ? arg2 : [], seed: arg3 ? String(arg3) : '' };
    }

    client.SimulateBattle(req, (err: any, res: any) => {
      if (err) return reject(err);
      // If the Rust core returns a structured CombatResult, use it directly
      if (res && (res.winner || res.winner === '')) {
        return resolve(res);
      }
      // Fallback: try parsing legacy json_result field
      if (res && res.json_result) {
        try {
          const parsed = JSON.parse(res.json_result);
          return resolve(parsed);
        } catch (error) {
          return resolve(res.json_result);
        }
      }
      return resolve(res);
    });
  });
}
