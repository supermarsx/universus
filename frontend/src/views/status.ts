/**
 * Public Status Page client logic
 * Provides a small, dependency-free frontend module that fetches the
 * `/status` endpoint and renders incidents and maintenance windows. When an
 * admin token is present the page reveals basic admin controls that call the
 * admin API endpoints to create incidents and maintenance windows.
 *
 * The code is intentionally simple and self-contained to ease integration
 * into existing static build pipelines.
 */

// @ts-nocheck
const API_BASE = '';

/**
 * Fetch the public status snapshot from the server and render it.
 * Errors are handled gracefully by updating the banner.
 * @returns {Promise<void>}
 */
async function fetchStatus() {
  try {
    const res = await fetch('/status');
    if (!res.ok) throw new Error('Failed to load status');
    const data = await res.json();
    renderStatus(data);
  } catch (err) {
    console.error(err);
    const el = document.getElementById('overallStatus');
    if (el) el.textContent = 'Error loading status';
  }
}

/**
 * Render the status snapshot into the DOM.
 * @param {Object} data - Public status payload
 * @param {string} data.overall_status - Overall status string ('good'|'degraded')
 * @param {Array<Object>} data.incidents - Array of incidents
 * @param {Array<Object>} data.maintenance - Array of maintenance windows
 */
function renderStatus(data) {
  const banner = document.getElementById('overallStatus');
  if (banner) {
    banner.textContent = `Overall: ${data.overall_status}`;
    banner.className = `status-banner status-${data.overall_status}`;
  }

  const incList = document.getElementById('incidentsList');
  if (incList) {
    if (data.incidents && data.incidents.length > 0) {
      incList.innerHTML = data.incidents.map(i => `
      <div class="incident">
        <div class="incident-title">${escapeHtml(i.title)} <span class="badge badge-${i.severity}">${i.severity}</span></div>
        <div class="incident-meta">${formatDateTime(i.start_time)} • ${i.affected_components.join(', ')}</div>
        <div class="incident-desc">${escapeHtml(i.description || '')}</div>
      </div>
    `).join('');
    } else {
      incList.innerHTML = '<div>No active incidents</div>';
    }
  }

  const maintList = document.getElementById('maintenanceList');
  if (maintList) {
    if (data.maintenance && data.maintenance.length > 0) {
      maintList.innerHTML = data.maintenance.map(m => `
      <div class="maintenance">
        <div class="maintenance-title">${escapeHtml(m.name)}</div>
        <div class="maintenance-meta">${formatDateTime(m.start_time)} → ${formatDateTime(m.end_time)}</div>
        <div class="maintenance-desc">${escapeHtml(m.description || '')}</div>
      </div>
    `).join('');
    } else {
      maintList.innerHTML = '<div>No upcoming maintenance</div>';
    }
  }
}

/**
 * Escape text to prevent XSS when inserting into innerHTML.
 * Uses textContent on a temporary DOM node.
 * @param {string} text
 * @returns {string}
 */
function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}

function formatDateTime(value) {
  const date = value ? new Date(value) : new Date();
  const locale = getLocale();
  if (typeof Intl !== 'undefined' && Intl.DateTimeFormat) {
    return new Intl.DateTimeFormat(locale, {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    }).format(date);
  }
  return date.toLocaleString();
}

function getLocale() {
  try {
    if (window.i18next && window.i18next.language) {
      return window.i18next.language;
    }
  } catch (error) {
    // ignore i18next access errors
  }
  try {
    const stored = localStorage.getItem('preferredLanguage');
    if (stored) return stored;
  } catch (error) {
    // ignore storage access errors
  }
  return navigator.language || 'en-US';
}

/**
 * Verify whether the current visitor is an admin by attempting to call
 * a protected admin endpoint with the stored token. If successful, reveal
 * admin controls and wire up handlers.
 * @returns {Promise<void>}
 */
async function checkAdmin() {
  const token = localStorage.getItem('token');
  if (!token) return;
  try {
    // attempt to call admin status read endpoint to confirm
    const res = await fetch('/api/admin/status/incidents', { headers: { 'Authorization': `Bearer ${token}` } });
    if (res.ok) {
      const adminControls = document.getElementById('adminControls');
      if (adminControls) adminControls.style.display = 'block';
      setupAdminButtons();
    }
  } catch (err) {
    // not admin or network error — silently ignore to keep UI clean
  }
}

/**
 * Wire up admin buttons for creating incidents and maintenance windows.
 * These handlers use simple `prompt()` dialogs to keep the UI dependency-free
 * — in future this should be replaced with proper modal forms.
 */
function setupAdminButtons() {
  const createIncidentBtn = document.getElementById('createIncidentBtn');
  if (createIncidentBtn) {
    createIncidentBtn.addEventListener('click', async () => {
      const title = prompt('Incident title');
      if (!title) return;
      const description = prompt('Description (optional)');
      const comps = prompt('Affected components (comma separated)') || '';
      const severity = prompt('Severity (low|medium|high|critical)', 'medium') || 'medium';

      const token = localStorage.getItem('token');
      await fetch('/api/admin/status/incidents', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', 'Authorization': `Bearer ${token}` },
        body: JSON.stringify({ title, description, affected_components: comps.split(',').map(s=>s.trim()).filter(Boolean), severity })
      });

      await fetchStatus();
    });
  }

  const createMaintenanceBtn = document.getElementById('createMaintenanceBtn');
  if (createMaintenanceBtn) {
    createMaintenanceBtn.addEventListener('click', async () => {
      const name = prompt('Maintenance name');
      if (!name) return;
      const description = prompt('Description (optional)');
      const start = prompt('Start ISO timestamp (or leave empty for now)');
      const end = prompt('End ISO timestamp');

      const token = localStorage.getItem('token');
      await fetch('/api/admin/status/maintenance', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', 'Authorization': `Bearer ${token}` },
        body: JSON.stringify({ name, description, start_time: start || new Date().toISOString(), end_time: end })
      });

      await fetchStatus();
    });
  }
}

// Initial load
document.addEventListener('DOMContentLoaded', async () => {
  await fetchStatus();
  await checkAdmin();
  setInterval(fetchStatus, 15000);
});

