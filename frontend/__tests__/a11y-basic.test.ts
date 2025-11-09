import { axe, toHaveNoViolations } from 'jest-axe';
expect.extend(toHaveNoViolations);

describe('Accessibility check', () => {
  it('should have no accessibility violations on a simple HTML snippet', async () => {
    const html = `<main><h1>Hello, Universus!</h1><p>This is a test.</p></main>`;
    const results = await axe(html);
    expect(results).toHaveNoViolations();
  });
});
