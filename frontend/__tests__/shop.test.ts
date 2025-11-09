import 'jest';

jest.mock('../src/api');

import api from '../src/api';
import Shop from '../src/shop';

// Rely on Jest's jsdom environment instead of importing JSDOM
declare const global: any;

// Ensure runtime code using global.api receives the mocked api
beforeEach(() => {
  global.api = api;
  // Use jsdom globals provided by Jest
  document.body.innerHTML = `
    <div id="shopCatalog"></div>
    <div id="perksList"></div>
    <div id="purchaseHistory"></div>
  `;
  (api.get as jest.Mock).mockClear();
  (api.post as jest.Mock).mockClear();
});

test('loads catalog and renders empty state', async () => {
  (api.get as jest.Mock).mockResolvedValueOnce([]);
  const shop = new Shop();
  await shop.loadCatalog();
  expect(api.get).toHaveBeenCalledWith('/shop/catalog');
  expect(document.getElementById('shopCatalog').innerHTML).toMatch(/No items available/);
});

test('creates payment intent', async () => {
  (api.get as jest.Mock).mockResolvedValueOnce([]);
  (api.post as jest.Mock).mockResolvedValueOnce({ clientSecret: 'sec_123' });
  const shop = new Shop();
  await shop.createPaymentIntent({ itemId: 1 });
  expect(api.post).toHaveBeenCalledWith('/shop/create-payment-intent', expect.any(Object));
});
