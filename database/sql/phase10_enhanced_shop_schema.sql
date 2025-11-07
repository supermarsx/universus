-- =====================================================
-- Phase 10: Enhanced Shop & Matrix Theme
-- Database Schema
-- =====================================================
-- 
-- Enhances existing shop system with:
-- - Cosmetic items (ship skins, themes, decorations)
-- - Promotions and limited-time offers
-- - Gift and transfer systems
-- - Shop analytics and revenue tracking
-- - Recommendation engine
-- - Enhanced security and fraud prevention
--
-- =====================================================

-- =====================================================
-- COSMETIC ITEMS SYSTEM
-- =====================================================

-- Cosmetic item categories
CREATE TABLE IF NOT EXISTS shop_cosmetic_categories (
    id SERIAL PRIMARY KEY,
    name VARCHAR(100) NOT NULL UNIQUE,
    description TEXT,
    icon_url VARCHAR(500),
    display_order INT DEFAULT 0,
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT NOW()
);

-- Cosmetic items (skins, themes, decorations)
CREATE TABLE IF NOT EXISTS shop_cosmetic_items (
    id SERIAL PRIMARY KEY,
    category_id INT REFERENCES shop_cosmetic_categories(id),
    item_code VARCHAR(100) NOT NULL UNIQUE,
    name VARCHAR(200) NOT NULL,
    description TEXT,
    item_type VARCHAR(50) NOT NULL, -- 'ship_skin', 'building_skin', 'theme', 'decoration', 'badge', 'avatar'
    target_entity VARCHAR(100), -- What it applies to (e.g., 'battleship', 'metal_mine', 'planet')
    rarity VARCHAR(20) DEFAULT 'common', -- 'common', 'rare', 'epic', 'legendary', 'matrix_exclusive'
    price_usd INT NOT NULL, -- Price in cents
    price_dark_matter INT, -- Alternative price in dark matter
    is_matrix_themed BOOLEAN DEFAULT FALSE,
    preview_image_url VARCHAR(500),
    preview_video_url VARCHAR(500),
    css_class VARCHAR(100), -- CSS class for applying the cosmetic
    effect_data JSONB, -- Visual effect configuration
    is_limited BOOLEAN DEFAULT FALSE,
    is_exclusive BOOLEAN DEFAULT FALSE,
    is_tradeable BOOLEAN DEFAULT TRUE,
    stock_quantity INT, -- NULL = unlimited
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

-- User cosmetic inventory
CREATE TABLE IF NOT EXISTS user_cosmetics (
    id SERIAL PRIMARY KEY,
    user_id INT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    cosmetic_item_id INT NOT NULL REFERENCES shop_cosmetic_items(id),
    quantity INT DEFAULT 1,
    purchased_at TIMESTAMP DEFAULT NOW(),
    is_equipped BOOLEAN DEFAULT FALSE,
    equipped_at TIMESTAMP,
    source VARCHAR(50) DEFAULT 'purchase', -- 'purchase', 'gift', 'promotion', 'achievement'
    UNIQUE(user_id, cosmetic_item_id)
);

-- =====================================================
-- PROMOTIONS & LIMITED OFFERS
-- =====================================================

-- Promotional campaigns
CREATE TABLE IF NOT EXISTS shop_promotions (
    id SERIAL PRIMARY KEY,
    promo_code VARCHAR(50) UNIQUE,
    name VARCHAR(200) NOT NULL,
    description TEXT,
    promotion_type VARCHAR(50) NOT NULL, -- 'discount', 'bundle', 'flash_sale', 'seasonal', 'first_purchase'
    discount_percentage DECIMAL(5,2), -- Percentage discount (0-100)
    discount_amount INT, -- Fixed discount in cents
    applicable_items JSONB, -- Array of item IDs or categories
    min_purchase_amount INT, -- Minimum purchase to apply
    max_uses INT, -- Max total uses (NULL = unlimited)
    max_uses_per_user INT DEFAULT 1,
    uses_count INT DEFAULT 0,
    start_date TIMESTAMP NOT NULL,
    end_date TIMESTAMP NOT NULL,
    is_active BOOLEAN DEFAULT TRUE,
    is_featured BOOLEAN DEFAULT FALSE,
    banner_image_url VARCHAR(500),
    created_at TIMESTAMP DEFAULT NOW()
);

-- Promotion usage tracking
CREATE TABLE IF NOT EXISTS shop_promotion_uses (
    id SERIAL PRIMARY KEY,
    promotion_id INT REFERENCES shop_promotions(id) ON DELETE CASCADE,
    user_id INT REFERENCES users(id) ON DELETE CASCADE,
    purchase_id INT, -- Will reference enhanced purchases table
    discount_applied INT, -- Actual discount in cents
    used_at TIMESTAMP DEFAULT NOW()
);

-- Flash sales and daily deals
CREATE TABLE IF NOT EXISTS shop_flash_sales (
    id SERIAL PRIMARY KEY,
    item_id VARCHAR(100) NOT NULL, -- References shop item
    item_type VARCHAR(50) NOT NULL, -- 'cosmetic', 'resource', 'officer', 'boost'
    original_price INT NOT NULL,
    sale_price INT NOT NULL,
    discount_percentage DECIMAL(5,2),
    stock_quantity INT,
    sold_quantity INT DEFAULT 0,
    start_time TIMESTAMP NOT NULL,
    end_time TIMESTAMP NOT NULL,
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT NOW()
);

-- =====================================================
-- GIFT & TRANSFER SYSTEM
-- =====================================================

-- Gift transactions
CREATE TABLE IF NOT EXISTS shop_gifts (
    id SERIAL PRIMARY KEY,
    sender_user_id INT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    recipient_user_id INT REFERENCES users(id) ON DELETE SET NULL,
    recipient_email VARCHAR(255), -- For gifts to non-users
    item_type VARCHAR(50) NOT NULL, -- 'cosmetic', 'dark_matter', 'officer', 'boost'
    item_id VARCHAR(100) NOT NULL,
    quantity INT DEFAULT 1,
    personal_message TEXT,
    gift_code VARCHAR(100) UNIQUE, -- Code for claiming
    purchase_price INT, -- Price paid in cents
    status VARCHAR(20) DEFAULT 'pending', -- 'pending', 'claimed', 'expired', 'refunded'
    purchased_at TIMESTAMP DEFAULT NOW(),
    claimed_at TIMESTAMP,
    expires_at TIMESTAMP,
    stripe_payment_id VARCHAR(200)
);

-- =====================================================
-- SHOP ANALYTICS & TRACKING
-- =====================================================

-- Enhanced purchase records
CREATE TABLE IF NOT EXISTS shop_purchases_enhanced (
    id SERIAL PRIMARY KEY,
    user_id INT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    item_type VARCHAR(50) NOT NULL,
    item_id VARCHAR(100) NOT NULL,
    quantity INT DEFAULT 1,
    price_usd INT NOT NULL, -- Price in cents
    currency VARCHAR(3) DEFAULT 'USD',
    payment_method VARCHAR(50), -- 'stripe', 'dark_matter', 'gift'
    stripe_payment_intent_id VARCHAR(200),
    stripe_charge_id VARCHAR(200),
    promotion_id INT REFERENCES shop_promotions(id),
    discount_applied INT DEFAULT 0,
    final_price INT NOT NULL,
    status VARCHAR(20) DEFAULT 'pending', -- 'pending', 'completed', 'failed', 'refunded'
    ip_address INET,
    user_agent TEXT,
    device_type VARCHAR(50),
    referrer VARCHAR(500),
    created_at TIMESTAMP DEFAULT NOW(),
    completed_at TIMESTAMP,
    refunded_at TIMESTAMP
);

-- Revenue analytics (aggregated daily)
CREATE TABLE IF NOT EXISTS shop_revenue_analytics (
    id SERIAL PRIMARY KEY,
    date DATE NOT NULL UNIQUE,
    total_revenue INT DEFAULT 0, -- Total in cents
    total_purchases INT DEFAULT 0,
    total_refunds INT DEFAULT 0,
    unique_purchasers INT DEFAULT 0,
    new_purchasers INT DEFAULT 0,
    repeat_purchasers INT DEFAULT 0,
    avg_purchase_value INT DEFAULT 0,
    most_popular_item VARCHAR(100),
    revenue_by_category JSONB, -- { 'cosmetic': 5000, 'boost': 3000, ... }
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

-- User purchase behavior
CREATE TABLE IF NOT EXISTS shop_user_analytics (
    user_id INT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    total_spent INT DEFAULT 0, -- Lifetime spending in cents
    total_purchases INT DEFAULT 0,
    first_purchase_date TIMESTAMP,
    last_purchase_date TIMESTAMP,
    favorite_category VARCHAR(50),
    avg_purchase_value INT DEFAULT 0,
    is_vip BOOLEAN DEFAULT FALSE,
    vip_tier INT, -- 1-5 based on spending
    preferred_items JSONB, -- Array of frequently purchased items
    recommendations JSONB, -- Recommended items based on behavior
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

-- Item popularity tracking
CREATE TABLE IF NOT EXISTS shop_item_analytics (
    id SERIAL PRIMARY KEY,
    item_type VARCHAR(50) NOT NULL,
    item_id VARCHAR(100) NOT NULL,
    views INT DEFAULT 0,
    add_to_cart_count INT DEFAULT 0,
    purchase_count INT DEFAULT 0,
    total_revenue INT DEFAULT 0,
    avg_rating DECIMAL(3,2),
    rating_count INT DEFAULT 0,
    last_purchased TIMESTAMP,
    trend_score DECIMAL(5,2), -- Trending calculation
    conversion_rate DECIMAL(5,2), -- (purchases / views) * 100
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW(),
    UNIQUE(item_type, item_id)
);

-- =====================================================
-- RECOMMENDATION ENGINE
-- =====================================================

-- Item recommendations
CREATE TABLE IF NOT EXISTS shop_recommendations (
    id SERIAL PRIMARY KEY,
    user_id INT REFERENCES users(id) ON DELETE CASCADE,
    item_type VARCHAR(50) NOT NULL,
    item_id VARCHAR(100) NOT NULL,
    recommendation_reason VARCHAR(100), -- 'popular', 'personalized', 'trending', 'similar'
    confidence_score DECIMAL(3,2), -- 0.0 to 1.0
    created_at TIMESTAMP DEFAULT NOW(),
    expires_at TIMESTAMP
);

-- Item bundles and packages
CREATE TABLE IF NOT EXISTS shop_bundles (
    id SERIAL PRIMARY KEY,
    bundle_code VARCHAR(100) NOT NULL UNIQUE,
    name VARCHAR(200) NOT NULL,
    description TEXT,
    bundle_type VARCHAR(50), -- 'starter', 'premium', 'matrix_special', 'seasonal'
    items JSONB NOT NULL, -- Array of {item_type, item_id, quantity}
    original_total_price INT NOT NULL,
    bundle_price INT NOT NULL,
    savings_percentage DECIMAL(5,2),
    is_matrix_themed BOOLEAN DEFAULT FALSE,
    banner_image_url VARCHAR(500),
    is_limited BOOLEAN DEFAULT FALSE,
    available_from TIMESTAMP,
    available_until TIMESTAMP,
    stock_quantity INT,
    sold_quantity INT DEFAULT 0,
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT NOW()
);

-- =====================================================
-- PREMIUM ACCOUNT FEATURES
-- =====================================================

-- Premium subscriptions
CREATE TABLE IF NOT EXISTS shop_premium_subscriptions (
    id SERIAL PRIMARY KEY,
    user_id INT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    subscription_tier VARCHAR(50) NOT NULL, -- 'basic', 'premium', 'matrix_elite'
    features JSONB NOT NULL, -- Array of feature flags
    price_monthly INT NOT NULL,
    stripe_subscription_id VARCHAR(200),
    stripe_customer_id VARCHAR(200),
    status VARCHAR(20) DEFAULT 'active', -- 'active', 'paused', 'cancelled', 'expired'
    started_at TIMESTAMP DEFAULT NOW(),
    current_period_start TIMESTAMP,
    current_period_end TIMESTAMP,
    cancelled_at TIMESTAMP,
    ended_at TIMESTAMP
);

-- Premium feature usage tracking
CREATE TABLE IF NOT EXISTS premium_feature_usage (
    id SERIAL PRIMARY KEY,
    user_id INT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    feature_name VARCHAR(100) NOT NULL,
    usage_count INT DEFAULT 0,
    last_used TIMESTAMP,
    created_at TIMESTAMP DEFAULT NOW()
);

-- =====================================================
-- SECURITY & FRAUD PREVENTION
-- =====================================================

-- Suspicious transaction logging
CREATE TABLE IF NOT EXISTS shop_security_logs (
    id SERIAL PRIMARY KEY,
    user_id INT REFERENCES users(id) ON DELETE SET NULL,
    event_type VARCHAR(50) NOT NULL, -- 'suspicious_purchase', 'refund_abuse', 'multiple_failed_attempts'
    event_description TEXT,
    severity VARCHAR(20) DEFAULT 'low', -- 'low', 'medium', 'high', 'critical'
    ip_address INET,
    user_agent TEXT,
    metadata JSONB,
    action_taken VARCHAR(100), -- 'blocked', 'flagged', 'manual_review'
    created_at TIMESTAMP DEFAULT NOW()
);

-- Refund tracking
CREATE TABLE IF NOT EXISTS shop_refunds (
    id SERIAL PRIMARY KEY,
    purchase_id INT NOT NULL REFERENCES shop_purchases_enhanced(id),
    user_id INT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    refund_amount INT NOT NULL,
    refund_reason VARCHAR(200),
    refund_type VARCHAR(50), -- 'full', 'partial', 'chargeback'
    stripe_refund_id VARCHAR(200),
    status VARCHAR(20) DEFAULT 'pending',
    requested_at TIMESTAMP DEFAULT NOW(),
    processed_at TIMESTAMP,
    processed_by INT REFERENCES users(id)
);

-- =====================================================
-- MATRIX THEME EXCLUSIVE DATA
-- =====================================================

-- Matrix theme unlocks and progression
CREATE TABLE IF NOT EXISTS matrix_theme_progress (
    user_id INT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    theme_unlocked BOOLEAN DEFAULT FALSE,
    unlock_date TIMESTAMP,
    matrix_level INT DEFAULT 1, -- 1-10 progression
    matrix_points INT DEFAULT 0,
    exclusive_items_owned INT DEFAULT 0,
    special_effects_enabled JSONB, -- Array of enabled effects
    custom_color_scheme JSONB, -- Custom matrix colors
    achievement_unlocks JSONB, -- Matrix-specific achievements
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

-- =====================================================
-- INDEXES FOR PERFORMANCE
-- =====================================================

CREATE INDEX idx_cosmetic_items_type ON shop_cosmetic_items(item_type);
CREATE INDEX idx_cosmetic_items_rarity ON shop_cosmetic_items(rarity);
CREATE INDEX idx_cosmetic_items_matrix ON shop_cosmetic_items(is_matrix_themed);
CREATE INDEX idx_cosmetic_items_active ON shop_cosmetic_items(is_active);

CREATE INDEX idx_user_cosmetics_user ON user_cosmetics(user_id);
CREATE INDEX idx_user_cosmetics_equipped ON user_cosmetics(user_id, is_equipped);

CREATE INDEX idx_promotions_active ON shop_promotions(is_active, start_date, end_date);
CREATE INDEX idx_promotions_code ON shop_promotions(promo_code);
CREATE INDEX idx_promotions_featured ON shop_promotions(is_featured, is_active);

CREATE INDEX idx_flash_sales_active ON shop_flash_sales(is_active, start_time, end_time);

CREATE INDEX idx_gifts_recipient ON shop_gifts(recipient_user_id);
CREATE INDEX idx_gifts_code ON shop_gifts(gift_code);
CREATE INDEX idx_gifts_status ON shop_gifts(status);

CREATE INDEX idx_purchases_user ON shop_purchases_enhanced(user_id);
CREATE INDEX idx_purchases_date ON shop_purchases_enhanced(created_at);
CREATE INDEX idx_purchases_status ON shop_purchases_enhanced(status);

CREATE INDEX idx_revenue_analytics_date ON shop_revenue_analytics(date);

CREATE INDEX idx_item_analytics_trending ON shop_item_analytics(trend_score DESC);

CREATE INDEX idx_recommendations_user ON shop_recommendations(user_id, expires_at);

CREATE INDEX idx_bundles_active ON shop_bundles(is_active, available_from, available_until);

CREATE INDEX idx_subscriptions_user ON shop_premium_subscriptions(user_id);
CREATE INDEX idx_subscriptions_status ON shop_premium_subscriptions(status);

CREATE INDEX idx_security_logs_severity ON shop_security_logs(severity, created_at);

-- =====================================================
-- SEED DATA
-- =====================================================

-- Insert cosmetic categories
INSERT INTO shop_cosmetic_categories (name, description, display_order) VALUES
('Ship Skins', 'Unique visual designs for your fleet', 1),
('Building Skins', 'Custom appearances for planetary structures', 2),
('Themes', 'Complete UI theme packages', 3),
('Decorations', 'Planet and base decorations', 4),
('Badges', 'Profile badges and titles', 5),
('Avatars', 'Custom profile avatars', 6)
ON CONFLICT (name) DO NOTHING;

-- Insert sample Matrix-themed cosmetic items
INSERT INTO shop_cosmetic_items (
    category_id, item_code, name, description, item_type, target_entity, 
    rarity, price_usd, is_matrix_themed, preview_image_url, css_class, effect_data
) VALUES
(1, 'matrix_battleship', 'Matrix Digital Battleship', 'Sleek battleship with green digital effects and code streams', 'ship_skin', 'battleship', 'legendary', 1999, TRUE, '/assets/ships/matrix-battleship.png', 'matrix-ship-skin', '{"glow": "green", "particles": "code"}'),
(1, 'matrix_cruiser', 'Matrix Sentinel Cruiser', 'Cruiser with digital rain effect and glitch animations', 'ship_skin', 'cruiser', 'epic', 1499, TRUE, '/assets/ships/matrix-cruiser.png', 'matrix-ship-skin', '{"glow": "green", "particles": "rain"}'),
(3, 'matrix_complete', 'Complete Matrix Theme', 'Full UI transformation with green digital aesthetic', 'theme', 'ui', 'legendary', 2999, TRUE, '/assets/themes/matrix-preview.png', 'matrix-theme', '{"background": "digital_rain", "colors": "green_matrix"}'),
(4, 'matrix_decoration', 'Digital Rain Monument', 'Animated decoration with falling code', 'decoration', 'planet', 'rare', 999, TRUE, '/assets/decorations/digital-rain.png', 'matrix-decoration', '{"animation": "digital_rain"}'),
(5, 'matrix_badge', 'Matrix Awakened Badge', 'Exclusive badge for Matrix theme users', 'badge', 'profile', 'legendary', 499, TRUE, '/assets/badges/matrix-awakened.png', 'matrix-badge', '{"glow": "green"}')
ON CONFLICT (item_code) DO NOTHING;

-- Insert sample bundles
INSERT INTO shop_bundles (
    bundle_code, name, description, bundle_type, items, 
    original_total_price, bundle_price, savings_percentage, is_matrix_themed
) VALUES
(
    'matrix_starter', 
    'Matrix Awakening Pack', 
    'Everything you need to enter the Matrix universe',
    'matrix_special',
    '[
        {"item_type": "cosmetic", "item_id": "matrix_complete", "quantity": 1},
        {"item_type": "cosmetic", "item_id": "matrix_badge", "quantity": 1},
        {"item_type": "dark_matter", "item_id": "dm_medium", "quantity": 1}
    ]',
    4998,
    3499,
    30.0,
    TRUE
),
(
    'matrix_elite', 
    'Matrix Elite Collection', 
    'Complete Matrix experience with all exclusive items',
    'matrix_special',
    '[
        {"item_type": "cosmetic", "item_id": "matrix_complete", "quantity": 1},
        {"item_type": "cosmetic", "item_id": "matrix_battleship", "quantity": 1},
        {"item_type": "cosmetic", "item_id": "matrix_cruiser", "quantity": 1},
        {"item_type": "cosmetic", "item_id": "matrix_decoration", "quantity": 1},
        {"item_type": "cosmetic", "item_id": "matrix_badge", "quantity": 1}
    ]',
    8495,
    5999,
    29.4,
    TRUE
)
ON CONFLICT (bundle_code) DO NOTHING;

-- =====================================================
-- VIEWS FOR ANALYTICS
-- =====================================================

-- Active promotions view
CREATE OR REPLACE VIEW v_active_promotions AS
SELECT 
    p.*,
    (SELECT COUNT(*) FROM shop_promotion_uses WHERE promotion_id = p.id) as current_uses,
    CASE 
        WHEN p.max_uses IS NOT NULL AND (SELECT COUNT(*) FROM shop_promotion_uses WHERE promotion_id = p.id) >= p.max_uses 
        THEN FALSE 
        ELSE TRUE 
    END as is_available
FROM shop_promotions p
WHERE p.is_active = TRUE
    AND NOW() BETWEEN p.start_date AND p.end_date;

-- Top selling items view
CREATE OR REPLACE VIEW v_top_selling_items AS
SELECT 
    sia.item_type,
    sia.item_id,
    sia.purchase_count,
    sia.total_revenue,
    sia.conversion_rate,
    sia.trend_score,
    sia.last_purchased
FROM shop_item_analytics sia
WHERE sia.purchase_count > 0
ORDER BY sia.purchase_count DESC, sia.total_revenue DESC
LIMIT 50;

-- VIP users view
CREATE OR REPLACE VIEW v_vip_users AS
SELECT 
    u.id,
    u.username,
    u.email,
    sua.total_spent,
    sua.total_purchases,
    sua.vip_tier,
    sua.first_purchase_date,
    sua.last_purchase_date
FROM users u
JOIN shop_user_analytics sua ON u.id = sua.user_id
WHERE sua.is_vip = TRUE
ORDER BY sua.total_spent DESC;

-- Matrix theme users view
CREATE OR REPLACE VIEW v_matrix_users AS
SELECT 
    u.id,
    u.username,
    mtp.theme_unlocked,
    mtp.matrix_level,
    mtp.matrix_points,
    mtp.exclusive_items_owned,
    mtp.unlock_date
FROM users u
JOIN matrix_theme_progress mtp ON u.id = mtp.user_id
WHERE mtp.theme_unlocked = TRUE
ORDER BY mtp.matrix_level DESC, mtp.matrix_points DESC;

-- =====================================================
-- FUNCTIONS
-- =====================================================

-- Calculate user VIP tier based on spending
CREATE OR REPLACE FUNCTION calculate_vip_tier(user_id_param INT)
RETURNS INT AS $$
DECLARE
    total_spent INT;
    tier INT;
BEGIN
    SELECT total_spent INTO total_spent 
    FROM shop_user_analytics 
    WHERE user_id = user_id_param;
    
    IF total_spent IS NULL THEN
        RETURN 0;
    END IF;
    
    -- Tier thresholds (in cents)
    IF total_spent >= 50000 THEN tier := 5;      -- $500+
    ELSIF total_spent >= 20000 THEN tier := 4;   -- $200+
    ELSIF total_spent >= 10000 THEN tier := 3;   -- $100+
    ELSIF total_spent >= 5000 THEN tier := 2;    -- $50+
    ELSIF total_spent >= 1000 THEN tier := 1;    -- $10+
    ELSE tier := 0;
    END IF;
    
    RETURN tier;
END;
$$ LANGUAGE plpgsql;

-- Update shop analytics (call after each purchase)
CREATE OR REPLACE FUNCTION update_shop_analytics()
RETURNS TRIGGER AS $$
BEGIN
    -- Update item analytics
    INSERT INTO shop_item_analytics (item_type, item_id, purchase_count, total_revenue, last_purchased, updated_at)
    VALUES (NEW.item_type, NEW.item_id, 1, NEW.final_price, NEW.completed_at, NOW())
    ON CONFLICT (item_type, item_id) 
    DO UPDATE SET 
        purchase_count = shop_item_analytics.purchase_count + 1,
        total_revenue = shop_item_analytics.total_revenue + NEW.final_price,
        last_purchased = NEW.completed_at,
        updated_at = NOW();
    
    -- Update user analytics
    INSERT INTO shop_user_analytics (user_id, total_spent, total_purchases, first_purchase_date, last_purchase_date, updated_at)
    VALUES (NEW.user_id, NEW.final_price, 1, NEW.completed_at, NEW.completed_at, NOW())
    ON CONFLICT (user_id)
    DO UPDATE SET
        total_spent = shop_user_analytics.total_spent + NEW.final_price,
        total_purchases = shop_user_analytics.total_purchases + 1,
        last_purchase_date = NEW.completed_at,
        updated_at = NOW();
    
    -- Update VIP status
    UPDATE shop_user_analytics
    SET 
        is_vip = (total_spent >= 1000), -- $10+ makes VIP
        vip_tier = calculate_vip_tier(NEW.user_id)
    WHERE user_id = NEW.user_id;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to update analytics on purchase completion
CREATE TRIGGER trigger_update_shop_analytics
AFTER UPDATE OF status ON shop_purchases_enhanced
FOR EACH ROW
WHEN (OLD.status != 'completed' AND NEW.status = 'completed')
EXECUTE FUNCTION update_shop_analytics();

-- =====================================================
-- COMMENTS
-- =====================================================

COMMENT ON TABLE shop_cosmetic_items IS 'Cosmetic items including ship skins, themes, decorations with Matrix exclusives';
COMMENT ON TABLE shop_promotions IS 'Promotional campaigns, discounts, and limited-time offers';
COMMENT ON TABLE shop_gifts IS 'Gift transactions allowing users to send items to others';
COMMENT ON TABLE shop_purchases_enhanced IS 'Enhanced purchase tracking with detailed analytics';
COMMENT ON TABLE shop_revenue_analytics IS 'Daily aggregated revenue statistics';
COMMENT ON TABLE shop_user_analytics IS 'Individual user purchase behavior and VIP tracking';
COMMENT ON TABLE shop_bundles IS 'Item bundles and package deals including Matrix special collections';
COMMENT ON TABLE matrix_theme_progress IS 'Matrix theme progression and exclusive content unlocks';
