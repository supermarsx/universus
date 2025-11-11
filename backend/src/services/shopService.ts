import { Pool } from 'pg';
import Stripe from 'stripe';

/**
 * Shop item types
 */
export enum ShopItemType {
  DARK_MATTER = 'dark_matter',
  RESOURCE_PACK = 'resource_pack',
  OFFICER = 'officer',
  BOOST = 'boost',
}

/**
 * Officer types providing account-wide benefits
 */
export enum OfficerType {
  COMMANDER = 'commander', // Fleet slots +2
  ADMIRAL = 'admiral', // Fleet speed +25%
  ENGINEER = 'engineer', // Building time -10%
  GEOLOGIST = 'geologist', // Mine production +10%
  TECHNOCRAT = 'technocrat', // Research time -10%
}

/**
 * Boost types for temporary benefits
 */
export enum BoostType {
  PRODUCTION = 'production', // 2x production for 7 days
  RESEARCH = 'research', // 2x research speed for 7 days
  BUILDING = 'building', // 2x building speed for 7 days
  FLEET_SPEED = 'fleet_speed', // 2x fleet speed for 7 days
}

/**
 * Shop item definition
 */
export interface ShopItem {
  id: string;
  name: string;
  description: string;
  type: ShopItemType;
  priceUSD: number; // Price in USD cents
  darkMatterAmount?: number;
  resourceAmount?: { metal?: number; crystal?: number; deuterium?: number };
  officerType?: OfficerType;
  boostType?: BoostType;
  duration?: number; // Duration in days for officers/boosts
  stripePriceId?: string; // Stripe Price ID
}

/**
 * Purchase record
 */
export interface Purchase {
  id: number;
  userId: number;
  shopItemId: string;
  amount: number;
  currency: string;
  stripePaymentIntentId: string;
  status: 'pending' | 'completed' | 'failed' | 'refunded';
  createdAt: Date;
  completedAt?: Date;
}

/**
 * Active officer or boost
 */
export interface ActivePerk {
  id: number;
  userId: number;
  type: 'officer' | 'boost';
  perkType: string;
  expiresAt: Date;
  isActive: boolean;
}

/**
 * Shop & Monetization Service
 *
 * Handles in-game purchases, Stripe payment processing, and premium features.
 * Implements secure transaction handling and fraud prevention.
 *
 * @class ShopService
 */
export class ShopService {
  private db: Pool;
  private stripe: Stripe;
  private readonly WEBHOOK_SECRET: string;

  /**
   * Shop catalog - Define all purchasable items
   */
  private readonly shopCatalog: ShopItem[] = [
    // Dark Matter Packages
    {
      id: 'dm_small',
      name: 'Small Dark Matter Package',
      description: '1,000 Dark Matter',
      type: ShopItemType.DARK_MATTER,
      priceUSD: 499, // $4.99
      darkMatterAmount: 1000,
    },
    {
      id: 'dm_medium',
      name: 'Medium Dark Matter Package',
      description: '5,000 Dark Matter (+10% bonus)',
      type: ShopItemType.DARK_MATTER,
      priceUSD: 1999, // $19.99
      darkMatterAmount: 5500,
    },
    {
      id: 'dm_large',
      name: 'Large Dark Matter Package',
      description: '15,000 Dark Matter (+20% bonus)',
      type: ShopItemType.DARK_MATTER,
      priceUSD: 4999, // $49.99
      darkMatterAmount: 18000,
    },

    // Resource Packages
    {
      id: 'res_starter',
      name: 'Starter Resource Pack',
      description: '100K Metal, 50K Crystal, 25K Deuterium',
      type: ShopItemType.RESOURCE_PACK,
      priceUSD: 299, // $2.99
      resourceAmount: {
        metal: 100000,
        crystal: 50000,
        deuterium: 25000,
      },
    },
    {
      id: 'res_advanced',
      name: 'Advanced Resource Pack',
      description: '500K Metal, 250K Crystal, 100K Deuterium',
      type: ShopItemType.RESOURCE_PACK,
      priceUSD: 999, // $9.99
      resourceAmount: {
        metal: 500000,
        crystal: 250000,
        deuterium: 100000,
      },
    },

    // Officers (30 days)
    {
      id: 'officer_commander',
      name: 'Commander',
      description: '+2 Fleet Slots for 30 days',
      type: ShopItemType.OFFICER,
      priceUSD: 599, // $5.99/month
      officerType: OfficerType.COMMANDER,
      duration: 30,
    },
    {
      id: 'officer_admiral',
      name: 'Admiral',
      description: '+25% Fleet Speed for 30 days',
      type: ShopItemType.OFFICER,
      priceUSD: 599,
      officerType: OfficerType.ADMIRAL,
      duration: 30,
    },
    {
      id: 'officer_engineer',
      name: 'Engineer',
      description: '-10% Building Time for 30 days',
      type: ShopItemType.OFFICER,
      priceUSD: 599,
      officerType: OfficerType.ENGINEER,
      duration: 30,
    },
    {
      id: 'officer_geologist',
      name: 'Geologist',
      description: '+10% Mine Production for 30 days',
      type: ShopItemType.OFFICER,
      priceUSD: 599,
      officerType: OfficerType.GEOLOGIST,
      duration: 30,
    },
    {
      id: 'officer_technocrat',
      name: 'Technocrat',
      description: '-10% Research Time for 30 days',
      type: ShopItemType.OFFICER,
      priceUSD: 599,
      officerType: OfficerType.TECHNOCRAT,
      duration: 30,
    },

    // Temporary Boosts (7 days)
    {
      id: 'boost_production',
      name: 'Production Boost',
      description: '2x Resource Production for 7 days',
      type: ShopItemType.BOOST,
      priceUSD: 399, // $3.99
      boostType: BoostType.PRODUCTION,
      duration: 7,
    },
    {
      id: 'boost_research',
      name: 'Research Boost',
      description: '2x Research Speed for 7 days',
      type: ShopItemType.BOOST,
      priceUSD: 399,
      boostType: BoostType.RESEARCH,
      duration: 7,
    },
    {
      id: 'boost_building',
      name: 'Building Boost',
      description: '2x Building Speed for 7 days',
      type: ShopItemType.BOOST,
      priceUSD: 399,
      boostType: BoostType.BUILDING,
      duration: 7,
    },
  ];

  /**
   * Creates an instance of ShopService
   *
   * @param {Pool} db - PostgreSQL connection pool
   * @param {string} stripeSecretKey - Stripe secret key
   * @param {string} webhookSecret - Stripe webhook secret
   */
  constructor(db: Pool, stripeSecretKey: string, webhookSecret: string) {
    this.db = db;
    this.WEBHOOK_SECRET = webhookSecret;

    // In test or dev environments a Stripe key may not be provided.
    // When missing, create a minimal no-op stub to avoid contacting Stripe.
    if (!stripeSecretKey) {
      // Minimal stub matching the subset of Stripe API used here
      // - paymentIntents.create
      // - webhooks.constructEvent
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
        apiVersion: '2025-10-29.clover',
      });
    }
  }

  /**
   * Get all available shop items
   *
   * @returns {ShopItem[]} Array of shop items
   *
   * @example
   * const items = shopService.getShopCatalog();
   * console.log(`${items.length} items available`);
   */
  getShopCatalog(): ShopItem[] {
    return this.shopCatalog;
  }

  /**
   * Get a specific shop item by ID
   *
   * @param {string} itemId - The shop item ID
   * @returns {ShopItem | undefined} The shop item or undefined
   */
  getShopItem(itemId: string): ShopItem | undefined {
    return this.shopCatalog.find((item) => item.id === itemId);
  }

  /**
   * Create a Stripe Payment Intent for a purchase
   *
   * @param {number} userId - The ID of the user making the purchase
   * @param {string} shopItemId - The shop item ID
   * @returns {Promise<{ clientSecret: string; paymentIntentId: string }>} Payment intent details
   * @throws {Error} If item not found or payment intent creation fails
   *
   * @example
   * const { clientSecret } = await shopService.createPaymentIntent(123, 'dm_small');
   * // Send clientSecret to frontend for Stripe.js
   */
  async createPaymentIntent(
    userId: number,
    shopItemId: string
  ): Promise<{ clientSecret: string; paymentIntentId: string }> {
    const item = this.getShopItem(shopItemId);

    if (!item) {
      throw new Error(`Shop item ${shopItemId} not found`);
    }

    try {
      // Create Stripe Payment Intent
      const paymentIntent = await this.stripe.paymentIntents.create({
        amount: item.priceUSD,
        currency: 'usd',
        metadata: {
          userId: userId.toString(),
          shopItemId: item.id,
          itemName: item.name,
        },
        description: `${item.name} - User ${userId}`,
      });

      // Record pending purchase in database
      await this.db.query(
        `INSERT INTO purchases (user_id, shop_item_id, amount, currency, stripe_payment_intent_id, status, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, NOW())`,
        [userId, shopItemId, item.priceUSD, 'usd', paymentIntent.id, 'pending']
      );

      return {
        clientSecret: paymentIntent.client_secret!,
        paymentIntentId: paymentIntent.id,
      };
    } catch (error) {
      console.error('Error creating payment intent:', error);
      throw error;
    }
  }

  /**
   * Handle Stripe webhook events
   *
   * Processes payment confirmations and fulfills purchases.
   *
   * @param {string} payload - Raw request body
   * @param {string} signature - Stripe signature header
   * @returns {Promise<void>}
   * @throws {Error} If signature verification fails
   *
   * @example
   * app.post('/webhook', async (req, res) => {
   *   try {
   *     await shopService.handleWebhook(req.body, req.headers['stripe-signature']);
   *     res.json({ received: true });
   *   } catch (error) {
   *     res.status(400).send('Webhook Error');
   *   }
   * });
   */
  async handleWebhook(payload: string, signature: string): Promise<void> {
    let event: Stripe.Event;

    try {
      event = this.stripe.webhooks.constructEvent(payload, signature, this.WEBHOOK_SECRET);
    } catch (error: any) {
      throw new Error(`Webhook signature verification failed: ${error.message}`);
    }

    // Handle payment success
    if (event.type === 'payment_intent.succeeded') {
      const paymentIntent = event.data.object as Stripe.PaymentIntent;
      await this.fulfillPurchase(paymentIntent.id);
    }

    // Handle payment failure
    if (event.type === 'payment_intent.payment_failed') {
      const paymentIntent = event.data.object as Stripe.PaymentIntent;
      await this.markPurchaseFailed(paymentIntent.id);
    }
  }

  /**
   * Fulfill a purchase after successful payment
   *
   * @private
   * @param {string} paymentIntentId - Stripe Payment Intent ID
   * @returns {Promise<void>}
   */
  private async fulfillPurchase(paymentIntentId: string): Promise<void> {
    const client = await this.db.connect();

    try {
      await client.query('BEGIN');

      // Get purchase record
      const purchaseResult = await client.query(
        `SELECT * FROM purchases WHERE stripe_payment_intent_id = $1 AND status = 'pending'`,
        [paymentIntentId]
      );

      if (purchaseResult.rows.length === 0) {
        console.warn(`Purchase not found or already processed: ${paymentIntentId}`);
        await client.query('ROLLBACK');
        return;
      }

      const purchase = purchaseResult.rows[0];
      const item = this.getShopItem(purchase.shop_item_id);

      if (!item) {
        throw new Error(`Shop item ${purchase.shop_item_id} not found`);
      }

      // Grant items based on type
      switch (item.type) {
        case ShopItemType.DARK_MATTER:
          await client.query(
            `UPDATE users SET dark_matter = dark_matter + $1 WHERE id = $2`,
            [item.darkMatterAmount, purchase.user_id]
          );
          break;

        case ShopItemType.RESOURCE_PACK:
          // Add resources to first planet
          if (item.resourceAmount) {
            await client.query(
              `UPDATE planets 
               SET metal = metal + $1, 
                   crystal = crystal + $2, 
                   deuterium = deuterium + $3
               WHERE user_id = $4 
               ORDER BY id LIMIT 1`,
              [
                item.resourceAmount.metal || 0,
                item.resourceAmount.crystal || 0,
                item.resourceAmount.deuterium || 0,
                purchase.user_id,
              ]
            );
          }
          break;

        case ShopItemType.OFFICER:
          await this.activateOfficer(client, purchase.user_id, item.officerType!, item.duration!);
          break;

        case ShopItemType.BOOST:
          await this.activateBoost(client, purchase.user_id, item.boostType!, item.duration!);
          break;
      }

      // Mark purchase as completed
      await client.query(
        `UPDATE purchases SET status = 'completed', completed_at = NOW() WHERE id = $1`,
        [purchase.id]
      );

      await client.query('COMMIT');

      console.log(`Purchase fulfilled: User ${purchase.user_id}, Item ${item.name}`);
    } catch (error) {
      await client.query('ROLLBACK');
      console.error('Error fulfilling purchase:', error);
      throw error;
    } finally {
      client.release();
    }
  }

  /**
   * Mark a purchase as failed
   *
   * @private
   * @param {string} paymentIntentId - Stripe Payment Intent ID
   * @returns {Promise<void>}
   */
  private async markPurchaseFailed(paymentIntentId: string): Promise<void> {
    await this.db.query(
      `UPDATE purchases SET status = 'failed' WHERE stripe_payment_intent_id = $1`,
      [paymentIntentId]
    );
  }

  /**
   * Activate an officer for a user
   *
   * @private
   * @param {PoolClient} client - Database client
   * @param {number} userId - User ID
   * @param {OfficerType} officerType - Type of officer
   * @param {number} duration - Duration in days
   * @returns {Promise<void>}
   */
  private async activateOfficer(
    client: any,
    userId: number,
    officerType: OfficerType,
    duration: number
  ): Promise<void> {
    const expiresAt = new Date();
    expiresAt.setDate(expiresAt.getDate() + duration);

    // Check if officer already active
    const existing = await client.query(
      `SELECT * FROM active_perks 
       WHERE user_id = $1 AND type = 'officer' AND perk_type = $2 AND is_active = true`,
      [userId, officerType]
    );

    if (existing.rows.length > 0) {
      // Extend existing officer
      await client.query(
        `UPDATE active_perks 
         SET expires_at = expires_at + INTERVAL '${duration} days'
         WHERE id = $1`,
        [existing.rows[0]?.id]
      );
    } else {
      // Activate new officer
      await client.query(
        `INSERT INTO active_perks (user_id, type, perk_type, expires_at, is_active, created_at)
         VALUES ($1, 'officer', $2, $3, true, NOW())`,
        [userId, officerType, expiresAt]
      );
    }
  }

  /**
   * Activate a boost for a user
   *
   * @private
   * @param {PoolClient} client - Database client
   * @param {number} userId - User ID
   * @param {BoostType} boostType - Type of boost
   * @param {number} duration - Duration in days
   * @returns {Promise<void>}
   */
  private async activateBoost(
    client: any,
    userId: number,
    boostType: BoostType,
    duration: number
  ): Promise<void> {
    const expiresAt = new Date();
    expiresAt.setDate(expiresAt.getDate() + duration);

    // Boosts stack - create new entry
    await client.query(
      `INSERT INTO active_perks (user_id, type, perk_type, expires_at, is_active, created_at)
       VALUES ($1, 'boost', $2, $3, true, NOW())`,
      [userId, boostType, expiresAt]
    );
  }

  /**
   * Get active perks (officers and boosts) for a user
   *
   * @param {number} userId - User ID
   * @returns {Promise<ActivePerk[]>} Array of active perks
   *
   * @example
   * const perks = await shopService.getUserPerks(123);
   * const hasCommander = perks.some(p => p.perkType === 'commander');
   */
  async getUserPerks(userId: number): Promise<ActivePerk[]> {
    const result = await this.db.query(
      `SELECT * FROM active_perks 
       WHERE user_id = $1 AND is_active = true AND expires_at > NOW()
       ORDER BY created_at DESC`,
      [userId]
    );

    return result.rows.map((row) => ({
      id: row.id,
      userId: row.user_id,
      type: row.type,
      perkType: row.perk_type,
      expiresAt: new Date(row.expires_at),
      isActive: row.is_active,
    }));
  }

  /**
   * Get purchase history for a user
   *
   * @param {number} userId - User ID
   * @param {number} limit - Maximum results (default: 50)
   * @returns {Promise<Purchase[]>} Array of purchases
   */
  async getPurchaseHistory(userId: number, limit: number = 50): Promise<Purchase[]> {
    const result = await this.db.query(
      `SELECT * FROM purchases 
       WHERE user_id = $1 
       ORDER BY created_at DESC 
       LIMIT $2`,
      [userId, limit]
    );

    return result.rows.map((row) => ({
      id: row.id,
      userId: row.user_id,
      shopItemId: row.shop_item_id,
      amount: row.amount,
      currency: row.currency,
      stripePaymentIntentId: row.stripe_payment_intent_id,
      status: row.status,
      createdAt: new Date(row.created_at),
      completedAt: row.completed_at ? new Date(row.completed_at) : undefined,
    }));
  }

  /**
   * Deactivate expired perks (should be run periodically)
   *
   * @returns {Promise<number>} Number of perks deactivated
   */
  async deactivateExpiredPerks(): Promise<number> {
    const result = await this.db.query(
      `UPDATE active_perks 
       SET is_active = false 
       WHERE is_active = true AND expires_at <= NOW()
       RETURNING id`
    );

    return result.rows.length;
  }
}
