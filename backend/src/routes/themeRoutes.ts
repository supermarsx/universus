/**
 * @module backend/routes/themeRoutes
 *
 * API endpoints for the Seasonal Theme System. Provides public endpoints for
 * retrieving themes and admin endpoints for creating, updating and scheduling
 * seasonal themes and assets.
 */

// =====================================================
// Phase 8: Seasonal Theme System - API Routes
// =====================================================

import express, { Request, Response } from 'express';
import { AuthRequest } from '../types';
import { getUserId } from '../utils/authHelpers';
import { ThemeService } from '../services/themeService';
import { authenticateToken } from '../middleware/auth';
import { requireAdmin } from '../middleware/adminAuth';
import {
    ThemeCategory,
    ThemeActivationType,
    CreateThemeRequest,
    UpdateThemeRequest,
    CreateThemeScheduleRequest,
    UpdateThemeScheduleRequest,
    CreateThemeAssetRequest,
    UpdateThemeAssetRequest,
    UpdateThemePreferencesRequest
} from '../types/seasonalTheme';

const router = express.Router();

// =====================================================
// PUBLIC ENDPOINTS (No Authentication Required)
// =====================================================

/**
 * GET /api/themes/current
 * Get currently active theme with full data
 */
router.get('/current', async (req: Request, res: Response) => {
    try {
        const themeData = await ThemeService.getActiveThemeData();
        
        if (!themeData) {
            return res.json({
                success: true,
                theme: null,
                message: 'No active theme'
            });
        }

        res.json({
            success: true,
            theme: themeData.theme,
            assets: themeData.assets,
            cssVariables: themeData.cssVariables,
            customCSS: themeData.customCSS
        });
    } catch (error) {
        console.error('Error getting current theme:', error);
        res.status(500).json({
            success: false,
            message: 'Failed to get current theme'
        });
    }
});

/**
 * GET /api/themes
 * Get all available themes (public view)
 */
router.get('/', async (req: Request, res: Response) => {
    try {
        const { category, available } = req.query;

        const filters: any = {
            is_available: available === 'false' ? false : true
        };

        if (category) {
            filters.category = category as ThemeCategory;
        }

        const themes = await ThemeService.getAllThemes(filters);

        res.json({
            success: true,
            themes,
            total: themes.length
        });
    } catch (error) {
        console.error('Error getting themes:', error);
        res.status(500).json({
            success: false,
            message: 'Failed to get themes'
        });
    }
});

/**
 * GET /api/themes/:id
 * Get specific theme by ID
 */
router.get('/:id', async (req: Request, res: Response) => {
    try {
        const themeId = parseInt(req.params.id);
        const theme = await ThemeService.getThemeById(themeId);

        if (!theme) {
            return res.status(404).json({
                success: false,
                message: 'Theme not found'
            });
        }

        // Get additional data
        const assets = await ThemeService.getThemeAssets(themeId);
        const configurations = await ThemeService.getThemeConfigurations(themeId);

        res.json({
            success: true,
            theme,
            assets,
            configurations
        });
    } catch (error) {
        console.error('Error getting theme:', error);
        res.status(500).json({
            success: false,
            message: 'Failed to get theme'
        });
    }
});

/**
 * GET /api/themes/key/:key
 * Get specific theme by key
 */
router.get('/key/:key', async (req: Request, res: Response) => {
    try {
        const theme = await ThemeService.getThemeByKey(req.params.key);

        if (!theme) {
            return res.status(404).json({
                success: false,
                message: 'Theme not found'
            });
        }

        res.json({
            success: true,
            theme
        });
    } catch (error) {
        console.error('Error getting theme by key:', error);
        res.status(500).json({
            success: false,
            message: 'Failed to get theme'
        });
    }
});

// =====================================================
// USER AUTHENTICATED ENDPOINTS
// =====================================================

/**
 * GET /api/themes/preferences
 * Get current user's theme preferences
 */
router.get('/user/preferences', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const preferences = await ThemeService.getUserPreferences(userId);

        res.json({
            success: true,
            preferences
        });
    } catch (error) {
        console.error('Error getting user preferences:', error);
        res.status(500).json({
            success: false,
            message: 'Failed to get preferences'
        });
    }
});

/**
 * PUT /api/themes/preferences
 * Update current user's theme preferences
 */
router.put('/user/preferences', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const data: UpdateThemePreferencesRequest = req.body;

        const preferences = await ThemeService.updateUserPreferences(userId, data);

        res.json({
            success: true,
            preferences,
            message: 'Preferences updated successfully'
        });
    } catch (error) {
        console.error('Error updating user preferences:', error);
        res.status(500).json({
            success: false,
            message: 'Failed to update preferences'
        });
    }
});

/**
 * GET /api/themes/user/custom-css
 * Retrieve the current user's custom CSS snippet
 */
router.get('/user/custom-css', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const result = await ThemeService.getUserCustomCSS(userId);

        res.json({
            success: true,
            customCSS: result.custom_css,
            updatedAt: result.custom_css_updated_at
        });
    } catch (error) {
        console.error('Error fetching custom CSS:', error);
        res.status(500).json({
            success: false,
            message: 'Failed to load custom CSS'
        });
    }
});

/**
 * PUT /api/themes/user/custom-css
 * Update the current user's custom CSS
 */
router.put('/user/custom-css', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const css = typeof req.body?.css === 'string' ? req.body.css : '';

        const result = await ThemeService.updateUserCustomCSS(userId, css);

        res.json({
            success: true,
            customCSS: result.custom_css,
            updatedAt: result.custom_css_updated_at,
            message: result.custom_css ? 'Custom CSS updated' : 'Custom CSS cleared'
        });
    } catch (error: any) {
        console.error('Error updating custom CSS:', error);
        res.status(400).json({
            success: false,
            message: error.message || 'Failed to update custom CSS'
        });
    }
});

// =====================================================
// ADMIN ENDPOINTS (Admin Authentication Required)
// =====================================================

/**
 * POST /api/themes
 * Create new theme (Admin only)
 */
router.post('/', authenticateToken, requireAdmin, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const data: CreateThemeRequest = req.body;

        // Validate required fields
        if (!data.theme_key || !data.name || !data.primary_color || !data.secondary_color || !data.accent_color) {
            return res.status(400).json({
                success: false,
                message: 'Missing required fields'
            });
        }

        const theme = await ThemeService.createTheme(data, userId);

        res.status(201).json({
            success: true,
            theme,
            message: 'Theme created successfully'
        });
    } catch (error: any) {
        console.error('Error creating theme:', error);
        
        if (error.code === '23505') { // Unique violation
            return res.status(409).json({
                success: false,
                message: 'Theme with this key already exists'
            });
        }

        res.status(500).json({
            success: false,
            message: 'Failed to create theme'
        });
    }
});

/**
 * PUT /api/themes/:id
 * Update theme (Admin only)
 */
router.put('/:id', authenticateToken, requireAdmin, async (req: AuthRequest, res: Response) => {
    try {
        const themeId = parseInt(req.params.id);
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const data: UpdateThemeRequest = req.body;

        const theme = await ThemeService.updateTheme(themeId, data, userId);

        if (!theme) {
            return res.status(404).json({
                success: false,
                message: 'Theme not found'
            });
        }

        res.json({
            success: true,
            theme,
            message: 'Theme updated successfully'
        });
    } catch (error) {
        console.error('Error updating theme:', error);
        res.status(500).json({
            success: false,
            message: 'Failed to update theme'
        });
    }
});

/**
 * DELETE /api/themes/:id
 * Delete theme (Admin only)
 */
router.delete('/:id', authenticateToken, requireAdmin, async (req: Request, res: Response) => {
    try {
        const themeId = parseInt(req.params.id);
        const success = await ThemeService.deleteTheme(themeId);

        if (!success) {
            return res.status(404).json({
                success: false,
                message: 'Theme not found'
            });
        }

        res.json({
            success: true,
            message: 'Theme deleted successfully'
        });
    } catch (error) {
        console.error('Error deleting theme:', error);
        res.status(500).json({
            success: false,
            message: 'Failed to delete theme'
        });
    }
});

/**
 * POST /api/themes/:id/activate
 * Manually activate a theme (Admin only)
 */
router.post('/:id/activate', authenticateToken, requireAdmin, async (req: AuthRequest, res: Response) => {
    try {
        const themeId = parseInt(req.params.id);
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const { reason } = req.body;

        const activation = await ThemeService.activateTheme(
            themeId,
            userId,
            ThemeActivationType.MANUAL,
            reason
        );

        res.json({
            success: true,
            activation,
            message: 'Theme activated successfully'
        });
    } catch (error) {
        console.error('Error activating theme:', error);
        res.status(500).json({
            success: false,
            message: 'Failed to activate theme'
        });
    }
});

/**
 * POST /api/themes/:id/deactivate
 * Deactivate a theme (Admin only)
 */
router.post('/:id/deactivate', authenticateToken, requireAdmin, async (req: Request, res: Response) => {
    try {
        const themeId = parseInt(req.params.id);
        await ThemeService.deactivateTheme(themeId);

        res.json({
            success: true,
            message: 'Theme deactivated successfully'
        });
    } catch (error) {
        console.error('Error deactivating theme:', error);
        res.status(500).json({
            success: false,
            message: 'Failed to deactivate theme'
        });
    }
});

/**
 * POST /api/themes/:id/preview
 * Enable preview mode for a theme (Admin only)
 */
router.post('/:id/preview', authenticateToken, requireAdmin, async (req: AuthRequest, res: Response) => {
    try {
        const themeId = parseInt(req.params.id);
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });

        const theme = await ThemeService.enablePreviewMode(themeId, userId);

        res.json({
            success: true,
            theme,
            message: 'Preview mode enabled'
        });
    } catch (error) {
        console.error('Error enabling preview:', error);
        res.status(500).json({
            success: false,
            message: 'Failed to enable preview mode'
        });
    }
});

/**
 * POST /api/themes/:id/preview/disable
 * Disable preview mode (Admin only)
 */
router.post('/:id/preview/disable', authenticateToken, requireAdmin, async (req: AuthRequest, res: Response) => {
    try {
        const themeId = parseInt(req.params.id);
        await ThemeService.disablePreviewMode(themeId);

        res.json({
            success: true,
            message: 'Preview mode disabled'
        });
    } catch (error) {
        console.error('Error disabling preview:', error);
        res.status(500).json({
            success: false,
            message: 'Failed to disable preview mode'
        });
    }
});

// =====================================================
// SCHEDULE ENDPOINTS (Admin only)
// =====================================================

/**
 * GET /api/themes/schedules
 * Get all schedules
 */
router.get('/admin/schedules', authenticateToken, requireAdmin, async (req: Request, res: Response) => {
    try {
        const { theme_id } = req.query;
        const themeId = theme_id ? parseInt(theme_id as string) : undefined;

        const schedules = await ThemeService.getAllSchedules(themeId);

        res.json({
            success: true,
            schedules,
            total: schedules.length
        });
    } catch (error) {
        console.error('Error getting schedules:', error);
        res.status(500).json({
            success: false,
            message: 'Failed to get schedules'
        });
    }
});

/**
 * GET /api/themes/schedules/active
 * Get currently active schedules
 */
router.get('/admin/schedules/active', authenticateToken, requireAdmin, async (req: Request, res: Response) => {
    try {
        const schedules = await ThemeService.getActiveSchedules();

        res.json({
            success: true,
            schedules,
            total: schedules.length
        });
    } catch (error) {
        console.error('Error getting active schedules:', error);
        res.status(500).json({
            success: false,
            message: 'Failed to get active schedules'
        });
    }
});

/**
 * POST /api/themes/schedules
 * Create new schedule
 */
router.post('/admin/schedules', authenticateToken, requireAdmin, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const data: CreateThemeScheduleRequest = req.body;

        if (!data.theme_id || !data.schedule_name || !data.start_date || !data.end_date) {
            return res.status(400).json({
                success: false,
                message: 'Missing required fields'
            });
        }

        const schedule = await ThemeService.createSchedule(data, userId);

        res.status(201).json({
            success: true,
            schedule,
            message: 'Schedule created successfully'
        });
    } catch (error) {
        console.error('Error creating schedule:', error);
        res.status(500).json({
            success: false,
            message: 'Failed to create schedule'
        });
    }
});

/**
 * PUT /api/themes/schedules/:id
 * Update schedule
 */
router.put('/admin/schedules/:id', authenticateToken, requireAdmin, async (req: Request, res: Response) => {
    try {
        const scheduleId = parseInt(req.params.id);
        const data: UpdateThemeScheduleRequest = req.body;

        const schedule = await ThemeService.updateSchedule(scheduleId, data);

        if (!schedule) {
            return res.status(404).json({
                success: false,
                message: 'Schedule not found'
            });
        }

        res.json({
            success: true,
            schedule,
            message: 'Schedule updated successfully'
        });
    } catch (error) {
        console.error('Error updating schedule:', error);
        res.status(500).json({
            success: false,
            message: 'Failed to update schedule'
        });
    }
});

/**
 * DELETE /api/themes/schedules/:id
 * Delete schedule
 */
router.delete('/admin/schedules/:id', authenticateToken, requireAdmin, async (req: Request, res: Response) => {
    try {
        const scheduleId = parseInt(req.params.id);
        const success = await ThemeService.deleteSchedule(scheduleId);

        if (!success) {
            return res.status(404).json({
                success: false,
                message: 'Schedule not found'
            });
        }

        res.json({
            success: true,
            message: 'Schedule deleted successfully'
        });
    } catch (error) {
        console.error('Error deleting schedule:', error);
        res.status(500).json({
            success: false,
            message: 'Failed to delete schedule'
        });
    }
});

// =====================================================
// ASSET ENDPOINTS (Admin only)
// =====================================================

/**
 * GET /api/themes/:id/assets
 * Get theme assets
 */
router.get('/:id/assets', async (req: Request, res: Response) => {
    try {
        const themeId = parseInt(req.params.id);
        const { usage_context } = req.query;

        const assets = await ThemeService.getThemeAssets(
            themeId,
            usage_context as string | undefined
        );

        res.json({
            success: true,
            assets,
            total: assets.length
        });
    } catch (error) {
        console.error('Error getting assets:', error);
        res.status(500).json({
            success: false,
            message: 'Failed to get assets'
        });
    }
});

/**
 * POST /api/themes/:id/assets
 * Create new asset
 */
router.post('/:id/assets', authenticateToken, requireAdmin, async (req: Request, res: Response) => {
    try {
        const themeId = parseInt(req.params.id);
        const data: CreateThemeAssetRequest = {
            ...req.body,
            theme_id: themeId
        };

        if (!data.asset_key || !data.asset_type || !data.file_path || !data.usage_context) {
            return res.status(400).json({
                success: false,
                message: 'Missing required fields'
            });
        }

        const asset = await ThemeService.createAsset(data);

        res.status(201).json({
            success: true,
            asset,
            message: 'Asset created successfully'
        });
    } catch (error: any) {
        console.error('Error creating asset:', error);

        if (error.code === '23505') {
            return res.status(409).json({
                success: false,
                message: 'Asset with this key already exists'
            });
        }

        res.status(500).json({
            success: false,
            message: 'Failed to create asset'
        });
    }
});

/**
 * PUT /api/themes/assets/:id
 * Update asset
 */
router.put('/admin/assets/:id', authenticateToken, requireAdmin, async (req: Request, res: Response) => {
    try {
        const assetId = parseInt(req.params.id);
        const data: UpdateThemeAssetRequest = req.body;

        const asset = await ThemeService.updateAsset(assetId, data);

        if (!asset) {
            return res.status(404).json({
                success: false,
                message: 'Asset not found'
            });
        }

        res.json({
            success: true,
            asset,
            message: 'Asset updated successfully'
        });
    } catch (error) {
        console.error('Error updating asset:', error);
        res.status(500).json({
            success: false,
            message: 'Failed to update asset'
        });
    }
});

/**
 * DELETE /api/themes/assets/:id
 * Delete asset
 */
router.delete('/admin/assets/:id', authenticateToken, requireAdmin, async (req: Request, res: Response) => {
    try {
        const assetId = parseInt(req.params.id);
        const success = await ThemeService.deleteAsset(assetId);

        if (!success) {
            return res.status(404).json({
                success: false,
                message: 'Asset not found'
            });
        }

        res.json({
            success: true,
            message: 'Asset deleted successfully'
        });
    } catch (error) {
        console.error('Error deleting asset:', error);
        res.status(500).json({
            success: false,
            message: 'Failed to delete asset'
        });
    }
});

// =====================================================
// ANALYTICS ENDPOINTS (Admin only)
// =====================================================

/**
 * GET /api/themes/:id/analytics
 * Get theme analytics
 */
router.get('/:id/analytics', authenticateToken, requireAdmin, async (req: Request, res: Response) => {
    try {
        const themeId = parseInt(req.params.id);
        const analytics = await ThemeService.getThemeAnalytics(themeId);

        res.json({
            success: true,
            analytics
        });
    } catch (error) {
        console.error('Error getting analytics:', error);
        res.status(500).json({
            success: false,
            message: 'Failed to get analytics'
        });
    }
});

/**
 * GET /api/themes/admin/analytics
 * Get all themes analytics
 */
router.get('/admin/analytics/all', authenticateToken, requireAdmin, async (req: Request, res: Response) => {
    try {
        const analytics = await ThemeService.getAllThemeAnalytics();

        res.json({
            success: true,
            analytics,
            total: analytics.length
        });
    } catch (error) {
        console.error('Error getting all analytics:', error);
        res.status(500).json({
            success: false,
            message: 'Failed to get analytics'
        });
    }
});

/**
 * GET /api/themes/:id/activations
 * Get theme activation history
 */
router.get('/:id/activations', authenticateToken, requireAdmin, async (req: Request, res: Response) => {
    try {
        const themeId = parseInt(req.params.id);
        const limit = req.query.limit ? parseInt(req.query.limit as string) : 50;

        const activations = await ThemeService.getThemeActivations(themeId, limit);

        res.json({
            success: true,
            activations,
            total: activations.length
        });
    } catch (error) {
        console.error('Error getting activations:', error);
        res.status(500).json({
            success: false,
            message: 'Failed to get activations'
        });
    }
});

/**
 * POST /api/themes/check-schedules
 * Manually trigger schedule check (Admin only)
 */
router.post('/admin/check-schedules', authenticateToken, requireAdmin, async (req: Request, res: Response) => {
    try {
        const result = await ThemeService.checkScheduledThemes();

        res.json({
            success: true,
            activated: result.activated,
            theme: result.theme,
            message: result.activated ? 'Theme activated by schedule' : 'No schedule changes'
        });
    } catch (error) {
        console.error('Error checking schedules:', error);
        res.status(500).json({
            success: false,
            message: 'Failed to check schedules'
        });
    }
});

export default router;
