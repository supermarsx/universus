// @ts-nocheck
/**
 * User Theme Preferences Component
 * Allows users to customize theme experience
 */

class ThemePreferences {
    constructor() {
        this.preferences = null;
        this.apiUrl = '/api/themes/user/preferences';
        this.customCssApiUrl = '/api/themes/user/custom-css';
        this.customCss = '';
        this.maxCustomCssLength = 8000;
    }

    /**
     * Initialize preferences UI
     */
    async init() {
        await this.loadPreferences();
        await this.loadCustomCSS();
        this.setupEventListeners();
    }

    /**
     * Load user preferences from API
     */
    async loadPreferences() {
        try {
            const token = localStorage.getItem('token');
            if (!token) {
                console.warn('[ThemePreferences] No token found');
                return;
            }

            const response = await fetch(this.apiUrl, {
                headers: {
                    'Authorization': `Bearer ${token}`
                }
            });

            if (response.ok) {
                const data = await response.json();
                if (data.success) {
                    this.preferences = data.preferences;
                    this.updateUI();
                }
            }
        } catch (error) {
            console.error('[ThemePreferences] Error loading preferences:', error);
        }
    }

    /**
     * Update UI with current preferences
     */
    updateUI() {
        if (!this.preferences) return;

        const enabledCheckbox = document.getElementById('theme-enabled');
        const visualEffectsCheckbox = document.getElementById('visual-effects-enabled');
        const soundEffectsCheckbox = document.getElementById('sound-effects-enabled');
        const animationsCheckbox = document.getElementById('animations-enabled');
        const decorationsCheckbox = document.getElementById('decorations-enabled');
        const reduceMotionCheckbox = document.getElementById('reduce-motion');
        const effectIntensitySlider = document.getElementById('effect-intensity');
        const soundVolumeSlider = document.getElementById('sound-volume');
        const animationSpeedSlider = document.getElementById('animation-speed');

        if (enabledCheckbox) enabledCheckbox.checked = this.preferences.enabled;
        if (visualEffectsCheckbox) visualEffectsCheckbox.checked = this.preferences.enable_visual_effects;
        if (soundEffectsCheckbox) soundEffectsCheckbox.checked = this.preferences.enable_sound_effects;
        if (animationsCheckbox) animationsCheckbox.checked = this.preferences.enable_animations;
        if (decorationsCheckbox) decorationsCheckbox.checked = this.preferences.enable_decorations;
        if (reduceMotionCheckbox) reduceMotionCheckbox.checked = this.preferences.reduce_motion;
        if (effectIntensitySlider) {
            effectIntensitySlider.value = this.preferences.effect_intensity;
            document.getElementById('effect-intensity-value').textContent = this.preferences.effect_intensity + '%';
        }
        if (soundVolumeSlider) {
            soundVolumeSlider.value = this.preferences.sound_volume;
            document.getElementById('sound-volume-value').textContent = this.preferences.sound_volume + '%';
        }
        if (animationSpeedSlider) {
            animationSpeedSlider.value = this.preferences.animation_speed;
            document.getElementById('animation-speed-value').textContent = this.preferences.animation_speed + '%';
        }
    }

    /**
     * Setup event listeners
     */
    setupEventListeners() {
        // Save button
        const saveButton = document.getElementById('save-theme-preferences');
        if (saveButton) {
            saveButton.addEventListener('click', () => this.savePreferences());
        }

        // Sliders - update display values
        const effectIntensitySlider = document.getElementById('effect-intensity');
        if (effectIntensitySlider) {
            effectIntensitySlider.addEventListener('input', (e) => {
                document.getElementById('effect-intensity-value').textContent = e.target.value + '%';
            });
        }

        const soundVolumeSlider = document.getElementById('sound-volume');
        if (soundVolumeSlider) {
            soundVolumeSlider.addEventListener('input', (e) => {
                document.getElementById('sound-volume-value').textContent = e.target.value + '%';
                
                // Update volume in real-time if theme loader exists
                if (window.themeLoader && window.themeLoader.backgroundMusic) {
                    window.themeLoader.backgroundMusic.volume = e.target.value / 100;
                }
            });
        }

        const animationSpeedSlider = document.getElementById('animation-speed');
        if (animationSpeedSlider) {
            animationSpeedSlider.addEventListener('input', (e) => {
                document.getElementById('animation-speed-value').textContent = e.target.value + '%';
            });
        }

        const customCssInput = document.getElementById('customCssInput') as HTMLTextAreaElement | null;
        if (customCssInput) {
            customCssInput.addEventListener('input', (e) => {
                this.updateCustomCssCounter(e.target.value.length);
            });
        }

        const saveCustomCssButton = document.getElementById('save-custom-css');
        if (saveCustomCssButton) {
            saveCustomCssButton.addEventListener('click', () => this.saveCustomCSS());
        }

        const clearCustomCssButton = document.getElementById('clear-custom-css');
        if (clearCustomCssButton) {
            clearCustomCssButton.addEventListener('click', () => this.clearCustomCSS());
        }

        // Theme enabled toggle - immediate effect
        const enabledCheckbox = document.getElementById('theme-enabled');
        if (enabledCheckbox) {
            enabledCheckbox.addEventListener('change', (e) => {
                if (window.themeLoader) {
                    if (!e.target.checked) {
                        window.themeLoader.clearTheme();
                    } else {
                        window.themeLoader.loadCurrentTheme();
                    }
                }
            });
        }

        // Visual effects toggle
        const visualEffectsCheckbox = document.getElementById('visual-effects-enabled');
        if (visualEffectsCheckbox) {
            visualEffectsCheckbox.addEventListener('change', (e) => {
                if (window.themeLoader) {
                    window.themeLoader.toggleEffects(e.target.checked);
                }
            });
        }

        // Sound effects toggle
        const soundEffectsCheckbox = document.getElementById('sound-effects-enabled');
        if (soundEffectsCheckbox) {
            soundEffectsCheckbox.addEventListener('change', (e) => {
                if (window.themeLoader) {
                    window.themeLoader.toggleSounds(e.target.checked);
                }
            });
        }
    }

    /**
     * Load custom CSS snippet
     */
    async loadCustomCSS() {
        try {
            const token = localStorage.getItem('token');
            if (!token) return;

            const response = await fetch(this.customCssApiUrl, {
                headers: {
                    'Authorization': `Bearer ${token}`
                }
            });

            if (!response.ok) return;

            const data = await response.json();
            if (data.success) {
                this.customCss = data.customCSS || '';
                this.updateCustomCssUI();
            }
        } catch (error) {
            console.error('[ThemePreferences] Error loading custom CSS:', error);
        }
    }

    updateCustomCssUI() {
        const textarea = document.getElementById('customCssInput') as HTMLTextAreaElement | null;
        if (textarea) {
            textarea.value = this.customCss || '';
        }
        this.updateCustomCssCounter((this.customCss || '').length);
    }

    updateCustomCssCounter(length: number) {
        const counter = document.getElementById('custom-css-counter');
        if (counter) {
            counter.textContent = `${length}/${this.maxCustomCssLength}`;
            counter.classList.toggle('text-error', length > this.maxCustomCssLength);
        }
    }

    async saveCustomCSS() {
        try {
            const token = localStorage.getItem('token');
            if (!token) {
                this.showNotification('You must be logged in to save custom CSS.', 'error');
                return;
            }

            const textarea = document.getElementById('customCssInput') as HTMLTextAreaElement | null;
            const css = textarea?.value || '';

            if (css.length > this.maxCustomCssLength) {
                this.showNotification(`Custom CSS is limited to ${this.maxCustomCssLength} characters.`, 'error');
                return;
            }

            const response = await fetch(this.customCssApiUrl, {
                method: 'PUT',
                headers: {
                    'Content-Type': 'application/json',
                    'Authorization': `Bearer ${token}`
                },
                body: JSON.stringify({ css })
            });

            const data = await response.json();

            if (!response.ok || !data.success) {
                throw new Error(data?.message || 'Failed to save custom CSS');
            }

            this.customCss = data.customCSS || '';
            this.updateCustomCssUI();
            this.showNotification(data.message || 'Custom CSS saved.', 'success');

            window.dispatchEvent(new CustomEvent('userCustomCssUpdated', {
                detail: { css: this.customCss }
            }));
        } catch (error) {
            console.error('[ThemePreferences] Error saving custom CSS:', error);
            const message = error instanceof Error ? error.message : 'Failed to save custom CSS';
            this.showNotification(message, 'error');
        }
    }

    async clearCustomCSS() {
        const textarea = document.getElementById('customCssInput') as HTMLTextAreaElement | null;
        if (textarea) {
            textarea.value = '';
        }
        this.customCss = '';
        await this.saveCustomCSS();
    }

    /**
     * Save preferences to API
     */
    async savePreferences() {
        try {
            const token = localStorage.getItem('token');
            if (!token) {
                alert('Please log in to save preferences');
                return;
            }

            const preferencesData = {
                enabled: document.getElementById('theme-enabled')?.checked ?? true,
                enable_visual_effects: document.getElementById('visual-effects-enabled')?.checked ?? true,
                enable_sound_effects: document.getElementById('sound-effects-enabled')?.checked ?? true,
                enable_animations: document.getElementById('animations-enabled')?.checked ?? true,
                enable_decorations: document.getElementById('decorations-enabled')?.checked ?? true,
                reduce_motion: document.getElementById('reduce-motion')?.checked ?? false,
                effect_intensity: parseInt(document.getElementById('effect-intensity')?.value ?? 100),
                sound_volume: parseInt(document.getElementById('sound-volume')?.value ?? 50),
                animation_speed: parseInt(document.getElementById('animation-speed')?.value ?? 100)
            };

            const response = await fetch(this.apiUrl, {
                method: 'PUT',
                headers: {
                    'Authorization': `Bearer ${token}`,
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify(preferencesData)
            });

            if (response.ok) {
                const data = await response.json();
                if (data.success) {
                    this.preferences = data.preferences;
                    
                    // Notify theme loader
                    window.dispatchEvent(new CustomEvent('themePreferencesChanged', {
                        detail: { preferences: data.preferences }
                    }));

                    this.showNotification('Preferences saved successfully!', 'success');
                } else {
                    this.showNotification('Error saving preferences: ' + data.message, 'error');
                }
            } else {
                this.showNotification('Error saving preferences', 'error');
            }
        } catch (error) {
            console.error('[ThemePreferences] Error saving preferences:', error);
            this.showNotification('Error saving preferences', 'error');
        }
    }

    /**
     * Show notification
     */
    showNotification(message, type) {
        const notification = document.createElement('div');
        notification.className = `theme-notification ${type}`;
        notification.textContent = message;
        document.body.appendChild(notification);

        setTimeout(() => {
            notification.style.animation = 'slide-out-right 0.5s ease-out';
            setTimeout(() => notification.remove(), 500);
        }, 3000);
    }

    /**
     * Reset to defaults
     */
    async resetToDefaults() {
        if (!confirm('Reset all theme preferences to default values?')) return;

        const defaultPreferences = {
            enabled: true,
            enable_visual_effects: true,
            enable_sound_effects: true,
            enable_animations: true,
            enable_decorations: true,
            reduce_motion: false,
            effect_intensity: 100,
            sound_volume: 50,
            animation_speed: 100
        };

        try {
            const token = localStorage.getItem('token');
            const response = await fetch(this.apiUrl, {
                method: 'PUT',
                headers: {
                    'Authorization': `Bearer ${token}`,
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify(defaultPreferences)
            });

            if (response.ok) {
                const data = await response.json();
                if (data.success) {
                    this.preferences = data.preferences;
                    this.updateUI();
                    this.showNotification('Preferences reset to defaults', 'success');
                    
                    // Reload theme
                    if (window.themeLoader) {
                        window.themeLoader.loadCurrentTheme();
                    }
                }
            }
        } catch (error) {
            console.error('[ThemePreferences] Error resetting preferences:', error);
            this.showNotification('Error resetting preferences', 'error');
        }
    }
}

// Create global instance
window.themePreferences = new ThemePreferences();

// Initialize if preferences UI exists
document.addEventListener('DOMContentLoaded', () => {
    if (document.getElementById('theme-preferences-section')) {
        window.themePreferences.init();
    }
});

// Add CSS for notification animations
const style = document.createElement('style');
style.textContent = `
    @keyframes slide-out-right {
        0% {
            transform: translateX(0);
            opacity: 1;
        }
        100% {
            transform: translateX(400px);
            opacity: 0;
        }
    }
    
    .theme-notification.success {
        background: #28a745;
    }
    
    .theme-notification.error {
        background: #dc3545;
    }
`;
document.head.appendChild(style);
