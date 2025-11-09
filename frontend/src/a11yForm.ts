import i18next from 'i18next';
// @ts-nocheck
export class A11yFormManager {
  containerId: string;
  container: HTMLElement | null;
  form: HTMLFormElement | null;
  submitBtn: HTMLButtonElement | null;
  messageEl: HTMLElement | null;

  api: any;
  constructor(containerId = 'a11yFormContainer', api?: any) {
    this.containerId = containerId;
    this.container = typeof document !== 'undefined' ? document.getElementById(containerId) : null;
    this.form = null;
    this.submitBtn = null;
    this.messageEl = null;
    this.api = api ?? (typeof globalThis !== 'undefined' ? (globalThis as any).api : (typeof (window as any) !== 'undefined' ? (window as any).api : null));
    this.init();
  }

  init() {
    if (!this.container) return;
    this.container.innerHTML = this.template();
    this.form = this.container.querySelector('form');
    this.submitBtn = this.container.querySelector('button[type="submit"]');
    this.messageEl = this.container.querySelector('#a11yFormMessage');
    const desc = this.container.querySelector('#a11yDescription');
    if (desc) {
      (desc as HTMLElement).setAttribute('aria-describedby', 'a11yFormMessage');
    }
    if (this.form) {
      this.form.setAttribute('aria-describedby', 'a11yFormIntro');
      this.form.addEventListener('submit', (e) => this.onSubmit(e));
    }
    const resetBtn = this.container.querySelector('#a11yFormReset');
    if (resetBtn && this.form) resetBtn.addEventListener('click', () => this.form && this.form.reset());
  }

  template() {
    const t = (k: string, opts?: any) => {
      try {
        const res = i18next && (i18next as any).t ? (i18next as any).t(k, opts) : k;
        if (res === undefined || res === null) return k;
        return String(res);
      } catch (err) {
        return k;
      }
    };

    return `
      <section aria-labelledby="a11yFormTitle" aria-describedby="a11yFormIntro">
        <h2 id="a11yFormTitle">${t('a11y.title')}</h2>
        <form id="a11yForm" novalidate>
          <p id="a11yFormIntro">${t('a11y.intro')}</p>

          <div class="form-row">
            <label for="a11yName">${t('a11y.nameLabel')}</label>
            <input id="a11yName" name="name" type="text" placeholder="${t('a11y.namePlaceholder')}" aria-label="${t('a11y.nameLabel')}" />
          </div>

          <div class="form-row">
            <label for="a11yEmail">${t('a11y.emailLabel')}</label>
            <input id="a11yEmail" name="email" type="email" placeholder="${t('a11y.emailPlaceholder')}" aria-label="${t('a11y.emailLabel')}" />
          </div>

          <fieldset>
            <legend>${t('a11y.issueTypeLegend')}</legend>
            <div>
              <input type="radio" id="issue-contrast" name="issueType" value="contrast" checked />
              <label for="issue-contrast">${t('a11y.issue.contrast')}</label>
            </div>
            <div>
              <input type="radio" id="issue-navigation" name="issueType" value="navigation" />
              <label for="issue-navigation">${t('a11y.issue.navigation')}</label>
            </div>
            <div>
              <input type="radio" id="issue-other" name="issueType" value="other" />
              <label for="issue-other">${t('a11y.issue.other')}</label>
            </div>
          </fieldset>

          <div class="form-row">
            <label for="a11yDescription">${t('a11y.descriptionLabel')}</label>
            <textarea id="a11yDescription" name="description" required aria-required="true" rows="4" placeholder="${t('a11y.descriptionPlaceholder')}"></textarea>
          </div>

          <div class="form-row">
            <label for="a11yScreenshot">${t('a11y.screenshotLabel')}</label>
            <input id="a11yScreenshot" name="screenshot" type="url" placeholder="${t('a11y.screenshotPlaceholder')}" aria-label="${t('a11y.screenshotLabel')}" />
          </div>

          <div class="form-actions">
            <button type="submit" class="btn">${t('a11y.submit')}</button>
            <button type="button" id="a11yFormReset">${t('a11y.reset')}</button>
          </div>

          <div id="a11yFormMessage" role="status" aria-live="polite" aria-atomic="true"></div>
        </form>
      </section>
    `;
  }

  async onSubmit(e: Event) {
    e.preventDefault();
    const data: any = this.serialize();
    const descEl = this.container ? this.container.querySelector('#a11yDescription') as HTMLElement : null;
    if (!data.description || String(data.description).trim() === '') {
      if (descEl) {
        descEl.setAttribute('aria-invalid', 'true');
      }
      this.showMessage((i18next as any).t('a11y.requiredDescription'), 'error');
      if (descEl && typeof (descEl as any).focus === 'function') (descEl as any).focus();
      return;
    }

    try {
      if (this.api && typeof this.api.post === 'function') {
        await this.api.post('/a11y/report', data);
      } else {
        await Promise.resolve({ success: true });
      }

      if (descEl) {
        descEl.removeAttribute('aria-invalid');
      }
      this.showMessage((i18next as any).t('a11y.successMessage'), 'success');
      this.form && this.form.reset();
    } catch (err) {
      if (descEl) {
        descEl.setAttribute('aria-invalid', 'true');
      }
      this.showMessage((i18next as any).t('a11y.failureMessage'), 'error');
    }
  }

  serialize() {
    const fd = new FormData(this.form as HTMLFormElement);
    const obj: Record<string, any> = {};
    fd.forEach((v, k) => (obj[k] = v));
    return obj;
  }

  showMessage(msg: string, type = 'info') {
    if (!this.messageEl) return;
    // set live region behavior based on severity for assistive tech
    if (type === 'error') {
      this.messageEl.setAttribute('role', 'alert');
      this.messageEl.setAttribute('aria-live', 'assertive');
    } else {
      this.messageEl.setAttribute('role', 'status');
      this.messageEl.setAttribute('aria-live', 'polite');
    }
    this.messageEl.textContent = msg;
    this.messageEl.className = `a11y-form-message ${type}`;
    // ensure screen readers notice changes
    try {
      this.messageEl.setAttribute('tabindex', '-1');
      (this.messageEl as HTMLElement).focus();
    } catch (e) {
      // ignore focus errors in non-DOM test envs
    }
  }
}

// Auto-init if loaded in the browser (for demo pages)
if (typeof document !== 'undefined') {
  document.addEventListener('DOMContentLoaded', () => {
    const el = document.getElementById('a11yFormContainer');
    if (el) new A11yFormManager('a11yFormContainer', (typeof globalThis !== 'undefined' ? (globalThis as any).api : undefined));
  });
}

