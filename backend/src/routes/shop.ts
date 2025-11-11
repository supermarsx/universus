import express from 'express';
import { ShopService } from '../services/shopService';
import { authenticateToken, assertAuthenticated } from '../middleware/auth';
import { AuthRequest } from '../types';
import { pool } from '../config/database';

const router = express.Router();

// Initialize Stripe with environment variables
const stripeSecretKey = process.env.STRIPE_SECRET_KEY || '';
const stripeWebhookSecret = process.env.STRIPE_WEBHOOK_SECRET || '';

const shopService = new ShopService(pool, stripeSecretKey, stripeWebhookSecret);

// Apply authentication to all routes except webhook
router.use((req, res, next) => {
  if (req.path === '/webhook') {
    next();
  } else {
    authenticateToken(req, res, (err?: any) => {
      if (err) return next(err);
      assertAuthenticated(req, res, next);
    });
  }
});

/**
 * GET /shop/catalog
 * Get all available shop items
 */
router.get('/catalog', async (req: AuthRequest, res) => {
  try {
    const catalog = shopService.getShopCatalog();

    res.json({
      success: true,
      data: catalog,
    });
  } catch (error: any) {
    console.error('Error fetching shop catalog:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to fetch shop catalog',
    });
  }
});

/**
 * POST /shop/create-payment-intent
 * Create a Stripe Payment Intent for a purchase
 * Body: { shopItemId }
 */
router.post('/create-payment-intent', async (req: AuthRequest, res) => {
  try {
    const authReq = req as AuthRequest;
    const userId = authReq.user!.id;
    const { shopItemId } = req.body;

    if (!shopItemId) {
      return res.status(400).json({
        success: false,
        error: 'shopItemId is required',
      });
    }

    const { clientSecret, paymentIntentId } = await shopService.createPaymentIntent(
      userId,
      shopItemId
    );

    res.json({
      success: true,
      data: {
        clientSecret,
        paymentIntentId,
      },
    });
  } catch (error: any) {
    console.error('Error creating payment intent:', error);
    res.status(500).json({
      success: false,
      error: error.message || 'Failed to create payment intent',
    });
  }
});

/**
 * POST /shop/webhook
 * Stripe webhook endpoint
 * NOTE: This must be raw body, not parsed JSON
 */
router.post(
  '/webhook',
  express.raw({ type: 'application/json' }),
  async (req, res) => {
    try {
      const signature = req.headers['stripe-signature'] as string;

      if (!signature) {
        return res.status(400).send('Missing signature');
      }

      await shopService.handleWebhook(req.body.toString(), signature);

      res.json({ received: true });
    } catch (error: any) {
      console.error('Webhook error:', error);
      res.status(400).send(`Webhook Error: ${error.message}`);
    }
  }
);

/**
 * GET /shop/perks
 * Get active perks (officers and boosts) for current user
 */
router.get('/perks', async (req: AuthRequest, res) => {
  try {
    const authReq = req as AuthRequest;
    const userId = authReq.user!.id;

    const perks = await shopService.getUserPerks(userId);

    res.json({
      success: true,
      data: perks,
    });
  } catch (error: any) {
    console.error('Error fetching perks:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to fetch perks',
    });
  }
});

/**
 * GET /shop/purchases
 * Get purchase history for current user
 */
router.get('/purchases', async (req: AuthRequest, res) => {
  try {
    const authReq = req as AuthRequest;
    const userId = authReq.user!.id;
    const limit = parseInt(req.query.limit as string) || 50;

    const purchases = await shopService.getPurchaseHistory(userId, limit);

    res.json({
      success: true,
      data: purchases,
    });
  } catch (error: any) {
    console.error('Error fetching purchase history:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to fetch purchase history',
    });
  }
});

/**
 * POST /shop/deactivate-expired
 * Manually trigger deactivation of expired perks (admin/cron)
 */
router.post('/deactivate-expired', async (req: AuthRequest, res) => {
  try {
    // In production, add admin check here
    const count = await shopService.deactivateExpiredPerks();

    res.json({
      success: true,
      data: { deactivatedCount: count },
      message: `${count} perks deactivated`,
    });
  } catch (error: any) {
    console.error('Error deactivating expired perks:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to deactivate expired perks',
    });
  }
});

export default router;
