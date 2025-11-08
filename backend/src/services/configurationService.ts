// Phase 7: Configuration Service
// Service layer for managing dynamic game configuration

import { Pool } from 'pg';
import { Redis } from 'ioredis';
import { Server as SocketIOServer } from 'socket.io';
import {
    ConfigParameterModel,
    ConfigParameterResponse,
    ConfigChangeHistoryModel,
    ConfigTemplateModel,
    ConfigCategoryResponse,
    ConfigParameterCreateRequest,
    ConfigParameterUpdateRequest,
    ConfigBulkUpdateRequest,
    ConfigTemplateCreateRequest,
    ConfigValidationResult,
    ConfigValidationError,
    ConfigValidationWarning,
    ConfigSnapshot,
    ConfigDataType,
    GameConfiguration,
    CombatConfig,
    ResourceConfig,
    BuildingConfig,
    ResearchConfig,
    FleetConfig,
    UniverseConfig,
    AllianceConfig,
    GameplayConfig,
    ConfigBulkUpdateResult,
    ConfigUpdateResult,
    ConfigExportOptions,
    ConfigImportRequest,
    ConfigDiffResult
} from '../types/configuration';

const GAME_CONFIG_SNAPSHOT_KEY = 'config:game_snapshot';
const CATEGORY_MAP: Record<string, keyof GameConfiguration> = {
    combat: 'combat',
    resources: 'resources',
    buildings: 'buildings',
    research: 'research',
    ships: 'fleet',
    fleet: 'fleet',
    universe: 'universe',
    alliances: 'alliance',
    alliance: 'alliance',
    gameplay: 'gameplay',
};

export class ConfigurationService {
    private pool: Pool;
    private redis: Redis;
    private io?: SocketIOServer;
    private configCache: Map<string, any>;
    private cacheTimeout: number = 300000; // 5 minutes

    constructor(pool: Pool, redis: Redis, io?: SocketIOServer) {
        this.pool = pool;
        this.redis = redis;
        this.io = io;
        this.configCache = new Map();
        this.initializeCache();
    }

    // ============================================
    // INITIALIZATION AND CACHING
    // ============================================

    private async initializeCache(): Promise<void> {
        try {
            const result = await this.pool.query(
                'SELECT parameter_key, current_value, data_type FROM config_parameters WHERE is_editable = TRUE'
            );

            for (const row of result.rows) {
                const value = this.parseConfigValue(row.current_value, row.data_type);
                this.configCache.set(row.parameter_key, value);
                await this.redis.set(
                    `config:${row.parameter_key}`,
                    JSON.stringify(value),
                    'EX',
                    this.cacheTimeout / 1000
                );
            }

            await this.refreshGameConfigSnapshot();

            console.log(`Configuration cache initialized with ${result.rowCount} parameters`);
        } catch (error) {
            console.error('Failed to initialize configuration cache:', error);
        }
    }

    async refreshCache(): Promise<void> {
        this.configCache.clear();
        await this.clearRedisConfigEntries();
        await this.initializeCache();
    }

    private async fetchCategoryFromDb(category: string): Promise<Record<string, any>> {
        const result = await this.pool.query(
            `SELECT cp.parameter_key, cp.current_value, cp.data_type
             FROM config_parameters cp
             JOIN config_categories cc ON cp.category_id = cc.category_id
             WHERE cc.category_name = $1 AND cp.is_editable = TRUE`,
            [category]
        );

        const config: Record<string, any> = {};
        for (const row of result.rows) {
            const key = row.parameter_key.split('.')[1];
            config[key] = this.parseConfigValue(row.current_value, row.data_type);
        }

        return config;
    }

    private async loadGameConfigFromDb(): Promise<GameConfiguration> {
        const [combat, resources, buildings, research, fleet, universe, alliance, gameplay] =
            await Promise.all([
                this.fetchCategoryFromDb('combat'),
                this.fetchCategoryFromDb('resources'),
                this.fetchCategoryFromDb('buildings'),
                this.fetchCategoryFromDb('research'),
                this.fetchCategoryFromDb('ships'),
                this.fetchCategoryFromDb('universe'),
                this.fetchCategoryFromDb('alliances'),
                this.fetchCategoryFromDb('gameplay'),
            ]);

        return {
            combat,
            resources,
            buildings,
            research,
            fleet,
            universe,
            alliance,
            gameplay,
        };
    }

    async getGameConfigSnapshot(force = false): Promise<GameConfiguration> {
        if (!force) {
            const cached = await this.redis.get(GAME_CONFIG_SNAPSHOT_KEY);
            if (cached) {
                return JSON.parse(cached);
            }
        }

        return this.refreshGameConfigSnapshot();
    }

    async refreshGameConfigSnapshot(): Promise<GameConfiguration> {
        const snapshot = await this.loadGameConfigFromDb();
        await this.redis.set(GAME_CONFIG_SNAPSHOT_KEY, JSON.stringify(snapshot));
        return snapshot;
    }

    private async invalidateGameConfigSnapshot(): Promise<void> {
        await this.redis.del(GAME_CONFIG_SNAPSHOT_KEY);
    }

    private async clearRedisConfigEntries(): Promise<void> {
        const keys = await this.redis.keys('config:*');
        if (keys.length) {
            await this.redis.del(...keys);
        }
    }

    // ============================================
    // GET CONFIGURATION VALUES
    // ============================================

    async getValue(key: string): Promise<any> {
        // Check memory cache first
        if (this.configCache.has(key)) {
            return this.configCache.get(key);
        }

        // Check Redis cache
        const cached = await this.redis.get(`config:${key}`);
        if (cached) {
            const value = JSON.parse(cached);
            this.configCache.set(key, value);
            return value;
        }

        // Load from database
        const result = await this.pool.query(
            'SELECT current_value, data_type FROM config_parameters WHERE parameter_key = $1',
            [key]
        );

        if (result.rows.length === 0) {
            throw new Error(`Configuration parameter not found: ${key}`);
        }

        const value = this.parseConfigValue(result.rows[0]?.current_value, result.rows[0]?.data_type);
        
        // Update caches
        this.configCache.set(key, value);
        await this.redis.set(`config:${key}`, JSON.stringify(value), 'EX', this.cacheTimeout / 1000);

        return value;
    }

    async getCategory(category: string): Promise<Record<string, any>> {
        const mapped = CATEGORY_MAP[category];
        if (mapped) {
            const snapshot = await this.getGameConfigSnapshot();
            const segment = snapshot[mapped];
            if (segment) {
                return { ...segment };
            }
        }
        return this.fetchCategoryFromDb(category);
    }

    async getCombatConfig(): Promise<CombatConfig> {
        const snapshot = await this.getGameConfigSnapshot();
        return snapshot.combat;
    }

    async getResourceConfig(): Promise<ResourceConfig> {
        const snapshot = await this.getGameConfigSnapshot();
        return snapshot.resources;
    }

    async getBuildingConfig(): Promise<BuildingConfig> {
        const snapshot = await this.getGameConfigSnapshot();
        return snapshot.buildings;
    }

    async getResearchConfig(): Promise<ResearchConfig> {
        const snapshot = await this.getGameConfigSnapshot();
        return snapshot.research;
    }

    async getFleetConfig(): Promise<FleetConfig> {
        const snapshot = await this.getGameConfigSnapshot();
        return snapshot.fleet;
    }

    async getUniverseConfig(): Promise<UniverseConfig> {
        const snapshot = await this.getGameConfigSnapshot();
        return snapshot.universe;
    }

    async getAllianceConfig(): Promise<AllianceConfig> {
        const snapshot = await this.getGameConfigSnapshot();
        return snapshot.alliance;
    }

    async getGameplayConfig(): Promise<GameplayConfig> {
        const snapshot = await this.getGameConfigSnapshot();
        return snapshot.gameplay;
    }

    async getAllConfig(): Promise<GameConfiguration> {
        const [combat, resources, buildings, research, fleet, universe, alliance, gameplay] =
            await Promise.all([
                this.getCombatConfig(),
                this.getResourceConfig(),
                this.getBuildingConfig(),
                this.getResearchConfig(),
                this.getFleetConfig(),
                this.getUniverseConfig(),
                this.getAllianceConfig(),
                this.getGameplayConfig()
            ]);

        return {
            combat,
            resources,
            buildings,
            research,
            fleet,
            universe,
            alliance,
            gameplay
        };
    }

    // ============================================
    // SET CONFIGURATION VALUES
    // ============================================

    async setValue(
        key: string,
        value: any,
        userId: number,
        reason?: string,
        options?: { suppressSnapshotRefresh?: boolean }
    ): Promise<ConfigUpdateResult> {
        const client = await this.pool.connect();
        
        try {
            await client.query('BEGIN');

            // Get parameter details
            const paramResult = await client.query(
                'SELECT parameter_id, current_value, data_type, requires_restart, min_value, max_value FROM config_parameters WHERE parameter_key = $1',
                [key]
            );

            if (paramResult.rows.length === 0) {
                throw new Error(`Configuration parameter not found: ${key}`);
            }

            const param = paramResult.rows[0];
            if (!param) {
                throw new Error('Parameter data is invalid');
            }
            const oldValue = this.parseConfigValue(param.current_value, param.data_type);

            let normalizedValue = value;
            if (param.data_type === ConfigDataType.NUMBER) {
                normalizedValue = Number(value);
                if (isNaN(normalizedValue)) {
                    throw new Error('Value must be a valid number');
                }

                if (key === 'gameplay.difficulty_factor') {
                    normalizedValue = Number(normalizedValue.toFixed(2));
                }
            }

            // Validate value
            const validation = await this.validateValue(key, normalizedValue, param);
            if (!validation.is_valid) {
                throw new Error(`Validation failed: ${validation.errors.map(e => e.message).join(', ')}`);
            }

            const stringValue = this.stringifyConfigValue(normalizedValue, param.data_type);

            // Record change history
            await client.query(
                'INSERT INTO config_change_history (parameter_id, old_value, new_value, changed_by, change_reason) VALUES ($1, $2, $3, $4, $5)',
                [param.parameter_id, param.current_value, stringValue, userId, reason]
            );

            // Update parameter
            await client.query(
                'UPDATE config_parameters SET current_value = $1, updated_at = CURRENT_TIMESTAMP WHERE parameter_id = $2',
                [stringValue, param.parameter_id]
            );

            await client.query('COMMIT');

            // Invalidate caches
            this.configCache.delete(key);
            await this.redis.del(`config:${key}`);
            await this.invalidateGameConfigSnapshot();
            if (!options?.suppressSnapshotRefresh) {
                await this.refreshGameConfigSnapshot();
            }
            
            // Broadcast change event via Redis pub/sub
            await this.redis.publish('config:changed', JSON.stringify({
                key,
                oldValue,
                newValue: normalizedValue,
                requires_restart: param.requires_restart,
                userId,
                timestamp: new Date()
            }));

            // Broadcast via Socket.io if available
            if (this.io) {
                // Get username for broadcast
                const userResult = await client.query('SELECT username FROM users WHERE id = $1', [userId]);
                const username = userResult.rows[0]?.username || 'Unknown';

                this.io.to('config:updates').emit('config:changed', {
                    key,
                    oldValue,
                    newValue: normalizedValue,
                    changedBy: userId,
                    changedByUsername: username,
                    requiresRestart: param.requires_restart,
                    timestamp: new Date()
                });
            }

            return {
                success: true,
                parameter_key: key,
                old_value: oldValue,
                new_value: normalizedValue,
                requires_restart: param.requires_restart
            };

        } catch (error) {
            await client.query('ROLLBACK');
            throw error;
        } finally {
            client.release();
        }
    }

    async bulkUpdate(
        updates: ConfigBulkUpdateRequest,
        userId: number
    ): Promise<ConfigBulkUpdateResult> {
        const results: ConfigUpdateResult[] = [];
        let requiresRestart = false;

        for (const update of updates.updates) {
            try {
                const result = await this.setValue(
                    update.parameter_key,
                    update.value,
                    userId,
                    updates.change_reason,
                    { suppressSnapshotRefresh: true }
                );
                results.push(result);
                if (result.requires_restart) {
                    requiresRestart = true;
                }
            } catch (error: any) {
                results.push({
                    success: false,
                    parameter_key: update.parameter_key,
                    old_value: '',
                    new_value: update.value,
                    requires_restart: false,
                    message: error.message
                });
            }
        }

        const successCount = results.filter(r => r.success).length;
        const failedCount = results.filter(r => !r.success).length;

        // Broadcast bulk update via Socket.io if available
        if (this.io && successCount > 0) {
            const userResult = await this.pool.query('SELECT username FROM users WHERE id = $1', [userId]);
            const username = userResult.rows[0]?.username || 'Unknown';

            const changes = results
                .filter(r => r.success)
                .map(r => ({
                    key: r.parameter_key,
                    oldValue: r.old_value,
                    newValue: r.new_value
                }));

            this.io.to('config:updates').emit('config:bulk_update', {
                changes,
                changedBy: userId,
                changedByUsername: username,
                requiresRestart,
                timestamp: new Date()
            });
        }

        if (successCount > 0) {
            await this.refreshGameConfigSnapshot();
        }

        return {
            success: failedCount === 0,
            updated_count: successCount,
            failed_count: failedCount,
            results,
            requires_restart: requiresRestart
        };
    }

    // ============================================
    // VALIDATION
    // ============================================

    private async validateValue(
        key: string,
        value: any,
        param: any
    ): Promise<ConfigValidationResult> {
        const errors: ConfigValidationError[] = [];
        const warnings: ConfigValidationWarning[] = [];

        // Type validation
        if (param.data_type === ConfigDataType.NUMBER) {
            if (typeof value !== 'number' || isNaN(value)) {
                errors.push({
                    parameter_key: key,
                    error_type: 'type_mismatch',
                    message: `Value must be a number`
                });
            }
            
            // Range validation
            if (param.min_value !== null && value < param.min_value) {
                errors.push({
                    parameter_key: key,
                    error_type: 'out_of_range',
                    message: `Value must be >= ${param.min_value}`
                });
            }
            
            if (param.max_value !== null && value > param.max_value) {
                errors.push({
                    parameter_key: key,
                    error_type: 'out_of_range',
                    message: `Value must be <= ${param.max_value}`
                });
            }
        } else if (param.data_type === ConfigDataType.BOOLEAN) {
            if (typeof value !== 'boolean') {
                errors.push({
                    parameter_key: key,
                    error_type: 'type_mismatch',
                    message: `Value must be a boolean`
                });
            }
        } else if (param.data_type === ConfigDataType.STRING) {
            if (typeof value !== 'string') {
                errors.push({
                    parameter_key: key,
                    error_type: 'type_mismatch',
                    message: `Value must be a string`
                });
            }
        }

        // Warn about significant changes
        if (param.data_type === ConfigDataType.NUMBER) {
            const oldValue = parseFloat(param.current_value);
            const changePercent = Math.abs((value - oldValue) / oldValue) * 100;
            
            if (changePercent > 50) {
                warnings.push({
                    parameter_key: key,
                    warning_type: 'significant_change',
                    message: `Value changed by ${changePercent.toFixed(1)}% - this may significantly impact gameplay`
                });
            }
        }

        return {
            is_valid: errors.length === 0,
            errors,
            warnings
        };
    }

    // ============================================
    // HISTORY AND ROLLBACK
    // ============================================

    async getChangeHistory(
        parameterKey?: string,
        limit: number = 100
    ): Promise<ConfigChangeHistoryModel[]> {
        let query = `
            SELECT ch.*, cp.parameter_key, cp.parameter_name, u.username as changed_by_username
            FROM config_change_history ch
            JOIN config_parameters cp ON ch.parameter_id = cp.parameter_id
            LEFT JOIN users u ON ch.changed_by = u.user_id
        `;
        
        const params: any[] = [];
        
        if (parameterKey) {
            query += ' WHERE cp.parameter_key = $1';
            params.push(parameterKey);
        }
        
        query += ' ORDER BY ch.applied_at DESC LIMIT $' + (params.length + 1);
        params.push(limit);

        const result = await this.pool.query(query, params);
        return result.rows;
    }

    async rollbackChange(changeId: number, userId: number): Promise<boolean> {
        const result = await this.pool.query(
            'SELECT rollback_config_change($1, $2)',
            [changeId, userId]
        );

        if (result.rows[0]?.rollback_config_change) {
            await this.refreshCache();
            return true;
        }

        return false;
    }

    // ============================================
    // TEMPLATES
    // ============================================

    async createTemplate(
        name: string,
        description: string | undefined,
        userId: number,
        categories?: string[]
    ): Promise<ConfigTemplateModel> {
        const snapshot = await this.exportConfig({ categories });

        const result = await this.pool.query(
            'INSERT INTO config_templates (template_name, description, template_data, created_by) VALUES ($1, $2, $3, $4) RETURNING *',
            [name, description, snapshot, userId]
        );

        if (result.rows.length === 0) {
          throw new Error('Template not found');
        }
        return result.rows[0];
    }

    async getTemplates(isPublic?: boolean): Promise<ConfigTemplateModel[]> {
        let query = 'SELECT * FROM config_templates';
        const params: any[] = [];

        if (isPublic !== undefined) {
            query += ' WHERE is_public = $1';
            params.push(isPublic);
        }

        query += ' ORDER BY created_at DESC';

        const result = await this.pool.query(query, params);
        return result.rows;
    }

    async applyTemplate(templateId: number, userId: number): Promise<ConfigBulkUpdateResult> {
        const template = await this.pool.query(
            'SELECT template_data FROM config_templates WHERE template_id = $1',
            [templateId]
        );

        if (template.rows.length === 0) {
            throw new Error('Template not found');
        }

        if (template.rows.length === 0) {
          throw new Error('Template not found');
        }
        const data = template.rows[0]?.template_data;
        if (!data) {
          throw new Error('Template data is invalid');
        }
        const updates = Object.entries(data).map(([key, value]) => ({
            parameter_key: key,
            value: value as string
        }));

        // Update usage count
        await this.pool.query(
            'UPDATE config_templates SET usage_count = usage_count + 1 WHERE template_id = $1',
            [templateId]
        );

        return await this.bulkUpdate({ updates, change_reason: `Applied template #${templateId}` }, userId);
    }

    // ============================================
    // IMPORT/EXPORT
    // ============================================

    async exportConfig(options: ConfigExportOptions = {}): Promise<Record<string, any>> {
        let query = `
            SELECT cp.parameter_key, cp.current_value, cp.data_type
            FROM config_parameters cp
            JOIN config_categories cc ON cp.category_id = cc.category_id
            WHERE cp.is_editable = TRUE
        `;

        const params: any[] = [];

        if (options.categories && options.categories.length > 0) {
            query += ' AND cc.category_name = ANY($1)';
            params.push(options.categories);
        }

        const result = await this.pool.query(query, params);

        const config: Record<string, any> = {};
        for (const row of result.rows) {
            config[row.parameter_key] = this.parseConfigValue(row.current_value, row.data_type);
        }

        return config;
    }

    async importConfig(data: Record<string, any>, userId: number, validateOnly: boolean = false): Promise<ConfigValidationResult | ConfigBulkUpdateResult> {
        const errors: ConfigValidationError[] = [];
        const warnings: ConfigValidationWarning[] = [];

        // Validate all parameters exist
        for (const key of Object.keys(data)) {
            const exists = await this.pool.query(
                'SELECT parameter_id FROM config_parameters WHERE parameter_key = $1',
                [key]
            );

            if (exists.rows.length === 0) {
                errors.push({
                    parameter_key: key,
                    error_type: 'not_found',
                    message: `Configuration parameter does not exist: ${key}`
                });
            }
        }

        if (validateOnly || errors.length > 0) {
            return {
                is_valid: errors.length === 0,
                errors,
                warnings
            };
        }

        // Apply import
        const updates = Object.entries(data).map(([key, value]) => ({
            parameter_key: key,
            value
        }));

        return await this.bulkUpdate({ updates, change_reason: 'Configuration import' }, userId);
    }

    async compareConfigs(config1: Record<string, any>, config2: Record<string, any>): Promise<ConfigDiffResult> {
        const keys1 = new Set(Object.keys(config1));
        const keys2 = new Set(Object.keys(config2));

        const added = Array.from(keys2).filter(k => !keys1.has(k));
        const removed = Array.from(keys1).filter(k => !keys2.has(k));
        const modified = Array.from(keys1)
            .filter(k => keys2.has(k) && config1[k] !== config2[k])
            .map(k => ({
                key: k,
                old_value: config1[k],
                new_value: config2[k]
            }));

        return { added, modified, removed };
    }

    // ============================================
    // UTILITY METHODS
    // ============================================

    private parseConfigValue(value: string, dataType: string): any {
        switch (dataType) {
            case ConfigDataType.NUMBER:
                return parseFloat(value);
            case ConfigDataType.BOOLEAN:
                return value === 'true';
            case ConfigDataType.JSON:
                return JSON.parse(value);
            case ConfigDataType.STRING:
            case ConfigDataType.FORMULA:
            default:
                return value;
        }
    }

    private stringifyConfigValue(value: any, dataType: string): string {
        switch (dataType) {
            case ConfigDataType.JSON:
                return JSON.stringify(value);
            case ConfigDataType.BOOLEAN:
                return value ? 'true' : 'false';
            default:
                return String(value);
        }
    }

    async resetToDefaults(category?: string, userId?: number): Promise<number> {
        let query = `
            UPDATE config_parameters 
            SET current_value = default_value, 
                updated_at = CURRENT_TIMESTAMP
            WHERE is_editable = TRUE
        `;

        const params: any[] = [];

        if (category) {
            query += ` AND category_id = (SELECT category_id FROM config_categories WHERE category_name = $1)`;
            params.push(category);
        }

        const result = await this.pool.query(query, params);

        await this.refreshCache();

        return result.rowCount || 0;
    }

    async getSnapshot(): Promise<ConfigSnapshot> {
        const parameters = await this.exportConfig();
        const gameplayConfig = await this.getGameplayConfig();

        return {
            timestamp: new Date(),
            parameters,
            metadata: {
                version: '7.0.0',
                server_name: gameplayConfig.server_name,
                total_parameters: Object.keys(parameters).length
            }
        };
    }
}

export default ConfigurationService;
