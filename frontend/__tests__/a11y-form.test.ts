import { axe, toHaveNoViolations } from 'jest-axe';
import mockI18next from '../src/__mocks__/i18next';
jest.mock('i18next', () => mockI18next);
import { A11yFormManager } from '../src/a11yForm';
import i18next from 'i18next';

expect.extend(toHaveNoViolations);

describe('A11yFormManager', () => {
  let container;

  beforeEach(() => {
    document.body.innerHTML = '<div id="a11yFormContainer"></div>';
    container = document.getElementById('a11yFormContainer');
    // ensure a clean mock api is available
    (globalThis as any).api = { post: jest.fn(async () => ({ success: true })) };
  });

  afterEach(() => {
    jest.resetAllMocks();
    delete (globalThis as any).api;
  });

  it('submits when description is provided and shows success', async () => {
    const apiMock = { post: jest.fn(async () => ({ success: true })) };
    const mgr = new A11yFormManager('a11yFormContainer', apiMock as any);
    const form = container.querySelector('form#a11yForm');
    const desc = form.querySelector('#a11yDescription');
    desc.value = 'There is a contrast issue on the home page.';

    const submit = form.querySelector('button[type="submit"]');
    submit.click();
    // wait for async submit handler
    await Promise.resolve();
    await Promise.resolve();

    expect(apiMock.post).toHaveBeenCalledWith('/a11y/report', expect.objectContaining({ description: expect.any(String) }));

    const msg = container.querySelector('#a11yFormMessage');
    expect(msg.textContent).toBe(i18next.t('a11y.successMessage'));

  });

  it('handles API failures and shows error message', async () => {
    const apiMock = { post: jest.fn(async () => { throw new Error('network'); }) };
    const mgr = new A11yFormManager('a11yFormContainer', apiMock as any);
    const form = container.querySelector('form#a11yForm');
    const desc = form.querySelector('#a11yDescription');
    desc.value = 'There is a focus problem on the nav.';

    const submit = form.querySelector('button[type="submit"]');
    submit.click();
    await Promise.resolve();
    await Promise.resolve();

    expect(apiMock.post).toHaveBeenCalled();
    const msg = container.querySelector('#a11yFormMessage');
    expect(msg.textContent).toBe(i18next.t('a11y.failureMessage'));
  });


  it('renders the form into the container', () => {
    new A11yFormManager('a11yFormContainer');
    expect(container.querySelector('form#a11yForm')).not.toBeNull();
    expect(container.querySelector('#a11yFormTitle').textContent).toContain(i18next.t('a11y.title'));

  });

  it('shows validation message when description is missing', async () => {
    const mgr = new A11yFormManager('a11yFormContainer');
    const form = container.querySelector('form#a11yForm');
    const submit = form.querySelector('button[type="submit"]');

    // submit empty form
    submit.click();
    // wait a tick for event handlers
    await Promise.resolve();

    const msg = container.querySelector('#a11yFormMessage');
    expect(msg.textContent).toBe(i18next.t('a11y.requiredDescription'));

    expect((globalThis as any).api.post).not.toHaveBeenCalled();
  });

  it('submits when description is provided and shows success', async () => {
    const mgr = new A11yFormManager('a11yFormContainer');
    const form = container.querySelector('form#a11yForm');
    const desc = form.querySelector('#a11yDescription');
    desc.value = 'There is a contrast issue on the home page.';

    const submit = form.querySelector('button[type="submit"]');
    submit.click();
    // wait for async submit handler
    await Promise.resolve();
    await Promise.resolve();

    expect((globalThis as any).api.post).toHaveBeenCalledWith('/a11y/report', expect.objectContaining({ description: expect.any(String) }));

    const msg = container.querySelector('#a11yFormMessage');
    expect(msg.textContent).toBe(i18next.t('a11y.successMessage'));

  });

  it('has no accessibility violations (axe)', async () => {
    new A11yFormManager('a11yFormContainer');
    const results = await axe(document.body.innerHTML);
    expect(results).toHaveNoViolations();
  });
});
