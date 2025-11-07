-- Migration: Add shop and payment tables

-- Purchases table - tracks all transactions
CREATE TABLE IF NOT EXISTS purchases (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    shop_item_id VARCHAR(50) NOT NULL,
    amount INTEGER NOT NULL, -- Amount in cents
    currency VARCHAR(3) DEFAULT 'usd',
    stripe_payment_intent_id VARCHAR(255) UNIQUE NOT NULL,
    status VARCHAR(20) NOT NULL CHECK (status IN ('pending', 'completed', 'failed', 'refunded')),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    completed_at TIMESTAMP,
    CONSTRAINT purchases_amount_positive CHECK (amount > 0)
);

-- Active perks table - tracks active officers and boosts
CREATE TABLE IF NOT EXISTS active_perks (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    type VARCHAR(20) NOT NULL CHECK (type IN ('officer', 'boost')),
    perk_type VARCHAR(50) NOT NULL,
    expires_at TIMESTAMP NOT NULL,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_purchases_user_id ON purchases(user_id);
CREATE INDEX IF NOT EXISTS idx_purchases_status ON purchases(status);
CREATE INDEX IF NOT EXISTS idx_purchases_stripe_payment_intent ON purchases(stripe_payment_intent_id);
CREATE INDEX IF NOT EXISTS idx_purchases_created_at ON purchases(created_at DESC);

CREATE INDEX IF NOT EXISTS idx_active_perks_user_id ON active_perks(user_id);
CREATE INDEX IF NOT EXISTS idx_active_perks_active ON active_perks(is_active) WHERE is_active = true;
CREATE INDEX IF NOT EXISTS idx_active_perks_expires ON active_perks(expires_at) WHERE is_active = true;
CREATE INDEX IF NOT EXISTS idx_active_perks_user_active ON active_perks(user_id, is_active) WHERE is_active = true;
