// Phase 7: Configuration API Routes
// REST API endpoints for configuration management

import { Router, Response } from 'express';
import { AuthRequest } from '../types';
import { ConfigurationService } from '../services/configurationService';
import { authenticateToken, assertAuthenticated } from '../middleware/auth';
import { requirePermission } from '../middleware/adminAuth';
import { pool } from '../config/database';
import { redis } from '../config/redis';
import { io } from '../index';
import { getUserId as getUserIdFromUtils } from '../utils/authHelpers';


const router = Router();
const configService = new ConfigurationService(pool, redis, io);

// All routes require admin authentication
router.use(authenticateToken, assertAuthenticated, requirePermission('config:read'));

const getUserId = getUserIdFromUtils;


// ============================================
// CATEGORIES
// ============================================

// GET /api/config/categories - Get all configuration categories
router.get('/categories', async (req: AuthRequest, res: Response) => {
    try {
        const result = await pool.query(`
            SELECT 
                cc.*,
                COUNT(cp.parameter_id) as parameter_count,
                COUNT(CASE WHEN cp.current_value != cp.default_value THEN 1 END) as modified_count
            FROM config_categories cc
            LEFT JOIN config_parameters cp ON cc.category_id = cp.category_id
            WHERE cc.is_active = TRUE
            GROUP BY cc.category_id
            ORDER BY cc.sort_order
        `);

        res.json({
            success: true,
            categories: result.rows
        });
    } catch (error: any) {
        res.status(500).json({ success: false, error: error.message });
    }
});

// GET /api/config/categories/:category - Get specific category details
router.get('/categories/:category', async (req: AuthRequest, res: Response) => {
    try {
        const { category } = req.params;

        const result = await pool.query(`
            SELECT cp.*, cc.category_name, cc.display_name as category_display_name
            FROM config_parameters cp
            JOIN config_categories cc ON cp.category_id = cc.category_id
            WHERE cc.category_name = $1 AND cp.is_editable = TRUE
            ORDER BY cp.sort_order
        `, [category]);

        res.json({
            success: true,
            parameters: result.rows
        });
    } catch (error: any) {
        res.status(500).json({ success: false, error: error.message });
    }
});

// ============================================
// PARAMETERS
// ============================================

// GET /api/config/parameters - Get all configuration parameters
router.get('/parameters', async (req: AuthRequest, res: Response) => {
    try {
        const { category, search } = req.query;

        let query = `
            SELECT cp.*, cc.category_name, cc.display_name as category_display_name
            FROM config_parameters cp
            JOIN config_categories cc ON cp.category_id = cc.category_id
            WHERE cp.is_editable = TRUE
        `;
        
        const params: any[] = [];
        
        if (category) {
            query += ' AND cc.category_name = $1';
            params.push(category);
        }
        
        if (search) {
            const searchParam = params.length + 1;
            query += ` AND (cp.parameter_name ILIKE $${searchParam} OR cp.description ILIKE $${searchParam})`;
            params.push(`%${search}%`);
        }

        query += ' ORDER BY cc.sort_order, cp.sort_order';

        const result = await pool.query(query, params);

        res.json({
            success: true,
            parameters: result.rows
        });
    } catch (error: any) {
        res.status(500).json({ success: false, error: error.message });
    }
});

// GET /api/config/parameters/:key - Get specific parameter
router.get('/parameters/:key', async (req: AuthRequest, res: Response) => {
    try {
        const { key } = req.params;

        const result = await pool.query(`
            SELECT cp.*, cc.category_name, cc.display_name as category_display_name
            FROM config_parameters cp
            JOIN config_categories cc ON cp.category_id = cc.category_id
            WHERE cp.parameter_key = $1
        `, [key]);

        if (result.rows.length === 0) {
            return res.status(404).json({ success: false, error: 'Parameter not found' });
        }

        res.json({
            success: true,
            parameter: result.rows[0]
        });
    } catch (error: any) {
        res.status(500).json({ success: false, error: error.message });
    }
});

// PUT /api/config/parameters/:key - Update parameter value
router.put('/parameters/:key', requirePermission('config:write'), async (req: AuthRequest, res: Response) => {
    try {
        const { key } = req.params;
        const { value, reason } = req.body;
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, error: 'Unauthorized' });

        const result = await configService.setValue(key, value, userId, reason);

        res.json({
            success: true,
            result
        });
    } catch (error: any) {
        res.status(400).json({ success: false, error: error.message });
    }
});

// POST /api/config/parameters/bulk-update - Bulk update parameters
router.post('/parameters/bulk-update', requirePermission('config:write'), async (req: AuthRequest, res: Response) => {
    try {
        const { updates, change_reason } = req.body;
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, error: 'Unauthorized' });

        const result = await configService.bulkUpdate({ updates, change_reason }, userId);

        res.json({
            success: true,
            result
        });
    } catch (error: any) {
        res.status(400).json({ success: false, error: error.message });
    }
});

// POST /api/config/parameters/:key/reset - Reset parameter to default
router.post('/parameters/:key/reset', requirePermission('config:write'), async (req: AuthRequest, res: Response) => {
    try {
        const { key } = req.params;
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, error: 'Unauthorized' });

        // Get default value
        const paramResult = await pool.query(
            'SELECT default_value, data_type FROM config_parameters WHERE parameter_key = $1',
            [key]
        );

        if (paramResult.rows.length === 0) {
            return res.status(404).json({ success: false, error: 'Parameter not found' });
        }

        const defaultValue = configService['parseConfigValue'](
            paramResult.rows[0].default_value,
            paramResult.rows[0].data_type
        );

        const result = await configService.setValue(key, defaultValue, userId, 'Reset to default');

        res.json({
            success: true,
            result
        });
    } catch (error: any) {
        res.status(400).json({ success: false, error: error.message });
    }
});

// ============================================
// GAME CONFIG SNAPSHOT
// ============================================

router.get('/game-config', async (req: AuthRequest, res: Response) => {
    try {
        const snapshot = await configService.getGameConfigSnapshot();
        res.json({ success: true, config: snapshot });
    } catch (error: any) {
        console.error('Failed to fetch game config snapshot:', error);
        res.status(500).json({ success: false, error: 'Failed to load game configuration' });
    }
});

router.post('/game-config/refresh', requirePermission('config:write'), async (req: AuthRequest, res: Response) => {
    try {
        const snapshot = await configService.refreshGameConfigSnapshot();
        res.json({ success: true, config: snapshot });
    } catch (error: any) {
        console.error('Failed to refresh game config snapshot:', error);
        res.status(500).json({ success: false, error: 'Failed to refresh configuration snapshot' });
    }
});

// ============================================
// HISTORY
// ============================================

// GET /api/config/history - Get change history
router.get('/history', async (req: AuthRequest, res: Response) => {
    try {
        const { parameter_key, limit = 100 } = req.query;

        const history = await configService.getChangeHistory(
            parameter_key as string,
            parseInt(limit as string)
        );

        res.json({
            success: true,
            history
        });
    } catch (error: any) {
        res.status(500).json({ success: false, error: error.message });
    }
});

// POST /api/config/history/:changeId/rollback - Rollback a change
router.post('/history/:changeId/rollback', requirePermission('config:write'), async (req: AuthRequest, res: Response) => {
    try {
        const { changeId } = req.params;
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, error: 'Unauthorized' });

        const success = await configService.rollbackChange(parseInt(changeId), userId);

        if (success) {
            res.json({
                success: true,
                message: 'Configuration change rolled back successfully'
            });
        } else {
            res.status(400).json({
                success: false,
                error: 'Failed to rollback change'
            });
        }
    } catch (error: any) {
        res.status(500).json({ success: false, error: error.message });
    }
});

// ============================================
// TEMPLATES
// ============================================

// GET /api/config/templates - Get all templates
router.get('/templates', async (req: AuthRequest, res: Response) => {
    try {
        const { public_only } = req.query;
        
        const templates = await configService.getTemplates(
            public_only === 'true' ? true : undefined
        );

        res.json({
            success: true,
            templates
        });
    } catch (error: any) {
        res.status(500).json({ success: false, error: error.message });
    }
});

// POST /api/config/templates - Create new template
router.post('/templates', requirePermission('config:write'), async (req: AuthRequest, res: Response) => {
    try {
        const { name, description, categories } = req.body;
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, error: 'Unauthorized' });

        const template = await configService.createTemplate(name, description, userId, categories);

        res.json({
            success: true,
            template
        });
    } catch (error: any) {
        res.status(400).json({ success: false, error: error.message });
    }
});

// POST /api/config/templates/:templateId/apply - Apply template
router.post('/templates/:templateId/apply', requirePermission('config:write'), async (req: AuthRequest, res: Response) => {
    try {
        const { templateId } = req.params;
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, error: 'Unauthorized' });

        const result = await configService.applyTemplate(parseInt(templateId), userId);

        res.json({
            success: true,
            result
        });
    } catch (error: any) {
        res.status(400).json({ success: false, error: error.message });
    }
});

// DELETE /api/config/templates/:templateId - Delete template
router.delete('/templates/:templateId', requirePermission('config:write'), async (req: AuthRequest, res: Response) => {
    try {
        const { templateId } = req.params;

        await pool.query(
            'DELETE FROM config_templates WHERE template_id = $1',
            [templateId]
        );

        res.json({
            success: true,
            message: 'Template deleted successfully'
        });
    } catch (error: any) {
        res.status(500).json({ success: false, error: error.message });
    }
});

// ============================================
// IMPORT/EXPORT
// ============================================

// GET /api/config/export - Export configuration
router.get('/export', async (req: AuthRequest, res: Response) => {
    try {
        const { categories, format = 'json' } = req.query;

        const categoriesArray = categories 
            ? (categories as string).split(',') 
            : undefined;

        const config = await configService.exportConfig({ categories: categoriesArray });

        if (format === 'json') {
            res.json({
                success: true,
                config,
                exported_at: new Date().toISOString()
            });
        } else {
            // For file download
            res.setHeader('Content-Type', 'application/json');
            res.setHeader('Content-Disposition', `attachment; filename=config_export_${Date.now()}.json`);
            res.send(JSON.stringify(config, null, 2));
        }
    } catch (error: any) {
        res.status(500).json({ success: false, error: error.message });
    }
});

// POST /api/config/import - Import configuration
router.post('/import', requirePermission('config:write'), async (req: AuthRequest, res: Response) => {
    try {
        const { config, validate_only = false } = req.body;
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, error: 'Unauthorized' });

        const result = await configService.importConfig(config, userId, validate_only);

        res.json({
            success: true,
            result
        });
    } catch (error: any) {
        res.status(400).json({ success: false, error: error.message });
    }
});

// POST /api/config/compare - Compare two configurations
router.post('/compare', async (req: AuthRequest, res: Response) => {
    try {
        const { config1, config2 } = req.body;

        const diff = await configService.compareConfigs(config1, config2);

        res.json({
            success: true,
            diff
        });
    } catch (error: any) {
        res.status(500).json({ success: false, error: error.message });
    }
});

// ============================================
// UTILITIES
// ============================================

// POST /api/config/reset - Reset configuration
router.post('/reset', requirePermission('config:write'), async (req: AuthRequest, res: Response) => {
    try {
        const { category, confirm } = req.body;
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, error: 'Unauthorized' });

        if (!confirm) {
            return res.status(400).json({
                success: false,
                error: 'Confirmation required to reset configuration'
            });
        }

        const count = await configService.resetToDefaults(category, userId);

        res.json({
            success: true,
            message: `Reset ${count} parameters to default values`,
            count
        });
    } catch (error: any) {
        res.status(500).json({ success: false, error: error.message });
    }
});

// POST /api/config/cache/refresh - Refresh configuration cache
router.post('/cache/refresh', requirePermission('config:write'), async (req: AuthRequest, res: Response) => {
    try {
        await configService.refreshCache();

        res.json({
            success: true,
            message: 'Configuration cache refreshed successfully'
        });
    } catch (error: any) {
        res.status(500).json({ success: false, error: error.message });
    }
});

// GET /api/config/snapshot - Get current configuration snapshot
router.get('/snapshot', async (req: AuthRequest, res: Response) => {
    try {
        const snapshot = await configService.getSnapshot();

        res.json({
            success: true,
            snapshot
        });
    } catch (error: any) {
        res.status(500).json({ success: false, error: error.message });
    }
});

// GET /api/config/stats - Get configuration statistics
router.get('/stats', async (req: AuthRequest, res: Response) => {
    try {
        const result = await pool.query(`
            SELECT * FROM v_config_statistics
        `);

        const totalParams = await pool.query(`
            SELECT COUNT(*) as total FROM config_parameters WHERE is_editable = TRUE
        `);

        const modifiedParams = await pool.query(`
            SELECT COUNT(*) as modified 
            FROM config_parameters 
            WHERE is_editable = TRUE AND current_value != default_value
        `);

        const recentChanges = await pool.query(`
            SELECT COUNT(*) as recent_changes
            FROM config_change_history
            WHERE applied_at > NOW() - INTERVAL '24 hours'
        `);

        res.json({
            success: true,
            stats: {
                total_parameters: parseInt(totalParams.rows[0].total),
                modified_parameters: parseInt(modifiedParams.rows[0].modified),
                recent_changes_24h: parseInt(recentChanges.rows[0].recent_changes),
                categories: result.rows
            }
        });
    } catch (error: any) {
        res.status(500).json({ success: false, error: error.message });
    }
});

// GET /api/config/search - Search configuration parameters
router.get('/search', async (req: AuthRequest, res: Response) => {
    try {
        const { q } = req.query;

        if (!q || (q as string).length < 2) {
            return res.status(400).json({
                success: false,
                error: 'Search query must be at least 2 characters'
            });
        }

        const result = await pool.query(`
            SELECT cp.*, cc.category_name, cc.display_name as category_display_name
            FROM config_parameters cp
            JOIN config_categories cc ON cp.category_id = cc.category_id
            WHERE cp.is_editable = TRUE
            AND (
                cp.parameter_key ILIKE $1 OR
                cp.parameter_name ILIKE $1 OR
                cp.description ILIKE $1
            )
            ORDER BY cc.sort_order, cp.sort_order
            LIMIT 50
        `, [`%${q}%`]);

        res.json({
            success: true,
            results: result.rows
        });
    } catch (error: any) {
        res.status(500).json({ success: false, error: error.message });
    }
});

export default router;
