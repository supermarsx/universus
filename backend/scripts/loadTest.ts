import autocannon from 'autocannon';

type Scenario = {
  name: string;
  path: string;
  method?: 'GET' | 'POST';
  body?: string;
  requiresAuth?: boolean;
};

const baseUrl = process.env.LOADTEST_BASE_URL || 'http://localhost:3000';
const token = process.env.LOADTEST_TOKEN;
const connections = Number(process.env.LOADTEST_CONNECTIONS || 40);
const duration = Number(process.env.LOADTEST_DURATION || 30);

const scenarios: Scenario[] = [
  {
    name: 'Galaxy scans',
    path: '/api/galaxy?galaxy=1&system=1',
    requiresAuth: true,
  },
  {
    name: 'Leaderboard pulls',
    path: '/api/leaderboard',
  },
  {
    name: 'Admin scaling metrics',
    path: '/api/admin/monitoring/scaling',
    requiresAuth: true,
  },
];

async function runScenario(scenario: Scenario) {
  if (scenario.requiresAuth && !token) {
    console.warn(`[loadTest] Skipping ${scenario.name} (requires LOADTEST_TOKEN).`);
    return;
  }

  console.log(`\n[loadTest] Running ${scenario.name} for ${duration}s with ${connections} connections`);

  const url = `${baseUrl}${scenario.path}`;
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
  };

  if (scenario.requiresAuth && token) {
    headers.Authorization = `Bearer ${token}`;
  }

  return new Promise<void>((resolve, reject) => {
    const instance = autocannon(
      {
        url,
        method: scenario.method || 'GET',
        headers,
        body: scenario.body,
        connections,
        duration,
        title: scenario.name,
      },
      (err, result) => {
        if (err) {
          reject(err);
          return;
        }

        console.log(
          `[loadTest] ${scenario.name} | req/s: ${result.requests.average} | latency (p95): ${result.latency.p95}ms | errors: ${result.errors}`
        );
        resolve();
      }
    );

    process.once('SIGINT', () => {
      instance.stop();
      resolve();
    });
  });
}

async function main() {
  console.log('[loadTest] Base URL:', baseUrl);
  for (const scenario of scenarios) {
    await runScenario(scenario);
  }
  console.log('\n[loadTest] All scenarios completed.');
}

main().catch((error) => {
  console.error('[loadTest] Failed:', error);
  process.exit(1);
});
