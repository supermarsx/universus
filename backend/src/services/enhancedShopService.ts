// Phase 10: Enhanced Shop Service
// Comprehensive shop management with Matrix theme support

import { Pool } from 'pg';
import Stripe from 'stripe';
import { redis } from '../config/redis';
import {
    CosmeticItem,
    UserCosmetic,
    Promotion,
    Gift,
    EnhancedPurchase,
    Bundle,
    MatrixThemeProgress,
    PurchaseCosmeticRequest,
    SendGiftRequest,
    ClaimGiftRequest,
    ApplyPromotionRequest,
    ApplyPromotionResponse,
    UserShopProfile,
    ShopAnalyticsDashboard,
    RecommendationReason,
    SecurityLog,
    SecurityEventType,
    SecuritySeverity,
    PurchaseStatus
} from '../types/enhancedShop';

export class EnhancedShopService {
    private db: Pool;
    private stripe: Stripe;
    private readonly CACHE_TTL = 300; // 5 minutes

    constructor(db: Pool, stripeSecretKey: string) {
        this.db = db;

        // In test or dev environments a Stripe key may not be provided.
        // When missing, create a minimal no-op stub to avoid contacting Stripe.
        if (!stripeSecretKey) {
            this.stripe = {
                paymentIntents: {
                    create: async (opts: any) => ({ id: 'pi_stub', client_secret: 'cs_stub', ...opts }),
                },
                webhooks: {
                    constructEvent: (payload: string, signature: string, secret: string) => JSON.parse(payload),
                },
            } as unknown as Stripe;
        } else {
            this.stripe = new Stripe(stripeSecretKey, {
                apiVersion: '2025-10-29.clover'
            });
        }
    }

    // =====================================================
    // COSMETIC ITEMS MANAGEMENT
    // =====================================================

    async getAllCosmetics(filters?: {
        category?: number;
        item_type?: string;
        rarity?: string;
        matrix_only?: boolean;
    }): Promise<CosmeticItem[]> {
        const cacheKey = `cosmetics:${JSON.stringify(filters || {})}`;
        
        // Try cache first
        try {
            const cached = await redis.get(cacheKey);
            if (cached) {
                return JSON.parse(cached);
            }
        } catch (error) {
            console.error('Redis cache error:', error);
        }

        let query = 'SELECT * FROM shop_cosmetic_items WHERE is_active = TRUE';
        const params: any[] = [];
        let paramCount = 1;

        if (filters?.category) {
            query += ` AND category_id = $${paramCount}`;
            params.push(filters.category);
            paramCount++;
        }

        if (filters?.item_type) {
            query += ` AND item_type = $${paramCount}`;
            params.push(filters.item_type);
            paramCount++;
        }

        if (filters?.rarity) {
            query += ` AND rarity = $${paramCount}`;
            params.push(filters.rarity);
            paramCount++;
        }

        if (filters?.matrix_only) {
            query += ` AND is_matrix_themed = TRUE`;
        }

        query += ' ORDER BY rarity DESC, price_usd DESC';

        const result = await this.db.query(query, params);
        
        // Cache results
        try {
            await redis.setex(cacheKey, this.CACHE_TTL, JSON.stringify(result.rows));
        } catch (error) {
            console.error('Redis cache set error:', error);
        }

        return result.rows;
    }

    async getCosmeticById(cosmeticId: number): Promise<CosmeticItem | null> {
        const result = await this.db.query(
            'SELECT * FROM shop_cosmetic_items WHERE id = $1 AND is_active = TRUE',
            [cosmeticId]
        );
        return result.rows[0] || null;
    }

    async getUserCosmetics(userId: number): Promise<UserCosmetic[]> {
        const result = await this.db.query(
            `SELECT uc.*, sci.name, sci.description, sci.item_type, sci.preview_image_url, sci.css_class
             FROM user_cosmetics uc
             JOIN shop_cosmetic_items sci ON uc.cosmetic_item_id = sci.id
             WHERE uc.user_id = $1
             ORDER BY uc.purchased_at DESC`,
            [userId]
        );
        return result.rows;
    }

    async purchaseCosmetic(request: PurchaseCosmeticRequest): Promise<EnhancedPurchase> {
        const cosmetic = await this.getCosmeticById(request.cosmetic_item_id);
        
        if (!cosmetic) {
            throw new Error('Cosmetic item not found');
        }

        // Check stock
        if (cosmetic.stock_quantity !== null && cosmetic.stock_quantity !== undefined && cosmetic.stock_quantity <= 0) {
            throw new Error('Item out of stock');
        }

        // Check if user already owns this (non-stackable items)
        const existing = await this.db.query(
            'SELECT * FROM user_cosmetics WHERE user_id = $1 AND cosmetic_item_id = $2',
            [request.user_id, request.cosmetic_item_id]
        );

        if (existing.rows.length > 0 && cosmetic.item_type !== 'decoration') {
            throw new Error('You already own this item');
        }

        let finalPrice = cosmetic.price_usd;
        let promotionId: number | undefined;

        // Apply promotion if provided
        if (request.promo_code) {
            const promoResult = await this.applyPromotion({
                promo_code: request.promo_code,
                user_id: request.user_id,
                cart_items: [{
                    item_type: 'cosmetic',
                    item_id: cosmetic.item_code,
                    price: cosmetic.price_usd
                }]
            });

            if (promoResult.is_valid) {
                finalPrice = promoResult.final_total;
                promotionId = promoResult.promotion_details?.id;
            }
        }

        // Process payment
        if (request.payment_method === 'usd') {
            // Create Stripe payment intent
            const paymentIntent = await this.stripe.paymentIntents.create({
                amount: finalPrice,
                currency: 'usd',
                metadata: {
                    user_id: request.user_id.toString(),
                    item_type: 'cosmetic',
                    item_id: cosmetic.item_code
                }
            });

            // Create purchase record
            const purchase = await this.createEnhancedPurchase({
                user_id: request.user_id,
                item_type: 'cosmetic',
                item_id: cosmetic.item_code,
                quantity: 1,
                price_usd: cosmetic.price_usd,
                discount_applied: cosmetic.price_usd - finalPrice,
                final_price: finalPrice,
                promotion_id: promotionId,
                payment_method: 'stripe',
                stripe_payment_intent_id: paymentIntent.id,
                status: PurchaseStatus.PENDING
            });

            return purchase;

        } else if (request.payment_method === 'dark_matter') {
            if (!cosmetic.price_dark_matter) {
                throw new Error('This item cannot be purchased with Dark Matter');
            }

            // Check user's dark matter balance
            const userResult = await this.db.query(
                'SELECT dark_matter FROM users WHERE id = $1',
                [request.user_id]
            );

            const userDM = userResult.rows[0]?.dark_matter || 0;

            if (userDM < cosmetic.price_dark_matter) {
                throw new Error('Insufficient Dark Matter');
            }

            // Deduct dark matter
            await this.db.query(
                'UPDATE users SET dark_matter = dark_matter - $1 WHERE id = $2',
                [cosmetic.price_dark_matter, request.user_id]
            );

            // Grant cosmetic immediately
            await this.grantCosmetic(request.user_id, request.cosmetic_item_id, 'purchase');

            // Create purchase record
            const purchase = await this.createEnhancedPurchase({
                user_id: request.user_id,
                item_type: 'cosmetic',
                item_id: cosmetic.item_code,
                quantity: 1,
                price_usd: 0,
                discount_applied: 0,
                final_price: 0,
                payment_method: 'dark_matter',
                status: PurchaseStatus.COMPLETED
            });

            return purchase;
        }

        throw new Error('Invalid payment method');
    }

    async grantCosmetic(userId: number, cosmeticItemId: number, source: string): Promise<void> {
        await this.db.query(
            `INSERT INTO user_cosmetics (user_id, cosmetic_item_id, source)
             VALUES ($1, $2, $3)
             ON CONFLICT (user_id, cosmetic_item_id) 
             DO UPDATE SET quantity = user_cosmetics.quantity + 1`,
            [userId, cosmeticItemId, source]
        );

        // Update stock
        await this.db.query(
            `UPDATE shop_cosmetic_items 
             SET stock_quantity = stock_quantity - 1 
             WHERE id = $1 AND stock_quantity IS NOT NULL`,
            [cosmeticItemId]
        );
    }

    async equipCosmetic(userId: number, cosmeticItemId: number): Promise<void> {
        // Check if user owns this cosmetic
        const owned = await this.db.query(
            'SELECT * FROM user_cosmetics WHERE user_id = $1 AND cosmetic_item_id = $2',
            [userId, cosmeticItemId]
        );

        if (owned.rows.length === 0) {
            throw new Error('You do not own this cosmetic');
        }

        // Get cosmetic details to know what slot it uses
        const cosmetic = await this.getCosmeticById(cosmeticItemId);
        if (!cosmetic) {
            throw new Error('Cosmetic not found');
        }

        // Unequip other cosmetics of the same type/target
        await this.db.query(
            `UPDATE user_cosmetics 
             SET is_equipped = FALSE 
             WHERE user_id = $1 
             AND cosmetic_item_id IN (
                 SELECT id FROM shop_cosmetic_items 
                 WHERE item_type = $2 AND target_entity = $3
             )`,
            [userId, cosmetic.item_type, cosmetic.target_entity]
        );

        // Equip this cosmetic
        await this.db.query(
            `UPDATE user_cosmetics 
             SET is_equipped = TRUE, equipped_at = NOW() 
             WHERE user_id = $1 AND cosmetic_item_id = $2`,
            [userId, cosmeticItemId]
        );
    }

    // =====================================================
    // PROMOTIONS & FLASH SALES
    // =====================================================

    async getActivePromotions(): Promise<Promotion[]> {
        const result = await this.db.query(
            `SELECT * FROM v_active_promotions
             ORDER BY is_featured DESC, discount_percentage DESC
             LIMIT 20`
        );
        return result.rows;
    }

    async applyPromotion(request: ApplyPromotionRequest): Promise<ApplyPromotionResponse> {
        const result = await this.db.query(
            `SELECT * FROM shop_promotions 
             WHERE promo_code = $1 
             AND is_active = TRUE 
             AND NOW() BETWEEN start_date AND end_date`,
            [request.promo_code]
        );

        if (result.rows.length === 0) {
            return {
                is_valid: false,
                discount_amount: 0,
                final_total: request.cart_items.reduce((sum, item) => sum + item.price, 0)
            };
        }

        const promotion = result.rows[0];

        // Check usage limits
        if (promotion.max_uses && promotion.uses_count >= promotion.max_uses) {
            return {
                is_valid: false,
                discount_amount: 0,
                final_total: request.cart_items.reduce((sum, item) => sum + item.price, 0)
            };
        }

        // Check user usage limit
        const userUses = await this.db.query(
            'SELECT COUNT(*) as count FROM shop_promotion_uses WHERE promotion_id = $1 AND user_id = $2',
            [promotion.id, request.user_id]
        );

        if (!userUses.rows[0] || userUses.rows[0].count >= promotion.max_uses_per_user) {
            return {
                is_valid: false,
                discount_amount: 0,
                final_total: request.cart_items.reduce((sum, item) => sum + item.price, 0)
            };
        }

        // Calculate discount
        const cartTotal = request.cart_items.reduce((sum, item) => sum + item.price, 0);

        if (promotion.min_purchase_amount && cartTotal < promotion.min_purchase_amount) {
            return {
                is_valid: false,
                discount_amount: 0,
                final_total: cartTotal
            };
        }

        let discountAmount = 0;

        if (promotion.discount_percentage) {
            discountAmount = Math.floor(cartTotal * (promotion.discount_percentage / 100));
        } else if (promotion.discount_amount) {
            discountAmount = promotion.discount_amount;
        }

        const finalTotal = Math.max(0, cartTotal - discountAmount);

        return {
            is_valid: true,
            discount_amount: discountAmount,
            final_total: finalTotal,
            promotion_details: promotion
        };
    }

    async getActiveFlashSales(): Promise<any[]> {
        const result = await this.db.query(
            `SELECT * FROM shop_flash_sales 
             WHERE is_active = TRUE 
             AND NOW() BETWEEN start_time AND end_time
             AND (stock_quantity IS NULL OR stock_quantity > sold_quantity)
             ORDER BY discount_percentage DESC`
        );
        return result.rows;
    }

    // =====================================================
    // GIFT SYSTEM
    // =====================================================

    async sendGift(request: SendGiftRequest): Promise<Gift> {
        // Generate unique gift code
        const giftCode = this.generateGiftCode();
        
        // Get item details and price
        let itemPrice = 0;
        
        if (request.item_type === 'cosmetic') {
            const cosmetic = await this.db.query(
                'SELECT price_usd FROM shop_cosmetic_items WHERE item_code = $1',
                [request.item_id]
            );
            if (cosmetic.rows.length > 0 && cosmetic.rows[0]) {
                itemPrice = cosmetic.rows[0].price_usd;
            }
        }

        // Check if recipient exists
        const recipient = await this.db.query(
            'SELECT id FROM users WHERE email = $1',
            [request.recipient_email]
        );

        const recipientId = recipient.rows.length > 0 ? recipient.rows[0].id : null;

        // Create gift record
        const result = await this.db.query(
            `INSERT INTO shop_gifts (
                sender_user_id, recipient_user_id, recipient_email, 
                item_type, item_id, quantity, personal_message,
                gift_code, purchase_price, expires_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW() + INTERVAL '30 days')
            RETURNING *`,
            [
                request.sender_user_id,
                recipientId,
                request.recipient_email,
                request.item_type,
                request.item_id,
                request.quantity || 1,
                request.personal_message,
                giftCode,
                itemPrice
            ]
        );

        if (result.rows.length === 0) {
            throw new Error('Gift creation failed');
        }

        try {
            const senderResult = await this.db.query(
                'SELECT username FROM users WHERE id = $1',
                [request.sender_user_id]
            );
            const senderName = senderResult.rows[0]?.username;
            const { EmailService } = await import('./emailService');
            await EmailService.sendGiftNotification(
                request.recipient_email,
                giftCode,
                senderName,
                request.personal_message
            );
        } catch (error) {
            console.error('Failed to send gift notification email:', error);
        }

        return result.rows[0];
    }

    async claimGift(request: ClaimGiftRequest): Promise<void> {
        const result = await this.db.query(
            `SELECT * FROM shop_gifts 
             WHERE gift_code = $1 
             AND status = 'pending'
             AND expires_at > NOW()`,
            [request.gift_code]
        );

        if (result.rows.length === 0) {
            throw new Error('Invalid or expired gift code');
        }

        const gift = result.rows[0];
        if (!gift) {
            throw new Error('Gift data is null');
        }

        // Grant the item
        if (gift.item_type === 'cosmetic') {
            const cosmetic = await this.db.query(
                'SELECT id FROM shop_cosmetic_items WHERE item_code = $1',
                [gift.item_id]
            );
            if (cosmetic.rows.length > 0) {
                await this.grantCosmetic(request.user_id, cosmetic.rows[0].id, 'gift');
            }
        }

        // Mark gift as claimed
        await this.db.query(
            `UPDATE shop_gifts 
             SET status = 'claimed', 
                 claimed_at = NOW(),
                 recipient_user_id = $1
             WHERE id = $2`,
            [request.user_id, gift.id]
        );
    }

    private generateGiftCode(): string {
        const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789';
        let code = '';
        for (let i = 0; i < 12; i++) {
            code += chars.charAt(Math.floor(Math.random() * chars.length));
        }
        return code;
    }

    // =====================================================
    // BUNDLES
    // =====================================================

    async getAvailableBundles(matrixOnly?: boolean): Promise<Bundle[]> {
        let query = `
            SELECT * FROM shop_bundles 
            WHERE is_active = TRUE
            AND (available_from IS NULL OR available_from <= NOW())
            AND (available_until IS NULL OR available_until >= NOW())
            AND (stock_quantity IS NULL OR stock_quantity > sold_quantity)
        `;

        if (matrixOnly) {
            query += ' AND is_matrix_themed = TRUE';
        }

        query += ' ORDER BY savings_percentage DESC';

        const result = await this.db.query(query);
        return result.rows;
    }

    // =====================================================
    // MATRIX THEME PROGRESSION
    // =====================================================

    async getMatrixProgress(userId: number): Promise<MatrixThemeProgress> {
        let result = await this.db.query(
            'SELECT * FROM matrix_theme_progress WHERE user_id = $1',
            [userId]
        );

        if (result.rows.length === 0) {
            // Initialize progress
            await this.db.query(
                'INSERT INTO matrix_theme_progress (user_id) VALUES ($1)',
                [userId]
            );
            result = await this.db.query(
                'SELECT * FROM matrix_theme_progress WHERE user_id = $1',
                [userId]
            );
        }

        return result.rows[0] || {
            user_id: userId,
            theme_unlocked: false,
            matrix_level: 1,
            matrix_points: 0
        };
    }

    async unlockMatrixTheme(userId: number): Promise<void> {
        await this.db.query(
            `UPDATE matrix_theme_progress 
             SET theme_unlocked = TRUE, unlock_date = NOW(), updated_at = NOW()
             WHERE user_id = $1`,
            [userId]
        );
    }

    async addMatrixPoints(userId: number, points: number): Promise<void> {
        await this.db.query(
            `UPDATE matrix_theme_progress 
             SET matrix_points = matrix_points + $1, updated_at = NOW()
             WHERE user_id = $2`,
            [points, userId]
        );

        // Check level up
        const progress = await this.getMatrixProgress(userId);
        const newLevel = Math.floor(progress.matrix_points / 1000) + 1;
        
        if (newLevel > progress.matrix_level && newLevel <= 10) {
            await this.db.query(
                `UPDATE matrix_theme_progress 
                 SET matrix_level = $1, updated_at = NOW()
                 WHERE user_id = $2`,
                [newLevel, userId]
            );
        }
    }

    // =====================================================
    // ANALYTICS & RECOMMENDATIONS
    // =====================================================

    async getShopAnalyticsDashboard(): Promise<ShopAnalyticsDashboard> {
        // Today's revenue
        const todayRevenue = await this.db.query(
            `SELECT 
                COALESCE(SUM(final_price), 0) as revenue,
                COUNT(*) as purchases
             FROM shop_purchases_enhanced
             WHERE DATE(created_at) = CURRENT_DATE
             AND status = 'completed'`
        );

        // Total revenue
        const totalRevenue = await this.db.query(
            `SELECT 
                COALESCE(SUM(final_price), 0) as revenue,
                COUNT(*) as purchases
             FROM shop_purchases_enhanced
             WHERE status = 'completed'`
        );

        // Top items
        const topItems = await this.db.query(
            'SELECT * FROM v_top_selling_items LIMIT 10'
        );

        // VIP count
        const vipCount = await this.db.query(
            'SELECT COUNT(*) as count FROM shop_user_analytics WHERE is_vip = TRUE'
        );

        return {
            today_revenue: todayRevenue.rows[0].revenue,
            today_purchases: todayRevenue.rows[0].purchases,
            total_revenue: totalRevenue.rows[0].revenue,
            total_purchases: totalRevenue.rows[0].purchases,
            top_items: topItems.rows,
            trending_items: [],
            vip_count: vipCount.rows[0].count,
            active_promotions: await this.getActivePromotions(),
            flash_sales: await this.getActiveFlashSales()
        };
    }

    async getUserShopProfile(userId: number): Promise<UserShopProfile> {
        // Get user analytics
        let userAnalytics = await this.db.query(
            'SELECT * FROM shop_user_analytics WHERE user_id = $1',
            [userId]
        );

        if (userAnalytics.rows.length === 0) {
            // Initialize
            await this.db.query(
                'INSERT INTO shop_user_analytics (user_id) VALUES ($1)',
                [userId]
            );
            userAnalytics = await this.db.query(
                'SELECT * FROM shop_user_analytics WHERE user_id = $1',
                [userId]
            );
        }

        return {
            user_analytics: userAnalytics.rows[0] || {},
            owned_cosmetics: await this.getUserCosmetics(userId),
            equipped_cosmetics: await this.db.query(
                `SELECT * FROM user_cosmetics WHERE user_id = $1 AND is_equipped = TRUE`,
                [userId]
            ).then(r => r.rows),
            matrix_progress: await this.getMatrixProgress(userId),
            recommendations: await this.getRecommendations(userId),
            available_promotions: await this.getActivePromotions()
        };
    }

    async getRecommendations(userId: number, limit: number = 6): Promise<any[]> {
        // Get user's purchase history
        const purchases = await this.db.query(
            `SELECT item_type, item_id FROM shop_purchases_enhanced 
             WHERE user_id = $1 AND status = 'completed'
             ORDER BY completed_at DESC
             LIMIT 10`,
            [userId]
        );

        // Get trending items
        const trending = await this.db.query(
            `SELECT item_type, item_id FROM shop_item_analytics
             WHERE trend_score > 0.5
             ORDER BY trend_score DESC
             LIMIT $1`,
            [limit]
        );

        return trending.rows.map(item => ({
            ...item,
            recommendation_reason: RecommendationReason.TRENDING,
            confidence_score: 0.8
        }));
    }

    // =====================================================
    // SECURITY
    // =====================================================

    async logSecurityEvent(event: {
        user_id?: number;
        event_type: SecurityEventType;
        event_description?: string;
        severity: SecuritySeverity;
        ip_address?: string;
        user_agent?: string;
        metadata?: Record<string, any>;
        action_taken?: string;
    }): Promise<void> {
        await this.db.query(
            `INSERT INTO shop_security_logs (
                user_id, event_type, event_description, severity,
                ip_address, user_agent, metadata, action_taken
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)`,
            [
                event.user_id,
                event.event_type,
                event.event_description,
                event.severity,
                event.ip_address,
                event.user_agent,
                JSON.stringify(event.metadata || {}),
                event.action_taken
            ]
        );
    }

    // =====================================================
    // HELPER METHODS
    // =====================================================

    private async createEnhancedPurchase(data: {
        user_id: number;
        item_type: string;
        item_id: string;
        quantity: number;
        price_usd: number;
        discount_applied: number;
        final_price: number;
        promotion_id?: number;
        payment_method?: string;
        stripe_payment_intent_id?: string;
        status: PurchaseStatus;
    }): Promise<EnhancedPurchase> {
        const result = await this.db.query(
            `INSERT INTO shop_purchases_enhanced (
                user_id, item_type, item_id, quantity, price_usd,
                discount_applied, final_price, promotion_id, payment_method,
                stripe_payment_intent_id, status
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING *`,
            [
                data.user_id,
                data.item_type,
                data.item_id,
                data.quantity,
                data.price_usd,
                data.discount_applied,
                data.final_price,
                data.promotion_id,
                data.payment_method,
                data.stripe_payment_intent_id,
                data.status
            ]
        );

        if (result.rows.length === 0) {
            throw new Error('Purchase creation failed');
        }
        return result.rows[0];
    }

    // =====================================================
    // STRIPE WEBHOOK HANDLING
    // =====================================================

    async handleStripeWebhook(event: Stripe.Event): Promise<void> {
        switch (event.type) {
            case 'payment_intent.succeeded':
                await this.handlePaymentSuccess(event.data.object as Stripe.PaymentIntent);
                break;
            
            case 'payment_intent.payment_failed':
                await this.handlePaymentFailed(event.data.object as Stripe.PaymentIntent);
                break;
            
            case 'charge.refunded':
                await this.handleRefund(event.data.object as Stripe.Charge);
                break;
            
            default:
                console.log(`Unhandled webhook event type: ${event.type}`);
        }
    }

    private async handlePaymentSuccess(paymentIntent: Stripe.PaymentIntent): Promise<void> {
        const userId = parseInt(paymentIntent.metadata.user_id);
        const itemType = paymentIntent.metadata.item_type;
        const itemId = paymentIntent.metadata.item_id;

        // Update purchase status
        await this.db.query(
            `UPDATE shop_purchases_enhanced 
             SET status = 'completed', completed_at = NOW()
             WHERE stripe_payment_intent_id = $1`,
            [paymentIntent.id]
        );

        // Grant the item based on type
        if (itemType === 'cosmetic') {
            const cosmetic = await this.db.query(
                'SELECT id FROM shop_cosmetic_items WHERE item_code = $1',
                [itemId]
            );
            if (cosmetic.rows.length > 0) {
                await this.grantCosmetic(userId, cosmetic.rows[0].id, 'purchase');
            }
        } else if (itemType === 'bundle') {
            // Handle bundle items
            const bundle = await this.db.query(
                'SELECT items FROM shop_bundles WHERE bundle_code = $1',
                [itemId]
            );
            if (bundle.rows.length > 0) {
                const items = bundle.rows[0].items;
                for (const item of items) {
                    if (item.item_type === 'cosmetic') {
                        const cosmetic = await this.db.query(
                            'SELECT id FROM shop_cosmetic_items WHERE item_code = $1',
                            [item.item_id]
                        );
                        if (cosmetic.rows.length > 0) {
                            await this.grantCosmetic(userId, cosmetic.rows[0].id, 'purchase');
                        }
                    }
                }
            }
        }

        // Log successful payment
        console.log(`Payment successful for user ${userId}: ${paymentIntent.id}`);
    }

    private async handlePaymentFailed(paymentIntent: Stripe.PaymentIntent): Promise<void> {
        // Update purchase status
        await this.db.query(
            `UPDATE shop_purchases_enhanced 
             SET status = 'failed'
             WHERE stripe_payment_intent_id = $1`,
            [paymentIntent.id]
        );

        // Log security event
        await this.logSecurityEvent({
            user_id: parseInt(paymentIntent.metadata.user_id),
            event_type: SecurityEventType.MULTIPLE_FAILED_ATTEMPTS,
            event_description: `Payment failed: ${paymentIntent.last_payment_error?.message || 'Unknown error'}`,
            severity: SecuritySeverity.MEDIUM,
            metadata: {
                payment_intent_id: paymentIntent.id,
                error: paymentIntent.last_payment_error
            }
        });

        console.log(`Payment failed: ${paymentIntent.id}`);
    }

    private async handleRefund(charge: Stripe.Charge): Promise<void> {
        // Find the purchase
        const purchase = await this.db.query(
            `SELECT * FROM shop_purchases_enhanced 
             WHERE stripe_charge_id = $1`,
            [charge.id]
        );

        if (purchase.rows.length > 0) {
            const purchaseData = purchase.rows[0];

            // Create refund record
            await this.db.query(
                `INSERT INTO shop_refunds (
                    purchase_id, user_id, refund_amount, refund_type, 
                    stripe_refund_id, status, processed_at
                ) VALUES ($1, $2, $3, $4, $5, $6, NOW())`,
                [
                    purchaseData.id,
                    purchaseData.user_id,
                    charge.amount_refunded,
                    'full',
                    charge.refunds?.data[0]?.id,
                    'completed'
                ]
            );

            // Update purchase status
            await this.db.query(
                `UPDATE shop_purchases_enhanced 
                 SET status = 'refunded', refunded_at = NOW()
                 WHERE id = $1`,
                [purchaseData.id]
            );

            // Remove cosmetic if it was granted
            if (purchaseData.item_type === 'cosmetic') {
                const cosmetic = await this.db.query(
                    'SELECT id FROM shop_cosmetic_items WHERE item_code = $1',
                    [purchaseData.item_id]
                );
                if (cosmetic.rows.length > 0) {
                    await this.db.query(
                        'DELETE FROM user_cosmetics WHERE user_id = $1 AND cosmetic_item_id = $2',
                        [purchaseData.user_id, cosmetic.rows[0].id]
                    );
                }
            }

            // Log security event
            await this.logSecurityEvent({
                user_id: purchaseData.user_id,
                event_type: SecurityEventType.REFUND_ABUSE,
                event_description: `Refund processed for charge ${charge.id}`,
                severity: SecuritySeverity.LOW,
                metadata: {
                    charge_id: charge.id,
                    refund_amount: charge.amount_refunded
                }
            });
        }
    }
}
