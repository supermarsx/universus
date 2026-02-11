/* eslint-disable no-console */
import path from 'path';
import { calculateFleetMovementRust } from '../src/coreAdapter/rustCoreClient';
import { calculateFleetMovementNapi } from '../src/coreAdapter/rustCoreNapiClient';

type ShipSpec = {
  ship_type: string;
  count: number;
  base_speed: number;
  fuel_consumption: number;
  cargo: number;
};

type BenchmarkCase = {
  name: string;
  run: (iteration: number) => Promise<void>;
};

const toMs = (ns: bigint): number => Number(ns) / 1_000_000;

const percentile = (values: number[], p: number): number => {
  if (!values.length) return 0;
  const sorted = [...values].sort((a, b) => a - b);
  const index = Math.min(sorted.length - 1, Math.max(0, Math.floor((p / 100) * sorted.length)));
  return sorted[index];
};

const mean = (values: number[]): number =>
  values.length ? values.reduce((sum, value) => sum + value, 0) / values.length : 0;

const minMax = (values: number[]): { min: number; max: number } => {
  if (!values.length) return { min: 0, max: 0 };
  let min = values[0];
  let max = values[0];
  for (let i = 1; i < values.length; i++) {
    const value = values[i];
    if (value < min) min = value;
    if (value > max) max = value;
  }
  return { min, max };
};

const buildMovementRequest = (iteration: number) => ({
  origin_galaxy: 1,
  origin_system: 120 + (iteration % 3),
  origin_position: 8,
  target_galaxy: 1,
  target_system: 360 + (iteration % 5),
  target_position: 12,
  ships: [
    { ship_type: 'small_cargo', count: 120, base_speed: 5000, fuel_consumption: 10, cargo: 5000 },
    { ship_type: 'large_cargo', count: 40, base_speed: 7500, fuel_consumption: 50, cargo: 25000 },
    { ship_type: 'light_fighter', count: 60, base_speed: 12500, fuel_consumption: 20, cargo: 50 },
    { ship_type: 'cruiser', count: 12, base_speed: 15000, fuel_consumption: 300, cargo: 800 },
    { ship_type: 'battleship', count: 8, base_speed: 10000, fuel_consumption: 500, cargo: 1500 },
  ] as ShipSpec[],
});

const calculateFleetMovementTs = (request: ReturnType<typeof buildMovementRequest>) => {
  const distance =
    request.origin_galaxy !== request.target_galaxy
      ? Math.abs(request.origin_galaxy - request.target_galaxy) * 20000
      : request.origin_system !== request.target_system
      ? Math.abs(request.origin_system - request.target_system) * 5 * 19 + 2700
      : Math.abs(request.origin_position - request.target_position) * 5 + 1000;

  let minSpeed = Number.POSITIVE_INFINITY;
  let fuelNeeded = 0;
  let cargoCapacity = 0;
  for (const ship of request.ships) {
    if (ship.count <= 0) continue;
    if (ship.base_speed > 0) {
      minSpeed = Math.min(minSpeed, ship.base_speed);
    }
    fuelNeeded += ship.fuel_consumption * ship.count * (distance / 100);
    cargoCapacity += ship.cargo * ship.count;
  }

  const fleetSpeed = Number.isFinite(minSpeed) ? minSpeed : 0;
  const travelTimeSeconds = fleetSpeed > 0 ? Math.ceil((distance / fleetSpeed) * 3600) : 0;
  cargoCapacity -= fuelNeeded;
  return { distance, fleetSpeed, travelTimeSeconds, fuelNeeded, cargoCapacity };
};

const benchmark = async (
  testCase: BenchmarkCase,
  warmup: number,
  iterations: number
): Promise<{ ok: true; samples: number[] } | { ok: false; error: string }> => {
  try {
    for (let i = 0; i < warmup; i++) {
      await testCase.run(i);
    }
  } catch (error: any) {
    return { ok: false, error: error?.message || String(error) };
  }

  const samples: number[] = [];
  for (let i = 0; i < iterations; i++) {
    const start = process.hrtime.bigint();
    await testCase.run(i + warmup);
    const end = process.hrtime.bigint();
    samples.push(toMs(end - start));
  }
  return { ok: true, samples };
};

async function main() {
  const iterations = Math.max(20, Number(process.env.BENCH_ITERATIONS || 150));
  const warmup = Math.max(5, Number(process.env.BENCH_WARMUP || 25));

  const tsCase: BenchmarkCase = {
    name: 'ts',
    run: async (iteration) => {
      calculateFleetMovementTs(buildMovementRequest(iteration));
    },
  };

  const grpcCase: BenchmarkCase = {
    name: 'grpc',
    run: async (iteration) => {
      await calculateFleetMovementRust(buildMovementRequest(iteration));
    },
  };

  const napiCase: BenchmarkCase = {
    name: 'napi',
    run: async (iteration) => {
      await calculateFleetMovementNapi(buildMovementRequest(iteration));
    },
  };

  const suite = [tsCase, grpcCase, napiCase];

  console.log('Core Transport Benchmark (Fleet Movement)');
  console.log(`iterations: ${iterations}`);
  console.log(`warmup: ${warmup}`);
  console.log(`actions per transport: ${iterations}`);
  console.log(`grpc target: ${process.env.BACKEND_CORE_ADDR || 'backend-core:50051'}`);
  console.log(`napi binding path: ${process.env.CORE_NAPI_BINDING_PATH || '(auto-detect)'}`);
  console.log('');

  for (const testCase of suite) {
    const result = await benchmark(testCase, warmup, iterations);
    if (!result.ok) {
      console.log(`${testCase.name}: skipped (${result.error})`);
      continue;
    }

    const p50 = percentile(result.samples, 50);
    const p95 = percentile(result.samples, 95);
    const p99 = percentile(result.samples, 99);
    const avg = mean(result.samples);
    const { min, max } = minMax(result.samples);

    console.log(
      `${testCase.name}: p50=${p50.toFixed(3)}ms p95=${p95.toFixed(3)}ms p99=${p99.toFixed(3)}ms avg=${avg.toFixed(
        3
      )}ms min=${min.toFixed(3)}ms max=${max.toFixed(3)}ms`
    );
  }

  console.log('');
  console.log('Tip: start backend-core for gRPC and set CORE_NAPI_BINDING_PATH for N-API.');
  console.log(`Example N-API binding: ${path.resolve(process.cwd(), '..', 'backend-core-napi', 'index.node')}`);
}

main().catch((error) => {
  console.error('Benchmark failed:', error);
  process.exitCode = 1;
});
