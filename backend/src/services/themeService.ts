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
     * Get all themes with optional filtering.
     *
     * @param filters - Optional filters for category, is_active and is_available.
     * @returns Array of Theme objects.
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
     * Get a single theme by its database id.
     *
     * @param themeId - Theme id to load.
     * @returns Theme or null if not found.
     */
    static async getThemeById(themeId: number): Promise<Theme | null> {
        const result = await pool.query(
            'SELECT * FROM themes WHERE id = $1',
            [themeId]
        );
        return result.rows[0] || null;
    }

    /**
     * Get a single theme by its unique theme key.
     *
     * @param themeKey - Unique string key of the theme.
     * @returns Theme or null when not found.
     */
    static async getThemeByKey(themeKey: string): Promise<Theme | null> {
        const result = await pool.query(
            'SELECT * FROM themes WHERE theme_key = $1',
            [themeKey]
        );
        return result.rows[0] || null;
    }

    /**
     * Create a new theme record.
     *
     * @param data - CreateThemeRequest payload containing theme fields.
     * @param userId - Optional id of the user creating the theme.
     * @returns The created Theme row.
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
     * Update an existing theme with partial data. Returns the updated theme.
     * If no fields are provided the current theme is returned.
     *
     * @param themeId - ID of the theme to update.
     * @param data - Partial update payload.
     * @param userId - Optional id of the user performing the update.
     * @returns Updated Theme or null when not found.
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
     * Delete a theme by id.
     *
     * @param themeId - ID of the theme to delete.
     * @returns True when a row was deleted, false otherwise.
     */
    static async deleteTheme(themeId: number): Promise<boolean> {
        const result = await pool.query(
            'DELETE FROM themes WHERE id = $1',
            [themeId]
        );
        return (result.rowCount ?? 0) > 0;
    }

    /**
     * Return the currently active theme (if any).
     *
     * @returns The currently active Theme or null when none is active.
     */
    static async getCurrentTheme(): Promise<Theme | null> {
        const result = await pool.query(
            'SELECT * FROM v_current_theme LIMIT 1'
        );
        return result.rows[0] || null;
    }

    /**
     * Return active theme data including assets and configuration for the current theme.
     *
     * @returns ActiveThemeData or null when no theme is active.
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
     * Activate a theme manually. Deactivates any currently active theme, marks
     * the requested theme as active and logs the activation.
     *
     * @param themeId - ID of the theme to activate.
     * @param userId - Optional id of the user performing the activation.
     * @param activationType - The activation type (manual, scheduled, preview, etc.).
     * @param reason - Optional reason for activation.
     * @returns The ThemeActivation row created for this activation.
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
     * Deactivate a theme and update its activation record with deactivation time.
     *
     * @param themeId - ID of the theme to deactivate.
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
     * Check for scheduled themes that should be activated and activate them when found.
     * Intended for invocation by a scheduler task.
     *
     * @returns Object indicating whether an activation occurred and the activated theme when applicable.
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
     * Return theme schedules optionally filtered by theme id.
     *
     * @param themeId - Optional theme id to filter schedules.
     * @returns Array of ThemeSchedule rows.
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

    /**
     * Load a theme schedule by id.
     *
     * @param scheduleId - Schedule id to load.
     * @returns ThemeSchedule or null when not found.
     */
    static async getScheduleById(scheduleId: number): Promise<ThemeSchedule | null> {
        const result = await pool.query(
            'SELECT * FROM theme_schedules WHERE id = $1',
            [scheduleId]
        );
        return result.rows[0] || null;
    }

    /**
     * Create a new theme schedule.
     *
     * @param data - CreateThemeScheduleRequest payload.
     * @param userId - Optional id of the creator.
     * @returns The created ThemeSchedule row.
     */
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

    /**
     * Update a theme schedule with partial data.
     *
     * @param scheduleId - Schedule id to update.
     * @param data - Partial update payload.
     * @returns Updated ThemeSchedule or null when not found.
     */
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

    /**
     * Delete a theme schedule by id.
     *
     * @param scheduleId - Schedule id to delete.
     * @returns True when deleted, false otherwise.
     */
    static async deleteSchedule(scheduleId: number): Promise<boolean> {
        const result = await pool.query(
            'DELETE FROM theme_schedules WHERE id = $1',
            [scheduleId]
        );
        return (result.rowCount ?? 0) > 0;
    }

    /**
     * Return currently active theme schedules.
     *
     * @returns Array of ThemeSchedule rows.
     */
    static async getActiveSchedules(): Promise<ThemeSchedule[]> {
        const result = await pool.query(
            'SELECT * FROM v_active_theme_schedules'
        );
        return result.rows;
    }

    /**
     * Retrieve active theme assets for a theme with optional usage context filtering.
     *
     * @param themeId - Theme id to load assets for.
     * @param usageContext - Optional usage context string to filter assets.
     * @returns Array of ThemeAsset rows.
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

    /**
     * Load a theme asset by id.
     *
     * @param assetId - Asset id to load.
     * @returns ThemeAsset or null when not found.
     */
    static async getAssetById(assetId: number): Promise<ThemeAsset | null> {
        const result = await pool.query(
            'SELECT * FROM theme_assets WHERE id = $1',
            [assetId]
        );
        return result.rows[0] || null;
    }

    /**
     * Create a new theme asset record.
     *
     * @param data - CreateThemeAssetRequest payload describing the asset.
     * @returns The created ThemeAsset row.
     */
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

    /**
     * Update an existing theme asset partially.
     *
     * @param assetId - Asset id to update.
     * @param data - Partial update payload.
     * @returns Updated ThemeAsset or null when not found.
     */
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

    /**
     * Delete a theme asset by id.
     *
     * @param assetId - Asset id to delete.
     * @returns True when deleted, false otherwise.
     */
    static async deleteAsset(assetId: number): Promise<boolean> {
        const result = await pool.query(
            'DELETE FROM theme_assets WHERE id = $1',
            [assetId]
        );
        return (result.rowCount ?? 0) > 0;
    }

    /**
     * Return active theme configuration key/value rows for a theme.
     *
     * @param themeId - Theme id to fetch configurations for.
     * @returns Array of ThemeConfiguration rows.
     */
    static async getThemeConfigurations(themeId: number): Promise<ThemeConfiguration[]> {
        const result = await pool.query(
            'SELECT * FROM theme_configurations WHERE theme_id = $1 AND is_active = true ORDER BY category, config_key',
            [themeId]
        );
        return result.rows;
    }

    /**
     * Get activation history for a theme.
     *
     * @param themeId - Theme id to query.
     * @param limit - Maximum number of activation rows to return.
     * @returns Array of ThemeActivation rows.
     */
    static async getThemeActivations(themeId: number, limit: number = 50): Promise<ThemeActivation[]> {
        const result = await pool.query(
            'SELECT * FROM theme_activations WHERE theme_id = $1 ORDER BY activated_at DESC LIMIT $2',
            [themeId, limit]
        );
        return result.rows;
    }

    /**
     * Retrieve aggregated analytics for a theme.
     *
     * @param themeId - Theme id to compute analytics for.
     * @returns Analytics object (shape depends on DB view).
     */
    static async getThemeAnalytics(themeId: number): Promise<any> {
        const result = await pool.query(
            'SELECT * FROM calculate_theme_stats($1)',
            [themeId]
        );
        return result.rows[0];
    }

    /**
     * Retrieve analytics across all themes.
     *
     * @returns Array of analytics rows.
     */
    static async getAllThemeAnalytics(): Promise<any[]> {
        const result = await pool.query(
            'SELECT * FROM v_theme_analytics ORDER BY activation_count DESC'
        );
        return result.rows;
    }

    /**
     * Get or create theme preferences for a user.
     *
     * @param userId - User id to load preferences for.
     * @returns ThemePreferences row.
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

    /**
     * Create default preferences for a user (idempotent).
     *
     * @param userId - User id to create preferences for.
     * @returns The created or existing ThemePreferences row.
     */
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

    /**
     * Update a user's theme preferences with partial data.
     *
     * @param userId - User id to update.
     * @param data - Partial preference updates.
     * @returns Updated ThemePreferences row.
     */
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

    /**
     * Return a user's custom CSS and last update timestamp.
     *
     * @param userId - User id to query.
     * @returns Object with custom_css and custom_css_updated_at fields.
     */
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

    /**
     * Update a user's custom CSS after sanitization and set the updated timestamp.
     *
     * @param userId - User id to update.
     * @param css - CSS string or null to clear custom CSS.
     * @returns Object with the stored custom_css and custom_css_updated_at.
     */
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
     * Enable preview mode for a theme and log the preview activation.
     *
     * @param themeId - Theme id to preview.
     * @param userId - User id requesting the preview.
     * @returns The theme that was set to preview mode.
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

    /**
     * Disable preview mode for a theme.
     *
     * @param themeId - Theme id to disable preview for.
     */
    static async disablePreviewMode(themeId: number): Promise<void> {
        await pool.query(
            'UPDATE themes SET preview_mode = false WHERE id = $1',
            [themeId]
        );
    }

    /**
     * Get available themes by category.
     *
     * @param category - ThemeCategory enum value.
     * @returns Array of Theme rows.
     */
    static async getThemesByCategory(category: ThemeCategory): Promise<Theme[]> {
        const result = await pool.query(
            'SELECT * FROM themes WHERE category = $1 AND is_available = true ORDER BY name',
            [category]
        );
        return result.rows;
    }

    /**
     * Search themes by name, description or key (case-insensitive).
     *
     * @param searchTerm - Term used for ILIKE search.
     * @returns Array of matching Theme rows.
     */
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
     * Update analytics counters for a theme activation.
     * Only provided fields will be updated.
     *
     * @param activationId - Activation id to update.
     * @param data - Partial analytics data to update.
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

    /**
     * Sanitize custom CSS input using the project's CustomCssSanitizer utility.
     * Truncates or rejects content exceeding MAX_CUSTOM_CSS_LENGTH.
     *
     * @private
     * @param css - Raw CSS string or null.
     * @returns Sanitized CSS string or null.
     */
    private static sanitizeCustomCSS(css?: string | null): string | null {
        return CustomCssSanitizer.sanitize(css, this.MAX_CUSTOM_CSS_LENGTH);
    }
}
