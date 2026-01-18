// @ts-nocheck
const API_URL = '/api/admin/sms-service/config';
const CHANNEL_OPTIONS = [
    { value: 'sms_twilio', label: 'Twilio SMS' },
    { value: 'sms_http', label: 'Custom HTTP SMS' },
    { value: 'whatsapp_twilio', label: 'Twilio WhatsApp' },
    { value: 'whatsapp_baileys', label: 'WhatsApp (Baileys)' },
    { value: 'telegram', label: 'Telegram Bot' },
    { value: 'discord', label: 'Discord Bot DM' },
    { value: 'custom_http', label: 'Generic HTTP Gateway' }
];

let currentConfig = null;

document.addEventListener('DOMContentLoaded', () => {
    renderChannelOptions();
    bindEvents();
    loadConfig();
    loadPermissions();
    loadMetrics();
});

function renderChannelOptions() {
    const select = document.getElementById('defaultChannel');
    if (!select) return;
    select.innerHTML = CHANNEL_OPTIONS.map((option) => `<option value="${option.value}">${option.label}</option>`).join('');
}

function bindEvents() {
    const form = document.getElementById('smsConfigForm');
    if (form) {
        form.addEventListener('submit', async (event) => {
            event.preventDefault();
            await saveConfig();
        });
    }
    const fallbackInput = document.getElementById('fallbackChannels');
    if (fallbackInput) {
        fallbackInput.addEventListener('blur', () => renderFallbackPreview());
    }
    const refreshMetrics = document.getElementById('refreshMetrics');
    if (refreshMetrics) {
        refreshMetrics.addEventListener('click', () => loadMetrics(true));
    }
}

async function loadConfig() {
    setStatus('Loading configuration...', 'info');
    try {
        const response = await fetch(API_URL, {
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            }
        });
        if (!response.ok) {
            const err = await response.json();
            throw new Error(err.error || 'Failed to load configuration');
        }
        const data = await response.json();
        currentConfig = data.config;
        populateForm(currentConfig);
        setStatus('Configuration loaded.', 'success');
    } catch (error) {
        console.error('Failed to load SMS config:', error);
        setStatus(error.message || 'Failed to load SMS configuration', 'error');
    }
}

function populateForm(config) {
    const urlInput = document.getElementById('serviceUrl');
    const defaultChannel = document.getElementById('defaultChannel');
    const fallbackInput = document.getElementById('fallbackChannels');
    const apiKeyStatus = document.getElementById('apiKeyStatus');
    const updatedAt = document.getElementById('smsConfigUpdatedAt');

    if (urlInput) urlInput.value = config.service_url || '';
    if (defaultChannel) defaultChannel.value = config.default_channel || CHANNEL_OPTIONS[0].value;
    if (fallbackInput) fallbackInput.value = (config.fallback_channels || []).join(', ');
    if (apiKeyStatus) {
        apiKeyStatus.textContent = config.api_key_set ? 'Stored' : 'Not Set';
        apiKeyStatus.className = `badge ${config.api_key_set ? 'badge-success' : 'badge-danger'}`;
    }
    if (updatedAt) {
        updatedAt.textContent = config.updated_at ? formatDateTime(config.updated_at) : '-';
    }

    renderFallbackPreview();
}

function getPayload() {
    const updates = {};
    const urlInput = document.getElementById('serviceUrl');
    const defaultChannel = document.getElementById('defaultChannel');
    const fallbackInput = document.getElementById('fallbackChannels');
    const apiKeyInput = document.getElementById('apiKeyInput');
    const clearApiKey = document.getElementById('clearApiKey');

    if (urlInput && urlInput.value.trim() && urlInput.value.trim() !== currentConfig?.service_url) {
        updates.service_url = urlInput.value.trim();
    }

    if (defaultChannel && defaultChannel.value !== currentConfig?.default_channel) {
        updates.default_channel = defaultChannel.value;
    }

    if (fallbackInput) {
        const channels = fallbackInput.value
            .split(',')
            .map((c) => c.trim())
            .filter(Boolean);
        const normalized = normalizeChannels(channels);
        if (JSON.stringify(normalized) !== JSON.stringify(currentConfig?.fallback_channels || [])) {
            updates.fallback_channels = normalized;
        }
    }

    if (clearApiKey && clearApiKey.checked) {
        updates.api_key = null;
    } else if (apiKeyInput && apiKeyInput.value.trim().length > 0) {
        updates.api_key = apiKeyInput.value.trim();
    }

    return updates;
}

function normalizeChannels(channels) {
    const set = new Set();
    const normalized = [];
    channels.forEach((channel) => {
        const option = CHANNEL_OPTIONS.find((opt) => opt.value === channel);
        if (option && !set.has(option.value)) {
            set.add(option.value);
            normalized.push(option.value);
        }
    });
    return normalized;
}

async function saveConfig() {
    const payload = getPayload();
    if (Object.keys(payload).length === 0) {
        setStatus('No changes to save.', 'warning');
        return;
    }

    setStatus('Saving configuration...', 'info');
    try {
        const response = await fetch(API_URL, {
            method: 'PUT',
            headers: {
                'Content-Type': 'application/json',
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            },
            body: JSON.stringify(payload)
        });

        if (!response.ok) {
            const err = await response.json();
            throw new Error(err.error || 'Failed to save configuration');
        }

        const data = await response.json();
        currentConfig = data.config;
        populateForm(currentConfig);
        const apiKeyInput = document.getElementById('apiKeyInput');
        const clearApiKey = document.getElementById('clearApiKey');
        if (apiKeyInput) apiKeyInput.value = '';
        if (clearApiKey) clearApiKey.checked = false;
        setStatus('SMS service configuration updated.', 'success');
    } catch (error) {
        console.error('Failed to save SMS config:', error);
        setStatus(error.message || 'Failed to save configuration', 'error');
    }
}

function setStatus(message, type = 'info') {
    const banner = document.getElementById('smsConfigStatus');
    if (!banner) return;
    banner.textContent = message;
    banner.className = `status-banner status-${type}`;
}

function renderFallbackPreview() {
    const preview = document.getElementById('fallbackPreview');
    const input = document.getElementById('fallbackChannels');
    if (!preview || !input) return;
    const channels = normalizeChannels(
        input.value.split(',').map((c) => c.trim()).filter(Boolean)
    );
    if (channels.length === 0) {
        preview.innerHTML = '<span class="text-muted">No fallback channels configured.</span>';
        return;
    }
    preview.innerHTML = channels
        .map((channel) => {
            const option = CHANNEL_OPTIONS.find((opt) => opt.value === channel);
            const label = option ? option.label : channel;
            return `<span class="channel-pill">${label}</span>`;
        })
        .join('');
}
async function loadPermissions() {
    const list = document.getElementById('smsPermissionRoles');
    if (list) {
        list.innerHTML = '<li class="text-muted">Loading…</li>';
    }
    try {
        const response = await fetch('/api/admin/sms-service/permissions', {
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            }
        });
        if (!response.ok) throw new Error('Failed to load permissions');
        const data = await response.json();
        renderPermissions(data.roles || []);
    } catch (error) {
        console.error('Failed to load permissions', error);
        if (list) list.innerHTML = '<li class="text-muted">Unable to load roles</li>';
    }
}

function renderPermissions(roles) {
    const list = document.getElementById('smsPermissionRoles');
    if (!list) return;
    if (!roles.length) {
        list.innerHTML = '<li class="text-muted">No roles granted access yet.</li>';
        return;
    }
    list.innerHTML = roles
        .map((role) => {
            const perms = (role.permissions || []).join(', ');
            return `<li><strong>${role.name}</strong><br><small>${perms}</small></li>`;
        })
        .join('');
}

async function loadMetrics(manual = false) {
    const status = document.getElementById('metricsStatus');
    if (status) status.textContent = manual ? 'Refreshing…' : 'Loading metrics…';
    try {
        const response = await fetch('/api/admin/sms-service/metrics', {
            headers: { 'Authorization': `Bearer ${localStorage.getItem('token')}` }
        });
        if (!response.ok) throw new Error('Failed to load metrics');
        const data = await response.json();
        renderMetrics(data);
        if (status) status.textContent = `Updated ${formatTime(new Date())}`;
    } catch (error) {
        console.error('Metrics fetch failed', error);
        if (status) status.textContent = error.message || 'Failed to load metrics';
    }
}

function renderMetrics(payload) {
    const metrics = payload?.metrics || {};
    const history = payload?.history || [];
    setText('metricRequests', metrics.requests ?? 0);
    setText('metricSuccesses', metrics.successes ?? 0);
    setText('metricFailures', metrics.failures ?? 0);
    const avg = metrics.avgResponseMs ? `${metrics.avgResponseMs.toFixed(1)} ms` : '0 ms';
    setText('metricLatency', avg);

    const channelStats = document.getElementById('channelStats');
    if (!channelStats) return;
    if (!history.length) {
        channelStats.innerHTML = '<tr><td colspan="3" class="text-muted">No data yet</td></tr>';
        return;
    }
    channelStats.innerHTML = history
        .map((row) => {
            const success = row.status === 'success' ? row.count : 0;
            const failure = row.status === 'failed' ? row.count : 0;
            return `<tr>
                <td>${row.channel}</td>
                <td>${success}</td>
                <td>${failure}</td>
            </tr>`;
        })
        .join('');
}

function setText(id, value) {
    const el = document.getElementById(id);
    if (el) el.textContent = value;
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

function formatTime(value) {
    const date = value instanceof Date ? value : new Date(value);
    const locale = getLocale();
    if (typeof Intl !== 'undefined' && Intl.DateTimeFormat) {
        return new Intl.DateTimeFormat(locale, {
            hour: '2-digit',
            minute: '2-digit',
            second: '2-digit',
        }).format(date);
    }
    return date.toLocaleTimeString();
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
