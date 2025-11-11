import express, { Request, Response } from 'express';
import { authenticateToken } from '../middleware/auth';
import { pool } from '../config/database';
import { AuthRequest } from '../types';

const router = express.Router();
router.use(authenticateToken);

// GET /api/marketplace/listings
router.get('/listings', async (req: AuthRequest, res: Response) => {
  // Query params: type, resource_type, fleet_type, wanted_type, min, max, page, pageSize
  const rawQuery = req.query || {};
  const {
    type,
    resource_type,
    fleet_type,
    wanted_type,
    min,
    max,
  } = rawQuery;
  const page = parseInt((rawQuery.page as string) || '1', 10);
  const pageSize = parseInt((rawQuery.pageSize as string) || '20', 10);
  const filters = ['status = \'active\''];
  const params = [];
  let idx = 1;
  if (type) { filters.push(`listing_type = $${idx++}`); params.push(type); }
  if (resource_type) { filters.push(`resource_type = $${idx++}`); params.push(resource_type); }
  if (fleet_type) { filters.push(`fleet_type = $${idx++}`); params.push(fleet_type); }
  if (wanted_type) { filters.push(`wanted_type = $${idx++}`); params.push(wanted_type); }
  if (min) { filters.push(`wanted_amount >= $${idx++}`); params.push(min); }
  if (max) { filters.push(`wanted_amount <= $${idx++}`); params.push(max); }
  const where = filters.length ? `WHERE ${filters.join(' AND ')}` : '';
  const offset = (page - 1) * pageSize;
  try {
    const listingsResult = await pool.query(
      `SELECT * FROM shard_market_listings ${where} ORDER BY created_at DESC LIMIT $${idx++} OFFSET $${idx++}`,
      [...params, pageSize, offset]
    );
    const countResult = await pool.query(`SELECT COUNT(*) FROM shard_market_listings ${where}`, params);
    res.json({ listings: listingsResult.rows, total: parseInt(countResult.rows[0].count) });
  } catch (err) {
    res.status(500).json({ error: err instanceof Error ? err.message : String(err) });
  }
});


// POST /api/marketplace/listings
router.post('/listings', async (req: AuthRequest, res: Response) => {
  if (!req.user) {
    return res.status(401).json({ error: 'Authentication required' });
  }
  const userId = req.user.id;
  const {
    listing_type = 'resource',
    planet_id,
    resource_type,
    quantity,
    price_per_unit,
    total_price,
    fleet_type,
    fleet_quantity,
    wanted_type = 'metal',
    wanted_amount = 0
  } = req.body;

  if (!planet_id) {
    return res.status(400).json({ error: 'planet_id is required' });
  }

  const client = await pool.connect();
  try {
    await client.query('BEGIN');
    // Validate planet ownership
    const planetResult = await client.query(
      'SELECT * FROM planets WHERE id = $1 AND user_id = $2',
      [planet_id, userId]
    );
    if (planetResult.rows.length === 0) {
      await client.query('ROLLBACK');
      return res.status(403).json({ error: 'You do not own this planet' });
    }
    const planet = planetResult.rows[0];

    let insertFields = {
      user_id: userId,
      planet_id,
      listing_type,
      wanted_type,
      wanted_amount,
      created_at: new Date(),
      status: 'active',
    };

    if (listing_type === 'resource') {
      if (!resource_type || !quantity || !price_per_unit || !total_price) {
        await client.query('ROLLBACK');
        return res.status(400).json({ error: 'Missing resource listing fields' });
      }
      if (planet[resource_type] < quantity) {
        await client.query('ROLLBACK');
        return res.status(400).json({ error: 'Insufficient resources' });
      }
      // Deduct/lock resources
      await client.query(
        `UPDATE planets SET ${resource_type} = ${resource_type} - $1 WHERE id = $2`,
        [quantity, planet_id]
      );
      Object.assign(insertFields, {
        resource_type,
        quantity,
        price_per_unit,
        total_price,
        fleet_type: null,
        fleet_quantity: null,
      });
    } else if (listing_type === 'fleet') {
      if (!fleet_type || !fleet_quantity || !price_per_unit || !total_price) {
        await client.query('ROLLBACK');
        return res.status(400).json({ error: 'Missing fleet listing fields' });
      }
      if (planet[fleet_type] < fleet_quantity) {
        await client.query('ROLLBACK');
        return res.status(400).json({ error: 'Insufficient ships' });
      }
      // Deduct/lock ships
      await client.query(
        `UPDATE planets SET ${fleet_type} = ${fleet_type} - $1 WHERE id = $2`,
        [fleet_quantity, planet_id]
      );
      Object.assign(insertFields, {
        resource_type: null,
        quantity: null,
        price_per_unit,
        total_price,
        fleet_type,
        fleet_quantity,
      });
    } else {
      await client.query('ROLLBACK');
      return res.status(400).json({ error: 'Invalid listing_type' });
    }

    // Insert listing
    const fields = Object.keys(insertFields);
    const values = Object.values(insertFields);
    const placeholders = fields.map((_, i) => `$${i + 1}`);
    const insertSql = `INSERT INTO shard_market_listings (${fields.join(',')}) VALUES (${placeholders.join(',')}) RETURNING *`;
    const listingResult = await client.query(insertSql, values);
    await client.query('COMMIT');
    res.json({ listing: listingResult.rows[0] });
  } catch (err) {
    await client.query('ROLLBACK');
    res.status(500).json({ error: err instanceof Error ? err.message : String(err) });
  } finally {
    client.release();
  }
});


// POST /api/marketplace/listings/:id/accept
router.post('/listings/:id/accept', async (req: AuthRequest, res: Response) => {
  if (!req.user) {
    return res.status(401).json({ error: 'Authentication required' });
  }
  const userId = req.user.id;
  const { buyer_planet_id } = req.body;
  const listingId = req.params.id;
  if (!buyer_planet_id) {
    return res.status(400).json({ error: 'buyer_planet_id is required' });
  }
  const client = await pool.connect();
  try {
    await client.query('BEGIN');
    // Fetch listing
    const listingResult = await client.query('SELECT * FROM shard_market_listings WHERE id = $1 FOR UPDATE', [listingId]);
    if (listingResult.rows.length === 0) {
      await client.query('ROLLBACK');
      return res.status(404).json({ error: 'Listing not found' });
    }
    const listing = listingResult.rows[0];
    if (listing.status !== 'active') {
      await client.query('ROLLBACK');
      return res.status(400).json({ error: 'Listing is not active' });
    }
    if (listing.user_id === userId) {
      await client.query('ROLLBACK');
      return res.status(400).json({ error: 'Cannot accept your own listing' });
    }
    // Validate buyer planet
    const buyerPlanetResult = await client.query('SELECT * FROM planets WHERE id = $1 AND user_id = $2', [buyer_planet_id, userId]);
    if (buyerPlanetResult.rows.length === 0) {
      await client.query('ROLLBACK');
      return res.status(403).json({ error: 'You do not own the buyer planet' });
    }
    const buyerPlanet = buyerPlanetResult.rows[0];
    // Validate buyer has enough wanted_type
    if (buyerPlanet[listing.wanted_type] < listing.wanted_amount) {
      await client.query('ROLLBACK');
      return res.status(400).json({ error: 'Insufficient resources to accept offer' });
    }
    // Transfer wanted resource from buyer to seller
    await client.query(
      `UPDATE planets SET ${listing.wanted_type} = ${listing.wanted_type} - $1 WHERE id = $2`,
      [listing.wanted_amount, buyer_planet_id]
    );
    await client.query(
      `UPDATE planets SET ${listing.wanted_type} = ${listing.wanted_type} + $1 WHERE id = $2`,
      [listing.wanted_amount, listing.planet_id]
    );
    // Deliver asset to buyer
    let delivery_eta = null;
    if (listing.listing_type === 'resource') {
      // Add resource to buyer
      await client.query(
        `UPDATE planets SET ${listing.resource_type} = ${listing.resource_type} + $1 WHERE id = $2`,
        [listing.quantity, buyer_planet_id]
      );
    } else if (listing.listing_type === 'fleet') {
      // Add ships to buyer
      await client.query(
        `UPDATE planets SET ${listing.fleet_type} = ${listing.fleet_type} + $1 WHERE id = $2`,
        [listing.fleet_quantity, buyer_planet_id]
      );
    }
    // Mark listing as completed
    await client.query('UPDATE shard_market_listings SET status = $1, completed_at = NOW(), buyer_id = $2, buyer_planet_id = $3 WHERE id = $4', [
      'completed', userId, buyer_planet_id, listingId
    ]);
    await client.query('COMMIT');
    res.json({ success: true, delivery_eta, transaction: { listing_id: listingId, buyer_id: userId, buyer_planet_id, seller_id: listing.user_id, seller_planet_id: listing.planet_id } });
  } catch (err) {
    await client.query('ROLLBACK');
    res.status(500).json({ error: err instanceof Error ? err.message : String(err) });
  } finally {
    client.release();
  }
});


// DELETE /api/marketplace/listings/:id
router.delete('/listings/:id', async (req: AuthRequest, res: Response) => {
  if (!req.user) {
    return res.status(401).json({ error: 'Authentication required' });
  }
  const userId = req.user.id;
  const listingId = req.params.id;
  const client = await pool.connect();
  try {
    await client.query('BEGIN');
    // Fetch listing
    const listingResult = await client.query('SELECT * FROM shard_market_listings WHERE id = $1 FOR UPDATE', [listingId]);
    if (listingResult.rows.length === 0) {
      await client.query('ROLLBACK');
      return res.status(404).json({ error: 'Listing not found' });
    }
    const listing = listingResult.rows[0];
    if (listing.user_id !== userId) {
      await client.query('ROLLBACK');
      return res.status(403).json({ error: 'You do not own this listing' });
    }
    if (listing.status !== 'active') {
      await client.query('ROLLBACK');
      return res.status(400).json({ error: 'Listing is not active' });
    }
    // Mark as cancelled
    await client.query('UPDATE shard_market_listings SET status = $1, cancelled_at = NOW() WHERE id = $2', ['cancelled', listingId]);
    // Return locked asset to seller
    if (listing.listing_type === 'resource' && listing.resource_type && listing.quantity) {
      await client.query(
        `UPDATE planets SET ${listing.resource_type} = ${listing.resource_type} + $1 WHERE id = $2`,
        [listing.quantity, listing.planet_id]
      );
    } else if (listing.listing_type === 'fleet' && listing.fleet_type && listing.fleet_quantity) {
      await client.query(
        `UPDATE planets SET ${listing.fleet_type} = ${listing.fleet_type} + $1 WHERE id = $2`,
        [listing.fleet_quantity, listing.planet_id]
      );
    }
    await client.query('COMMIT');
    res.json({ success: true });
  } catch (err) {
    await client.query('ROLLBACK');
    res.status(500).json({ error: err instanceof Error ? err.message : String(err) });
  } finally {
    client.release();
  }
});


// GET /api/marketplace/my-listings
router.get('/my-listings', async (req: AuthRequest, res: Response) => {
  if (!req.user) {
    return res.status(401).json({ error: 'Authentication required' });
  }
  const userId = req.user.id;
  try {
    const result = await pool.query('SELECT * FROM shard_market_listings WHERE user_id = $1 ORDER BY created_at DESC', [userId]);
    res.json({ listings: result.rows });
  } catch (err) {
    res.status(500).json({ error: err instanceof Error ? err.message : String(err) });
  }
});


// GET /api/marketplace/my-history
router.get('/my-history', async (req: AuthRequest, res: Response) => {
  if (!req.user) {
    return res.status(401).json({ error: 'Authentication required' });
  }
  const userId = req.user.id;
  try {
    const result = await pool.query(
      `SELECT * FROM shard_market_listings 
       WHERE (user_id = $1 OR buyer_id = $1) AND status = 'completed' 
       ORDER BY completed_at DESC LIMIT 100`,
      [userId]
    );
    res.json({ transactions: result.rows });
  } catch (err) {
    res.status(500).json({ error: err instanceof Error ? err.message : String(err) });
  }
});


// GET /api/marketplace/listings/:id
router.get('/listings/:id', async (req: AuthRequest, res: Response) => {
  const listingId = req.params.id;
  try {
    const result = await pool.query('SELECT * FROM shard_market_listings WHERE id = $1', [listingId]);
    if (result.rows.length === 0) {
      return res.status(404).json({ error: 'Listing not found' });
    }
    res.json({ listing: result.rows[0] });
  } catch (err) {
    res.status(500).json({ error: err instanceof Error ? err.message : String(err) });
  }
});


export default router;
