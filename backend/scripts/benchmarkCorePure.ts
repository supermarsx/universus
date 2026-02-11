/* eslint-disable no-console */
import fs from 'fs';
import path from 'path';
import { execSync, spawn } from 'child_process';

interface ProcessRunResult {
  name: string;
  wallMs: number;
  stdout: string;
  stderr: string;
  exitCode: number | null;
  signal: NodeJS.Signals | null;
  timedOut: boolean;
}

interface WorkerResult {
  impl: string;
  iterations: number;
  warmup: number;
  totalMs: number;
  opsPerSec: number;
  peakResidentBytes: number;
  sink: number;
}

const ITERATIONS = Math.max(1, Number(process.env.BENCH_PURE_ITERATIONS || 1_000_000));
const WARMUP = Math.max(0, Number(process.env.BENCH_PURE_WARMUP || 100_000));
const SAMPLE_EVERY = Math.max(1, Number(process.env.BENCH_PURE_SAMPLE_EVERY || 1024));
const TIMEOUT_MS = Math.max(10_000, Number(process.env.BENCH_PURE_TIMEOUT_MS || 300_000));

const backendDir = path.resolve(__dirname, '..');
const repoRoot = path.resolve(backendDir, '..');
const benchmarksDir = path.join(backendDir, 'benchmarks');
const historyDir = path.join(benchmarksDir, 'history');
const tmpDir = path.join(benchmarksDir, 'tmp');

const tsWorkerPath = path.join(tmpDir, 'ts_pure_bench.js');
const rustWorkerPath = path.join(tmpDir, 'rust_pure_bench.rs');
const rustBinaryName = process.platform === 'win32' ? 'rust_pure_bench.exe' : 'rust_pure_bench';
const rustBinaryPath = path.join(tmpDir, rustBinaryName);

const tsWorkerSource = String.raw`const iterations = Math.max(1, Number(process.env.BENCH_PURE_ITERATIONS || 1000000));
const warmup = Math.max(0, Number(process.env.BENCH_PURE_WARMUP || 100000));
const sampleEvery = Math.max(1, Number(process.env.BENCH_PURE_SAMPLE_EVERY || 1024));

function buildReq(i) {
  return {
    origin_galaxy: 1,
    origin_system: 120 + (i % 3),
    origin_position: 8,
    target_galaxy: 1,
    target_system: 360 + (i % 5),
    target_position: 12,
    ships: [
      { count: 120, base_speed: 5000, fuel_consumption: 10, cargo: 5000 },
      { count: 40, base_speed: 7500, fuel_consumption: 50, cargo: 25000 },
      { count: 60, base_speed: 12500, fuel_consumption: 20, cargo: 50 },
      { count: 12, base_speed: 15000, fuel_consumption: 300, cargo: 800 },
      { count: 8, base_speed: 10000, fuel_consumption: 500, cargo: 1500 }
    ]
  };
}

function calc(req) {
  const distance = req.origin_galaxy !== req.target_galaxy
    ? Math.abs(req.origin_galaxy - req.target_galaxy) * 20000
    : req.origin_system !== req.target_system
      ? Math.abs(req.origin_system - req.target_system) * 5 * 19 + 2700
      : Math.abs(req.origin_position - req.target_position) * 5 + 1000;

  let minSpeed = Number.POSITIVE_INFINITY;
  let fuelNeeded = 0;
  let cargoCapacity = 0;

  for (const ship of req.ships) {
    if (ship.count <= 0) continue;
    if (ship.base_speed > 0) {
      minSpeed = Math.min(minSpeed, ship.base_speed);
    }
    fuelNeeded += ship.fuel_consumption * ship.count * (distance / 100);
    cargoCapacity += ship.cargo * ship.count;
  }

  const fleetSpeed = Number.isFinite(minSpeed) ? minSpeed : 0;
  const travelTimeSeconds = fleetSpeed > 0 ? Math.ceil((distance / fleetSpeed) * 3600) : 0;
  return distance + fleetSpeed + travelTimeSeconds + fuelNeeded + (cargoCapacity - fuelNeeded);
}

let sink = 0;
let peakResidentBytes = process.memoryUsage().rss;

for (let i = 0; i < warmup; i++) {
  sink += calc(buildReq(i));
  if ((i + 1) % sampleEvery === 0) {
    peakResidentBytes = Math.max(peakResidentBytes, process.memoryUsage().rss);
  }
}

const start = process.hrtime.bigint();
for (let i = 0; i < iterations; i++) {
  sink += calc(buildReq(i + warmup));
  if ((i + 1) % sampleEvery === 0) {
    peakResidentBytes = Math.max(peakResidentBytes, process.memoryUsage().rss);
  }
}
const end = process.hrtime.bigint();

peakResidentBytes = Math.max(peakResidentBytes, process.memoryUsage().rss);

const totalMs = Number(end - start) / 1000000;
const opsPerSec = iterations / (totalMs / 1000);

console.log(JSON.stringify({
  impl: 'ts_pure',
  iterations,
  warmup,
  totalMs,
  opsPerSec,
  peakResidentBytes,
  sink
}));
`;

const rustWorkerSource = String.raw`use std::env;
use std::time::Instant;

#[cfg(target_os = "windows")]
mod platform_memory {
    use std::ffi::c_void;

    #[repr(C)]
    struct ProcessMemoryCounters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut c_void;
    }

    #[link(name = "psapi")]
    unsafe extern "system" {
        fn K32GetProcessMemoryInfo(
            process: *mut c_void,
            counters: *mut ProcessMemoryCounters,
            cb: u32,
        ) -> i32;
    }

    pub fn current_resident_bytes() -> u64 {
        unsafe {
            let mut counters = ProcessMemoryCounters {
                cb: std::mem::size_of::<ProcessMemoryCounters>() as u32,
                page_fault_count: 0,
                peak_working_set_size: 0,
                working_set_size: 0,
                quota_peak_paged_pool_usage: 0,
                quota_paged_pool_usage: 0,
                quota_peak_non_paged_pool_usage: 0,
                quota_non_paged_pool_usage: 0,
                pagefile_usage: 0,
                peak_pagefile_usage: 0,
            };
            let ok = K32GetProcessMemoryInfo(
                GetCurrentProcess(),
                &mut counters,
                counters.cb,
            );
            if ok != 0 {
                counters.working_set_size as u64
            } else {
                0
            }
        }
    }
}

#[cfg(target_os = "linux")]
mod platform_memory {
    pub fn current_resident_bytes() -> u64 {
        let status = match std::fs::read_to_string("/proc/self/status") {
            Ok(content) => content,
            Err(_) => return 0,
        };

        for line in status.lines() {
            if !line.starts_with("VmRSS:") {
                continue;
            }
            let mut parts = line.split_whitespace();
            let _ = parts.next();
            if let Some(value_kb) = parts.next() {
                if let Ok(kb) = value_kb.parse::<u64>() {
                    return kb.saturating_mul(1024);
                }
            }
            break;
        }
        0
    }
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
mod platform_memory {
    pub fn current_resident_bytes() -> u64 {
        0
    }
}

#[derive(Clone, Copy)]
struct Ship {
    count: i32,
    base_speed: f64,
    fuel_consumption: f64,
    cargo: f64,
}

#[derive(Clone, Copy)]
struct Request {
    origin_galaxy: i32,
    origin_system: i32,
    origin_position: i32,
    target_galaxy: i32,
    target_system: i32,
    target_position: i32,
    ships: [Ship; 5],
}

fn read_env_i32(name: &str, fallback: i32) -> i32 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<i32>().ok())
        .filter(|value| *value >= 0)
        .unwrap_or(fallback)
}

fn build_request(i: i32) -> Request {
    Request {
        origin_galaxy: 1,
        origin_system: 120 + (i % 3),
        origin_position: 8,
        target_galaxy: 1,
        target_system: 360 + (i % 5),
        target_position: 12,
        ships: [
            Ship { count: 120, base_speed: 5000.0, fuel_consumption: 10.0, cargo: 5000.0 },
            Ship { count: 40, base_speed: 7500.0, fuel_consumption: 50.0, cargo: 25000.0 },
            Ship { count: 60, base_speed: 12500.0, fuel_consumption: 20.0, cargo: 50.0 },
            Ship { count: 12, base_speed: 15000.0, fuel_consumption: 300.0, cargo: 800.0 },
            Ship { count: 8, base_speed: 10000.0, fuel_consumption: 500.0, cargo: 1500.0 },
        ],
    }
}

fn calc(request: Request) -> f64 {
    let distance = if request.origin_galaxy != request.target_galaxy {
        (request.origin_galaxy - request.target_galaxy).abs() * 20000
    } else if request.origin_system != request.target_system {
        (request.origin_system - request.target_system).abs() * 5 * 19 + 2700
    } else {
        (request.origin_position - request.target_position).abs() * 5 + 1000
    };

    let mut min_speed = f64::INFINITY;
    let mut fuel_needed = 0.0_f64;
    let mut cargo_capacity = 0.0_f64;

    for ship in request.ships {
        if ship.count <= 0 {
            continue;
        }
        if ship.base_speed > 0.0 {
            min_speed = min_speed.min(ship.base_speed);
        }
        let count = ship.count as f64;
        fuel_needed += ship.fuel_consumption * count * (distance as f64 / 100.0);
        cargo_capacity += ship.cargo * count;
    }

    let fleet_speed = if min_speed.is_finite() { min_speed } else { 0.0 };
    let travel_time_seconds = if fleet_speed > 0.0 {
        ((distance as f64 / fleet_speed) * 3600.0).ceil()
    } else {
        0.0
    };

    distance as f64 + fleet_speed + travel_time_seconds + fuel_needed + (cargo_capacity - fuel_needed)
}

fn main() {
    let iterations = read_env_i32("BENCH_PURE_ITERATIONS", 1_000_000).max(1);
    let warmup = read_env_i32("BENCH_PURE_WARMUP", 100_000).max(0);
    let sample_every = read_env_i32("BENCH_PURE_SAMPLE_EVERY", 1024).max(1);

    let mut sink = 0.0_f64;
    let mut peak_resident_bytes = platform_memory::current_resident_bytes();

    for i in 0..warmup {
        sink += calc(build_request(i));
        if ((i + 1) % sample_every) == 0 {
            peak_resident_bytes = peak_resident_bytes.max(platform_memory::current_resident_bytes());
        }
    }

    let start = Instant::now();
    for i in 0..iterations {
        sink += calc(build_request(i + warmup));
        if ((i + 1) % sample_every) == 0 {
            peak_resident_bytes = peak_resident_bytes.max(platform_memory::current_resident_bytes());
        }
    }
    let total_ms = start.elapsed().as_secs_f64() * 1000.0;

    peak_resident_bytes = peak_resident_bytes.max(platform_memory::current_resident_bytes());

    let ops_per_sec = (iterations as f64) / (total_ms / 1000.0);
    println!(
        "{{\"impl\":\"rust_pure\",\"iterations\":{},\"warmup\":{},\"totalMs\":{:.6},\"opsPerSec\":{:.3},\"peakResidentBytes\":{},\"sink\":{:.3}}}",
        iterations,
        warmup,
        total_ms,
        ops_per_sec,
        peak_resident_bytes,
        sink
    );
}
`;

const runProcess = (
  name: string,
  command: string,
  args: string[],
  timeoutMs: number,
  env: NodeJS.ProcessEnv
): Promise<ProcessRunResult> =>
  new Promise((resolve, reject) => {
    const started = process.hrtime.bigint();
    const child = spawn(command, args, {
      cwd: backendDir,
      env,
      stdio: ['ignore', 'pipe', 'pipe'],
      windowsHide: true,
    });

    let stdout = '';
    let stderr = '';
    let done = false;
    let timedOut = false;

    child.stdout.on('data', (chunk) => {
      stdout += chunk.toString();
    });

    child.stderr.on('data', (chunk) => {
      stderr += chunk.toString();
    });

    child.on('error', (error) => {
      if (done) return;
      done = true;
      reject(error);
    });

    const timer = setTimeout(() => {
      timedOut = true;
      if (!child.killed) {
        child.kill('SIGKILL');
      }
    }, timeoutMs);

    child.on('close', (exitCode, signal) => {
      if (done) return;
      done = true;
      clearTimeout(timer);
      const wallMs = Number(process.hrtime.bigint() - started) / 1_000_000;
      resolve({ name, wallMs, stdout, stderr, exitCode, signal, timedOut });
    });
  });

const parseWorkerResult = (label: string, output: string): WorkerResult => {
  const lines = output
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0);

  for (let i = lines.length - 1; i >= 0; i--) {
    const line = lines[i];
    try {
      const parsed = JSON.parse(line) as WorkerResult;
      if (
        typeof parsed.impl === 'string' &&
        typeof parsed.iterations === 'number' &&
        typeof parsed.totalMs === 'number' &&
        typeof parsed.opsPerSec === 'number' &&
        typeof parsed.peakResidentBytes === 'number'
      ) {
        return parsed;
      }
    } catch {
      // keep scanning for last JSON line
    }
  }

  throw new Error(`Could not parse benchmark output for ${label}`);
};

const getShortCommit = (): string => {
  try {
    return execSync('git rev-parse --short HEAD', { cwd: repoRoot, stdio: ['ignore', 'pipe', 'ignore'] })
      .toString()
      .trim();
  } catch {
    return 'unknown';
  }
};

const ensureRustWorkerCompiled = () => {
  fs.mkdirSync(tmpDir, { recursive: true });
  fs.writeFileSync(tsWorkerPath, tsWorkerSource, 'utf-8');
  fs.writeFileSync(rustWorkerPath, rustWorkerSource, 'utf-8');

  const compileArgs = ['-C', 'opt-level=3', rustWorkerPath, '-o', rustBinaryPath];
  const result = spawn('rustc', compileArgs, {
    cwd: backendDir,
    stdio: 'pipe',
    windowsHide: true,
  });

  return new Promise<void>((resolve, reject) => {
    let stderr = '';
    result.stderr.on('data', (chunk) => {
      stderr += chunk.toString();
    });

    result.on('error', (error) => reject(error));
    result.on('close', (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`rustc failed with exit code ${code}. ${stderr.trim()}`));
      }
    });
  });
};

const bytesToMiB = (bytes: number): number => Math.round((bytes / (1024 * 1024)) * 1000) / 1000;
const roundMs = (value: number): number => Math.round(value * 1000) / 1000;

async function main() {
  fs.mkdirSync(historyDir, { recursive: true });
  fs.mkdirSync(tmpDir, { recursive: true });

  console.log('Core Pure Benchmark (TS vs Rust direct process)');
  console.log(`iterations=${ITERATIONS} warmup=${WARMUP} sampleEvery=${SAMPLE_EVERY} timeoutMs=${TIMEOUT_MS}`);

  await ensureRustWorkerCompiled();

  const benchEnv: NodeJS.ProcessEnv = {
    ...process.env,
    BENCH_PURE_ITERATIONS: String(ITERATIONS),
    BENCH_PURE_WARMUP: String(WARMUP),
    BENCH_PURE_SAMPLE_EVERY: String(SAMPLE_EVERY),
  };

  const tsRun = await runProcess('ts_pure', process.execPath, [tsWorkerPath], TIMEOUT_MS, benchEnv);
  if (tsRun.timedOut) {
    throw new Error(`ts_pure timed out after ${TIMEOUT_MS}ms`);
  }
  if (tsRun.exitCode !== 0) {
    throw new Error(`ts_pure failed with exit code ${tsRun.exitCode}: ${tsRun.stderr || tsRun.stdout}`);
  }

  const rustRun = await runProcess('rust_pure', rustBinaryPath, [], TIMEOUT_MS, benchEnv);
  if (rustRun.timedOut) {
    throw new Error(`rust_pure timed out after ${TIMEOUT_MS}ms`);
  }
  if (rustRun.exitCode !== 0) {
    throw new Error(`rust_pure failed with exit code ${rustRun.exitCode}: ${rustRun.stderr || rustRun.stdout}`);
  }

  const tsResult = parseWorkerResult('ts_pure', tsRun.stdout);
  const rustResult = parseWorkerResult('rust_pure', rustRun.stdout);

  const now = new Date();
  const timestamp = now.toISOString();
  const safeTimestamp = timestamp.replace(/[:.]/g, '-');

  const snapshot = {
    timestamp,
    gitCommit: getShortCommit(),
    benchmarkType: 'pure-operation',
    workload: {
      actions: ITERATIONS,
      warmup: WARMUP,
      sampleEvery: SAMPLE_EVERY,
      movementKernel: 'fleet-helper movement math equivalent in TS and Rust',
    },
    results: [
      {
        impl: tsResult.impl,
        processRuntimeMs: roundMs(tsRun.wallMs),
        benchmarkTotalMs: roundMs(tsResult.totalMs),
        opsPerSec: Math.round(tsResult.opsPerSec),
        peakResidentBytes: tsResult.peakResidentBytes,
        peakResidentMiB: bytesToMiB(tsResult.peakResidentBytes),
        sink: tsResult.sink,
      },
      {
        impl: rustResult.impl,
        processRuntimeMs: roundMs(rustRun.wallMs),
        benchmarkTotalMs: roundMs(rustResult.totalMs),
        opsPerSec: Math.round(rustResult.opsPerSec),
        peakResidentBytes: rustResult.peakResidentBytes,
        peakResidentMiB: bytesToMiB(rustResult.peakResidentBytes),
        sink: rustResult.sink,
      },
    ],
  };

  const outPath = path.join(historyDir, `core-pure-bench-${safeTimestamp}.json`);
  fs.writeFileSync(outPath, JSON.stringify(snapshot, null, 2), 'utf-8');

  for (const item of snapshot.results) {
    console.log(
      `${item.impl}: runtime=${item.processRuntimeMs}ms compute=${item.benchmarkTotalMs}ms ops/s=${item.opsPerSec} peak=${item.peakResidentMiB} MiB`
    );
  }
  console.log(`saved benchmark snapshot: ${outPath}`);
}

main().catch((error) => {
  console.error('Pure core benchmark failed:', error);
  process.exitCode = 1;
});
