// Phase 10: Enhanced Shop & Matrix Theme - TypeScript Types
// Comprehensive type definitions for the enhanced shop system

// =====================================================
// COSMETIC ITEMS
// =====================================================

export enum CosmeticItemType {
    SHIP_SKIN = 'ship_skin',
    BUILDING_SKIN = 'building_skin',
    THEME = 'theme',
    DECORATION = 'decoration',
    BADGE = 'badge',
    AVATAR = 'avatar'
}

export enum CosmeticRarity {
    COMMON = 'common',
    RARE = 'rare',
    EPIC = 'epic',
    LEGENDARY = 'legendary',
    MATRIX_EXCLUSIVE = 'matrix_exclusive'
}

export interface CosmeticCategory {
    id: number;
    name: string;
    description: string;
    icon_url?: string;
    display_order: number;
    is_active: boolean;
    created_at: Date;
}

export interface CosmeticItem {
    id: number;
    category_id: number;
    item_code: string;
    name: string;
    description: string;
    item_type: CosmeticItemType;
    target_entity?: string;
    rarity: CosmeticRarity;
    price_usd: number;
    price_dark_matter?: number;
    is_matrix_themed: boolean;
    preview_image_url?: string;
    preview_video_url?: string;
    css_class?: string;
    effect_data?: Record<string, any>;
    is_limited: boolean;
    is_exclusive: boolean;
    is_tradeable: boolean;
    stock_quantity?: number;
    is_active: boolean;
    created_at: Date;
    updated_at: Date;
}

export interface UserCosmetic {
    id: number;
    user_id: number;
    cosmetic_item_id: number;
    quantity: number;
    purchased_at: Date;
    is_equipped: boolean;
    equipped_at?: Date;
    source: 'purchase' | 'gift' | 'promotion' | 'achievement';
}

// =====================================================
// PROMOTIONS
// =====================================================

export enum PromotionType {
    DISCOUNT = 'discount',
    BUNDLE = 'bundle',
    FLASH_SALE = 'flash_sale',
    SEASONAL = 'seasonal',
    FIRST_PURCHASE = 'first_purchase'
}

export interface Promotion {
    id: number;
    promo_code?: string;
    name: string;
    description?: string;
    promotion_type: PromotionType;
    discount_percentage?: number;
    discount_amount?: number;
    applicable_items?: Record<string, any>;
    min_purchase_amount?: number;
    max_uses?: number;
    max_uses_per_user: number;
    uses_count: number;
    start_date: Date;
    end_date: Date;
    is_active: boolean;
    is_featured: boolean;
    banner_image_url?: string;
    created_at: Date;
}

export interface PromotionUse {
    id: number;
    promotion_id: number;
    user_id: number;
    purchase_id?: number;
    discount_applied: number;
    used_at: Date;
}

export interface FlashSale {
    id: number;
    item_id: string;
    item_type: string;
    original_price: number;
    sale_price: number;
    discount_percentage: number;
    stock_quantity?: number;
    sold_quantity: number;
    start_time: Date;
    end_time: Date;
    is_active: boolean;
    created_at: Date;
}

// =====================================================
// GIFTS
// =====================================================

export enum GiftStatus {
    PENDING = 'pending',
    CLAIMED = 'claimed',
    EXPIRED = 'expired',
    REFUNDED = 'refunded'
}

export interface Gift {
    id: number;
    sender_user_id: number;
    recipient_user_id?: number;
    recipient_email?: string;
    item_type: string;
    item_id: string;
    quantity: number;
    personal_message?: string;
    gift_code: string;
    purchase_price: number;
    status: GiftStatus;
    purchased_at: Date;
    claimed_at?: Date;
    expires_at: Date;
    stripe_payment_id?: string;
}

export interface SendGiftRequest {
    sender_user_id: number;
    recipient_email: string;
    item_type: string;
    item_id: string;
    quantity?: number;
    personal_message?: string;
}

export interface ClaimGiftRequest {
    user_id: number;
    gift_code: string;
}

// =====================================================
// ENHANCED PURCHASES
// =====================================================

export enum PurchaseStatus {
    PENDING = 'pending',
    COMPLETED = 'completed',
    FAILED = 'failed',
    REFUNDED = 'refunded'
}

export interface EnhancedPurchase {
    id: number;
    user_id: number;
    item_type: string;
    item_id: string;
    quantity: number;
    price_usd: number;
    currency: string;
    payment_method?: string;
    stripe_payment_intent_id?: string;
    stripe_charge_id?: string;
    promotion_id?: number;
    discount_applied: number;
    final_price: number;
    status: PurchaseStatus;
    ip_address?: string;
    user_agent?: string;
    device_type?: string;
    referrer?: string;
    created_at: Date;
    completed_at?: Date;
    refunded_at?: Date;
}

export interface CreatePurchaseRequest {
    user_id: number;
    item_type: string;
    item_id: string;
    quantity?: number;
    promo_code?: string;
    payment_method: 'stripe' | 'dark_matter';
}

// =====================================================
// ANALYTICS
// =====================================================

export interface RevenueAnalytics {
    id: number;
    date: Date;
    total_revenue: number;
    total_purchases: number;
    total_refunds: number;
    unique_purchasers: number;
    new_purchasers: number;
    repeat_purchasers: number;
    avg_purchase_value: number;
    most_popular_item?: string;
    revenue_by_category: Record<string, number>;
    created_at: Date;
    updated_at: Date;
}

export interface UserAnalytics {
    user_id: number;
    total_spent: number;
    total_purchases: number;
    first_purchase_date?: Date;
    last_purchase_date?: Date;
    favorite_category?: string;
    avg_purchase_value: number;
    is_vip: boolean;
    vip_tier?: number;
    preferred_items?: string[];
    recommendations?: string[];
    created_at: Date;
    updated_at: Date;
}

export interface ItemAnalytics {
    id: number;
    item_type: string;
    item_id: string;
    views: number;
    add_to_cart_count: number;
    purchase_count: number;
    total_revenue: number;
    avg_rating?: number;
    rating_count: number;
    last_purchased?: Date;
    trend_score?: number;
    conversion_rate?: number;
    created_at: Date;
    updated_at: Date;
}

// =====================================================
// RECOMMENDATIONS
// =====================================================

export enum RecommendationReason {
    POPULAR = 'popular',
    PERSONALIZED = 'personalized',
    TRENDING = 'trending',
    SIMILAR = 'similar'
}

export interface Recommendation {
    id: number;
    user_id?: number;
    item_type: string;
    item_id: string;
    recommendation_reason: RecommendationReason;
    confidence_score: number;
    created_at: Date;
    expires_at: Date;
}

// =====================================================
// BUNDLES
// =====================================================

export interface BundleItem {
    item_type: string;
    item_id: string;
    quantity: number;
}

export interface Bundle {
    id: number;
    bundle_code: string;
    name: string;
    description?: string;
    bundle_type?: string;
    items: BundleItem[];
    original_total_price: number;
    bundle_price: number;
    savings_percentage: number;
    is_matrix_themed: boolean;
    banner_image_url?: string;
    is_limited: boolean;
    available_from?: Date;
    available_until?: Date;
    stock_quantity?: number;
    sold_quantity: number;
    is_active: boolean;
    created_at: Date;
}

// =====================================================
// PREMIUM SUBSCRIPTIONS
// =====================================================

export enum SubscriptionTier {
    BASIC = 'basic',
    PREMIUM = 'premium',
    MATRIX_ELITE = 'matrix_elite'
}

export enum SubscriptionStatus {
    ACTIVE = 'active',
    PAUSED = 'paused',
    CANCELLED = 'cancelled',
    EXPIRED = 'expired'
}

export interface PremiumSubscription {
    id: number;
    user_id: number;
    subscription_tier: SubscriptionTier;
    features: string[];
    price_monthly: number;
    stripe_subscription_id?: string;
    stripe_customer_id?: string;
    status: SubscriptionStatus;
    started_at: Date;
    current_period_start?: Date;
    current_period_end?: Date;
    cancelled_at?: Date;
    ended_at?: Date;
}

export interface PremiumFeatureUsage {
    id: number;
    user_id: number;
    feature_name: string;
    usage_count: number;
    last_used?: Date;
    created_at: Date;
}

// =====================================================
// SECURITY
// =====================================================

export enum SecurityEventType {
    SUSPICIOUS_PURCHASE = 'suspicious_purchase',
    REFUND_ABUSE = 'refund_abuse',
    MULTIPLE_FAILED_ATTEMPTS = 'multiple_failed_attempts',
    UNUSUAL_ACTIVITY = 'unusual_activity'
}

export enum SecuritySeverity {
    LOW = 'low',
    MEDIUM = 'medium',
    HIGH = 'high',
    CRITICAL = 'critical'
}

export interface SecurityLog {
    id: number;
    user_id?: number;
    event_type: SecurityEventType;
    event_description?: string;
    severity: SecuritySeverity;
    ip_address?: string;
    user_agent?: string;
    metadata?: Record<string, any>;
    action_taken?: string;
    created_at: Date;
}

export interface Refund {
    id: number;
    purchase_id: number;
    user_id: number;
    refund_amount: number;
    refund_reason?: string;
    refund_type: 'full' | 'partial' | 'chargeback';
    stripe_refund_id?: string;
    status: string;
    requested_at: Date;
    processed_at?: Date;
    processed_by?: number;
}

// =====================================================
// MATRIX THEME
// =====================================================

export interface MatrixThemeProgress {
    user_id: number;
    theme_unlocked: boolean;
    unlock_date?: Date;
    matrix_level: number;
    matrix_points: number;
    exclusive_items_owned: number;
    special_effects_enabled?: string[];
    custom_color_scheme?: Record<string, string>;
    achievement_unlocks?: string[];
    created_at: Date;
    updated_at: Date;
}

// =====================================================
// REQUEST/RESPONSE TYPES
// =====================================================

export interface PurchaseCosmeticRequest {
    user_id: number;
    cosmetic_item_id: number;
    payment_method: 'usd' | 'dark_matter';
    promo_code?: string;
}

export interface EquipCosmeticRequest {
    user_id: number;
    cosmetic_item_id: number;
}

export interface ApplyPromotionRequest {
    promo_code: string;
    user_id: number;
    cart_items: Array<{
        item_type: string;
        item_id: string;
        price: number;
    }>;
}

export interface ApplyPromotionResponse {
    is_valid: boolean;
    discount_amount: number;
    final_total: number;
    promotion_details?: Promotion;
}

export interface GetRecommendationsRequest {
    user_id?: number;
    limit?: number;
    category?: string;
}

export interface ShopAnalyticsDashboard {
    today_revenue: number;
    today_purchases: number;
    total_revenue: number;
    total_purchases: number;
    top_items: ItemAnalytics[];
    trending_items: ItemAnalytics[];
    vip_count: number;
    active_promotions: Promotion[];
    flash_sales: FlashSale[];
}

export interface UserShopProfile {
    user_analytics: UserAnalytics;
    owned_cosmetics: UserCosmetic[];
    equipped_cosmetics: UserCosmetic[];
    premium_subscription?: PremiumSubscription;
    matrix_progress: MatrixThemeProgress;
    recommendations: Recommendation[];
    available_promotions: Promotion[];
}
