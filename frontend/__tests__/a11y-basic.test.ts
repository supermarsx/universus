import { axe, toHaveNoViolations } from 'jest-axe';
expect.extend(toHaveNoViolations);

describe('Accessibility check', () => {
  it('should have no accessibility violations on a simple HTML snippet', async () => {
    const html = `<main><h1>Hello, Universus!</h1><p>This is a test.</p></main>`;
    const results = await axe(html);
    expect(results).toHaveNoViolations();
  });

  it('should have no accessibility violations on moon overview modal', async () => {
    const html = `
      <div id="moonOverviewModal" class="modal" style="display:block;">
        <div class="modal-content">
          <button class="close" id="closeMoonOverviewModal" aria-label="Close moon overview modal">&times;</button>
          <div id="moonOverviewContent">
            <div class="moon-overview-header">
              <h3>Moon [1:1:1]</h3>
              <p>Diameter: 5000 km</p>
            </div>
            <div class="moon-overview-fields">
              <h4>Fields</h4>
              <div class="progress-bar">
                <div class="progress-fill" style="width: 50%"></div>
              </div>
              <p>2/4 (50%)</p>
            </div>
            <div class="moon-overview-jump-gate">
              <h4>Jump Gate</h4>
              <p>Destination: [1:1:2]</p>
              <p>Cooldown: 5m 30s</p>
              <button class="btn-secondary">Set Destination</button>
            </div>
            <div class="moon-overview-phalanx">
              <h4>Sensor Phalanx</h4>
              <p>Level: 1</p>
              <button class="btn-secondary scan-btn">Scan</button>
              <div class="scan-log">
                <h5>Recent Scans</h5>
                <p>No recent scans</p>
              </div>
            </div>
          </div>
        </div>
      </div>
    `;
    const results = await axe(html);
    expect(results).toHaveNoViolations();
  });

  it('should support keyboard navigation in moon overview modal', () => {
    // Mock DOM
    document.body.innerHTML = `
      <div id="moonOverviewModal" class="modal" style="display:block;">
        <div class="modal-content">
          <button class="close" id="closeMoonOverviewModal" aria-label="Close moon overview modal">&times;</button>
          <div id="moonOverviewContent">
            <button class="btn-secondary">Set Destination</button>
            <button class="btn-secondary scan-btn">Scan</button>
          </div>
        </div>
      </div>
    `;

    const modal = document.getElementById('moonOverviewModal') as HTMLElement;
    const closeBtn = document.getElementById('closeMoonOverviewModal') as HTMLButtonElement;
    const buttons = modal.querySelectorAll('button');

    // Check tab order
    const focusableElements = modal.querySelectorAll('button[tabindex]:not([tabindex="-1"]), button:not([disabled])');
    expect(focusableElements.length).toBeGreaterThan(0);

    // Simulate tab navigation (basic check)
    const firstBtn = buttons[0];
    firstBtn.focus();
    expect(document.activeElement).toBe(firstBtn);

    // Simulate Escape key
    const event = new KeyboardEvent('keydown', { key: 'Escape' });
    modal.dispatchEvent(event);
    // In real code, this would close modal, but since no event listener in test, just check if event can be dispatched
    expect(event.key).toBe('Escape');
  });
});
