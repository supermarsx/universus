/* eslint-disable no-console */
import fs from 'fs';
import path from 'path';

type PureResult = {
  impl: string;
  runtimeMs?: number;
  computeMs?: number;
  opsPerSec?: number;
  peakMemoryMB?: number;
  processRuntimeMs?: number;
  benchmarkTotalMs?: number;
  peakResidentMiB?: number;
  peakResidentBytes?: number;
};

type PureSnapshot = {
  timestamp?: string;
  gitCommit?: string;
  results?: PureResult[];
};

type NormalizedResult = {
  impl: string;
  runtimeMs: number;
  computeMs: number;
  opsPerSec: number;
  peakMemoryMB: number;
};

const dir = path.resolve(process.cwd(), 'benchmarks', 'history');
const files = fs
  .readdirSync(dir)
  .filter((file) => /^core-pure-bench-.*\.json$/.test(file))
  .sort();

if (files.length < 2) {
  console.log('Need at least 2 pure snapshots in benchmarks/history to compare.');
  process.exit(0);
}

const previousFile = files[files.length - 2];
const latestFile = files[files.length - 1];

const previous = JSON.parse(fs.readFileSync(path.join(dir, previousFile), 'utf-8')) as PureSnapshot;
const latest = JSON.parse(fs.readFileSync(path.join(dir, latestFile), 'utf-8')) as PureSnapshot;

const toMb = (bytes: number): number => bytes / 1_000_000;
const toNumber = (value: unknown): number => (typeof value === 'number' && Number.isFinite(value) ? value : 0);

const normalize = (result: PureResult): NormalizedResult => {
  const runtimeMs = toNumber(result.runtimeMs ?? result.processRuntimeMs);
  const computeMs = toNumber(result.computeMs ?? result.benchmarkTotalMs);
  const opsPerSec = toNumber(result.opsPerSec);
  const peakMemoryMB = toNumber(
    result.peakMemoryMB ?? result.peakResidentMiB ?? (typeof result.peakResidentBytes === 'number' ? toMb(result.peakResidentBytes) : 0)
  );

  return {
    impl: result.impl,
    runtimeMs,
    computeMs,
    opsPerSec,
    peakMemoryMB,
  };
};

const toMap = (snapshot: PureSnapshot): Record<string, NormalizedResult> =>
  Object.fromEntries((snapshot.results || []).map((result) => [result.impl, normalize(result)]));

const prevMap = toMap(previous);
const currMap = toMap(latest);
const impls = Object.keys(currMap);

console.log(`Previous: ${previousFile} (${previous.gitCommit || 'unknown'})`);
console.log(`Current : ${latestFile} (${latest.gitCommit || 'unknown'})`);
console.log('');

if (impls.length === 0) {
  console.log('Current pure snapshot has no results.');
  process.exit(0);
}

const deltaPct = (current: number, prior: number): string => {
  if (prior <= 0) {
    return 'n/a';
  }
  return `${(((current - prior) / prior) * 100).toFixed(2)}%`;
};

for (const impl of impls) {
  const curr = currMap[impl];
  const prev = prevMap[impl];

  if (!prev) {
    console.log(`${impl}: new in current snapshot`);
    continue;
  }

  console.log(
    `${impl}: runtimeMs ${prev.runtimeMs.toFixed(3)} -> ${curr.runtimeMs.toFixed(3)} (${deltaPct(
      curr.runtimeMs,
      prev.runtimeMs
    )}), computeMs ${prev.computeMs.toFixed(3)} -> ${curr.computeMs.toFixed(3)} (${deltaPct(
      curr.computeMs,
      prev.computeMs
    )}), opsPerSec ${prev.opsPerSec.toFixed(3)} -> ${curr.opsPerSec.toFixed(3)} (${deltaPct(
      curr.opsPerSec,
      prev.opsPerSec
    )}), peakMemoryMB ${prev.peakMemoryMB.toFixed(3)} -> ${curr.peakMemoryMB.toFixed(3)} (${deltaPct(
      curr.peakMemoryMB,
      prev.peakMemoryMB
    )})`
  );
}
