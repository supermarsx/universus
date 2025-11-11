// Jest setup file to ensure server start is skipped during tests
process.env.SKIP_SERVER_START = 'true';
// Also ensure NODE_ENV is 'test'
process.env.NODE_ENV = process.env.NODE_ENV || 'test';
