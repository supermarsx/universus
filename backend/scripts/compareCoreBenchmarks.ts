/* eslint-disable no-console */
import fs from 'fs';
import path from 'path';

type Summary = {
  name: string;
  totalMs: number;
  opsPerSec: number;
  p50: number;
  p95: number;
  p99: number;
  avg: number;
  min: number;
  max: number;
};

type Snapshot = {
  timestamp: string;
  gitCommit: string;
  iterations: number;
  warmup: number;
  summaries: Summary[];
};

const dir = path.resolve(process.cwd(), 'benchmarks', 'history');
const files = fs
  .readdirSync(dir)
  .filter((f) => f.endsWith('.json'))
  .sort();

if (files.length < 2) {
  console.log('Need at least 2 snapshots in benchmarks/history to compare.');
  process.exit(0);
}

const latest = JSON.parse(fs.readFileSync(path.join(dir, files[files.length - 1]), 'utf-8')) as Snapshot;
const previous = JSON.parse(fs.readFileSync(path.join(dir, files[files.length - 2]), 'utf-8')) as Snapshot;

const toMap = (snap: Snapshot): Record<string, Summary> =>
  Object.fromEntries((snap.summaries || []).map((s) => [s.name, s]));

const prevMap = toMap(previous);
const currMap = toMap(latest);

console.log(`Previous: ${files[files.length - 2]} (${previous.gitCommit})`);
console.log(`Current : ${files[files.length - 1]} (${latest.gitCommit})`);
if (previous.iterations !== latest.iterations) {
  console.log(
    `Warning: iteration counts differ (${previous.iterations} vs ${latest.iterations}); compare ops/s or avg latency, not total ms.`
  );
}
console.log('');

for (const name of Object.keys(currMap)) {
  const curr = currMap[name];
  const prev = prevMap[name];
  if (!prev) {
    console.log(`${name}: new in current snapshot`);
    continue;
  }
  const totalDeltaPct = prev.totalMs > 0 ? ((curr.totalMs - prev.totalMs) / prev.totalMs) * 100 : 0;
  const opsDeltaPct = prev.opsPerSec > 0 ? ((curr.opsPerSec - prev.opsPerSec) / prev.opsPerSec) * 100 : 0;
  console.log(
    `${name}: total ${prev.totalMs.toFixed(3)}ms -> ${curr.totalMs.toFixed(3)}ms (${totalDeltaPct.toFixed(
      2
    )}%), ops/s ${prev.opsPerSec.toFixed(0)} -> ${curr.opsPerSec.toFixed(0)} (${opsDeltaPct.toFixed(2)}%)`
  );
}
