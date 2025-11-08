/**
 * Game Configuration Adapter
 * Integrates ConfigurationService with existing game systems
 * Provides backward-compatible interface while using dynamic configuration
 */

import { ConfigurationService } from './configurationService';
import { pool } from '../config/database';
import { redis } from '../config/redis';
import { io } from '../index';
import * as DefaultConfig from '../config/gameConfig';

export class GameConfigAdapter {
    private static instance: GameConfigAdapter;
    private configService: ConfigurationService;
    private cache: Map<string, any>;
    private cacheTimeout: number = 60000; // 1 minute cache

    private constructor() {
        this.configService = new ConfigurationService(pool, redis, io);
        this.cache = new Map();
        
        // Subscribe to configuration changes to invalidate cache
        if (redis) {
            redis.subscribe('config:changed', (err) => {
                if (err) {
                    console.error('Failed to subscribe to config changes:', err);
                }
            });
            
            redis.on('message', (channel, message) => {
                if (channel === 'config:changed') {
                    try {
                        const change = JSON.parse(message);
                        this.cache.delete(change.key);
                        console.log(`[GameConfig] Cache invalidated for: ${change.key}`);
                    } catch (error) {
                        console.error('Failed to process config change:', error);
                    }
                }
            });
        }
    }

    public static getInstance(): GameConfigAdapter {
        if (!GameConfigAdapter.instance) {
            GameConfigAdapter.instance = new GameConfigAdapter();
        }
        return GameConfigAdapter.instance;
    }

    /**
     * Get a configuration value with caching and fallback
     */
    private async getConfigValue<T>(key: string, defaultValue: T): Promise<T> {
        // Check memory cache
        if (this.cache.has(key)) {
            const cached = this.cache.get(key);
            if (cached.expires > Date.now()) {
                return cached.value;
            }
            this.cache.delete(key);
        }

        try {
            // Try to get from configuration service
            const value = await this.configService.getValue(key);
            
            // Cache the value
            this.cache.set(key, {
                value,
                expires: Date.now() + this.cacheTimeout
            });
            
            return value;
        } catch (error) {
            // Fallback to default value
            console.warn(`[GameConfig] Using default for ${key}:`, defaultValue);
            return defaultValue;
        }
    }

    // ============================================
    // COMBAT CONFIGURATION
    // ============================================

    async getCombatMaxRounds(): Promise<number> {
        return this.getConfigValue('combat.max_rounds', 6);
    }

    async getCombatRapidFireMultiplier(): Promise<number> {
        return this.getConfigValue('combat.rapid_fire_multiplier', 1.0);
    }

    async getCombatShieldAbsorptionRate(): Promise<number> {
        return this.getConfigValue('combat.shield_absorption_rate', 1.0);
    }

    async getCombatHullDamageMultiplier(): Promise<number> {
        return this.getConfigValue('combat.hull_damage_multiplier', 1.0);
    }

    async getCombatDebrisFieldRate(): Promise<number> {
        return this.getConfigValue('combat.debris_field_rate', 0.3);
    }

    // ============================================
    // RESOURCE CONFIGURATION
    // ============================================

    async getResourceProductionMultiplier(): Promise<number> {
        return this.getConfigValue('resources.production_speed_multiplier', 1.0);
    }

    async getMetalProductionBase(): Promise<number> {
        return this.getConfigValue('resources.metal_production_base', 30);
    }

    async getCrystalProductionBase(): Promise<number> {
        return this.getConfigValue('resources.crystal_production_base', 20);
    }

    async getDeuteriumProductionBase(): Promise<number> {
        return this.getConfigValue('resources.deuterium_production_base', 10);
    }

    async getEnergyProductionBase(): Promise<number> {
        return this.getConfigValue('resources.energy_production_base', 20);
    }

    // ============================================
    // BUILDING CONFIGURATION
    // ============================================

    async getBuildingConstructionSpeedMultiplier(): Promise<number> {
        return this.getConfigValue('buildings.construction_speed_multiplier', 1.0);
    }

    async getBuildingCostMultiplier(): Promise<number> {
        return this.getConfigValue('buildings.cost_multiplier', 1.0);
    }

    async getBuildingTimeMultiplier(): Promise<number> {
        return this.getConfigValue('buildings.time_multiplier', 1.0);
    }

    async getBuildingQueueLimit(): Promise<number> {
        return this.getConfigValue('buildings.queue_limit', 5);
    }

    // ============================================
    // RESEARCH CONFIGURATION
    // ============================================

    async getResearchSpeedMultiplier(): Promise<number> {
        return this.getConfigValue('research.research_speed_multiplier', 1.0);
    }

    async getResearchCostMultiplier(): Promise<number> {
        return this.getConfigValue('research.cost_multiplier', 1.0);
    }

    async getResearchTimeMultiplier(): Promise<number> {
        return this.getConfigValue('research.time_multiplier', 1.0);
    }

    // ============================================
    // FLEET CONFIGURATION
    // ============================================

    async getFleetSpeedMultiplier(): Promise<number> {
        return this.getConfigValue('ships.fleet_speed_multiplier', 1.0);
    }

    async getFleetCostMultiplier(): Promise<number> {
        return this.getConfigValue('ships.cost_multiplier', 1.0);
    }

    async getFleetConstructionTimeMultiplier(): Promise<number> {
        return this.getConfigValue('ships.construction_time_multiplier', 1.0);
    }

    async getFleetCargoCapacityMultiplier(): Promise<number> {
        return this.getConfigValue('ships.cargo_capacity_multiplier', 1.0);
    }

    async getFleetFuelConsumptionMultiplier(): Promise<number> {
        return this.getConfigValue('ships.fuel_consumption_multiplier', 1.0);
    }

    // ============================================
    // UNIVERSE CONFIGURATION
    // ============================================

    async getGalaxyCount(): Promise<number> {
        return this.getConfigValue('universe.galaxy_count', 9);
    }

    async getSystemsPerGalaxy(): Promise<number> {
        return this.getConfigValue('universe.systems_per_galaxy', 499);
    }

    async getPositionsPerSystem(): Promise<number> {
        return this.getConfigValue('universe.positions_per_system', 15);
    }

    async getColonizationLimit(): Promise<number> {
        return this.getConfigValue('universe.colonization_limit', 9);
    }

    // ============================================
    // GAMEPLAY CONFIGURATION
    // ============================================

    async getBeginnerProtectionDays(): Promise<number> {
        return this.getConfigValue('gameplay.beginner_protection_days', 7);
    }

    async getInactivityDays(): Promise<number> {
        return this.getConfigValue('gameplay.inactivity_timeout_days', 28);
    }

    async getVacationModeMaxDays(): Promise<number> {
        return this.getConfigValue('gameplay.vacation_mode_max_days', 30);
    }

    async getGameplayDifficultyFactor(): Promise<number> {
        const rawValue = await this.getConfigValue('gameplay.difficulty_factor', 1.0);
        if (typeof rawValue !== 'number' || isNaN(rawValue)) {
            return 1.0;
        }
        // Clamp to supported bounds to avoid runaway difficulty changes
        const clamped = Math.min(5, Math.max(0.1, rawValue));
        return Math.round(clamped * 100) / 100;
    }

    async getAuthRateLimitWindowSeconds(): Promise<number> {
        return this.getConfigValue('gameplay.auth_rate_limit_window_seconds', 300);
    }

    async getAuthRateLimitMaxAttempts(): Promise<number> {
        return this.getConfigValue('gameplay.auth_rate_limit_max_attempts', 10);
    }

    async getAuthCaptchaFailureThreshold(): Promise<number> {
        return this.getConfigValue('gameplay.auth_captcha_failure_threshold', 3);
    }

    async getNotificationConfig() {
        try {
            const snapshot = await this.configService.getGameConfigSnapshot();
            return snapshot.notifications;
        } catch (error) {
            console.warn('[GameConfig] Failed to load notification config, returning defaults', error);
            return {
                email_provider: 'smtp',
                email_from_address: 'noreply@universus.game',
                email_from_name: 'Universus Command',
                queue_enabled: true
            };
        }
    }

    // ============================================
    // HELPER METHODS FOR COMPLEX CALCULATIONS
    // ============================================

    /**
     * Calculate resource production with configuration multipliers
     */
    async calculateResourceProduction(
        buildingType: string,
        buildingLevel: number,
        gameSpeed: number = 1
    ): Promise<number> {
        const building = DefaultConfig.BUILDINGS[buildingType];
        if (!building || !building.baseProduction) {
            return 0;
        }

        // Get configuration multipliers
        const productionMultiplier = await this.getResourceProductionMultiplier();
        
        // Get base production from configuration or default
        let baseProduction = building.baseProduction;
        if (buildingType === 'metal_mine') {
            baseProduction = await this.getMetalProductionBase();
        } else if (buildingType === 'crystal_mine') {
            baseProduction = await this.getCrystalProductionBase();
        } else if (buildingType === 'deuterium_synthesizer') {
            baseProduction = await this.getDeuteriumProductionBase();
        } else if (buildingType === 'solar_plant' || buildingType === 'fusion_reactor') {
            baseProduction = await this.getEnergyProductionBase();
        }

        // Calculate with multipliers
        const production = baseProduction * buildingLevel * Math.pow(building.productionMultiplier || 1.1, buildingLevel);
        const difficultyFactor = await this.getGameplayDifficultyFactor();
        const adjustedProduction = production * gameSpeed * productionMultiplier;

        return adjustedProduction / Math.max(difficultyFactor, 0.1);
    }

    /**
     * Calculate building time with configuration multipliers
     */
    async calculateBuildingTime(
        buildingType: string,
        level: number,
        roboticsLevel: number = 0,
        naniteLevel: number = 0
    ): Promise<number> {
        const building = DefaultConfig.BUILDINGS[buildingType];
        if (!building) {
            return 0;
        }

        // Get configuration multipliers
        const timeMultiplier = await this.getBuildingTimeMultiplier();
        const speedMultiplier = await this.getBuildingConstructionSpeedMultiplier();

        // Base time calculation
        const baseCost = building.baseCost.metal + building.baseCost.crystal;
        const cost = baseCost * Math.pow(building.costMultiplier, level - 1);
        const baseTime = (cost / 2500) * (1 / speedMultiplier);

        // Apply robotics and nanite bonuses
        const roboticsBonus = 1 - (roboticsLevel * 0.06);
        const naniteBonus = naniteLevel > 0 ? 1 / (2 ** naniteLevel) : 1;

        const difficultyFactor = await this.getGameplayDifficultyFactor();

        return baseTime * roboticsBonus * naniteBonus * timeMultiplier * Math.max(difficultyFactor, 0.1);
    }

    /**
     * Calculate ship build time with configuration multipliers
     */
    async calculateShipBuildTime(
        shipType: string,
        shipyardLevel: number = 10,
        naniteLevel: number = 0
    ): Promise<number> {
        const ship = DefaultConfig.SHIPS[shipType];
        if (!ship) {
            return 0;
        }

        // Get configuration multipliers
        const timeMultiplier = await this.getFleetConstructionTimeMultiplier();
        const difficultyFactor = await this.getGameplayDifficultyFactor();

        // Base time from ship config
        let baseTime = ship.buildTime;

        // Apply shipyard level bonus
        const shipyardBonus = 1 / (1 + shipyardLevel);
        
        // Apply nanite bonus
        const naniteBonus = naniteLevel > 0 ? 1 / (2 ** naniteLevel) : 1;

        return baseTime * shipyardBonus * naniteBonus * timeMultiplier * Math.max(difficultyFactor, 0.1);
    }

    /**
     * Calculate research time with configuration multipliers
     */
    async calculateResearchTime(
        researchType: string,
        level: number,
        labLevel: number = 10
    ): Promise<number> {
        const research = DefaultConfig.RESEARCH[researchType];
        if (!research) {
            return 0;
        }

        // Get configuration multipliers
        const timeMultiplier = await this.getResearchTimeMultiplier();
        const speedMultiplier = await this.getResearchSpeedMultiplier();
        const difficultyFactor = await this.getGameplayDifficultyFactor();

        // Base time calculation
        const baseCost = research.baseCost.metal + research.baseCost.crystal;
        const cost = baseCost * Math.pow(research.costMultiplier, level - 1);
        const baseTime = (cost / 1000) * (1 / speedMultiplier);

        // Apply lab level bonus
        const labBonus = 1 / (1 + labLevel);

        return baseTime * labBonus * timeMultiplier * Math.max(difficultyFactor, 0.1);
    }

    /**
     * Get all combat configuration at once
     */
    async getCombatConfig() {
        return {
            maxRounds: await this.getCombatMaxRounds(),
            rapidFireMultiplier: await this.getCombatRapidFireMultiplier(),
            shieldAbsorption: await this.getCombatShieldAbsorptionRate(),
            hullDamageMultiplier: await this.getCombatHullDamageMultiplier(),
            debrisFieldRate: await this.getCombatDebrisFieldRate()
        };
    }

    /**
     * Get all resource configuration at once
     */
    async getResourceConfig() {
        return {
            productionMultiplier: await this.getResourceProductionMultiplier(),
            metalBase: await this.getMetalProductionBase(),
            crystalBase: await this.getCrystalProductionBase(),
            deuteriumBase: await this.getDeuteriumProductionBase(),
            energyBase: await this.getEnergyProductionBase()
        };
    }
}

// Export singleton instance
export const gameConfig = GameConfigAdapter.getInstance();
