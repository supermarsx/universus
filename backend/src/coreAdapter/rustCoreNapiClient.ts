const DECOMMISSIONED_MESSAGE =
  'backend/src/coreAdapter/rustCoreNapiClient.ts is decommissioned. The Node N-API bridge has been retired from active runtime paths. Use rustCoreClient (gRPC/local fallback) instead.';

throw new Error(DECOMMISSIONED_MESSAGE);
