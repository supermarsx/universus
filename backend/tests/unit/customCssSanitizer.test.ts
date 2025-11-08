import CustomCssSanitizer from '../../src/utils/customCssSanitizer';

describe('CustomCssSanitizer', () => {
  it('scopes selectors and preserves safe declarations', () => {
    const input = `
      .resource-bar {
        color: #fff;
        background-color: rgba(0,0,0,0.8);
      }
    `;

    const sanitized = CustomCssSanitizer.sanitize(input, 2000);
    expect(sanitized).toContain('body.user-theme-scope .resource-bar');
    expect(sanitized).toContain('color: #fff');
    expect(sanitized).toContain('background-color: rgba(0,0,0,0.8)');
  });

  it('rejects disallowed CSS properties', () => {
    expect(() =>
      CustomCssSanitizer.sanitize('.resource-bar { display: none; }', 2000)
    ).toThrow(/not allowed/i);
  });

  it('rejects disallowed at-rules', () => {
    expect(() =>
      CustomCssSanitizer.sanitize('@import url("https://example.com");', 2000)
    ).toThrow(/disallowed content/i);
  });
});
