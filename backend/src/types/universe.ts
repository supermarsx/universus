// =====================================================
// PHASE 4: UNIVERSE SEEDING SYSTEM - TypeScript Types
// Type definitions for universe generation and management
// =====================================================

// =====================================================
// ENUMS
// =====================================================

export enum UniverseType {
  BALANCED = 'balanced',
  RESOURCE_RICH = 'resource_rich',
  COMBAT_FOCUSED = 'combat_focused',
  RESEARCH_HEAVY = 'research_heavy',
  MIXED_ECONOMY = 'mixed_economy',
  HARDCORE = 'hardcore'
}

export enum GalaxyType {
  STANDARD = 'standard',
  RESOURCE_RICH = 'resource_rich',
  MILITARY = 'military',
  RESEARCH = 'research',
  WASTELAND = 'wasteland',
  ENDGAME = 'endgame',
  SAFE_ZONE = 'safe_zone',
  PVP_ZONE = 'pvp_zone'
}

export enum DifficultyCurve {
  FLAT = 'flat',
  PROGRESSIVE = 'progressive',
  STEEP = 'steep',
  CUSTOM = 'custom'
}

export enum PlacementStrategy {
  RANDOM = 'random',
  BALANCED = 'balanced',
  CLUSTERED = 'clustered',
  DISPERSED = 'dispersed',
  ALLIANCE_GROUPED = 'alliance_grouped',
  SKILL_BASED = 'skill_based'
}

export enum BotPersonality {
  AGGRESSIVE = 'aggressive',
  DEFENSIVE = 'defensive',
  ECONOMIC = 'economic',
  EXPLORER = 'explorer',
  RESEARCHER = 'researcher',
  DIPLOMATIC = 'diplomatic',
  OPPORTUNIST = 'opportunist',
  BALANCED = 'balanced'
}

export enum BotSkillLevel {
  NOVICE = 'novice',
  INTERMEDIATE = 'intermediate',
  ADVANCED = 'advanced',
  EXPERT = 'expert'
}

export enum ResourceDistributionPattern {
  UNIFORM = 'uniform',
  CLUSTERED = 'clustered',
  RADIAL = 'radial',
  STRATEGIC = 'strategic',
  RANDOM = 'random'
}

export enum AllianceFormationStrategy {
  PRE_SEEDED = 'pre_seeded',
  PLAYER_CREATED = 'player_created',
  BOT_ALLIANCE = 'bot_alliance',
  MIXED = 'mixed'
}

export enum MaintenanceTaskType {
  POPULATION_BALANCE = 'population_balance',
  RESOURCE_BALANCE = 'resource_balance',
  BOT_MANAGEMENT = 'bot_management',
  CLEANUP = 'cleanup',
  ANALYTICS = 'analytics',
  PERFORMANCE = 'performance',
  SECURITY = 'security'
}

// =====================================================
// MAIN INTERFACES
// =====================================================

export enum UniverseRegistrationStatus {
  OPEN = 'open',
  CLOSED = 'closed',
  SCHEDULED = 'scheduled',
  PAUSED = 'paused'
}

export enum UniverseSpeedProgressionType {
  STATIC = 'static',
  SCHEDULED = 'scheduled',
  DYNAMIC = 'dynamic',
  DECREASING = 'decreasing'
}

export enum UniverseEndType {
  SHUTDOWN = 'shutdown',
  MERGE = 'merge',
  ARCHIVE = 'archive',
  OTHER = 'other'
}

export enum UniverseAnnouncementType {
  INFO = 'info',
  WARNING = 'warning',
  EVENT = 'event',
  CLOSURE = 'closure'
}

export interface UniverseSeed {
  id: number;
  universeName: string;
  universeType: UniverseType;
  galaxyCount: number;
  systemsPerGalaxy: number;
  positionsPerSystem: number;
  maxPlayers: number;
  currentPlayers: number;
  botPercentage: number;
  targetBotCount: number;
  resourceMultiplier: number;
  startingResourcesMetal: number;
  startingResourcesCrystal: number;
  startingResourcesDeuterium: number;
  difficultyCurve: DifficultyCurve;
  beginnerProtectionDays: number;
  isSeeded: boolean;
  seedVersion: number;
  seedingStartedAt?: Date;
  seedingCompletedAt?: Date;
  lastMaintainedAt?: Date;
  // --- Multi-universe management fields ---
  registrationStatus: UniverseRegistrationStatus;
  registrationOpenAt?: Date;
  registrationCloseAt?: Date;
  universeOpenAt?: Date;
  universeCloseAt?: Date;
  isActive: boolean;
  closureReason?: string;
  speedMultiplier: number;
  speedProgressionType: UniverseSpeedProgressionType;
  speedSchedule?: Record<string, any>;
  // Detached building/research speed
  buildingSpeedMultiplier: number;
  researchSpeedMultiplier: number;
  buildingSpeedSchedule?: Record<string, any>;
  researchSpeedSchedule?: Record<string, any>;
  // Base rations
  baseStorageRation?: Record<string, any>;
  baseProductionRation?: Record<string, any>;
  isMerging: boolean;
  mergeTargetUniverseId?: number;
  mergeScheduledAt?: Date;
  endOfUniverseEventAt?: Date;
  endOfUniverseType?: UniverseEndType;
  endOfUniverseAnnouncement?: string;
  announcement?: string;
  announcementType?: UniverseAnnouncementType;
  announcementExpiresAt?: Date;
  // ---
  createdAt: Date;
  updatedAt: Date;
  createdBy?: number;
  configuration?: Record<string, any>;
}

export interface GalaxySeed {
  id: number;
  universeId: number;
  galaxyNumber: number;
  galaxyName?: string;
  galaxyType: GalaxyType;
  systemCount: number;
  sectorDivisions: number;
  metalAbundance: number;
  crystalAbundance: number;
  deuteriumAbundance: number;
  rareMaterialsChance: number;
  baseDifficulty: number;
  npcStrengthMultiplier: number;
  maxPlayersPerGalaxy: number;
  currentPlayers: number;
  botCount: number;
  hasSafeZones: boolean;
  hasPvpZones: boolean;
  hasResourceZones: boolean;
  hasEventZones: boolean;
  isGenerated: boolean;
  generatedAt?: Date;
  createdAt: Date;
  updatedAt: Date;
}

export interface SectorConfiguration {
  id: number;
  galaxyId: number;
  sectorNumber: number;
  sectorName?: string;
  difficultyTier: number;
  recommendedLevel: number;
  systemStart: number;
  systemEnd: number;
  metalMultiplier: number;
  crystalMultiplier: number;
  deuteriumMultiplier: number;
  isSafeZone: boolean;
  isPvpZone: boolean;
  isBeginnerZone: boolean;
  isEndgameZone: boolean;
  npcDensity: number;
  npcStrengthLevel: number;
  createdAt: Date;
}

export interface PlayerPlacementRule {
  id: number;
  universeId: number;
  ruleName: string;
  rulePriority: number;
  isActive: boolean;
  playerLevelMin: number;
  playerLevelMax: number;
  preferredGalaxyTypes?: string[];
  preferredSectorTiers?: number[];
  strategy: PlacementStrategy;
  minDistanceFromPlayers: number;
  maxDistanceFromCenter?: number;
  avoidHighActivityZones: boolean;
  preferMetalRich: boolean;
  preferCrystalRich: boolean;
  preferDeuteriumRich: boolean;
  createdAt: Date;
  configuration?: Record<string, any>;
}

export interface PlayerPlacement {
  id: number;
  universeId: number;
  userId: number;
  galaxy: number;
  system: number;
  position: number;
  placementStrategy?: string;
  placementRuleId?: number;
  playerLevelAtPlacement: number;
  playerExperienceAtPlacement: number;
  preferredPlaystyle?: string;
  allianceId?: number;
  wasGroupedPlacement: boolean;
  startingMetal: number;
  startingCrystal: number;
  startingDeuterium: number;
  placementQualityScore?: number;
  resourceRichnessScore?: number;
  strategicValueScore?: number;
  placedAt: Date;
}

export interface BotGenerationTemplate {
  id: number;
  universeId: number;
  templateName: string;
  botPersonality: BotPersonality;
  skillLevel: BotSkillLevel;
  skillRandomness: number;
  aggressionLevel: number;
  expansionRate: number;
  tradingActivity: number;
  allianceParticipation: boolean;
  resourceFocus: string;
  buildingPriority?: string[];
  researchPriority?: string[];
  fleetComposition?: Record<string, any>;
  preferredShipTypes?: string[];
  combatWillingness: number;
  generationWeight: number;
  maxBotsFromTemplate?: number;
  currentBotsGenerated: number;
  createdAt: Date;
  updatedAt: Date;
  configuration?: Record<string, any>;
}

export interface GeneratedBot {
  id: number;
  universeId: number;
  userId: number;
  templateId?: number;
  botName: string;
  botPersonality: BotPersonality;
  skillLevel: BotSkillLevel;
  galaxy: number;
  system: number;
  position: number;
  isActive: boolean;
  activationDate: Date;
  deactivationDate?: Date;
  totalAttacks: number;
  totalDefenses: number;
  totalTrades: number;
  totalResourcesCollected: number;
  allianceId?: number;
  allianceRole?: string;
  createdAt: Date;
  updatedAt: Date;
}

export interface ResourceDistributionConfig {
  id: number;
  galaxyId: number;
  patternName: string;
  patternType: ResourceDistributionPattern;
  resourceType: string;
  baseAbundance: number;
  variationPercentage: number;
  clusterSize: number;
  clusterDensity: number;
  preferOuterSystems: boolean;
  preferCenterSystems: boolean;
  strategicChokepoints: boolean;
  isApplied: boolean;
  appliedAt?: Date;
  createdAt: Date;
  configuration?: Record<string, any>;
}

export interface PlanetResources {
  id: number;
  planetId?: number;
  galaxy?: number;
  system?: number;
  position?: number;
  metalRichness: number;
  crystalRichness: number;
  deuteriumRichness: number;
  hasRareMaterials: boolean;
  rareMaterialType?: string;
  rareMaterialAbundance: number;
  strategicValue: number;
  isChokepoint: boolean;
  isHidden: boolean;
  isDiscovered: boolean;
  discoveredBy?: number;
  discoveredAt?: Date;
  createdAt: Date;
}

export interface AllianceSeed {
  id: number;
  universeId: number;
  allianceName: string;
  allianceTag: string;
  allianceType?: string;
  formationStrategy: AllianceFormationStrategy;
  targetMemberCount: number;
  currentMemberCount: number;
  botMemberPercentage: number;
  homeGalaxy?: number;
  homeSector?: number;
  territorySystems?: string[];
  specialization?: string[];
  allianceBonuses?: Record<string, any>;
  isFormed: boolean;
  formedAt?: Date;
  createdAt: Date;
  updatedAt: Date;
  configuration?: Record<string, any>;
}

export interface UniverseMaintenanceTask {
  id: number;
  universeId: number;
  taskName: string;
  taskType: MaintenanceTaskType;
  runFrequencyHours: number;
  lastRunAt?: Date;
  nextRunAt?: Date;
  isActive: boolean;
  isRunning: boolean;
  autoAdjust: boolean;
  totalRuns: number;
  successfulRuns: number;
  failedRuns: number;
  averageDurationSeconds?: number;
  lastResult?: Record<string, any>;
  lastError?: string;
  triggerConditions?: Record<string, any>;
  actionParameters?: Record<string, any>;
  createdAt: Date;
  updatedAt: Date;
}

export interface UniverseAnalytics {
  id: number;
  universeId: number;
  snapshotDate: Date;
  snapshotHour: number;
  totalActivePlayers: number;
  totalActiveBots: number;
  newPlayers24h: number;
  churnedPlayers24h: number;
  totalMetalEconomy: number;
  totalCrystalEconomy: number;
  totalDeuteriumEconomy: number;
  averagePlayerResources: number;
  totalFleetPower: number;
  totalCombats24h: number;
  totalDebrisGenerated24h: number;
  giniCoefficient?: number;
  resourceDistributionVariance?: number;
  powerConcentrationTop10?: number;
  averageSessionDurationMinutes?: number;
  dailyActiveUsers?: number;
  peakConcurrentUsers?: number;
  totalAlliances: number;
  averageAllianceSize?: number;
  allianceWarCount: number;
  createdAt: Date;
}

// =====================================================
// REQUEST/RESPONSE TYPES
// =====================================================

export interface CreateUniverseRequest {
  universeName: string;
  universeType: UniverseType;
  galaxyCount?: number;
  systemsPerGalaxy?: number;
  maxPlayers?: number;
  botPercentage?: number;
  resourceMultiplier?: number;
  difficultyCurve?: DifficultyCurve;
  configuration?: Record<string, any>;
  // Multi-universe management fields
  registrationStatus?: string;
  registrationOpenAt?: string | null;
  registrationCloseAt?: string | null;
  universeOpenAt?: string | null;
  universeCloseAt?: string | null;
  isActive?: boolean;
  closureReason?: string | null;
  speedMultiplier?: number;
  speedProgressionType?: string;
  speedSchedule?: Record<string, any>;
  buildingSpeedMultiplier?: number;
  researchSpeedMultiplier?: number;
  buildingSpeedSchedule?: Record<string, any>;
  researchSpeedSchedule?: Record<string, any>;
  baseStorageRation?: Record<string, any>;
  baseProductionRation?: Record<string, any>;
  isMerging?: boolean;
  mergeTargetUniverseId?: number | null;
  mergeScheduledAt?: string | null;
  endOfUniverseEventAt?: string | null;
  endOfUniverseType?: string | null;
  endOfUniverseAnnouncement?: string | null;
  announcement?: string | null;
  announcementType?: string | null;
  announcementExpiresAt?: string | null;
}

export interface SeedUniverseRequest {
  universeId: number;
  generateGalaxies: boolean;
  generateBots: boolean;
  generateAlliances: boolean;
  distributeResources: boolean;
}

export interface PlacePlayerRequest {
  userId: number;
  universeId: number;
  preferredPlaystyle?: string;
  allianceId?: number;
  useCustomLocation?: boolean;
  customGalaxy?: number;
  customSystem?: number;
}

export interface GenerateBotsRequest {
  universeId: number;
  botCount: number;
  personalities?: BotPersonality[];
  skillLevels?: BotSkillLevel[];
  distributeEvenly?: boolean;
}

export interface DistributeResourcesRequest {
  galaxyId: number;
  patternType: ResourceDistributionPattern;
  resourceType: string;
  parameters?: Record<string, any>;
}

export interface CreateAllianceRequest {
  universeId: number;
  allianceName: string;
  allianceTag: string;
  formationStrategy: AllianceFormationStrategy;
  targetMemberCount?: number;
  botPercentage?: number;
}

// =====================================================
// RESULT TYPES
// =====================================================

export interface UniverseSeedingResult {
  success: boolean;
  universeId?: number;
  seedVersion?: number;
  galaxiesGenerated: number;
  botsGenerated: number;
  alliancesCreated: number;
  resourcePatternsApplied: number;
  seedingDuration: number;
  message: string;
  errors?: string[];
}

export interface PlayerPlacementResult {
  success: boolean;
  placement?: PlayerPlacement;
  qualityScore: number;
  alternativeLocations?: Array<{
    galaxy: number;
    system: number;
    position: number;
    score: number;
  }>;
  message: string;
}

export interface BotGenerationResult {
  success: boolean;
  botsGenerated: number;
  botDetails?: GeneratedBot[];
  message: string;
}

export interface ResourceDistributionResult {
  success: boolean;
  planetsAffected: number;
  averageRichness: number;
  clustersCreated: number;
  message: string;
}

export interface MaintenanceResult {
  success: boolean;
  taskType: MaintenanceTaskType;
  actionsPerformed: string[];
  metricsChanged: Record<string, any>;
  duration: number;
  message: string;
}

// =====================================================
// ANALYTICS TYPES
// =====================================================

export interface UniverseHealthMetrics {
  universeId: number;
  overallHealth: number;
  populationHealth: number;
  economicHealth: number;
  militaryBalance: number;
  activityLevel: number;
  balanceScore: number;
  issues: string[];
  recommendations: string[];
}

export interface GalaxyBalanceReport {
  galaxyId: number;
  galaxyNumber: number;
  playerDistribution: Record<number, number>;
  resourceBalance: {
    metal: number;
    crystal: number;
    deuterium: number;
  };
  difficultyBalance: number;
  recommendations: string[];
}

export interface PlayerDistributionHeatmap {
  galaxy: number;
  sector: number;
  playerCount: number;
  botCount: number;
  averageLevel: number;
  activityScore: number;
}

// =====================================================
// CONFIGURATION TYPES
// =====================================================

export interface GalaxyGenerationConfig {
  galaxyType: GalaxyType;
  sectorCount: number;
  difficultyProgression: 'linear' | 'exponential' | 'custom';
  resourceMultipliers: {
    metal: number;
    crystal: number;
    deuterium: number;
  };
  specialFeatures: string[];
}

export interface BotBehaviorConfig {
  personality: BotPersonality;
  aggressionLevel: number;
  expansionRate: number;
  tradingFrequency: number;
  combatAvoidance: number;
  allianceLoyalty: number;
  resourceManagement: {
    savingsRate: number;
    investmentRate: number;
    militarySpending: number;
  };
}

export interface PlacementAlgorithmConfig {
  strategy: PlacementStrategy;
  minPlayerDistance: number;
  maxPlayerDistance: number;
  clusteringFactor: number;
  avoidanceFactor: number;
  resourceWeighting: {
    metal: number;
    crystal: number;
    deuterium: number;
  };
}

// =====================================================
// UTILITY TYPES
// =====================================================

export interface Coordinates {
  galaxy: number;
  system: number;
  position: number;
}

export interface LocationScore {
  coordinates: Coordinates;
  totalScore: number;
  resourceScore: number;
  distanceScore: number;
  competitionScore: number;
  strategicScore: number;
}

export interface SectorInfo {
  sectorNumber: number;
  systemRange: [number, number];
  difficultyTier: number;
  playerCount: number;
  botCount: number;
  averageResources: number;
}

export interface GalaxyMap {
  galaxyNumber: number;
  sectors: SectorInfo[];
  totalPlayers: number;
  totalBots: number;
  resourceHotspots: Coordinates[];
  pvpZones: Coordinates[];
  safeZones: Coordinates[];
}

// =====================================================
// EXPORTS
// =====================================================

export default {
  UniverseType,
  GalaxyType,
  DifficultyCurve,
  PlacementStrategy,
  BotPersonality,
  BotSkillLevel,
  ResourceDistributionPattern,
  AllianceFormationStrategy,
  MaintenanceTaskType
};
