// @ts-nocheck
/**
 * Account Settings Interface
 * Handles profile, notification, display, and game preferences
 */

class AccountSettingsManager {
    constructor() {
        this.api = {
            profile: '/api/account/profile',
            settings: '/api/account/settings',
            info: '/api/account/info'
        };
        this.init();
    }

    init() {
        this.loadAccountInfo();
        this.loadProfileSettings();
        this.loadNotificationSettings();
        this.loadDisplaySettings();
        this.loadGamePreferences();
        this.attachEventListeners();
    }

    /**
     * Attach event listeners
     */
    attachEventListeners() {
        // Profile form
        const profileForm = document.getElementById('profileForm');
        if (profileForm) {
            profileForm.addEventListener('submit', (e) => {
                e.preventDefault();
                this.saveProfile();
            });
        }

        // Notifications form
        const notificationsForm = document.getElementById('notificationsForm');
        if (notificationsForm) {
            notificationsForm.addEventListener('submit', (e) => {
                e.preventDefault();
                this.saveNotificationSettings();
            });
        }

        // Display form
        const displayForm = document.getElementById('displayForm');
        if (displayForm) {
            displayForm.addEventListener('submit', (e) => {
                e.preventDefault();
                this.saveDisplaySettings();
            });
        }

        // Game preferences form
        const gamePrefsForm = document.getElementById('gamePrefsForm');
        if (gamePrefsForm) {
            gamePrefsForm.addEventListener('submit', (e) => {
                e.preventDefault();
                this.saveGamePreferences();
            });
        }

        // Theme change listener
        const themeSelect = document.getElementById('theme');
        if (themeSelect) {
            themeSelect.addEventListener('change', (e) => {
                this.applyTheme(e.target.value);
            });
        }
    }

    /**
     * Load account information
     */
    async loadAccountInfo() {
        try {
            const response = await fetch(this.api.info, {
                headers: {
                    'Authorization': `Bearer ${this.getToken()}`
                }
            });

            if (!response.ok) {
                throw new Error('Failed to load account info');
            }

            const data = await response.json();
            this.displayAccountInfo(data);

        } catch (error) {
            console.error('Error loading account info:', error);
        }
    }

    /**
     * Display account information
     */
    displayAccountInfo(data) {
        if (data.createdAt) {
            document.getElementById('accountCreated').textContent = 
                new Date(data.createdAt).toLocaleDateString();
        }

        if (data.lastLogin) {
            document.getElementById('lastLogin').textContent = 
                this.formatRelativeTime(new Date(data.lastLogin));
        }

        if (data.totalPlaytime) {
            document.getElementById('totalPlaytime').textContent = 
                this.formatPlaytime(data.totalPlaytime);
        }

        if (data.status) {
            const statusBadge = document.getElementById('accountStatus');
            statusBadge.textContent = data.status;
            statusBadge.className = `badge badge-${this.getStatusClass(data.status)}`;
        }
    }

    /**
     * Load profile settings
     */
    async loadProfileSettings() {
        try {
            const response = await fetch(this.api.profile, {
                headers: {
                    'Authorization': `Bearer ${this.getToken()}`
                }
            });

            if (!response.ok) {
                throw new Error('Failed to load profile');
            }

            const data = await response.json();
            this.applyProfileData(data);

        } catch (error) {
            console.error('Error loading profile:', error);
        }
    }

    /**
     * Apply profile data to form
     */
    applyProfileData(data) {
        if (data.username) {
            document.getElementById('username').value = data.username;
        }
        if (data.displayName) {
            document.getElementById('displayName').value = data.displayName;
        }
        if (data.email) {
            document.getElementById('email').value = data.email;
        }
        if (data.timezone) {
            document.getElementById('timezone').value = data.timezone;
        }
        if (data.language) {
            document.getElementById('language').value = data.language;
        }
    }

    /**
     * Save profile
     */
    async saveProfile() {
        const profileData = {
            username: document.getElementById('username').value.trim(),
            displayName: document.getElementById('displayName').value.trim(),
            timezone: document.getElementById('timezone').value,
            language: document.getElementById('language').value
        };

        if (!profileData.username) {
            this.showError('Username is required');
            return;
        }

        const btn = document.querySelector('#profileForm button[type="submit"]');
        const originalText = btn.innerHTML;
        btn.disabled = true;
        btn.innerHTML = '<span class="spinner"></span> Saving...';

        try {
            const response = await fetch(this.api.profile, {
                method: 'PUT',
                headers: {
                    'Authorization': `Bearer ${this.getToken()}`,
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify(profileData)
            });

            if (!response.ok) {
                const error = await response.json();
                throw new Error(error.message || 'Failed to save profile');
            }

            this.showSuccess('Profile updated successfully');

        } catch (error) {
            console.error('Error saving profile:', error);
            this.showError(error.message);
        } finally {
            btn.disabled = false;
            btn.innerHTML = originalText;
        }
    }

    /**
     * Load notification settings
     */
    async loadNotificationSettings() {
        try {
            const response = await fetch(`${this.api.settings}/notifications`, {
                headers: {
                    'Authorization': `Bearer ${this.getToken()}`
                }
            });

            if (!response.ok) return;

            const data = await response.json();
            this.applyNotificationSettings(data);

        } catch (error) {
            console.error('Error loading notification settings:', error);
        }
    }

    /**
     * Apply notification settings
     */
    applyNotificationSettings(settings) {
        // Game notifications
        if (settings.notifyBuildings !== undefined) {
            document.getElementById('notifyBuildings').checked = settings.notifyBuildings;
        }
        if (settings.notifyResearch !== undefined) {
            document.getElementById('notifyResearch').checked = settings.notifyResearch;
        }
        if (settings.notifyFleet !== undefined) {
            document.getElementById('notifyFleet').checked = settings.notifyFleet;
        }
        if (settings.notifyAttack !== undefined) {
            document.getElementById('notifyAttack').checked = settings.notifyAttack;
        }

        // Email notifications
        if (settings.emailSecurity !== undefined) {
            document.getElementById('emailSecurity').checked = settings.emailSecurity;
        }
        if (settings.emailUpdates !== undefined) {
            document.getElementById('emailUpdates').checked = settings.emailUpdates;
        }
        if (settings.emailMarketing !== undefined) {
            document.getElementById('emailMarketing').checked = settings.emailMarketing;
        }
    }

    /**
     * Save notification settings
     */
    async saveNotificationSettings() {
        const settings = {
            notifyBuildings: document.getElementById('notifyBuildings').checked,
            notifyResearch: document.getElementById('notifyResearch').checked,
            notifyFleet: document.getElementById('notifyFleet').checked,
            notifyAttack: document.getElementById('notifyAttack').checked,
            emailSecurity: document.getElementById('emailSecurity').checked,
            emailUpdates: document.getElementById('emailUpdates').checked,
            emailMarketing: document.getElementById('emailMarketing').checked
        };

        const btn = document.querySelector('#notificationsForm button[type="submit"]');
        const originalText = btn.innerHTML;
        btn.disabled = true;
        btn.innerHTML = '<span class="spinner"></span> Saving...';

        try {
            const response = await fetch(`${this.api.settings}/notifications`, {
                method: 'PUT',
                headers: {
                    'Authorization': `Bearer ${this.getToken()}`,
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify(settings)
            });

            if (!response.ok) {
                const error = await response.json();
                throw new Error(error.message || 'Failed to save notification settings');
            }

            this.showSuccess('Notification settings updated successfully');

        } catch (error) {
            console.error('Error saving notification settings:', error);
            this.showError(error.message);
        } finally {
            btn.disabled = false;
            btn.innerHTML = originalText;
        }
    }

    /**
     * Load display settings
     */
    async loadDisplaySettings() {
        try {
            const response = await fetch(`${this.api.settings}/display`, {
                headers: {
                    'Authorization': `Bearer ${this.getToken()}`
                }
            });

            if (!response.ok) return;

            const data = await response.json();
            this.applyDisplaySettings(data);

        } catch (error) {
            console.error('Error loading display settings:', error);
        }
    }

    /**
     * Apply display settings
     */
    applyDisplaySettings(settings) {
        if (settings.theme) {
            document.getElementById('theme').value = settings.theme;
            this.applyTheme(settings.theme);
        }
        if (settings.numberFormat) {
            document.getElementById('numberFormat').value = settings.numberFormat;
        }
        if (settings.dateFormat) {
            document.getElementById('dateFormat').value = settings.dateFormat;
        }
        if (settings.animations !== undefined) {
            document.getElementById('animations').checked = settings.animations;
        }
        if (settings.soundEffects !== undefined) {
            document.getElementById('soundEffects').checked = settings.soundEffects;
        }
    }

    /**
     * Save display settings
     */
    async saveDisplaySettings() {
        const settings = {
            theme: document.getElementById('theme').value,
            numberFormat: document.getElementById('numberFormat').value,
            dateFormat: document.getElementById('dateFormat').value,
            animations: document.getElementById('animations').checked,
            soundEffects: document.getElementById('soundEffects').checked
        };

        const btn = document.querySelector('#displayForm button[type="submit"]');
        const originalText = btn.innerHTML;
        btn.disabled = true;
        btn.innerHTML = '<span class="spinner"></span> Saving...';

        try {
            const response = await fetch(`${this.api.settings}/display`, {
                method: 'PUT',
                headers: {
                    'Authorization': `Bearer ${this.getToken()}`,
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify(settings)
            });

            if (!response.ok) {
                const error = await response.json();
                throw new Error(error.message || 'Failed to save display settings');
            }

            this.showSuccess('Display settings updated successfully');
            this.applyTheme(settings.theme);

        } catch (error) {
            console.error('Error saving display settings:', error);
            this.showError(error.message);
        } finally {
            btn.disabled = false;
            btn.innerHTML = originalText;
        }
    }

    /**
     * Load game preferences
     */
    async loadGamePreferences() {
        try {
            const response = await fetch(`${this.api.settings}/game`, {
                headers: {
                    'Authorization': `Bearer ${this.getToken()}`
                }
            });

            if (!response.ok) return;

            const data = await response.json();
            this.applyGamePreferences(data);

        } catch (error) {
            console.error('Error loading game preferences:', error);
        }
    }

    /**
     * Apply game preferences
     */
    applyGamePreferences(preferences) {
        if (preferences.defaultGalaxyView) {
            document.getElementById('defaultGalaxyView').value = preferences.defaultGalaxyView;
        }
        if (preferences.autoRefresh !== undefined) {
            document.getElementById('autoRefresh').checked = preferences.autoRefresh;
        }
        if (preferences.confirmActions !== undefined) {
            document.getElementById('confirmActions').checked = preferences.confirmActions;
        }
        if (preferences.quickBuild !== undefined) {
            document.getElementById('quickBuild').checked = preferences.quickBuild;
        }
    }

    /**
     * Save game preferences
     */
    async saveGamePreferences() {
        const preferences = {
            defaultGalaxyView: document.getElementById('defaultGalaxyView').value,
            autoRefresh: document.getElementById('autoRefresh').checked,
            confirmActions: document.getElementById('confirmActions').checked,
            quickBuild: document.getElementById('quickBuild').checked
        };

        const btn = document.querySelector('#gamePrefsForm button[type="submit"]');
        const originalText = btn.innerHTML;
        btn.disabled = true;
        btn.innerHTML = '<span class="spinner"></span> Saving...';

        try {
            const response = await fetch(`${this.api.settings}/game`, {
                method: 'PUT',
                headers: {
                    'Authorization': `Bearer ${this.getToken()}`,
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify(preferences)
            });

            if (!response.ok) {
                const error = await response.json();
                throw new Error(error.message || 'Failed to save game preferences');
            }

            this.showSuccess('Game preferences updated successfully');

        } catch (error) {
            console.error('Error saving game preferences:', error);
            this.showError(error.message);
        } finally {
            btn.disabled = false;
            btn.innerHTML = originalText;
        }
    }

    /**
     * Apply theme
     */
    applyTheme(theme) {
        const body = document.body;
        body.classList.remove('theme-light', 'theme-dark');

        if (theme === 'auto') {
            const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
            theme = prefersDark ? 'dark' : 'light';
        }

        body.classList.add(`theme-${theme}`);
        localStorage.setItem('theme', theme);
    }

    /**
     * Format relative time
     */
    formatRelativeTime(date) {
        const now = new Date();
        const diff = now - date;
        const seconds = Math.floor(diff / 1000);
        const minutes = Math.floor(seconds / 60);
        const hours = Math.floor(minutes / 60);
        const days = Math.floor(hours / 24);

        if (days > 0) return `${days} day${days > 1 ? 's' : ''} ago`;
        if (hours > 0) return `${hours} hour${hours > 1 ? 's' : ''} ago`;
        if (minutes > 0) return `${minutes} minute${minutes > 1 ? 's' : ''} ago`;
        return 'Just now';
    }

    /**
     * Format playtime
     */
    formatPlaytime(seconds) {
        const hours = Math.floor(seconds / 3600);
        const days = Math.floor(hours / 24);
        
        if (days > 0) {
            const remainingHours = hours % 24;
            return `${days}d ${remainingHours}h`;
        }
        
        const minutes = Math.floor((seconds % 3600) / 60);
        return `${hours}h ${minutes}m`;
    }

    /**
     * Get status badge class
     */
    getStatusClass(status) {
        const classes = {
            'active': 'success',
            'suspended': 'warning',
            'locked': 'danger',
            'pending': 'info'
        };
        return classes[status?.toLowerCase()] || 'default';
    }

    /**
     * Show success message
     */
    showSuccess(message) {
        // Simple alert for now, could be replaced with toast notification
        if (window.toast) { window.toast.success(message); } else { window.toast ? window.toast.success(message) : alert(message); }
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
}

// Initialize on page load
document.addEventListener('DOMContentLoaded', () => {
    new AccountSettingsManager();
});
