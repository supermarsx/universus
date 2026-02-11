/* eslint-disable no-console */
import path from 'path';
import fs from 'fs';
import { execSync } from 'child_process';
import { calculateFleetMovementRust } from '../src/coreAdapter/rustCoreClient';
import {
  calculateFleetMovementBatchNapi,
  calculateFleetMovementByTypeNapi,
  calculateFleetMovementNapi,
  isNapiAvailable,
} from '../src/coreAdapter/rustCoreNapiClient';

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
  actionsPerRun?: number;
};

type MemoryUsageBytes = {
  rss: number;
  heapUsed: number;
  heapTotal: number;
  external: number;
  arrayBuffers: number;
};

type MemorySummary = {
  name: string;
  totalMs: number;
  opsPerSec: number;
  baselineMb: {
    rss: number;
    heapUsed: number;
    heapTotal: number;
    external: number;
    arrayBuffers: number;
  };
  finalMb: {
    rss: number;
    heapUsed: number;
    heapTotal: number;
    external: number;
    arrayBuffers: number;
  };
  deltaMb: {
    rss: number;
    heapUsed: number;
    heapTotal: number;
    external: number;
    arrayBuffers: number;
  };
  peakMb: {
    rss: number;
    heapUsed: number;
    heapTotal: number;
    external: number;
    arrayBuffers: number;
  };
};

const toMs = (ns: bigint): number => Number(ns) / 1_000_000;
const toMb = (bytes: number): number => bytes / (1024 * 1024);
const round = (n: number): number => Math.round(n * 1000) / 1000;

const snapshotMemory = (): MemoryUsageBytes => {
  const usage = process.memoryUsage();
  return {
    rss: usage.rss,
    heapUsed: usage.heapUsed,
    heapTotal: usage.heapTotal,
    external: usage.external,
    arrayBuffers: usage.arrayBuffers,
  };
};

const maxMemory = (a: MemoryUsageBytes, b: MemoryUsageBytes): MemoryUsageBytes => ({
  rss: Math.max(a.rss, b.rss),
  heapUsed: Math.max(a.heapUsed, b.heapUsed),
  heapTotal: Math.max(a.heapTotal, b.heapTotal),
  external: Math.max(a.external, b.external),
  arrayBuffers: Math.max(a.arrayBuffers, b.arrayBuffers),
});

const toMemoryMb = (usage: MemoryUsageBytes) => ({
  rss: round(toMb(usage.rss)),
  heapUsed: round(toMb(usage.heapUsed)),
  heapTotal: round(toMb(usage.heapTotal)),
  external: round(toMb(usage.external)),
  arrayBuffers: round(toMb(usage.arrayBuffers)),
});

const deltaMemoryMb = (start: MemoryUsageBytes, end: MemoryUsageBytes) => ({
  rss: round(toMb(end.rss - start.rss)),
  heapUsed: round(toMb(end.heapUsed - start.heapUsed)),
  heapTotal: round(toMb(end.heapTotal - start.heapTotal)),
  external: round(toMb(end.external - start.external)),
  arrayBuffers: round(toMb(end.arrayBuffers - start.arrayBuffers)),
});

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

const maybeGc = () => {
  const gc = (global as any).gc;
  if (typeof gc === 'function') {
    gc();
  }
};

const benchmarkMemory = async (
  testCase: BenchmarkCase,
  warmup: number,
  iterations: number,
  sampleEvery: number
): Promise<{ ok: true; summary: MemorySummary } | { ok: false; error: string }> => {
  try {
    for (let i = 0; i < warmup; i++) {
      await testCase.run(i);
    }
  } catch (error: any) {
    return { ok: false, error: error?.message || String(error) };
  }

  maybeGc();
  const baseline = snapshotMemory();
  let peak = baseline;

  const start = process.hrtime.bigint();
  for (let i = 0; i < iterations; i++) {
    await testCase.run(i + warmup);
    if ((i + 1) % sampleEvery === 0) {
      peak = maxMemory(peak, snapshotMemory());
    }
  }
  const end = process.hrtime.bigint();

  maybeGc();
  const final = snapshotMemory();
  peak = maxMemory(peak, final);

  const totalMs = toMs(end - start);
  const actionsPerRun = testCase.actionsPerRun || 1;
  const totalActions = iterations * actionsPerRun;
  const opsPerSec = totalMs > 0 ? totalActions / (totalMs / 1000) : 0;

  return {
    ok: true,
    summary: {
      name: testCase.name,
      totalMs: round(totalMs),
      opsPerSec: round(opsPerSec),
      baselineMb: toMemoryMb(baseline),
      finalMb: toMemoryMb(final),
      deltaMb: deltaMemoryMb(baseline, final),
      peakMb: toMemoryMb(peak),
    },
  };
};

async function main() {
  const iterations = Math.max(50, Number(process.env.BENCH_ITERATIONS || 1000));
  const warmup = Math.max(10, Number(process.env.BENCH_WARMUP || 50));
  const sampleEvery = Math.max(1, Number(process.env.BENCH_MEMORY_SAMPLE_EVERY || 25));

  const suite: BenchmarkCase[] = [
    {
      name: 'node_ts_local',
      run: async (iteration) => {
        calculateFleetMovementTs(buildMovementRequest(iteration));
      },
    },
    {
      name: 'rust_napi_by_type',
      run: async (iteration) => {
        await calculateFleetMovementByTypeNapi(buildMovementRequest(iteration));
      },
    },
    {
      name: 'rust_napi_fast',
      run: async (iteration) => {
        await calculateFleetMovementNapi(buildMovementRequest(iteration));
      },
    },
    {
      name: 'rust_napi_batch_x256',
      actionsPerRun: 256,
      run: async (iteration) => {
        const batchSize = 256;
        const batch = Array.from({ length: batchSize }, (_, offset) => buildMovementRequest(iteration * batchSize + offset));
        await calculateFleetMovementBatchNapi(batch);
      },
    },
    {
      name: 'rust_grpc',
      run: async (iteration) => {
        await calculateFleetMovementRust(buildMovementRequest(iteration));
      },
    },
  ];

  console.log('Core Memory Benchmark (Rust vs Node movement paths)');
  console.log(`iterations: ${iterations}`);
  console.log(`warmup: ${warmup}`);
  console.log(`sampleEvery: ${sampleEvery}`);
  console.log(`grpc target: ${process.env.BACKEND_CORE_ADDR || 'backend-core:50051'}`);
  console.log(`napi binding path: ${process.env.CORE_NAPI_BINDING_PATH || '(auto-detect)'}`);
  console.log(`napi available: ${isNapiAvailable()}`);
  console.log(`gc exposed: ${typeof (global as any).gc === 'function'}`);
  console.log('');

  const summaries: MemorySummary[] = [];
  for (const testCase of suite) {
    const result = await benchmarkMemory(testCase, warmup, iterations, sampleEvery);
    if (!result.ok) {
      console.log(`${testCase.name}: skipped (${result.error})`);
      continue;
    }
    summaries.push(result.summary);
    const s = result.summary;
    console.log(
      `${s.name}: total=${s.totalMs}ms ops/s=${s.opsPerSec} delta(rss=${s.deltaMb.rss}MB heap=${s.deltaMb.heapUsed}MB) peak(rss=${s.peakMb.rss}MB heap=${s.peakMb.heapUsed}MB)`
    );
  }

  const saveDir = process.env.BENCH_SAVE_DIR || path.resolve(process.cwd(), 'benchmarks', 'history');
  fs.mkdirSync(saveDir, { recursive: true });
  const now = new Date();
  const timestamp = now.toISOString().replace(/[:.]/g, '-');
  let gitCommit = process.env.GIT_COMMIT || 'unknown';
  try {
    gitCommit = execSync('git rev-parse --short HEAD', { cwd: path.resolve(process.cwd(), '..') })
      .toString()
      .trim();
  } catch {
    // ignore git lookup failures
  }

  const output = {
    timestamp: now.toISOString(),
    gitCommit,
    iterations,
    warmup,
    sampleEvery,
    grpcTarget: process.env.BACKEND_CORE_ADDR || 'backend-core:50051',
    napiBindingPath: process.env.CORE_NAPI_BINDING_PATH || null,
    metadata: {
      benchmarkType: 'memory',
      rustVsNode: ['node_ts_local', 'rust_napi_by_type', 'rust_napi_fast', 'rust_napi_batch_x256', 'rust_grpc'],
    },
    summaries,
  };

  const outFile = path.join(saveDir, `core-memory-bench-${timestamp}.json`);
  fs.writeFileSync(outFile, JSON.stringify(output, null, 2), 'utf-8');
  console.log('');
  console.log(`saved memory benchmark snapshot: ${outFile}`);
}

main().catch((error) => {
  console.error('Memory benchmark failed:', error);
  process.exitCode = 1;
});
