# Stripe Payment Integration Guide

**Project:** Universus - Space Empire Game  
**Date:** 2025-11-06  
**Status:** Configuration Required

---

## Current Status

### Implementation Complete ✅
- ✅ Stripe SDK integrated in backend (`shopService.ts`)
- ✅ Payment processing logic implemented
- ✅ Shop catalog with 13+ purchasable items
- ✅ Purchase history tracking
- ✅ Officer and boost management
- ✅ Frontend shop UI with Stripe.js
- ✅ Webhook endpoint for payment events
- ✅ Secure transaction handling

### Configuration Required ⚠️
- ⚠️ Real Stripe API keys not configured
- ⚠️ End-to-end payment testing pending
- ⚠️ Webhook secret not set

---

## Stripe API Keys Setup

### Step 1: Create Stripe Account

1. Go to https://stripe.com
2. Sign up for a free account
3. Complete account verification

### Step 2: Get API Keys

1. Log in to Stripe Dashboard
2. Navigate to **Developers → API keys**
3. You'll see two types of keys:

#### Test Mode Keys (for development)
- **Publishable key**: `pk_test_...`
- **Secret key**: `sk_test_...`

#### Live Mode Keys (for production)
- **Publishable key**: `pk_live_...`
- **Secret key**: `sk_live_...`

**For testing, use Test Mode keys**

### Step 3: Configure Environment Variables

Edit `/workspace/ogame-rpg/backend/.env`:

```bash
# Replace with your actual Stripe keys
STRIPE_SECRET_KEY=sk_test_YOUR_SECRET_KEY_HERE
STRIPE_PUBLISHABLE_KEY=pk_test_YOUR_PUBLISHABLE_KEY_HERE
STRIPE_WEBHOOK_SECRET=whsec_YOUR_WEBHOOK_SECRET_HERE  # Optional for local testing
```

### Step 4: Update Frontend Configuration

Edit `/workspace/ogame-rpg/frontend/js/shop.js`:

Find the line with `pk_test_dummy_key_for_testing` and replace with your publishable key:

```javascript
// Initialize Stripe
const stripe = Stripe('pk_test_YOUR_PUBLISHABLE_KEY_HERE');
```

---

## Testing Payment Flow

### Test Credit Cards

Stripe provides test card numbers for different scenarios:

| Card Number | Scenario |
|-------------|----------|
| 4242 4242 4242 4242 | Successful payment |
| 4000 0000 0000 0002 | Card declined |
| 4000 0000 0000 9995 | Insufficient funds |
| 4000 0025 0000 3155 | Requires authentication (3D Secure) |

**Additional test data:**
- Expiry: Any future date (e.g., 12/25)
- CVC: Any 3 digits (e.g., 123)
- ZIP: Any 5 digits (e.g., 12345)

### End-to-End Payment Test

1. **Start the application:**
   ```bash
   cd /workspace/ogame-rpg/backend
   npm start
   ```

2. **Open the shop:**
   - Navigate to http://localhost:3000/shop.html
   - Log in with test account

3. **Make a test purchase:**
   - Click on any item (e.g., "Small Dark Matter Package")
   - Click "Purchase" button
   - Fill in test card: 4242 4242 4242 4242
   - Complete the purchase

4. **Verify in Stripe Dashboard:**
   - Go to Stripe Dashboard → Payments
   - You should see the test payment listed

5. **Verify in Application:**
   - Check purchase history in shop page
   - Check dark matter balance increased
   - Check database: `SELECT * FROM purchases;`

---

## Shop Catalog

### Dark Matter Packages

| Item | Amount | Price |
|------|--------|-------|
| Small Package | 1,000 DM | $4.99 |
| Medium Package | 2,500 DM | $9.99 |
| Large Package | 6,000 DM | $19.99 |
| Mega Package | 15,000 DM | $49.99 |

### Resource Packs

| Item | Resources | Price |
|------|-----------|-------|
| Starter Pack | 50k Metal, 25k Crystal, 10k Deuterium | $2.99 |
| Advanced Pack | 250k Metal, 125k Crystal, 50k Deuterium | $9.99 |
| Premium Pack | 1M Metal, 500k Crystal, 200k Deuterium | $29.99 |

### Officers (30 days)

| Officer | Benefit | Price |
|---------|---------|-------|
| Commander | +2 Fleet Slots | $9.99 |
| Admiral | +25% Fleet Speed | $9.99 |
| Engineer | -10% Building Time | $9.99 |
| Geologist | +10% Mine Production | $9.99 |
| Technocrat | -10% Research Time | $9.99 |

### Boosts (7 days)

| Boost | Effect | Price |
|-------|--------|-------|
| Production Boost | 2x Resource Production | $4.99 |
| Research Boost | 2x Research Speed | $4.99 |
| Building Boost | 2x Building Speed | $4.99 |
| Fleet Speed Boost | 2x Fleet Speed | $4.99 |

---

## Webhook Configuration (Production)

### Why Webhooks?

Webhooks ensure payment confirmations are processed even if the user closes the browser. Required for production.

### Setup Steps

1. **In Stripe Dashboard:**
   - Go to **Developers → Webhooks**
   - Click **Add endpoint**
   - Endpoint URL: `https://yourdomain.com/api/shop/webhook`
   - Select events: `payment_intent.succeeded`, `payment_intent.payment_failed`

2. **Get Webhook Secret:**
   - After creating endpoint, copy the **Signing secret**
   - Add to `.env`: `STRIPE_WEBHOOK_SECRET=whsec_...`

3. **Verify:**
   - Stripe will send test webhook
   - Check application logs for webhook processing

---

## Security Best Practices

### ✅ Already Implemented

1. **Server-side validation** - All payments processed server-side
2. **Amount verification** - Server validates payment amounts
3. **Idempotency** - Prevents duplicate charges
4. **Secure storage** - Stripe tokens never stored
5. **Error handling** - Graceful failure handling

### 🔒 Additional Recommendations

1. **HTTPS only** - Use SSL/TLS in production
2. **Rate limiting** - Prevent abuse (already configured)
3. **Logging** - Monitor all transactions
4. **Fraud detection** - Use Stripe Radar
5. **PCI compliance** - Stripe handles card data

---

## Testing Checklist

### Before Production Launch

- [ ] Stripe account verified
- [ ] Real API keys configured
- [ ] Test purchases completed successfully
- [ ] Webhook endpoint configured and tested
- [ ] Dark matter correctly credited
- [ ] Resource packs correctly applied
- [ ] Officers activate properly
- [ ] Boosts apply and expire correctly
- [ ] Purchase history displays correctly
- [ ] Refund process tested
- [ ] Error scenarios handled gracefully
- [ ] SSL certificate installed
- [ ] Payment page uses HTTPS
- [ ] Stripe Dashboard monitoring set up

### Test Scenarios

1. **Successful Purchase**
   - Item appears in purchase history
   - Resources/DM credited immediately
   - Email confirmation sent (if configured)

2. **Failed Payment**
   - User sees error message
   - No resources credited
   - Can retry payment

3. **Network Interruption**
   - Webhook handles completion
   - No duplicate charges
   - Resources eventually credited

4. **Refund Request**
   - Admin can process refund
   - Resources/DM deducted if applicable
   - Status updated in database

---

## Monitoring and Analytics

### Key Metrics to Track

1. **Revenue Metrics:**
   - Total revenue
   - Revenue by item type
   - Average transaction value
   - Conversion rate

2. **User Metrics:**
   - Paying vs. free users
   - Repeat purchase rate
   - Most popular items
   - Purchase frequency

3. **Technical Metrics:**
   - Payment success rate
   - Failed payment reasons
   - Webhook delivery success
   - API response times

### Query Examples

```sql
-- Total revenue
SELECT SUM(amount) / 100.0 AS total_revenue_usd
FROM purchases
WHERE status = 'completed';

-- Revenue by item
SELECT shop_item_id, COUNT(*) as sales, SUM(amount) / 100.0 as revenue_usd
FROM purchases
WHERE status = 'completed'
GROUP BY shop_item_id
ORDER BY revenue_usd DESC;

-- Paying users
SELECT COUNT(DISTINCT user_id) as paying_users
FROM purchases
WHERE status = 'completed';

-- Recent failures
SELECT * FROM purchases
WHERE status = 'failed'
ORDER BY created_at DESC
LIMIT 10;
```

---

## Troubleshooting

### Payment Not Completing

**Symptoms:** Payment appears to succeed but resources not credited

**Solutions:**
1. Check application logs for errors
2. Verify webhook is processing correctly
3. Check database `purchases` table for status
4. Look for Stripe errors in dashboard

### Stripe API Errors

**Common Errors:**

1. **Invalid API Key**
   - Check `.env` file has correct keys
   - Verify no extra spaces
   - Ensure using correct mode (test/live)

2. **Amount Too Small**
   - Stripe requires minimum amounts ($0.50 USD)
   - Check item prices in catalog

3. **Card Declined**
   - Normal for test cards simulating failures
   - Check Stripe Dashboard for decline reason

### Webhook Not Receiving Events

**Solutions:**
1. Verify endpoint URL is correct
2. Check firewall allows Stripe IPs
3. Use Stripe CLI for local testing:
   ```bash
   stripe listen --forward-to localhost:3000/api/shop/webhook
   ```

---

## Development vs. Production

### Development (Current)
- ✅ Test mode API keys
- ✅ Test credit cards
- ✅ No real money processed
- ✅ Full functionality testing
- ⚠️ No webhook verification

### Production (Before Launch)
- [ ] Live mode API keys
- [ ] Real credit cards
- [ ] Real money processing
- [ ] Webhook verification enabled
- [ ] HTTPS required
- [ ] SSL certificate installed
- [ ] Stripe account fully verified
- [ ] Bank account connected for payouts

---

## API Reference

### Create Payment Intent

**Endpoint:** `POST /api/shop/purchase`

**Request:**
```json
{
  "shopItemId": "dm_small",
  "userId": 1
}
```

**Response:**
```json
{
  "clientSecret": "pi_xxx_secret_xxx",
  "purchaseId": 123
}
```

### Get Purchase History

**Endpoint:** `GET /api/shop/purchases/:userId`

**Response:**
```json
{
  "purchases": [
    {
      "id": 123,
      "shopItemId": "dm_small",
      "amount": 499,
      "status": "completed",
      "createdAt": "2025-11-06T07:46:17Z"
    }
  ]
}
```

### Get Active Perks

**Endpoint:** `GET /api/shop/perks/:userId`

**Response:**
```json
{
  "officers": [
    {
      "type": "commander",
      "expiresAt": "2025-12-06T07:46:17Z"
    }
  ],
  "boosts": []
}
```

---

## Support Resources

### Stripe Documentation
- Dashboard: https://dashboard.stripe.com
- API Docs: https://stripe.com/docs/api
- Testing Guide: https://stripe.com/docs/testing
- Webhooks: https://stripe.com/docs/webhooks

### Universus Implementation
- Shop Service: `/workspace/ogame-rpg/backend/src/services/shopService.ts`
- Shop Routes: `/workspace/ogame-rpg/backend/src/routes/shop.ts`
- Shop Frontend: `/workspace/ogame-rpg/frontend/shop.html`
- Shop Script: `/workspace/ogame-rpg/frontend/js/shop.js`

---

## Conclusion

The Stripe payment system is **fully implemented** and ready for testing once API keys are configured. Follow the steps above to:

1. Get Stripe API keys (test mode)
2. Configure `.env` file
3. Update frontend with publishable key
4. Test with provided test cards
5. Verify payments in Stripe Dashboard
6. For production: switch to live mode keys

**Status:** Implementation Complete, Configuration Pending ✅

---

**Last Updated:** 2025-11-06  
**Version:** 1.0.0  
**Prepared by:** MiniMax Agent
