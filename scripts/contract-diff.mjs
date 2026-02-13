#!/usr/bin/env node

const DEFAULT_NODE_BASE = "http://localhost:3000";
const DEFAULT_RUST_BASE = "http://localhost:3300";
const DEFAULT_TIMEOUT_MS = 10000;

const DEFAULT_AUTH_LOGIN_PAYLOAD = {
  email: "contract-diff@example.com",
  password: "contract-diff-password",
};

const DEFAULT_FLEET_HELPER_PAYLOAD = {
  origin: { galaxy: 1, system: 120, position: 8 },
  target: { galaxy: 1, system: 121, position: 4 },
  ships: { lightFighter: 10, cruiser: 4 },
};

const CHECKS = {
  health: {
    node: { method: "GET", path: "/api/health" },
    rust: { method: "GET", path: "/health" },
  },
  "auth-login": {
    node: { method: "POST", path: "/api/auth/login", payloadEnv: "CONTRACT_DIFF_AUTH_LOGIN_PAYLOAD" },
    rust: { method: "POST", path: "/api/auth/login", payloadEnv: "CONTRACT_DIFF_AUTH_LOGIN_PAYLOAD" },
  },
  "fleet-helper-movement": {
    node: { method: "POST", path: "/api/fleet/helpers/movement", payloadEnv: "CONTRACT_DIFF_FLEET_HELPER_PAYLOAD", needsAuth: true },
    rust: { method: "POST", path: "/api/fleet/helpers/movement", payloadEnv: "CONTRACT_DIFF_FLEET_HELPER_PAYLOAD", needsAuth: true },
  },
  galaxy: {
    node: { method: "GET", pathEnv: "CONTRACT_DIFF_NODE_GALAXY_PATH", defaultPath: "/api/galaxy?galaxy=1&system=120", needsAuth: true },
    rust: { method: "GET", pathEnv: "CONTRACT_DIFF_RUST_GALAXY_PATH", defaultPath: "/api/galaxy/1/120", needsAuth: true },
  },
};

function parseArgs(argv) {
  const parsed = {
    nodeBase: process.env.CONTRACT_DIFF_NODE_BASE_URL || DEFAULT_NODE_BASE,
    rustBase: process.env.CONTRACT_DIFF_RUST_BASE_URL || DEFAULT_RUST_BASE,
    checks: Object.keys(CHECKS),
    timeoutMs: Number(process.env.CONTRACT_DIFF_TIMEOUT_MS || DEFAULT_TIMEOUT_MS),
    showHelp: false,
  };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--help" || arg === "-h") {
      parsed.showHelp = true;
      continue;
    }
    if (arg === "--node-base") {
      parsed.nodeBase = argv[i + 1];
      i += 1;
      continue;
    }
    if (arg === "--rust-base") {
      parsed.rustBase = argv[i + 1];
      i += 1;
      continue;
    }
    if (arg === "--checks") {
      parsed.checks = argv[i + 1].split(",").map((v) => v.trim()).filter(Boolean);
      i += 1;
      continue;
    }
    if (arg === "--timeout-ms") {
      parsed.timeoutMs = Number(argv[i + 1]);
      i += 1;
      continue;
    }
    throw new Error(`Unknown argument: ${arg}`);
  }

  return parsed;
}

function printHelp() {
  console.log("Contract diff harness (Node vs Rust)");
  console.log("");
  console.log("Usage:");
  console.log("  node scripts/contract-diff.mjs [--node-base URL] [--rust-base URL] [--checks list] [--timeout-ms N]");
  console.log("");
  console.log("Checks:");
  console.log(`  ${Object.keys(CHECKS).join(", ")}`);
  console.log("");
  console.log("Examples:");
  console.log("  node scripts/contract-diff.mjs");
  console.log("  node scripts/contract-diff.mjs --checks health,auth-login");
  console.log("  node scripts/contract-diff.mjs --node-base http://localhost:3000 --rust-base http://localhost:3300");
}

function parseJsonEnv(name, fallback) {
  const raw = process.env[name];
  if (!raw) return fallback;
  try {
    return JSON.parse(raw);
  } catch (error) {
    throw new Error(`${name} must be valid JSON: ${error.message}`);
  }
}

function authHeaderFor(label) {
  const explicit = process.env[`CONTRACT_DIFF_${label.toUpperCase()}_AUTH_HEADER`];
  if (explicit) return explicit;
  const token = process.env[`CONTRACT_DIFF_${label.toUpperCase()}_BEARER_TOKEN`];
  if (!token) return null;
  return `Bearer ${token}`;
}

function resolvePayload(payloadEnv) {
  if (!payloadEnv) return undefined;
  if (payloadEnv === "CONTRACT_DIFF_AUTH_LOGIN_PAYLOAD") {
    return parseJsonEnv(payloadEnv, DEFAULT_AUTH_LOGIN_PAYLOAD);
  }
  if (payloadEnv === "CONTRACT_DIFF_FLEET_HELPER_PAYLOAD") {
    return parseJsonEnv(payloadEnv, DEFAULT_FLEET_HELPER_PAYLOAD);
  }
  return parseJsonEnv(payloadEnv, {});
}

function resolvePath(endpoint, sideLabel) {
  if (endpoint.path) return endpoint.path;
  if (endpoint.pathEnv) {
    return process.env[endpoint.pathEnv] || endpoint.defaultPath;
  }
  throw new Error(`No path configured for ${sideLabel}`);
}

function buildRequest(sideLabel, baseUrl, endpoint) {
  const path = resolvePath(endpoint, sideLabel);
  const url = new URL(path, baseUrl).toString();
  const headers = {
    Accept: "application/json",
  };
  const payload = resolvePayload(endpoint.payloadEnv);
  if (payload !== undefined) {
    headers["Content-Type"] = "application/json";
  }
  if (endpoint.needsAuth) {
    const authHeader = authHeaderFor(sideLabel);
    if (authHeader) {
      headers.Authorization = authHeader;
    }
  }
  return {
    url,
    method: endpoint.method,
    headers,
    body: payload === undefined ? undefined : JSON.stringify(payload),
  };
}

async function fetchJson(request, timeoutMs) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const response = await fetch(request.url, {
      method: request.method,
      headers: request.headers,
      body: request.body,
      signal: controller.signal,
    });
    const text = await response.text();
    let body = text;
    if (text.length > 0) {
      try {
        body = JSON.parse(text);
      } catch {
        body = text;
      }
    } else {
      body = null;
    }
    return {
      ok: true,
      status: response.status,
      body,
      contentType: response.headers.get("content-type") || "",
    };
  } catch (error) {
    return {
      ok: false,
      error: error instanceof Error ? error.message : String(error),
    };
  } finally {
    clearTimeout(timeout);
  }
}

function isPlainObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function normalizeBody(body) {
  if (Array.isArray(body)) {
    return body.map(normalizeBody);
  }
  if (!isPlainObject(body)) {
    return body;
  }

  if (body.success === true && Object.prototype.hasOwnProperty.call(body, "data")) {
    return normalizeBody(body.data);
  }

  const out = {};
  const keys = Object.keys(body).sort();
  for (const key of keys) {
    if (key === "timestamp" || key === "engine" || key === "service") {
      continue;
    }
    out[key] = normalizeBody(body[key]);
  }
  return out;
}

function collectDiffs(nodeValue, rustValue, path = "$", diffs = []) {
  if (diffs.length >= 25) return diffs;

  const nodeType = Array.isArray(nodeValue) ? "array" : typeof nodeValue;
  const rustType = Array.isArray(rustValue) ? "array" : typeof rustValue;
  if (nodeType !== rustType) {
    diffs.push(`${path}: type mismatch node=${nodeType} rust=${rustType}`);
    return diffs;
  }

  if (Array.isArray(nodeValue) && Array.isArray(rustValue)) {
    if (nodeValue.length !== rustValue.length) {
      diffs.push(`${path}: array length mismatch node=${nodeValue.length} rust=${rustValue.length}`);
    }
    const len = Math.min(nodeValue.length, rustValue.length);
    for (let i = 0; i < len; i += 1) {
      collectDiffs(nodeValue[i], rustValue[i], `${path}[${i}]`, diffs);
    }
    return diffs;
  }

  if (isPlainObject(nodeValue) && isPlainObject(rustValue)) {
    const nodeKeys = new Set(Object.keys(nodeValue));
    const rustKeys = new Set(Object.keys(rustValue));
    for (const key of [...nodeKeys].sort()) {
      if (!rustKeys.has(key)) {
        diffs.push(`${path}.${key}: missing in rust`);
      }
    }
    for (const key of [...rustKeys].sort()) {
      if (!nodeKeys.has(key)) {
        diffs.push(`${path}.${key}: missing in node`);
      }
    }
    for (const key of [...nodeKeys].sort()) {
      if (rustKeys.has(key)) {
        collectDiffs(nodeValue[key], rustValue[key], `${path}.${key}`, diffs);
      }
      if (diffs.length >= 25) break;
    }
    return diffs;
  }

  if (nodeValue !== rustValue) {
    diffs.push(`${path}: value mismatch node=${JSON.stringify(nodeValue)} rust=${JSON.stringify(rustValue)}`);
  }
  return diffs;
}

function validateChecks(checks) {
  const unknown = checks.filter((name) => !CHECKS[name]);
  if (unknown.length > 0) {
    throw new Error(`Unknown checks: ${unknown.join(", ")}. Valid checks: ${Object.keys(CHECKS).join(", ")}`);
  }
}

async function runCheck(checkName, config, options) {
  const nodeRequest = buildRequest("node", options.nodeBase, config.node);
  const rustRequest = buildRequest("rust", options.rustBase, config.rust);

  const [nodeResult, rustResult] = await Promise.all([
    fetchJson(nodeRequest, options.timeoutMs),
    fetchJson(rustRequest, options.timeoutMs),
  ]);

  const summary = {
    name: checkName,
    nodeRequest,
    rustRequest,
    nodeResult,
    rustResult,
    matched: false,
    reasons: [],
  };

  if (!nodeResult.ok || !rustResult.ok) {
    if (!nodeResult.ok) {
      summary.reasons.push(`Node request failed: ${nodeResult.error}`);
    }
    if (!rustResult.ok) {
      summary.reasons.push(`Rust request failed: ${rustResult.error}`);
    }
    return summary;
  }

  if (nodeResult.status !== rustResult.status) {
    summary.reasons.push(`Status mismatch: node=${nodeResult.status} rust=${rustResult.status}`);
  }

  const nodeBody = normalizeBody(nodeResult.body);
  const rustBody = normalizeBody(rustResult.body);
  const bodyDiffs = collectDiffs(nodeBody, rustBody);
  if (bodyDiffs.length > 0) {
    summary.reasons.push(`Body mismatch (${bodyDiffs.length} diff${bodyDiffs.length > 1 ? "s" : ""})`);
    summary.bodyDiffs = bodyDiffs;
  }

  summary.matched = summary.reasons.length === 0;
  return summary;
}

function printResult(result) {
  if (result.matched) {
    console.log(`[PASS] ${result.name}`);
    return;
  }

  console.log(`[FAIL] ${result.name}`);
  for (const reason of result.reasons) {
    console.log(`  - ${reason}`);
  }
  if (result.bodyDiffs && result.bodyDiffs.length > 0) {
    for (const diff of result.bodyDiffs) {
      console.log(`    * ${diff}`);
    }
  }
  if (!result.nodeResult.ok || !result.rustResult.ok) {
    console.log(`  node url: ${result.nodeRequest.url}`);
    console.log(`  rust url: ${result.rustRequest.url}`);
  }
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.showHelp) {
    printHelp();
    process.exit(0);
  }

  validateChecks(args.checks);

  const options = {
    nodeBase: args.nodeBase,
    rustBase: args.rustBase,
    timeoutMs: Number.isFinite(args.timeoutMs) && args.timeoutMs > 0 ? args.timeoutMs : DEFAULT_TIMEOUT_MS,
  };

  console.log(`Node base: ${options.nodeBase}`);
  console.log(`Rust base: ${options.rustBase}`);
  console.log(`Checks: ${args.checks.join(", ")}`);
  console.log("");

  const results = [];
  for (const checkName of args.checks) {
    const result = await runCheck(checkName, CHECKS[checkName], options);
    printResult(result);
    results.push(result);
  }

  const failed = results.filter((r) => !r.matched);
  console.log("");
  if (failed.length > 0) {
    console.log(`Contract diff finished with ${failed.length} failing check(s).`);
    process.exit(1);
  }

  console.log("Contract diff finished with all checks matching.");
  process.exit(0);
}

main().catch((error) => {
  console.error(`Fatal error: ${error instanceof Error ? error.message : String(error)}`);
  process.exit(2);
});
