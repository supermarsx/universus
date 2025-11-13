import grpc from '@grpc/grpc-js';
import protoLoader from '@grpc/proto-loader';
import path from 'path';

const PROTO_PATH = path.join(__dirname, '../../backend-core/proto/core.proto');
const packageDefinition = protoLoader.loadSync(PROTO_PATH, { keepCase: true, longs: String, enums: String, defaults: true, oneofs: true });
const proto: any = grpc.loadPackageDefinition(packageDefinition).core;

const addr = process.env.BACKEND_CORE_ADDR || 'backend-core:50051';
const client = new proto.GameLoop(addr, grpc.credentials.createInsecure());

// Flexible adapter: accepts either the legacy signature
// (battleId: string, playerIds: string[], seed?: string)
// or a full SimulateRequest-like object.
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
