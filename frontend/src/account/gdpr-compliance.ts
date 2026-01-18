// @ts-nocheck
/**
 * GDPR Compliance Interface
 * Handles data export, deletion requests, and privacy settings
 */

class GDPRComplianceManager {
    constructor() {
        this.api = '/api/account/gdpr';
        this.confirmCallback = null;
        this.init();
    }

    init() {
        this.loadActiveRequests();
        this.loadPrivacySettings();
        this.attachEventListeners();
    }

    /**
     * Attach event listeners
     */
    attachEventListeners() {
        // Request data export
        const exportBtn = document.getElementById('requestExportBtn');
        if (exportBtn) {
            exportBtn.addEventListener('click', () => this.requestDataExport());
        }

        // Request data deletion
        const deletionBtn = document.getElementById('requestDeletionBtn');
        if (deletionBtn) {
            deletionBtn.addEventListener('click', () => this.requestDataDeletion());
        }

        // Privacy settings form
        const privacyForm = document.getElementById('privacySettingsForm');
        if (privacyForm) {
            privacyForm.addEventListener('submit', (e) => {
                e.preventDefault();
                this.savePrivacySettings();
            });
        }

        // Confirmation modal
        const cancelConfirmBtn = document.getElementById('cancelConfirmBtn');
        if (cancelConfirmBtn) {
            cancelConfirmBtn.addEventListener('click', () => this.closeConfirmation());
        }

        const proceedConfirmBtn = document.getElementById('proceedConfirmBtn');
        if (proceedConfirmBtn) {
            proceedConfirmBtn.addEventListener('click', () => this.confirmAction());
        }
    }

    /**
     * Load active GDPR requests
     */
    async loadActiveRequests() {
        try {
            const response = await fetch(`${this.api}/requests`, {
                headers: {
                    'Authorization': `Bearer ${this.getToken()}`
                }
            });

            if (!response.ok) {
                throw new Error('Failed to load requests');
            }

            const data = await response.json();
            if (data.requests && data.requests.length > 0) {
                this.displayActiveRequests(data.requests);
            }

        } catch (error) {
            console.error('Error loading active requests:', error);
        }
    }

    /**
     * Display active requests
     */
    displayActiveRequests(requests) {
        const card = document.getElementById('activeRequestsCard');
        const list = document.getElementById('requestsList');

        if (requests.length === 0) {
            card.style.display = 'none';
            return;
        }

        card.style.display = 'block';
        list.innerHTML = requests.map(req => this.renderRequestCard(req)).join('');
    }

    /**
     * Render individual request card
     */
    renderRequestCard(request) {
        const statusClass = {
            'pending': 'warning',
            'processing': 'info',
            'completed': 'success',
            'cancelled': 'danger'
        }[request.status] || 'default';

        const typeLabel = {
            'data_export': 'Data Export',
            'data_deletion': 'Data Deletion'
        }[request.request_type] || request.request_type;

        return `
            <div class="request-card">
                <div class="request-header">
                    <h4>${typeLabel}</h4>
                    <span class="badge badge-${statusClass}">${request.status}</span>
                </div>
                <div class="request-details">
                    <p><strong>Requested:</strong> ${this.formatDateTime(request.created_at)}</p>
                    ${request.completed_at ? `<p><strong>Completed:</strong> ${this.formatDateTime(request.completed_at)}</p>` : ''}
                    ${request.expires_at ? `<p><strong>Expires:</strong> ${this.formatDateTime(request.expires_at)}</p>` : ''}
                </div>
                <div class="request-actions">
                    ${request.status === 'completed' && request.request_type === 'data_export' ? 
                        `<button class="btn btn-primary btn-sm" onclick="gdprManager.downloadData('${request.id}')">
                            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor">
                                <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
                                <polyline points="7 10 12 15 17 10"></polyline>
                                <line x1="12" y1="15" x2="12" y2="3"></line>
                            </svg>
                            Download
                        </button>` : ''
                    }
                    ${request.status === 'pending' ? 
                        `<button class="btn btn-danger-outline btn-sm" onclick="gdprManager.cancelRequest('${request.id}')">
                            Cancel Request
                        </button>` : ''
                    }
                </div>
            </div>
        `;
    }

    /**
     * Request data export
     */
    async requestDataExport() {
        const options = {
            includeGameData: document.getElementById('exportGameData').checked,
            includeMessages: document.getElementById('exportMessages').checked,
            includeActivity: document.getElementById('exportActivity').checked,
            includeSecurity: document.getElementById('exportSecurity').checked
        };

        const btn = document.getElementById('requestExportBtn');
        const originalText = btn.innerHTML;
        btn.disabled = true;
        btn.innerHTML = '<span class="spinner"></span> Requesting...';

        try {
            const response = await fetch(`${this.api}/request`, {
                method: 'POST',
                headers: {
                    'Authorization': `Bearer ${this.getToken()}`,
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({
                    requestType: 'data_export',
                    options
                })
            });

            if (!response.ok) {
                const error = await response.json();
                throw new Error(error.message || 'Failed to request data export');
            }

            const data = await response.json();
            this.showSuccess(
                'Data Export Requested',
                'Your data export request has been submitted. You will receive an email when it\'s ready (up to 30 days).'
            );

            // Reload requests
            setTimeout(() => this.loadActiveRequests(), 1000);

        } catch (error) {
            console.error('Error requesting data export:', error);
            this.showError(error.message);
        } finally {
            btn.disabled = false;
            btn.innerHTML = originalText;
        }
    }

    /**
     * Request data deletion
     */
    requestDataDeletion() {
        this.showConfirmation(
            'Confirm Data Deletion',
            'This will permanently delete all your personal data. This action cannot be undone. Are you absolutely sure?',
            async (password) => {
                await this.executeDataDeletion(password);
            }
        );
    }

    /**
     * Execute data deletion
     */
    async executeDataDeletion(password) {
        try {
            const response = await fetch(`${this.api}/request`, {
                method: 'POST',
                headers: {
                    'Authorization': `Bearer ${this.getToken()}`,
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({
                    requestType: 'data_deletion',
                    password
                })
            });

            if (!response.ok) {
                const error = await response.json();
                throw new Error(error.message || 'Failed to request data deletion');
            }

            this.showSuccess(
                'Data Deletion Requested',
                'Your data deletion request has been submitted. Your account will be deleted within 90 days.'
            );

            // Reload requests
            setTimeout(() => this.loadActiveRequests(), 1000);

        } catch (error) {
            console.error('Error requesting data deletion:', error);
            this.showError(error.message);
        }
    }

    /**
     * Download exported data
     */
    async downloadData(requestId) {
        try {
            const response = await fetch(`${this.api}/download/${requestId}`, {
                headers: {
                    'Authorization': `Bearer ${this.getToken()}`
                }
            });

            if (!response.ok) {
                throw new Error('Failed to download data');
            }

            const blob = await response.blob();
            const url = window.URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url;
            a.download = `universus-data-export-${requestId}.zip`;
            document.body.appendChild(a);
            a.click();
            document.body.removeChild(a);
            window.URL.revokeObjectURL(url);

            this.showSuccess('Download Started', 'Your data export is being downloaded.');

        } catch (error) {
            console.error('Error downloading data:', error);
            this.showError(error.message);
        }
    }

    /**
     * Cancel a request
     */
    async cancelRequest(requestId) {
        if (!confirm('Are you sure you want to cancel this request?')) {
            return;
        }

        try {
            const response = await fetch(`${this.api}/cancel/${requestId}`, {
                method: 'POST',
                headers: {
                    'Authorization': `Bearer ${this.getToken()}`
                }
            });

            if (!response.ok) {
                throw new Error('Failed to cancel request');
            }

            this.showSuccess('Request Cancelled', 'The request has been cancelled successfully.');
            this.loadActiveRequests();

        } catch (error) {
            console.error('Error cancelling request:', error);
            this.showError(error.message);
        }
    }

    /**
     * Load privacy settings
     */
    async loadPrivacySettings() {
        try {
            const response = await fetch('/api/account/privacy/settings', {
                headers: {
                    'Authorization': `Bearer ${this.getToken()}`
                }
            });

            if (!response.ok) {
                throw new Error('Failed to load privacy settings');
            }

            const settings = await response.json();
            this.applyPrivacySettings(settings);

        } catch (error) {
            console.error('Error loading privacy settings:', error);
        }
    }

    /**
     * Apply privacy settings to form
     */
    applyPrivacySettings(settings) {
        if (settings.profileVisibility) {
            document.getElementById('profileVisibility').value = settings.profileVisibility;
        }
        if (settings.showActivity !== undefined) {
            document.getElementById('showActivity').checked = settings.showActivity;
        }
        if (settings.messagePermissions) {
            document.getElementById('messagePermissions').value = settings.messagePermissions;
        }
        if (settings.allowAnalytics !== undefined) {
            document.getElementById('allowAnalytics').checked = settings.allowAnalytics;
        }
    }

    /**
     * Save privacy settings
     */
    async savePrivacySettings() {
        const settings = {
            profileVisibility: document.getElementById('profileVisibility').value,
            showActivity: document.getElementById('showActivity').checked,
            messagePermissions: document.getElementById('messagePermissions').value,
            allowAnalytics: document.getElementById('allowAnalytics').checked
        };

        try {
            const response = await fetch('/api/account/privacy/settings', {
                method: 'POST',
                headers: {
                    'Authorization': `Bearer ${this.getToken()}`,
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify(settings)
            });

            if (!response.ok) {
                const error = await response.json();
                throw new Error(error.message || 'Failed to save privacy settings');
            }

            this.showSuccess('Settings Saved', 'Your privacy settings have been updated successfully.');

        } catch (error) {
            console.error('Error saving privacy settings:', error);
            this.showError(error.message);
        }
    }

    /**
     * Show confirmation modal
     */
    showConfirmation(title, message, callback) {
        document.getElementById('confirmTitle').textContent = title;
        document.getElementById('confirmMessage').textContent = message;
        document.getElementById('confirmPassword').value = '';
        document.getElementById('confirmationModal').style.display = 'flex';
        this.confirmCallback = callback;
    }

    /**
     * Close confirmation modal
     */
    closeConfirmation() {
        document.getElementById('confirmationModal').style.display = 'none';
        this.confirmCallback = null;
    }

    /**
     * Confirm action with password
     */
    async confirmAction() {
        const password = document.getElementById('confirmPassword').value;

        if (!password) {
            this.showError('Please enter your password');
            return;
        }

        this.closeConfirmation();

        if (this.confirmCallback) {
            await this.confirmCallback(password);
        }
    }

    /**
     * Show success message
     */
    showSuccess(title, message) {
        if (window.toast) { window.toast.success(`${title}: ${message}`); } else { window.toast ? window.toast.success(`${title}: ${message}`) : alert(`${title}\n\n${message}`); }
    }

    /**
     * Show error message
     */
    showError(message) {
        if (window.toast) { window.toast.error(message); } else { window.toast ? window.toast.error(message) : alert(`Error: ${message}`); }
    }

    /**
     * Get authentication token
     */
    getToken() {
        return localStorage.getItem('token') || '';
    }

    formatDateTime(value) {
        const date = value ? new Date(value) : new Date();
        const locale = this.getLocale();
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

    getLocale() {
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
}

// Initialize and expose globally
let gdprManager;
document.addEventListener('DOMContentLoaded', () => {
    gdprManager = new GDPRComplianceManager();
});
