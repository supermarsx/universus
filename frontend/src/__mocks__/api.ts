// Manual Jest mock for frontend/src/api.ts

const mockData = {
  '/acs': [],
  '/fleet': [],
  '/shop/catalog': [],
  '/shop/perks': [],
  '/users/me': { id: 1, username: 'test' },
  '/a11y/report': { success: true },
};

const api = {
  get: jest.fn(async (endpoint) => {
    if (endpoint in mockData) return mockData[endpoint];
    return {};
  }),
  post: jest.fn(async (endpoint, body) => {
    return { success: true, endpoint, body };
  }),
  put: jest.fn(async (endpoint, body) => {
    return { success: true, endpoint, body };
  }),
  delete: jest.fn(async (endpoint) => {
    return { success: true, endpoint };
  }),
};

export default api;
