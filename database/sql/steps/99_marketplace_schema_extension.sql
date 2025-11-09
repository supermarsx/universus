-- Marketplace Schema Extension for Fleet Trading and Offer Flexibility
-- Adds support for fleet listings, wanted type/amount, and delivery ETA

ALTER TABLE shard_market_listings
  ADD COLUMN listing_type VARCHAR(20) NOT NULL DEFAULT 'resource',
  ADD COLUMN fleet_type VARCHAR(50),
  ADD COLUMN fleet_quantity BIGINT,
  ADD COLUMN wanted_type VARCHAR(50) NOT NULL DEFAULT 'metal',
  ADD COLUMN wanted_amount BIGINT NOT NULL DEFAULT 0,
  ADD COLUMN delivery_eta TIMESTAMP,
  ADD COLUMN tax_paid BIGINT DEFAULT 0;

-- Allow resource_type, quantity, price_per_unit, total_price to be NULL for fleet listings
ALTER TABLE shard_market_listings
  ALTER COLUMN resource_type DROP NOT NULL,
  ALTER COLUMN quantity DROP NOT NULL,
  ALTER COLUMN price_per_unit DROP NOT NULL,
  ALTER COLUMN total_price DROP NOT NULL;

-- Add index for wanted_type for search
CREATE INDEX IF NOT EXISTS idx_market_wanted_type ON shard_market_listings(wanted_type);
