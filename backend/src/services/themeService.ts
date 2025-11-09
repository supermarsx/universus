/**
 * @module backend/services/themeService
 *
 * Theme management for seasonal and custom themes. Provides creation,
 * updates, scheduling and activation helpers for theme assets and
 * configuration. Intended to be used by the theme admin UI and scheduled
 * activation tasks.
 */

// =====================================================
// Phase 8: Seasonal Theme System - Theme Service
// =====================================================

import pool from '../config/database';
import CustomCssSanitizer from '../utils/customCssSanitizer';
import {
    Theme,
    ThemeSchedule,
    ThemeAsset,
    ThemeConfiguration,
    ThemeActivation,
    ThemePreferences,
    ThemeCategory,
    ThemeActivationType,
    ThemeAssetType,
    AssetLoadStrategy,
    TransitionType,
    CreateThemeRequest,
    UpdateThemeRequest,
    CreateThemeScheduleRequest,
    UpdateThemeScheduleRequest,
    CreateThemeAssetRequest,
    UpdateThemeAssetRequest,
    ThemeActivationRequest,
    UpdateThemePreferencesRequest,
    ActiveThemeData
} from '../types/seasonalTheme';

export class ThemeService {
    private static readonly MAX_CUSTOM_CSS_LENGTH = 8000;

    /**
     * Get all themes with optional filtering
     */
    static async getAllThemes(filters?: {
        category?: ThemeCategory;
        is_active?: boolean;
        is_available?: boolean;
    }): Promise<Theme[]> {
        let query = 'SELECT * FROM themes WHERE 1=1';
        const params: any[] = [];
        let paramCount = 1;

        if (filters?.category) {
            query += ` AND category = $${paramCount++}`;
            params.push(filters.category);
        }

        if (filters?.is_active !== undefined) {
            query += ` AND is_active = $${paramCount++}`;
            params.push(filters.is_active);
        }

        if (filters?.is_available !== undefined) {
            query += ` AND is_available = $${paramCount++}`;
            params.push(filters.is_available);
        }

        query += ' ORDER BY load_priority DESC, name ASC';

        const result = await pool.query(query, params);
        return result.rows;
    }

    /**
     * Get theme by ID
     */
    static async getThemeById(themeId: number): Promise<Theme | null> {
        const result = await pool.query(
            'SELECT * FROM themes WHERE id = $1',
            [themeId]
        );
        return result.rows[0] || null;
    }

    /**
     * Get theme by key
     */
    static async getThemeByKey(themeKey: string): Promise<Theme | null> {
        const result = await pool.query(
            'SELECT * FROM themes WHERE theme_key = $1',
            [themeKey]
        );
        return result.rows[0] || null;
    }

    /**
     * Create new theme
     */
    static async createTheme(data: CreateThemeRequest, userId?: number): Promise<Theme> {
        const result = await pool.query(
            `INSERT INTO themes (
                theme_key, name, description, category,
                primary_color, secondary_color, accent_color,
                background_color, text_color,
                visual_effects, sound_effects, animations, decorations,
                css_variables, custom_css, created_by
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            RETURNING *`,
            [
                data.theme_key,
                data.name,
                data.description,
                data.category,
                data.primary_color,
                data.secondary_color,
                data.accent_color,
                data.background_color,
                data.text_color,
                JSON.stringify(data.visual_effects || {}),
                JSON.stringify(data.sound_effects || {}),
                JSON.stringify(data.animations || {}),
                JSON.stringify(data.decorations || {}),
                JSON.stringify(data.css_variables || {}),
                data.custom_css,
                userId
            ]
        );
        return result.rows[0];
    }

    /**
     * Update theme
     */
    static async updateTheme(themeId: number, data: UpdateThemeRequest, userId?: number): Promise<Theme | null> {
        const updates: string[] = [];
        const params: any[] = [];
        let paramCount = 1;

        // Build dynamic update query
        const fields: (keyof UpdateThemeRequest)[] = [
            'name', 'description', 'category',
            'primary_color', 'secondary_color', 'accent_color',
            'background_color', 'text_color',
            'is_active', 'is_available', 'preview_mode',
            'custom_css'
        ];

        fields.forEach(field => {
            if (data[field] !== undefined) {
                updates.push(`${field} = $${paramCount++}`);
                params.push(data[field]);
            }
        });

        // Handle JSON fields
        const jsonFields: (keyof UpdateThemeRequest)[] = [
            'visual_effects', 'sound_effects', 'animations', 'decorations', 'css_variables'
        ];

        jsonFields.forEach(field => {
            if (data[field] !== undefined) {
                updates.push(`${field} = $${paramCount++}`);
                params.push(JSON.stringify(data[field]));
            }
        });

        if (userId) {
            updates.push(`updated_by = $${paramCount++}`);
            params.push(userId);
        }

        if (updates.length === 0) {
            return this.getThemeById(themeId);
        }

        params.push(themeId);

        const result = await pool.query(
            `UPDATE themes SET ${updates.join(', ')}, updated_at = CURRENT_TIMESTAMP
             WHERE id = $${paramCount} RETURNING *`,
            params
        );

        return result.rows[0] || null;
    }

    /**
     * Delete theme
     */
    static async deleteTheme(themeId: number): Promise<boolean> {
        const result = await pool.query(
            'DELETE FROM themes WHERE id = $1',
            [themeId]
        );
        return (result.rowCount ?? 0) > 0;
    }

    /**
     * Get current active theme
     */
    static async getCurrentTheme(): Promise<Theme | null> {
        const result = await pool.query(
            'SELECT * FROM v_current_theme LIMIT 1'
        );
        return result.rows[0] || null;
    }

    /**
     * Get active theme with full details (assets, configurations)
     */
    static async getActiveThemeData(): Promise<ActiveThemeData | null> {
        const theme = await this.getCurrentTheme();
        if (!theme) return null;

        const assets = await this.getThemeAssets(theme.id);
        
        return {
            theme,
            assets,
            cssVariables: theme.css_variables,
            customCSS: theme.custom_css
        };
    }

    /**
     * Activate theme manually
     */
    static async activateTheme(
        themeId: number,
        userId?: number,
        activationType: ThemeActivationType = ThemeActivationType.MANUAL,
        reason?: string
    ): Promise<ThemeActivation> {
        // Deactivate current theme
        await pool.query('UPDATE themes SET is_active = false WHERE is_active = true');

        // Activate new theme
        await pool.query(
            'UPDATE themes SET is_active = true WHERE id = $1',
            [themeId]
        );

        // Log activation
        const result = await pool.query(
            `INSERT INTO theme_activations (
                theme_id, activation_type, activated_by, activation_reason
            ) VALUES ($1, $2, $3, $4) RETURNING *`,
            [themeId, activationType, userId, reason]
        );

        return result.rows[0];
    }

    /**
     * Deactivate theme
     */
    static async deactivateTheme(themeId: number): Promise<void> {
        await pool.query(
            'UPDATE themes SET is_active = false WHERE id = $1',
            [themeId]
        );

        // Update activation record
        await pool.query(
            `UPDATE theme_activations 
             SET deactivated_at = CURRENT_TIMESTAMP,
                 duration_seconds = EXTRACT(EPOCH FROM (CURRENT_TIMESTAMP - activated_at))
             WHERE theme_id = $1 AND deactivated_at IS NULL`,
            [themeId]
        );
    }

    /**
     * Check and activate scheduled themes (called by scheduler)
     */
    static async checkScheduledThemes(): Promise<{ activated: boolean; theme?: Theme }> {
        const result = await pool.query<{ activate_scheduled_theme: number | null }>(
            'SELECT activate_scheduled_theme() as theme_id'
        );

        const themeId = result.rows[0]?.activate_scheduled_theme;
        
        if (themeId) {
            const theme = await this.getThemeById(themeId);
            return { activated: true, theme: theme || undefined };
        }

        return { activated: false };
    }

    /**
     * Theme Schedules
     */

    static async getAllSchedules(themeId?: number): Promise<ThemeSchedule[]> {
        let query = 'SELECT * FROM theme_schedules WHERE 1=1';
        const params: any[] = [];

        if (themeId) {
            query += ' AND theme_id = $1';
            params.push(themeId);
        }

        query += ' ORDER BY priority DESC, start_date ASC';

        const result = await pool.query(query, params);
        return result.rows;
    }

    static async getScheduleById(scheduleId: number): Promise<ThemeSchedule | null> {
        const result = await pool.query(
            'SELECT * FROM theme_schedules WHERE id = $1',
            [scheduleId]
        );
        return result.rows[0] || null;
    }

    static async createSchedule(data: CreateThemeScheduleRequest, userId?: number): Promise<ThemeSchedule> {
        const result = await pool.query(
            `INSERT INTO theme_schedules (
                theme_id, schedule_name, start_date, end_date,
                start_time, end_time, is_recurring, priority,
                transition_duration, transition_type, created_by
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) RETURNING *`,
            [
                data.theme_id,
                data.schedule_name,
                data.start_date,
                data.end_date,
                data.start_time || '00:00:00',
                data.end_time || '23:59:59',
                data.is_recurring ?? true,
                data.priority ?? 0,
                data.transition_duration ?? 1000,
                data.transition_type ?? TransitionType.FADE,
                userId
            ]
        );
        return result.rows[0];
    }

    static async updateSchedule(
        scheduleId: number,
        data: UpdateThemeScheduleRequest
    ): Promise<ThemeSchedule | null> {
        const updates: string[] = [];
        const params: any[] = [];
        let paramCount = 1;

        const fields: (keyof UpdateThemeScheduleRequest)[] = [
            'schedule_name', 'start_date', 'end_date', 'start_time', 'end_time',
            'is_recurring', 'priority', 'transition_duration', 'transition_type',
            'enabled'
        ];

        fields.forEach(field => {
            if (data[field] !== undefined) {
                updates.push(`${field} = $${paramCount++}`);
                params.push(data[field]);
            }
        });

        if (updates.length === 0) {
            return this.getScheduleById(scheduleId);
        }

        params.push(scheduleId);

        const result = await pool.query(
            `UPDATE theme_schedules SET ${updates.join(', ')}, updated_at = CURRENT_TIMESTAMP
             WHERE id = $${paramCount} RETURNING *`,
            params
        );

        return result.rows[0] || null;
    }

    static async deleteSchedule(scheduleId: number): Promise<boolean> {
        const result = await pool.query(
            'DELETE FROM theme_schedules WHERE id = $1',
            [scheduleId]
        );
        return (result.rowCount ?? 0) > 0;
    }

    static async getActiveSchedules(): Promise<ThemeSchedule[]> {
        const result = await pool.query(
            'SELECT * FROM v_active_theme_schedules'
        );
        return result.rows;
    }

    /**
     * Theme Assets
     */

    static async getThemeAssets(themeId: number, usageContext?: string): Promise<ThemeAsset[]> {
        let query = 'SELECT * FROM theme_assets WHERE theme_id = $1 AND is_active = true';
        const params: any[] = [themeId];

        if (usageContext) {
            query += ' AND usage_context = $2';
            params.push(usageContext);
        }

        query += ' ORDER BY z_index ASC, id ASC';

        const result = await pool.query(query, params);
        return result.rows;
    }

    static async getAssetById(assetId: number): Promise<ThemeAsset | null> {
        const result = await pool.query(
            'SELECT * FROM theme_assets WHERE id = $1',
            [assetId]
        );
        return result.rows[0] || null;
    }

    static async createAsset(data: CreateThemeAssetRequest): Promise<ThemeAsset> {
        const result = await pool.query(
            `INSERT INTO theme_assets (
                theme_id, asset_key, asset_type, file_path, file_url,
                usage_context, display_position, z_index, load_strategy
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING *`,
            [
                data.theme_id,
                data.asset_key,
                data.asset_type,
                data.file_path,
                data.file_url,
                data.usage_context,
                data.display_position,
                data.z_index ?? 1,
                data.load_strategy ?? AssetLoadStrategy.LAZY
            ]
        );
        return result.rows[0];
    }

    static async updateAsset(assetId: number, data: UpdateThemeAssetRequest): Promise<ThemeAsset | null> {
        const updates: string[] = [];
        const params: any[] = [];
        let paramCount = 1;

        const fields: (keyof UpdateThemeAssetRequest)[] = [
            'asset_key', 'asset_type', 'file_path', 'file_url',
            'usage_context', 'display_position', 'z_index',
            'load_strategy', 'is_active'
        ];

        fields.forEach(field => {
            if (data[field] !== undefined) {
                updates.push(`${field} = $${paramCount++}`);
                params.push(data[field]);
            }
        });

        if (updates.length === 0) {
            return this.getAssetById(assetId);
        }

        params.push(assetId);

        const result = await pool.query(
            `UPDATE theme_assets SET ${updates.join(', ')}, updated_at = CURRENT_TIMESTAMP
             WHERE id = $${paramCount} RETURNING *`,
            params
        );

        return result.rows[0] || null;
    }

    static async deleteAsset(assetId: number): Promise<boolean> {
        const result = await pool.query(
            'DELETE FROM theme_assets WHERE id = $1',
            [assetId]
        );
        return (result.rowCount ?? 0) > 0;
    }

    /**
     * Theme Configurations
     */

    static async getThemeConfigurations(themeId: number): Promise<ThemeConfiguration[]> {
        const result = await pool.query(
            'SELECT * FROM theme_configurations WHERE theme_id = $1 AND is_active = true ORDER BY category, config_key',
            [themeId]
        );
        return result.rows;
    }

    /**
     * Theme Activations & Analytics
     */

    static async getThemeActivations(themeId: number, limit: number = 50): Promise<ThemeActivation[]> {
        const result = await pool.query(
            'SELECT * FROM theme_activations WHERE theme_id = $1 ORDER BY activated_at DESC LIMIT $2',
            [themeId, limit]
        );
        return result.rows;
    }

    static async getThemeAnalytics(themeId: number): Promise<any> {
        const result = await pool.query(
            'SELECT * FROM calculate_theme_stats($1)',
            [themeId]
        );
        return result.rows[0];
    }

    static async getAllThemeAnalytics(): Promise<any[]> {
        const result = await pool.query(
            'SELECT * FROM v_theme_analytics ORDER BY activation_count DESC'
        );
        return result.rows;
    }

    /**
     * User Preferences
     */

    static async getUserPreferences(userId: number): Promise<ThemePreferences | null> {
        const result = await pool.query(
            'SELECT * FROM theme_preferences WHERE user_id = $1',
            [userId]
        );

        if (result.rows.length === 0) {
            // Create default preferences
            return this.createUserPreferences(userId);
        }

        return result.rows[0];
    }

    static async createUserPreferences(userId: number): Promise<ThemePreferences> {
        const result = await pool.query(
            `INSERT INTO theme_preferences (user_id)
             VALUES ($1)
             ON CONFLICT (user_id) DO UPDATE SET updated_at = CURRENT_TIMESTAMP
             RETURNING *`,
            [userId]
        );
        return result.rows[0];
    }

    static async updateUserPreferences(
        userId: number,
        data: UpdateThemePreferencesRequest
    ): Promise<ThemePreferences> {
        const updates: string[] = [];
        const params: any[] = [];
        let paramCount = 1;

        const fields: (keyof UpdateThemePreferencesRequest)[] = [
            'enabled', 'preferred_theme_id',
            'enable_visual_effects', 'enable_sound_effects',
            'enable_animations', 'enable_decorations',
            'reduce_motion', 'reduce_transparency',
            'effect_intensity', 'sound_volume', 'animation_speed'
        ];

        fields.forEach(field => {
            if (data[field] !== undefined) {
                updates.push(`${field} = $${paramCount++}`);
                params.push(data[field]);
            }
        });

        if (updates.length === 0) {
            const prefs = await this.getUserPreferences(userId);
            if (!prefs) throw new Error('User preferences not found');
            return prefs;
        }

        params.push(userId);

        const result = await pool.query(
            `INSERT INTO theme_preferences (user_id, ${fields.filter(f => data[f] !== undefined).join(', ')})
             VALUES ($${paramCount}, ${params.slice(0, -1).map((_, i) => `$${i + 1}`).join(', ')})
             ON CONFLICT (user_id) DO UPDATE SET
             ${updates.join(', ')}, updated_at = CURRENT_TIMESTAMP
             RETURNING *`,
            params
        );

        return result.rows[0];
    }

    static async getUserCustomCSS(userId: number): Promise<{
        custom_css: string | null;
        custom_css_updated_at: Date | null;
    }> {
        const prefs = await this.getUserPreferences(userId);
        return {
            custom_css: prefs?.custom_css || null,
            custom_css_updated_at: prefs?.custom_css_updated_at || null
        };
    }

    static async updateUserCustomCSS(
        userId: number,
        css: string | null
    ): Promise<{ custom_css: string | null; custom_css_updated_at: Date | null }> {
        const sanitized = this.sanitizeCustomCSS(css);

        const result = await pool.query(
            `INSERT INTO theme_preferences (user_id, custom_css, custom_css_updated_at)
             VALUES ($1, $2, CASE WHEN $2 IS NULL THEN NULL ELSE NOW() END)
             ON CONFLICT (user_id) DO UPDATE SET
                custom_css = EXCLUDED.custom_css,
                custom_css_updated_at = CASE WHEN EXCLUDED.custom_css IS NULL THEN NULL ELSE NOW() END,
                updated_at = CURRENT_TIMESTAMP
             RETURNING custom_css, custom_css_updated_at`,
            [userId, sanitized]
        );

        return result.rows[0];
    }

    /**
     * Preview Mode
     */

    static async enablePreviewMode(themeId: number, userId: number): Promise<Theme> {
        // Set preview mode for this theme
        await pool.query(
            'UPDATE themes SET preview_mode = true WHERE id = $1',
            [themeId]
        );

        // Log preview activation
        await pool.query(
            `INSERT INTO theme_activations (
                theme_id, activation_type, activated_by
            ) VALUES ($1, $2, $3)`,
            [themeId, ThemeActivationType.PREVIEW, userId]
        );

        const theme = await this.getThemeById(themeId);
        if (!theme) throw new Error('Theme not found');
        return theme;
    }

    static async disablePreviewMode(themeId: number): Promise<void> {
        await pool.query(
            'UPDATE themes SET preview_mode = false WHERE id = $1',
            [themeId]
        );
    }

    /**
     * Utility Methods
     */

    static async getThemesByCategory(category: ThemeCategory): Promise<Theme[]> {
        const result = await pool.query(
            'SELECT * FROM themes WHERE category = $1 AND is_available = true ORDER BY name',
            [category]
        );
        return result.rows;
    }

    static async searchThemes(searchTerm: string): Promise<Theme[]> {
        const result = await pool.query(
            `SELECT * FROM themes 
             WHERE (name ILIKE $1 OR description ILIKE $1 OR theme_key ILIKE $1)
             AND is_available = true
             ORDER BY name`,
            [`%${searchTerm}%`]
        );
        return result.rows;
    }

    /**
     * Update activation analytics
     */
    static async updateActivationAnalytics(
        activationId: number,
        data: {
            unique_viewers?: number;
            total_page_views?: number;
            avg_session_duration?: number;
            interaction_count?: number;
            avg_load_time_ms?: number;
            error_count?: number;
        }
    ): Promise<void> {
        const updates: string[] = [];
        const params: any[] = [];
        let paramCount = 1;

        Object.entries(data).forEach(([key, value]) => {
            if (value !== undefined) {
                updates.push(`${key} = $${paramCount++}`);
                params.push(value);
            }
        });

        if (updates.length === 0) return;

        params.push(activationId);

        await pool.query(
            `UPDATE theme_activations SET ${updates.join(', ')} WHERE id = $${paramCount}`,
            params
        );
    }

    private static sanitizeCustomCSS(css?: string | null): string | null {
        return CustomCssSanitizer.sanitize(css, this.MAX_CUSTOM_CSS_LENGTH);
    }
}
