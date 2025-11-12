import grpc from '@grpc/grpc-js';
import protoLoader from '@grpc/proto-loader';
import path from 'path';

const PROTO_PATH = path.join(__dirname, '../../backend-core/proto/core.proto');
const packageDefinition = protoLoader.loadSync(PROTO_PATH, { keepCase: true, longs: String, enums: String, defaults: true, oneofs: true });
const proto: any = grpc.loadPackageDefinition(packageDefinition).core;

const addr = process.env.BACKEND_CORE_ADDR || 'backend-core:50051';
const client = new proto.GameLoop(addr, grpc.credentials.createInsecure());

export function simulateBattleRust(battleId: string, playerIds: string[], seed?: string): Promise<any> {
  return new Promise((resolve, reject) => {
    const req = { battle_id: battleId, player_ids: playerIds, seed: seed || '' };
    client.SimulateBattle(req, (err: any, res: any) => {
      if (err) return reject(err);
      try {
        const parsed = JSON.parse(res.json_result);
        resolve(parsed);
      } catch (error) {
        resolve(res.json_result);
      }
    });
  });
}
