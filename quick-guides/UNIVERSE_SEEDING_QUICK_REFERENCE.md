# Universe Seeding System - Quick Reference Guide

**Last Updated:** 2025-11-06  
**Version:** 1.0.0

---

## Table of Contents
1. [API Endpoints](#api-endpoints)
2. [Service Methods](#service-methods)
3. [Database Schema](#database-schema)
4. [Configuration Options](#configuration-options)
5. [Common Use Cases](#common-use-cases)

---

## API Endpoints

### Universe Management

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| POST | `/api/universe/create` | Create new universe | Admin |
| GET | `/api/universe/:id/status` | Get universe status | Admin |
| GET | `/api/universe/list` | List all universes | Admin |
| POST | `/api/universe/:id/archive` | Archive universe | Admin |

### Galaxy Operations

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| POST | `/api/universe/:id/galaxy/generate` | Generate galaxy | Admin |
| GET | `/api/universe/:id/galaxy/:galaxyId` | Get galaxy details | Admin |

### Player Placement

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| POST | `/api/universe/:id/placement/calculate` | Calculate optimal position | Admin |
| POST | `/api/universe/:id/placement/place` | Place player | Admin |

### Bot Management

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| POST | `/api/universe/:id/bots/generate` | Generate bots | Admin |
| GET | `/api/universe/:id/bots/statistics` | Get bot stats | Admin |
| POST | `/api/universe/:id/bots/:botId/activate` | Activate bot | Admin |
| POST | `/api/universe/:id/bots/:botId/deactivate` | Deactivate bot | Admin |

### Analytics & Maintenance

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | `/api/universe/:id/analytics` | Get universe analytics | Admin |
| GET | `/api/universe/:id/maintenance/report` | Get maintenance report | Admin |
| POST | `/api/universe/:id/maintenance/trigger` | Trigger maintenance | Admin |

---

## API Request Examples

### Create Universe
```bash
curl -X POST http://localhost:3000/api/universe/create \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_ADMIN_TOKEN" \
  -d '{
    "name": "Alpha Universe",
    "size": "8x8",
    "galaxyTypes": ["spiral", "elliptical"],
    "playerCapacity": 5000,
    "botRatio": 0.5,
    "resourceAbundance": "medium",
    "difficultyProgression": "linear"
  }'
```

**Response:**
```json
{
  "universeId": 1,
  "status": "seeding",
  "configuration": { ... },
  "createdAt": "2025-11-06T07:46:17Z"
}
```

### Generate Galaxy
```bash
curl -X POST http://localhost:3000/api/universe/1/galaxy/generate \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_ADMIN_TOKEN" \
  -d '{
    "x": 5,
    "y": 5,
    "type": "spiral",
    "systemCount": 499
  }'
```

**Response:**
```json
{
  "galaxyId": 25,
  "coordinates": { "x": 5, "y": 5 },
  "type": "spiral",
  "systemCount": 499,
  "planetsGenerated": 2495
}
```

### Calculate Player Placement
```bash
curl -X POST http://localhost:3000/api/universe/1/placement/calculate \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_ADMIN_TOKEN" \
  -d '{
    "userId": 42,
    "skillLevel": "intermediate",
    "preferences": {
      "preferredGalaxy": 5
    }
  }'
```

**Response:**
```json
{
  "coordinates": {
    "galaxy": 5,
    "system": 123,
    "position": 8
  },
  "sector": 4,
  "difficulty": 1.5,
  "resourceAbundance": 1.0,
  "nearestNeighborDistance": 87
}
```

### Generate Bots
```bash
curl -X POST http://localhost:3000/api/universe/1/bots/generate \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_ADMIN_TOKEN" \
  -d '{
    "count": 500,
    "personalityDistribution": {
      "aggressive_conqueror": 0.15,
      "strategic_builder": 0.20,
      "diplomatic_negotiator": 0.10,
      "resource_hoarder": 0.15,
      "speed_rusher": 0.10,
      "tech_enthusiast": 0.15,
      "alliance_focused": 0.10,
      "solo_survivor": 0.05
    },
    "skillLevels": {
      "beginner": 0.20,
      "intermediate": 0.50,
      "advanced": 0.25,
      "expert": 0.05
    }
  }'
```

**Response:**
```json
{
  "botsCreated": 500,
  "distribution": {
    "byPersonality": { ... },
    "bySkillLevel": { ... }
  },
  "placementTime": 2.34
}
```

---

## Service Methods

### UniverseSeedingService

```typescript
// Create complete universe
const universe = await universeSeedingService.generateUniverse({
  name: string,
  size: '5x5' | '6x6' | '7x7' | '8x8' | '9x9' | '10x10',
  galaxyTypes: string[],
  playerCapacity: number,
  botRatio: number,
  resourceAbundance: 'low' | 'medium' | 'high',
  difficultyProgression: 'linear' | 'exponential'
});

// Generate single galaxy
const galaxy = await universeSeedingService.generateGalaxy(
  universeId: number,
  x: number,
  y: number,
  type: string,
  systemCount: number
);

// Seed bots
const bots = await universeSeedingService.seedBots(
  universeId: number,
  count: number,
  distribution: PersonalityDistribution
);

// Seed alliances
const alliances = await universeSeedingService.seedAlliances(
  universeId: number,
  allianceConfig: AllianceConfig
);

// Get universe status
const status = await universeSeedingService.getUniverseStatus(
  universeId: number
);

// Archive universe
await universeSeedingService.archiveUniverse(universeId: number);
```

### PlayerPlacementService

```typescript
// Calculate optimal placement
const placement = await playerPlacementService.calculateOptimalPlacement(
  userId: number,
  skillLevel: 'beginner' | 'intermediate' | 'advanced' | 'expert',
  preferences?: PlacementPreferences
);

// Place new player
const planet = await playerPlacementService.placeNewPlayer(
  userId: number,
  coordinates: Coordinates,
  universeId: number
);

// Calculate distance between coordinates
const distance = playerPlacementService.calculateDistance(
  coord1: Coordinates,
  coord2: Coordinates
);

// Evaluate player skill level
const skillLevel = await playerPlacementService.evaluateSkillLevel(
  userId: number
);
```

### BotGenerationService

```typescript
// Generate multiple bots
const bots = await botGenerationService.generateBots(
  universeId: number,
  count: number,
  personalityDistribution: PersonalityDistribution,
  skillLevels?: SkillLevelDistribution
);

// Generate single bot
const bot = await botGenerationService.generateSingleBot(
  universeId: number,
  personality: string,
  skillLevel: string
);

// Activate bot
await botGenerationService.activateBot(botId: number);

// Deactivate bot
await botGenerationService.deactivateBot(botId: number);

// Get bot statistics
const stats = await botGenerationService.getBotStatistics(
  universeId: number
);
```

### UniverseMaintenanceService

```typescript
// Perform maintenance
const report = await universeMaintenanceService.performMaintenance(
  universeId: number
);

// Manage bots (cleanup inactive)
const botsManaged = await universeMaintenanceService.manageBots(
  universeId: number
);

// Rebalance resources
const rebalanced = await universeMaintenanceService.rebalanceResources(
  universeId: number
);

// Monitor alliances
const allianceHealth = await universeMaintenanceService.monitorAlliances(
  universeId: number
);

// Generate maintenance report
const report = await universeMaintenanceService.generateMaintenanceReport(
  universeId: number
);
```

---

## Database Schema

### Tables

#### universe_seeds
```sql
id, name, size, galaxy_types, player_capacity, bot_ratio,
resource_abundance, difficulty_progression, status,
created_at, completed_at, configuration
```

#### galaxy_seeds
```sql
id, universe_id, galaxy_x, galaxy_y, type, system_count,
planet_count, resource_abundance_factor, created_at
```

#### player_placement
```sql
id, universe_id, user_id, placement_algorithm, galaxy, system,
position, sector, skill_level, placement_timestamp
```

#### bot_generation
```sql
id, universe_id, bot_id, personality_type, skill_level,
initial_resources, sector_assignment, activation_status,
created_at
```

#### resource_distribution
```sql
id, universe_id, sector, metal_abundance, crystal_abundance,
deuterium_abundance, scarcity_factor, supply_demand_model,
last_updated
```

#### difficulty_balancing
```sql
id, universe_id, sector, difficulty_level, experience_required,
challenge_scaling, progression_type, adjusted_at
```

#### alliance_seeding
```sql
id, universe_id, alliance_id, alliance_type, initial_members,
territory_assignment, startup_resources, formation_date
```

#### universe_maintenance
```sql
id, universe_id, maintenance_type, execution_timestamp,
bots_managed, resources_rebalanced, alliances_monitored,
status, details
```

### Views

#### universe_analytics_view
Aggregated universe statistics including player count, bot count, galaxy count, resource distribution, and activity metrics.

#### player_distribution_view
Player distribution across galaxies and sectors with density calculations.

### Functions

#### calculate_sector_from_coordinates(galaxy, system)
Returns sector number (1-10) based on coordinate distance from center.

#### calculate_distance_between_coordinates(coord1, coord2)
Returns numerical distance between two coordinate sets.

#### get_resource_abundance_for_sector(universe_id, sector)
Returns resource abundance multipliers for a specific sector.

---

## Configuration Options

### Universe Sizes

| Size | Galaxies | Max Systems/Galaxy | Max Players |
|------|----------|-------------------|-------------|
| 5x5 | 25 | 499 | 1,000 |
| 6x6 | 36 | 499 | 2,000 |
| 7x7 | 49 | 499 | 3,500 |
| 8x8 | 64 | 499 | 5,000 |
| 9x9 | 81 | 499 | 7,500 |
| 10x10 | 100 | 499 | 10,000 |

### Galaxy Types

- **spiral** - Classic spiral arm structure
- **elliptical** - Elliptical distribution
- **irregular** - Irregular cluster patterns

### Bot Personalities (8 Types)

| Personality | Focus | Aggression | Economy | Description |
|------------|-------|------------|---------|-------------|
| Aggressive Conqueror | Military | 0.9 | 0.4 | High military focus |
| Strategic Builder | Balanced | 0.5 | 0.8 | Economy focused |
| Diplomatic Negotiator | Alliance | 0.2 | 0.7 | Peaceful diplomat |
| Resource Hoarder | Economy | 0.3 | 0.95 | Resource maximizer |
| Speed Rusher | Early Game | 0.8 | 0.5 | Fast early aggression |
| Tech Enthusiast | Research | 0.4 | 0.6 | Technology focused |
| Alliance-Focused | Team | 0.5 | 0.7 | Team coordination |
| Solo Survivor | Defense | 0.3 | 0.8 | Independent defender |

### Skill Levels

- **beginner** - New players, low difficulty sectors (1-3)
- **intermediate** - Average players, medium sectors (4-6)
- **advanced** - Experienced players, high sectors (7-8)
- **expert** - Elite players, extreme sectors (9-10)

### Resource Abundance

- **low** - 0.6-0.8x base production
- **medium** - 0.9-1.1x base production  
- **high** - 1.2-1.5x base production

### Difficulty Progression

- **linear** - Steady linear increase across sectors
- **exponential** - Rapid exponential scaling in outer sectors

---

## Common Use Cases

### 1. Launch New Universe

```typescript
// Step 1: Create universe
const universe = await universeSeedingService.generateUniverse({
  name: 'Beta Universe',
  size: '8x8',
  galaxyTypes: ['spiral', 'elliptical'],
  playerCapacity: 5000,
  botRatio: 0.5,
  resourceAbundance: 'medium',
  difficultyProgression: 'linear'
});

// Step 2: Monitor progress
const status = await universeSeedingService.getUniverseStatus(universe.id);
console.log(`Progress: ${status.progress}%`);

// Step 3: Wait for completion
while (status.status !== 'active') {
  await new Promise(resolve => setTimeout(resolve, 5000));
  status = await universeSeedingService.getUniverseStatus(universe.id);
}

console.log('Universe ready!');
```

### 2. Place New Player

```typescript
// Calculate best position
const placement = await playerPlacementService.calculateOptimalPlacement(
  userId,
  'intermediate'
);

// Place player
const planet = await playerPlacementService.placeNewPlayer(
  userId,
  placement.coordinates,
  universeId
);

console.log(`Player placed at ${planet.galaxy}:${planet.system}:${planet.position}`);
```

### 3. Generate Bot Population

```typescript
// Generate 500 bots with balanced distribution
const bots = await botGenerationService.generateBots(
  universeId,
  500,
  {
    'aggressive_conqueror': 0.15,
    'strategic_builder': 0.20,
    'diplomatic_negotiator': 0.10,
    'resource_hoarder': 0.15,
    'speed_rusher': 0.10,
    'tech_enthusiast': 0.15,
    'alliance_focused': 0.10,
    'solo_survivor': 0.05
  },
  {
    'beginner': 0.20,
    'intermediate': 0.50,
    'advanced': 0.25,
    'expert': 0.05
  }
);

console.log(`Generated ${bots.botsCreated} bots`);
```

### 4. Perform Regular Maintenance

```typescript
// Run maintenance (can be scheduled with cron)
const report = await universeMaintenanceService.performMaintenance(universeId);

console.log(`Maintenance complete:
  - Bots managed: ${report.botsManaged}
  - Resources rebalanced: ${report.resourcesRebalanced}
  - Alliances monitored: ${report.alliancesMonitored}
`);
```

### 5. Get Universe Analytics

```typescript
const analytics = await fetch(
  `http://localhost:3000/api/universe/${universeId}/analytics`,
  {
    headers: { 'Authorization': `Bearer ${adminToken}` }
  }
).then(r => r.json());

console.log(`Universe Analytics:
  Players: ${analytics.playerDistribution.total}
  Active Bots: ${analytics.botStats.active}
  Resource Balance: ${analytics.resourceBalance.overall}
  Avg Difficulty: ${analytics.difficultyMetrics.average}
`);
```

---

## Integration with Existing Systems

### Phase 2: Admin System
- Bot personalities integrated from Phase 2
- Admin authentication required for all endpoints
- Monitoring integration via admin dashboard

### Phase 3: Debris System
- Resource distribution affects debris generation rates
- Sector difficulty influences salvage complexity
- Bot AI uses debris system for resource gathering

### Game Loop Integration
Universe maintenance can be scheduled in game loop:

```typescript
// In gameLoopService.ts
setInterval(async () => {
  const universes = await getActiveUniverses();
  for (const universe of universes) {
    await universeMaintenanceService.performMaintenance(universe.id);
  }
}, 3600000); // Every hour
```

---

## Performance Considerations

### Universe Generation
- Small universe (5x5): ~30-60 seconds
- Medium universe (7x7): ~2-4 minutes
- Large universe (10x10): ~5-10 minutes

### Database Indexes
All critical queries are indexed:
- Universe lookup by ID and status
- Galaxy lookup by coordinates
- Player placement by user and universe
- Bot lookup by universe and personality

### Recommended Practices
1. Generate universes during off-peak hours
2. Monitor maintenance task duration
3. Archive inactive universes regularly
4. Cache frequently accessed analytics
5. Use batch operations for bot generation

---

## Troubleshooting

### Universe Generation Stuck
```typescript
// Check status
const status = await universeSeedingService.getUniverseStatus(universeId);

// If stuck, check logs
const maintenanceReport = await fetch(
  `http://localhost:3000/api/universe/${universeId}/maintenance/report`
);
```

### Player Placement Fails
```typescript
// Check available space
const analytics = await fetch(
  `http://localhost:3000/api/universe/${universeId}/analytics`
);

// Verify skill level matches available sectors
const placement = await playerPlacementService.calculateOptimalPlacement(
  userId,
  'intermediate',
  { allowAnySkillLevel: true } // Fallback option
);
```

### Bot Generation Issues
```typescript
// Check bot statistics
const stats = await botGenerationService.getBotStatistics(universeId);

// Verify personality distribution sums to 1.0
const distribution = {
  'aggressive_conqueror': 0.15,
  // ... ensure total === 1.0
};
```

---

## Support & Documentation

### Full Documentation
- `UNIVERSE_SEEDING_COMPLETE.md` - Complete implementation details
- `UNIVERSE_SEEDING_QUICK_REFERENCE.md` - This document

### Code Files
- Database: `backend/src/database/universe_seeding_schema.sql`
- Types: `backend/src/types/universe.ts`
- Services: `backend/src/services/universe*.ts`
- Routes: `backend/src/routes/universeRoutes.ts`

### Contact
For issues or questions, refer to project documentation or admin panel logs.

---

**Last Updated:** 2025-11-06  
**System Version:** 1.0.0  
**Status:** Production Ready ✅
