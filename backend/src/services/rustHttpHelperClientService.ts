import type {
  CombatDistributionInput,
  CombatDistributionOutput,
  DefenseRebuildInput,
  DefenseRebuildOutput,
  EspionageOutcomeInput,
  EspionageOutcomeOutput,
  FleetMovementInput,
  FleetMovementOutput,
  HarvestCollectionInput,
  HarvestCollectionOutput,
  MissionCargoTransferInput,
  MissionCargoTransferOutput,
} from './fleetHelperService';
import type { RustSimulateRequest } from '../coreAdapter/rustCoreClient';

interface RustHelperEnvelope<T> {
  success?: boolean;
  data?: T;
  error?: string;
}

const DEFAULT_TIMEOUT_MS = 2000;
const DEFAULT_BASE_URL = '';

function toBaseUrl(raw: string | undefined): string {
  return String(raw || DEFAULT_BASE_URL).trim().replace(/\/+$/, '');
}

function toCoreHelperToken(raw: string | undefined): string | undefined {
  const token = String(raw || '').trim();
  return token.length > 0 ? token : undefined;
}

export class RustHttpHelperClientService {
  static isConfigured(): boolean {
    return toBaseUrl(process.env.RUST_HTTP_HELPER_URL).length > 0;
  }

  static async calculateMovement(input: FleetMovementInput): Promise<FleetMovementOutput> {
    return this.postJson<FleetMovementOutput>('/api/fleet/helpers/movement', input);
  }

  static async resolveDefenseRebuild(input: DefenseRebuildInput): Promise<DefenseRebuildOutput> {
    return this.postJson<DefenseRebuildOutput>('/api/fleet/helpers/combat/defense-rebuild', input);
  }

  static async computeAttackerDistribution(
    input: CombatDistributionInput
  ): Promise<CombatDistributionOutput> {
    return this.postJson<CombatDistributionOutput>('/api/fleet/helpers/combat/attacker-distribution', input);
  }

  static async computeEspionageOutcome(input: EspionageOutcomeInput): Promise<EspionageOutcomeOutput> {
    return this.postJson<EspionageOutcomeOutput>('/api/fleet/helpers/espionage-outcome', input);
  }

  static async computeMissionCargoTransfer(
    input: MissionCargoTransferInput
  ): Promise<MissionCargoTransferOutput> {
    return this.postJson<MissionCargoTransferOutput>('/api/fleet/helpers/mission-cargo-transfer', input);
  }

  static async computeHarvestCollection(input: HarvestCollectionInput): Promise<HarvestCollectionOutput> {
    return this.postJson<HarvestCollectionOutput>('/api/fleet/helpers/harvest-collection', input);
  }

  static async simulateCombat(input: RustSimulateRequest): Promise<any> {
    return this.postJson<any>('/api/combat/simulate', input);
  }

  private static async postJson<T>(path: string, payload: unknown): Promise<T> {
    const baseUrl = toBaseUrl(process.env.RUST_HTTP_HELPER_URL);
    if (!baseUrl) {
      throw new Error('RUST_HTTP_HELPER_URL is not configured');
    }
    const coreHelperToken = toCoreHelperToken(process.env.CORE_HTTP_HELPER_TOKEN);
    const headers: Record<string, string> = { 'content-type': 'application/json' };
    if (coreHelperToken) {
      headers['x-core-helper-token'] = coreHelperToken;
    }

    const timeoutMs = Math.max(100, Number(process.env.RUST_HTTP_HELPER_TIMEOUT_MS || DEFAULT_TIMEOUT_MS));
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), timeoutMs);

    try {
      const response = await fetch(`${baseUrl}${path}`, {
        method: 'POST',
        headers,
        body: JSON.stringify(payload),
        signal: controller.signal,
      });

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }

      const body = (await response.json()) as RustHelperEnvelope<T> | T;
      if (body && typeof body === 'object' && 'success' in body) {
        const envelope = body as RustHelperEnvelope<T>;
        if (envelope.success === false) {
          throw new Error(envelope.error || 'Rust helper request failed');
        }
        if ('data' in envelope) {
          return envelope.data as T;
        }
      }

      return body as T;
    } catch (error: any) {
      if (error?.name === 'AbortError') {
        throw new Error(`Rust helper request timeout after ${timeoutMs}ms`);
      }
      throw new Error(`Rust helper request failed (${path}): ${error?.message || String(error)}`);
    } finally {
      clearTimeout(timeoutId);
    }
  }
}
