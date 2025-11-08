import { AtRule, Rule } from 'postcss';
import safeParser from 'postcss-safe-parser';

const DISALLOWED_VALUE_PATTERNS = [
  /@import/i,
  /javascript:/i,
  /expression\s*\(/i,
  /url\s*\(\s*['"]?\s*data:/i,
  /behavior\s*:/i,
  /<\s*style/i,
  /<\/\s*style>/i,
];

const ALLOWED_AT_RULES = new Set(['media', 'supports', 'keyframes', 'layer']);

const ALLOWED_PROPERTIES = new Set([
  'background',
  'background-attachment',
  'background-blend-mode',
  'background-clip',
  'background-color',
  'background-image',
  'background-origin',
  'background-position',
  'background-repeat',
  'background-size',
  'border',
  'border-bottom',
  'border-bottom-color',
  'border-bottom-left-radius',
  'border-bottom-right-radius',
  'border-bottom-style',
  'border-bottom-width',
  'border-color',
  'border-image',
  'border-left',
  'border-left-color',
  'border-left-style',
  'border-left-width',
  'border-radius',
  'border-right',
  'border-right-color',
  'border-right-style',
  'border-right-width',
  'border-style',
  'border-top',
  'border-top-color',
  'border-top-left-radius',
  'border-top-right-radius',
  'border-top-style',
  'border-top-width',
  'border-width',
  'box-shadow',
  'caret-color',
  'color',
  'font-family',
  'font-feature-settings',
  'font-kerning',
  'font-optical-sizing',
  'font-size',
  'font-stretch',
  'font-style',
  'font-variant',
  'font-variant-caps',
  'font-variant-east-asian',
  'font-variant-ligatures',
  'font-variant-numeric',
  'font-variation-settings',
  'font-weight',
  'letter-spacing',
  'line-height',
  'text-align',
  'text-decoration',
  'text-decoration-color',
  'text-decoration-line',
  'text-decoration-style',
  'text-decoration-thickness',
  'text-rendering',
  'text-shadow',
  'text-transform',
  'text-underline-offset',
  'text-underline-position',
  'text-wrap',
  'white-space',
  'word-spacing',
  'outline',
  'outline-color',
  'outline-offset',
  'outline-style',
  'outline-width',
]);

export class CustomCssSanitizer {
  static sanitize(input?: string | null, maxLength: number = 8000): string | null {
    if (!input) return null;

    const trimmed = input.trim();
    if (!trimmed) {
      return null;
    }

    if (trimmed.length > maxLength) {
      throw new Error(`Custom CSS exceeds maximum length of ${maxLength} characters.`);
    }

    for (const pattern of DISALLOWED_VALUE_PATTERNS) {
      if (pattern.test(trimmed)) {
        throw new Error('Custom CSS contains disallowed content.');
      }
    }

    let root;
    try {
      root = safeParser(trimmed, { from: undefined });
    } catch (error) {
      throw new Error('Custom CSS is not valid.');
    }

    root.walkAtRules((atRule) => {
      const name = atRule.name?.toLowerCase();
      if (!name) {
        atRule.remove();
        return;
      }

      if (name === 'import' || name === 'charset') {
        throw new Error(`@${name} rules are not allowed in custom CSS.`);
      }

      if (!ALLOWED_AT_RULES.has(name)) {
        throw new Error(`@${atRule.name} rules are not allowed in custom CSS.`);
      }
    });

    root.walkRules((rule) => {
      if (!rule.selector) {
        return;
      }

      if (this.isKeyframeStep(rule)) {
        return;
      }

      const selectors = (rule.selectors || [])
        .map((selector) => this.scopeSelector(selector))
        .filter((selector): selector is string => Boolean(selector));

      if (selectors.length === 0) {
        rule.remove();
      } else {
        rule.selectors = selectors;
      }
    });

    root.walkDecls((decl) => {
      if (!this.isPropertyAllowed(decl.prop)) {
        throw new Error(`Property "${decl.prop}" is not allowed in custom CSS.`);
      }

      if (this.containsDisallowedValue(decl.value)) {
        throw new Error('Custom CSS contains disallowed content.');
      }
    });

    const sanitized = root.toString().trim();
    return sanitized || null;
  }

  private static isKeyframeStep(rule: Rule): boolean {
    if (rule.parent && rule.parent.type === 'atrule') {
      const name = (rule.parent as AtRule).name?.toLowerCase();
      return name === 'keyframes';
    }
    return false;
  }

  private static scopeSelector(selector: string): string | null {
    const trimmed = selector.trim();
    if (!trimmed) return null;

    if (trimmed.startsWith('body.user-theme-scope')) {
      return trimmed;
    }

    if (/^(body|html|:root)\b/i.test(trimmed)) {
      return trimmed.replace(/^(body|html|:root)/i, 'body.user-theme-scope');
    }

    if (trimmed.startsWith('@')) {
      return null;
    }

    return `body.user-theme-scope ${trimmed}`;
  }

  private static isPropertyAllowed(property: string): boolean {
    if (!property) return false;
    if (property.startsWith('--')) return true;
    return ALLOWED_PROPERTIES.has(property.toLowerCase());
  }

  private static containsDisallowedValue(value: string): boolean {
    if (!value) return false;
    return DISALLOWED_VALUE_PATTERNS.some((pattern) => pattern.test(value));
  }
}

export default CustomCssSanitizer;
