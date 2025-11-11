// Phase 10: Enhanced Shop API Routes
// REST API endpoints for the enhanced shop system with Matrix theme

import express, { Router, Request, Response } from 'express';
import { AuthRequest } from '../types';
import { pool } from '../config/database';
import { EnhancedShopService } from '../services/enhancedShopService';
import { authenticateToken } from '../middleware/auth';
import { requirePermission } from '../middleware/adminAuth';
import { redis } from '../config/redis';
import { getUserId } from '../utils/authHelpers';

const router = Router();
const shopService = new EnhancedShopService(
    pool,
    process.env.STRIPE_SECRET_KEY || ''
);

// =====================================================
// COSMETIC ITEMS
// =====================================================

// GET /api/shop/cosmetics - Get all cosmetic items with filters
router.get('/cosmetics', async (req: Request, res: Response) => {
    try {
        const filters = {
            category: req.query.category ? parseInt(req.query.category as string) : undefined,
            item_type: req.query.item_type as string,
            rarity: req.query.rarity as string,
            matrix_only: req.query.matrix_only === 'true'
        };

        const cosmetics = await shopService.getAllCosmetics(filters);
        res.json({ success: true, data: cosmetics });
    } catch (error: any) {
        res.status(500).json({ success: false, error: error.message });
    }
});

// GET /api/shop/cosmetics/:id - Get specific cosmetic item
router.get('/cosmetics/:id', async (req: Request, res: Response) => {
    try {
        const cosmetic = await shopService.getCosmeticById(parseInt(req.params.id));
        
        if (!cosmetic) {
            return res.status(404).json({ success: false, error: 'Cosmetic not found' });
        }

        res.json({ success: true, data: cosmetic });
    } catch (error: any) {
        res.status(500).json({ success: false, error: error.message });
    }
});

// GET /api/shop/my-cosmetics - Get user's owned cosmetics
router.get('/my-cosmetics', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, error: 'Unauthorized' });
        const cosmetics = await shopService.getUserCosmetics(userId);
        res.json({ success: true, data: cosmetics });
    } catch (error: any) {
        res.status(500).json({ success: false, error: error.message });
    }
});

// POST /api/shop/cosmetics/purchase - Purchase a cosmetic item
router.post('/cosmetics/purchase', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, error: 'Unauthorized' });
        const { cosmetic_item_id, payment_method, promo_code } = req.body;

        const purchase = await shopService.purchaseCosmetic({
            user_id: userId,
            cosmetic_item_id,
            payment_method,
            promo_code
        });

        res.json({ success: true, data: purchase });
    } catch (error: any) {
        res.status(400).json({ success: false, error: error.message });
    }
});

// POST /api/shop/cosmetics/equip - Equip a cosmetic item
router.post('/cosmetics/equip', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, error: 'Unauthorized' });
        const { cosmetic_item_id } = req.body;

        await shopService.equipCosmetic(userId, cosmetic_item_id);
        res.json({ success: true, message: 'Cosmetic equipped successfully' });
    } catch (error: any) {
        res.status(400).json({ success: false, error: error.message });
    }
});

// =====================================================
// PROMOTIONS
// =====================================================

// GET /api/shop/promotions - Get active promotions
router.get('/promotions', async (req: Request, res: Response) => {
    try {
        const promotions = await shopService.getActivePromotions();
        res.json({ success: true, data: promotions });
    } catch (error: any) {
        res.status(500).json({ success: false, error: error.message });
    }
});

// POST /api/shop/promotions/validate - Validate and apply promotion code
router.post('/promotions/validate', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, error: 'Unauthorized' });
        const { promo_code, cart_items } = req.body;

        const result = await shopService.applyPromotion({
            promo_code,
            user_id: userId,
            cart_items
        });

        res.json({ success: true, data: result });
    } catch (error: any) {
        res.status(400).json({ success: false, error: error.message });
    }
});

// GET /api/shop/flash-sales - Get active flash sales
router.get('/flash-sales', async (req: Request, res: Response) => {
    try {
        const flashSales = await shopService.getActiveFlashSales();
        res.json({ success: true, data: flashSales });
    } catch (error: any) {
        res.status(500).json({ success: false, error: error.message });
    }
});

// =====================================================
// BUNDLES
// =====================================================

// GET /api/shop/bundles - Get available bundles
router.get('/bundles', async (req: Request, res: Response) => {
    try {
        const matrixOnly = req.query.matrix_only === 'true';
        const bundles = await shopService.getAvailableBundles(matrixOnly);
        res.json({ success: true, data: bundles });
    } catch (error: any) {
        res.status(500).json({ success: false, error: error.message });
    }
});

// =====================================================
// GIFTS
// =====================================================

// POST /api/shop/gifts/send - Send a gift
router.post('/gifts/send', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, error: 'Unauthorized' });
        const { recipient_email, item_type, item_id, quantity, personal_message } = req.body;

        const gift = await shopService.sendGift({
            sender_user_id: userId,
            recipient_email,
            item_type,
            item_id,
            quantity,
            personal_message
        });

        res.json({ 
            success: true, 
            data: gift,
            message: 'Gift sent successfully! The recipient will receive an email with the gift code.'
        });
    } catch (error: any) {
        res.status(400).json({ success: false, error: error.message });
    }
});

// POST /api/shop/gifts/claim - Claim a gift
router.post('/gifts/claim', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, error: 'Unauthorized' });
        const { gift_code } = req.body;

        await shopService.claimGift({
            user_id: userId,
            gift_code
        });

        res.json({ success: true, message: 'Gift claimed successfully!' });
    } catch (error: any) {
        res.status(400).json({ success: false, error: error.message });
    }
});

// =====================================================
// MATRIX THEME
// =====================================================

// GET /api/shop/matrix/progress - Get user's Matrix theme progress
router.get('/matrix/progress', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, error: 'Unauthorized' });
        const progress = await shopService.getMatrixProgress(userId);
        res.json({ success: true, data: progress });
    } catch (error: any) {
        res.status(500).json({ success: false, error: error.message });
    }
});

// POST /api/shop/matrix/unlock - Unlock Matrix theme
router.post('/matrix/unlock', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, error: 'Unauthorized' });
        await shopService.unlockMatrixTheme(userId);
        res.json({ 
            success: true, 
            message: 'Welcome to the Matrix! Theme unlocked successfully.'
        });
    } catch (error: any) {
        res.status(400).json({ success: false, error: error.message });
    }
});

// POST /api/shop/matrix/points - Add Matrix points
router.post('/matrix/points', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, error: 'Unauthorized' });
        const { points } = req.body;

        await shopService.addMatrixPoints(userId, points);
        res.json({ success: true, message: `Added ${points} Matrix points` });
    } catch (error: any) {
        res.status(400).json({ success: false, error: error.message });
    }
});

// =====================================================
// USER PROFILE & RECOMMENDATIONS
// =====================================================

// GET /api/shop/profile - Get user's complete shop profile
router.get('/profile', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, error: 'Unauthorized' });
        const profile = await shopService.getUserShopProfile(userId);
        res.json({ success: true, data: profile });
    } catch (error: any) {
        res.status(500).json({ success: false, error: error.message });
    }
});

// GET /api/shop/recommendations - Get personalized recommendations
router.get('/recommendations', authenticateToken, async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, error: 'Unauthorized' });
        const limit = req.query.limit ? parseInt(req.query.limit as string) : 6;
        
        const recommendations = await shopService.getRecommendations(userId, limit);
        res.json({ success: true, data: recommendations });
    } catch (error: any) {
        res.status(500).json({ success: false, error: error.message });
    }
});

// =====================================================
// ANALYTICS (Admin only)
// =====================================================

// GET /api/shop/analytics/dashboard - Get shop analytics dashboard
router.get('/analytics/dashboard', authenticateToken, requirePermission('shop:analytics'), async (req: Request, res: Response) => {
    try {
        const dashboard = await shopService.getShopAnalyticsDashboard();
        res.json({ success: true, data: dashboard });
    } catch (error: any) {
        res.status(500).json({ success: false, error: error.message });
    }
});

// =====================================================
// WEBHOOK (Stripe payment completion)
// =====================================================

// POST /api/shop-enhanced/webhook/stripe - Handle Stripe webhooks
router.post('/webhook/stripe', express.raw({type: 'application/json'}), async (req: Request, res: Response) => {
    try {
        const sig = req.headers['stripe-signature'] as string;
        const webhookSecret = process.env.STRIPE_WEBHOOK_SECRET || '';

        if (!webhookSecret) {
            console.error('STRIPE_WEBHOOK_SECRET not configured');
            return res.status(500).json({ error: 'Webhook secret not configured' });
        }

        // Verify webhook signature
        const stripe = new (await import('stripe')).default(process.env.STRIPE_SECRET_KEY || '', {
            apiVersion: '2025-10-29.clover'
        });

        let event;
        try {
            event = stripe.webhooks.constructEvent(req.body, sig, webhookSecret);
        } catch (err: any) {
            console.error(`Webhook signature verification failed: ${err.message}`);
            return res.status(400).json({ error: `Webhook Error: ${err.message}` });
        }

        // Handle the event
        await shopService.handleStripeWebhook(event);

        res.json({ received: true });
    } catch (error: any) {
        console.error('Webhook handling error:', error);
        res.status(400).json({ success: false, error: error.message });
    }
});

export default router;
