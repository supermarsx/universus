// @ts-nocheck
// Admin Configuration Management Client
// Handles all configuration UI interactions and API calls

class ConfigurationManager {
    constructor() {
        this.currentCategory = 'combat';
        this.categories = [];
        this.parameters = {};
        this.pendingChanges = new Map();
        this.socket = null;
        this.snapshot = null;
        
        this.init();
    }

    async init() {
        await this.loadCategories();
        await this.loadStatistics();
        await this.loadGameConfigSnapshot();
        this.setupEventListeners();
        this.initializeSocket();
        this.renderCategoryTabs();
        await this.loadCategoryParameters(this.currentCategory);
    }

    // API Methods
    async apiCall(endpoint, options = {}) {
        const token = localStorage.getItem('token');
        const response = await fetch(`/api/config${endpoint}`, {
            ...options,
            headers: {
                'Content-Type': 'application/json',
                'Authorization': `Bearer ${token}`,
                ...options.headers
            }
        });

        if (!response.ok) {
            const error = await response.json();
            throw new Error(error.error || 'API request failed');
        }

        return await response.json();
    }

    async loadCategories() {
        try {
            const data = await this.apiCall('/categories');
            this.categories = data.categories;
        } catch (error) {
            this.showToast('Failed to load categories: ' + error.message, 'error');
        }
    }

    async loadStatistics() {
        try {
            const data = await this.apiCall('/stats');
            document.getElementById('totalParams').textContent = data.stats.total_parameters;
            document.getElementById('modifiedParams').textContent = data.stats.modified_parameters;
            document.getElementById('recentChanges').textContent = data.stats.recent_changes_24h;
            
            // Calculate restart required (would need to track this)
            document.getElementById('restartRequired').textContent = '0';
        } catch (error) {
            console.error('Failed to load statistics:', error);
        }
    }

    async loadGameConfigSnapshot(force = false) {
        try {
            const endpoint = force ? '/game-config/refresh' : '/game-config';
            const options = force ? { method: 'POST' } : {};
            const data = await this.apiCall(endpoint, options);
            this.snapshot = data.config || data;
            this.renderSnapshotSummary();
            if (force) {
                this.showToast('Runtime snapshot refreshed', 'success');
            }
        } catch (error) {
            this.showToast('Failed to load runtime snapshot: ' + error.message, 'error');
        }
    }

    renderSnapshotSummary() {
        const container = document.getElementById('snapshotSummary');
        if (!container) return;

        if (!this.snapshot) {
            container.innerHTML = '<div class="snapshot-placeholder">Snapshot unavailable.</div>';
            return;
        }

        const sections = [
            { key: 'combat', label: 'Combat' },
            { key: 'resources', label: 'Resources' },
            { key: 'buildings', label: 'Buildings' },
            { key: 'research', label: 'Research' },
            { key: 'fleet', label: 'Fleet' },
            { key: 'universe', label: 'Universe' },
            { key: 'alliance', label: 'Alliance' },
            { key: 'gameplay', label: 'Gameplay' },
            { key: 'notifications', label: 'Notifications' },
        ];

        container.innerHTML = sections
            .map((section) => {
                const segment = this.snapshot[section.key] || {};
                const count = Object.keys(segment).length;
                return `
                    <div class="snapshot-card">
                        <h4>${section.label}</h4>
                        <div class="snapshot-count">${count}</div>
                        <div class="snapshot-desc">parameters</div>
                    </div>
                `;
            })
            .join('');
    }

    showSnapshotModal() {
        if (!this.snapshot) {
            this.showToast('Snapshot unavailable.', 'error');
            return;
        }
        const pre = document.getElementById('snapshotJson');
        if (pre) {
            pre.textContent = JSON.stringify(this.snapshot, null, 2);
        }
        this.showModal('snapshotModal');
    }

    async loadCategoryParameters(category) {
        try {
            const data = await this.apiCall(`/categories/${category}`);
            this.parameters[category] = data.parameters;
            this.renderParameters(data.parameters);
        } catch (error) {
            this.showToast('Failed to load parameters: ' + error.message, 'error');
        }
    }

    async saveParameter(key, value, reason) {
        try {
            const result = await this.apiCall(`/parameters/${key}`, {
                method: 'PUT',
                body: JSON.stringify({ value, reason })
            });

            this.showToast('Parameter updated successfully', 'success');
            this.pendingChanges.delete(key);
            this.updateSaveButtonState();
            await this.loadStatistics();
            
            if (result.result.requires_restart) {
                this.showToast('Server restart required for this change to take effect', 'warning');
            }

            return true;
        } catch (error) {
            this.showToast('Failed to update parameter: ' + error.message, 'error');
            return false;
        }
    }

    async bulkSave() {
        if (this.pendingChanges.size === 0) return;

        const updates = Array.from(this.pendingChanges.entries()).map(([key, value]) => ({
            parameter_key: key,
            value
        }));

        const reason = await this.promptForReason('Bulk configuration update');
        if (reason === null) return;

        try {
            const result = await this.apiCall('/parameters/bulk-update', {
                method: 'POST',
                body: JSON.stringify({ updates, change_reason: reason })
            });

            if (result.success) {
                this.showToast(`Updated ${result.result.updated_count} parameters`, 'success');
                this.pendingChanges.clear();
                this.updateSaveButtonState();
                await this.loadCategoryParameters(this.currentCategory);
                await this.loadStatistics();

                if (result.result.requires_restart) {
                    this.showToast('Server restart required for some changes', 'warning');
                }
            }
        } catch (error) {
            this.showToast('Bulk update failed: ' + error.message, 'error');
        }
    }

    async resetCategory() {
        const confirmed = await this.confirm(
            'Reset Category',
            `Are you sure you want to reset all parameters in "${this.currentCategory}" to their default values? This cannot be undone.`
        );

        if (!confirmed) return;

        const reason = await this.promptForReason(`Reset ${this.currentCategory} to defaults`);
        if (reason === null) return;

        try {
            const result = await this.apiCall('/reset', {
                method: 'POST',
                body: JSON.stringify({ category: this.currentCategory, confirm: true })
            });

            this.showToast(result.message, 'success');
            this.pendingChanges.clear();
            await this.loadCategoryParameters(this.currentCategory);
            await this.loadStatistics();
        } catch (error) {
            this.showToast('Reset failed: ' + error.message, 'error');
        }
    }

    async refreshCache() {
        try {
            await this.apiCall('/cache/refresh', { method: 'POST' });
            this.showToast('Configuration cache refreshed', 'success');
        } catch (error) {
            this.showToast('Cache refresh failed: ' + error.message, 'error');
        }
    }

    async loadHistory() {
        try {
            const data = await this.apiCall('/history?limit=50');
            this.renderHistory(data.history);
            this.showModal('historyModal');
        } catch (error) {
            this.showToast('Failed to load history: ' + error.message, 'error');
        }
    }

    async rollbackChange(changeId) {
        const confirmed = await this.confirm(
            'Rollback Change',
            'Are you sure you want to rollback this configuration change?'
        );

        if (!confirmed) return;

        try {
            await this.apiCall(`/history/${changeId}/rollback`, { method: 'POST' });
            this.showToast('Configuration change rolled back', 'success');
            await this.loadHistory();
            await this.loadCategoryParameters(this.currentCategory);
            await this.loadStatistics();
        } catch (error) {
            this.showToast('Rollback failed: ' + error.message, 'error');
        }
    }

    async exportConfiguration() {
        try {
            const token = localStorage.getItem('token');
            const response = await fetch('/api/config/export?format=download', {
                headers: {
                    'Authorization': `Bearer ${token}`
                }
            });

            const blob = await response.blob();
            const url = window.URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url;
            a.download = `config_export_${Date.now()}.json`;
            document.body.appendChild(a);
            a.click();
            document.body.removeChild(a);
            window.URL.revokeObjectURL(url);

            this.showToast('Configuration exported successfully', 'success');
        } catch (error) {
            this.showToast('Export failed: ' + error.message, 'error');
        }
    }

    async importConfiguration(data, validateOnly = false) {
        try {
            const result = await this.apiCall('/import', {
                method: 'POST',
                body: JSON.stringify({ config: data, validate_only: validateOnly })
            });

            if (validateOnly) {
                return result.result;
            } else {
                this.showToast('Configuration imported successfully', 'success');
                await this.loadCategoryParameters(this.currentCategory);
                await this.loadStatistics();
                this.hideModal('importModal');
            }
        } catch (error) {
            this.showToast('Import failed: ' + error.message, 'error');
            return null;
        }
    }

    // UI Rendering Methods
    renderCategoryTabs() {
        const tabsContainer = document.getElementById('categoryTabs');
        tabsContainer.innerHTML = '';

        this.categories.forEach(category => {
            const tab = document.createElement('div');
            tab.className = 'tab' + (category.category_name === this.currentCategory ? ' active' : '');
            tab.textContent = category.display_name;
            tab.dataset.category = category.category_name;
            
            tab.addEventListener('click', () => {
                if (this.pendingChanges.size > 0) {
                    this.confirm(
                        'Unsaved Changes',
                        'You have unsaved changes. Do you want to discard them?'
                    ).then(confirmed => {
                        if (confirmed) {
                            this.pendingChanges.clear();
                            this.switchCategory(category.category_name);
                        }
                    });
                } else {
                    this.switchCategory(category.category_name);
                }
            });

            tabsContainer.appendChild(tab);
        });
    }

    switchCategory(category) {
        this.currentCategory = category;
        this.renderCategoryTabs();
        this.loadCategoryParameters(category);
    }

    renderParameters(parameters) {
        const container = document.getElementById('parametersList');
        container.innerHTML = '';

        if (parameters.length === 0) {
            container.innerHTML = '<p style="text-align:center;color:#8e8ea0;padding:40px;">No parameters in this category</p>';
            return;
        }

        parameters.forEach(param => {
            const item = this.createParameterItem(param);
            container.appendChild(item);
        });
    }

    createParameterItem(param) {
        const item = document.createElement('div');
        item.className = 'parameter-item';
        item.dataset.key = param.parameter_key;

        const rawValue = this.pendingChanges.get(param.parameter_key) ?? param.current_value;
        const currentValue = this.formatParameterValue(param, rawValue);
        const isModified = param.current_value !== param.default_value;
        const hasPendingChange = this.pendingChanges.has(param.parameter_key);

        if (isModified || hasPendingChange) {
            item.classList.add('modified');
        }

        item.innerHTML = `
            <div class="parameter-header">
                <div>
                    <div class="parameter-name">${param.parameter_name}</div>
                    <div class="parameter-key">${param.parameter_key}</div>
                </div>
                ${param.requires_restart ? '<span class="badge badge-warning">Restart Required</span>' : ''}
            </div>
            <div class="parameter-description">${param.description || 'No description'}</div>
            <div class="parameter-input-group">
                ${this.createInput(param, currentValue)}
                <div class="parameter-actions">
                    ${isModified ? '<button class="btn-sm btn-secondary" onclick="configManager.resetParameter(\'' + param.parameter_key + '\')">Reset</button>' : ''}
                </div>
            </div>
            ${param.min_value !== null || param.max_value !== null ? `
                <div class="parameter-hint">
                    Range: ${param.min_value ?? '-∞'} to ${param.max_value ?? '∞'}
                </div>
            ` : ''}
        `;

        const inputs = item.querySelectorAll('.parameter-input');
        if (inputs.length > 0) {
            const syncAndHandle = (event) => {
                const target = event.target;
                if (!target) return;
                inputs.forEach((inputEl) => {
                    if (inputEl !== target) {
                        inputEl.value = target.value;
                    }
                });
                this.handleParameterChange(param, target.value);
            };

            inputs.forEach((inputEl) => {
                inputEl.addEventListener('input', syncAndHandle);
                inputEl.addEventListener('change', syncAndHandle);
            });
        }

        return item;
    }

    formatParameterValue(param, value) {
        if (value === null || value === undefined) {
            return '';
        }

        if (param.data_type === 'number') {
            const numericValue = typeof value === 'number' ? value : parseFloat(value);
            if (isNaN(numericValue)) {
                return '';
            }
            if (param.parameter_key === 'gameplay.difficulty_factor') {
                return numericValue.toFixed(2);
            }
            return numericValue.toString();
        }

        if (param.data_type === 'boolean') {
            if (value === true) return 'true';
            if (value === false) return 'false';
        }

        return value;
    }

    createInput(param, value) {
        switch (param.data_type) {
            case 'boolean':
                return `
                    <select class="parameter-input">
                        <option value="true" ${value === 'true' ? 'selected' : ''}>Enabled</option>
                        <option value="false" ${value === 'false' ? 'selected' : ''}>Disabled</option>
                    </select>
                `;
            case 'number':
                if (param.min_value !== null && param.max_value !== null) {
                    return `
                        <input type="range" class="parameter-input" 
                               min="${param.min_value}" 
                               max="${param.max_value}" 
                               step="${this.getStep(param)}" 
                               value="${value}">
                        <input type="number" class="parameter-input" 
                               min="${param.min_value}" 
                               max="${param.max_value}" 
                               step="${this.getStep(param)}" 
                               value="${value}" 
                               style="width:100px;">
                    `;
                }
                return `<input type="number" class="parameter-input" value="${value}" step="any">`;
            default:
                return `<input type="text" class="parameter-input" value="${value}">`;
        }
    }

    getStep(param) {
        if (param.data_type !== 'number') return 'any';
        if (param.parameter_key === 'gameplay.difficulty_factor') {
            return '0.01';
        }
        const range = param.max_value - param.min_value;
        if (range <= 1) return '0.01';
        if (range <= 10) return '0.1';
        if (range <= 100) return '1';
        return '10';
    }

    handleParameterChange(param, value) {
        const key = param.parameter_key;
        const dataType = param.data_type;
        // Parse value based on data type
        let parsedValue = value;
        if (dataType === 'number') {
            parsedValue = parseFloat(value);
            if (isNaN(parsedValue)) {
                this.pendingChanges.delete(key);
                this.updateSaveButtonState();
                return;
            }
            if (param.min_value !== null && !isNaN(param.min_value)) {
                parsedValue = Math.max(param.min_value, parsedValue);
            }
            if (param.max_value !== null && !isNaN(param.max_value)) {
                parsedValue = Math.min(param.max_value, parsedValue);
            }
            if (param.parameter_key === 'gameplay.difficulty_factor') {
                parsedValue = Math.round(parsedValue * 100) / 100;
            }
        } else if (dataType === 'boolean') {
            parsedValue = value === 'true';
        }

        this.pendingChanges.set(key, parsedValue);
        this.updateSaveButtonState();
    }

    updateSaveButtonState() {
        const saveBtn = document.getElementById('saveChanges');
        const discardBtn = document.getElementById('discardChanges');
        const hasChanges = this.pendingChanges.size > 0;
        
        saveBtn.disabled = !hasChanges;
        discardBtn.disabled = !hasChanges;
    }

    resetParameter(key) {
        this.pendingChanges.delete(key);
        this.updateSaveButtonState();
        this.loadCategoryParameters(this.currentCategory);
    }

    discardChanges() {
        this.pendingChanges.clear();
        this.updateSaveButtonState();
        this.loadCategoryParameters(this.currentCategory);
    }

    renderHistory(history) {
        const container = document.getElementById('historyList');
        container.innerHTML = '';

        history.forEach(item => {
            const div = document.createElement('div');
            div.className = 'history-item';
            div.innerHTML = `
                <div class="history-header">
                    <div class="history-param">${item.parameter_name}</div>
                    <div class="history-time">${this.formatDateTime(item.applied_at)}</div>
                </div>
                <div class="history-change">
                    <div class="value-box value-old">${item.old_value}</div>
                    <div>→</div>
                    <div class="value-box value-new">${item.new_value}</div>
                </div>
                <div class="history-meta">
                    Changed by: ${item.changed_by_username || 'Unknown'} 
                    ${item.change_reason ? ' | Reason: ' + item.change_reason : ''}
                    ${item.is_rolled_back ? ' | <span style="color:#ff6b6b;">ROLLED BACK</span>' : ''}
                </div>
                ${!item.is_rolled_back ? `
                    <button class="btn-sm btn-danger" onclick="configManager.rollbackChange(${item.change_id})">
                        Rollback
                    </button>
                ` : ''}
            `;
            container.appendChild(div);
        });
    }

    // Socket.io Integration
    initializeSocket() {
        if (typeof io === 'undefined') return;

        const token = localStorage.getItem('token');
        this.socket = io({ auth: { token } });

        this.socket.on('config:changed', (data) => {
            this.showToast(`Configuration changed: ${data.key}`, 'info');
            
            // Reload if it's the current category
            const category = data.key.split('.')[0];
            if (category === this.currentCategory) {
                this.loadCategoryParameters(this.currentCategory);
            }
            
            this.loadStatistics();
        });

        this.socket.on('config:reload', () => {
            this.showToast('Configuration reloaded by another admin', 'info');
            this.loadCategoryParameters(this.currentCategory);
            this.loadStatistics();
        });
    }

    // Event Listeners
    setupEventListeners() {
        document.getElementById('saveChanges')?.addEventListener('click', () => this.bulkSave());
        document.getElementById('discardChanges')?.addEventListener('click', () => this.discardChanges());
        document.getElementById('resetCategory')?.addEventListener('click', () => this.resetCategory());
        document.getElementById('refreshCache')?.addEventListener('click', () => this.refreshCache());
        document.getElementById('exportConfig')?.addEventListener('click', () => this.exportConfiguration());
        document.getElementById('importConfig')?.addEventListener('click', () => this.showModal('importModal'));
        document.getElementById('viewHistory')?.addEventListener('click', () => this.loadHistory());

        const refreshSnapshotButtons = [
            document.getElementById('refreshSnapshot'),
            document.getElementById('snapshotRefreshInline'),
        ].filter(Boolean);
        refreshSnapshotButtons.forEach((btn) =>
            btn.addEventListener('click', () => this.loadGameConfigSnapshot(true))
        );

        const viewSnapshotButtons = [
            document.getElementById('viewSnapshot'),
            document.getElementById('snapshotViewJson'),
        ].filter(Boolean);
        viewSnapshotButtons.forEach((btn) =>
            btn.addEventListener('click', () => this.showSnapshotModal())
        );

        // Search
        document.getElementById('searchParams')?.addEventListener('input', (e) => {
            this.filterParameters(e.target.value);
        });

        // Modal close buttons
        document.querySelectorAll('.modal-close').forEach(btn => {
            btn.addEventListener('click', (e) => {
                const modal = e.target.closest('.modal');
                this.hideModal(modal.id);
            });
        });

        // Click outside modal to close
        document.querySelectorAll('.modal').forEach(modal => {
            modal.addEventListener('click', (e) => {
                if (e.target === modal) {
                    this.hideModal(modal.id);
                }
            });
        });

        // Import method radio buttons
        document.querySelectorAll('input[name="importMethod"]').forEach(radio => {
            radio.addEventListener('change', (e) => {
                document.getElementById('importFileSection').style.display = 
                    e.target.value === 'file' ? 'block' : 'none';
                document.getElementById('importPasteSection').style.display = 
                    e.target.value === 'paste' ? 'block' : 'none';
            });
        });

        // Import validation
        document.getElementById('validateImport')?.addEventListener('click', () => this.validateImport());
        document.getElementById('performImport')?.addEventListener('click', () => this.performImport());

        // Import file upload
        document.getElementById('importFile')?.addEventListener('change', (e) => {
            const file = e.target.files[0];
            if (file) {
                const reader = new FileReader();
                reader.onload = (e) => {
                    document.getElementById('importJSON').value = e.target.result;
                };
                reader.readAsText(file);
            }
        });
    }

    async validateImport() {
        try {
            const json = document.getElementById('importJSON').value;
            const data = JSON.parse(json);
            
            const result = await this.importConfiguration(data, true);
            
            const validationDiv = document.getElementById('importValidation');
            if (result.is_valid) {
                validationDiv.className = 'import-validation success';
                validationDiv.innerHTML = 'Validation successful. Ready to import.';
                document.getElementById('performImport').disabled = false;
            } else {
                validationDiv.className = 'import-validation error';
                validationDiv.innerHTML = `
                    <strong>Validation failed:</strong>
                    <ul>
                        ${result.errors.map(e => `<li>${e.parameter_key}: ${e.message}</li>`).join('')}
                    </ul>
                `;
                document.getElementById('performImport').disabled = true;
            }
        } catch (error) {
            document.getElementById('importValidation').className = 'import-validation error';
            document.getElementById('importValidation').textContent = 'Invalid JSON: ' + error.message;
            document.getElementById('performImport').disabled = true;
        }
    }

    async performImport() {
        try {
            const json = document.getElementById('importJSON').value;
            const data = JSON.parse(json);
            await this.importConfiguration(data, false);
        } catch (error) {
            this.showToast('Import failed: ' + error.message, 'error');
        }
    }

    filterParameters(searchTerm) {
        const items = document.querySelectorAll('.parameter-item');
        const term = searchTerm.toLowerCase();

        items.forEach(item => {
            const name = item.querySelector('.parameter-name').textContent.toLowerCase();
            const key = item.querySelector('.parameter-key').textContent.toLowerCase();
            const desc = item.querySelector('.parameter-description').textContent.toLowerCase();

            if (name.includes(term) || key.includes(term) || desc.includes(term)) {
                item.style.display = 'block';
            } else {
                item.style.display = 'none';
            }
        });
    }

    // Utility Methods
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

    showModal(modalId) {
        document.getElementById(modalId).classList.add('active');
    }

    hideModal(modalId) {
        document.getElementById(modalId).classList.remove('active');
    }

    showToast(message, type = 'info') {
        const toast = document.createElement('div');
        toast.className = `toast ${type}`;
        toast.textContent = message;
        document.body.appendChild(toast);

        setTimeout(() => {
            toast.remove();
        }, 5000);
    }

    async confirm(title, message) {
        return new Promise((resolve) => {
            document.getElementById('confirmTitle').textContent = title;
            document.getElementById('confirmMessage').textContent = message;
            document.getElementById('confirmReason').value = '';

            const modal = document.getElementById('confirmModal');
            this.showModal('confirmModal');

            const handleConfirm = () => {
                this.hideModal('confirmModal');
                cleanup();
                resolve(true);
            };

            const handleCancel = () => {
                this.hideModal('confirmModal');
                cleanup();
                resolve(false);
            };

            const cleanup = () => {
                document.getElementById('confirmOk').removeEventListener('click', handleConfirm);
                document.getElementById('confirmCancel').removeEventListener('click', handleCancel);
            };

            document.getElementById('confirmOk').addEventListener('click', handleConfirm);
            document.getElementById('confirmCancel').addEventListener('click', handleCancel);
        });
    }

    async promptForReason(defaultReason) {
        const confirmed = await this.confirm('Provide Reason', 'Please provide a reason for this change:');
        if (!confirmed) return null;
        
        const reason = document.getElementById('confirmReason').value || defaultReason;
        return reason;
    }
}

// Initialize on page load
let configManager;
document.addEventListener('DOMContentLoaded', () => {
    configManager = new ConfigurationManager();
});
