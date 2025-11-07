// @ts-nocheck
/**
 * Theme Loader - Loads and applies seasonal themes dynamically
 * Part of Phase 8: Seasonal Theme System
 */

class ThemeLoader {
    constructor() {
        this.currentTheme = null;
        this.activeEffects = [];
        this.checkInterval = 5 * 60 * 1000; // Check every 5 minutes
        this.effectsEnabled = true;
        this.soundsEnabled = true;
    }

    /**
     * Initialize theme loader
     */
    async init() {
        console.log('[ThemeLoader] Initializing...');
        
        // Load user preferences
        await this.loadUserPreferences();
        
        // Load current theme
        await this.loadCurrentTheme();
        
        // Set up periodic checking
        setInterval(() => this.loadCurrentTheme(), this.checkInterval);
        
        // Listen for theme change events
        this.setupEventListeners();
        
        console.log('[ThemeLoader] Initialized successfully');
    }

    /**
     * Load user preferences from API
     */
    async loadUserPreferences() {
        try {
            const token = localStorage.getItem('token');
            if (!token) return;

            const response = await fetch('/api/themes/user/preferences', {
                headers: {
                    'Authorization': `Bearer ${token}`
                }
            });

            if (response.ok) {
                const data = await response.json();
                if (data.success && data.preferences) {
                    this.applyUserPreferences(data.preferences);
                }
            }
        } catch (error) {
            console.warn('[ThemeLoader] Failed to load preferences:', error);
        }
    }

    /**
     * Apply user preferences
     */
    applyUserPreferences(preferences) {
        this.effectsEnabled = preferences.enabled && preferences.enable_visual_effects;
        this.soundsEnabled = preferences.enabled && preferences.enable_sound_effects;
        
        if (preferences.reduce_motion) {
            document.documentElement.classList.add('reduce-motion');
        }
    }

    /**
     * Load current active theme
     */
    async loadCurrentTheme() {
        try {
            const response = await fetch('/api/themes/current');
            const data = await response.json();

            if (data.success && data.theme) {
                const themeChanged = !this.currentTheme || 
                                   this.currentTheme.id !== data.theme.id;

                if (themeChanged) {
                    console.log(`[ThemeLoader] Switching to theme: ${data.theme.name}`);
                    await this.applyTheme(data.theme, data.cssVariables, data.customCSS, data.assets);
                }
            } else {
                // No active theme, clear current theme
                if (this.currentTheme) {
                    console.log('[ThemeLoader] No active theme, clearing...');
                    this.clearTheme();
                }
            }
        } catch (error) {
            console.error('[ThemeLoader] Failed to load theme:', error);
        }
    }

    /**
     * Apply theme to page
     */
    async applyTheme(theme, cssVariables, customCSS, assets) {
        // Store current theme
        const previousTheme = this.currentTheme;
        this.currentTheme = theme;

        // Apply transition
        if (previousTheme) {
            document.body.classList.add('theme-transitioning');
        }

        // Apply CSS variables
        this.applyCSSVariables(cssVariables);

        // Inject custom CSS
        this.injectCustomCSS(customCSS);

        // Apply visual effects
        if (this.effectsEnabled) {
            await this.applyVisualEffects(theme.visual_effects);
        }

        // Apply decorations
        if (this.effectsEnabled) {
            this.applyDecorations(theme.decorations, assets);
        }

        // Load sound effects
        if (this.soundsEnabled) {
            this.loadSoundEffects(theme.sound_effects);
        }

        // Update theme class on body
        document.body.className = document.body.className
            .replace(/theme-\w+/g, '')
            .trim();
        document.body.classList.add(`theme-${theme.theme_key}`);

        // Remove transition class after animation
        setTimeout(() => {
            document.body.classList.remove('theme-transitioning');
        }, 1000);

        // Dispatch event
        window.dispatchEvent(new CustomEvent('themeChanged', {
            detail: { theme, previousTheme }
        }));

        console.log(`[ThemeLoader] Theme applied: ${theme.name}`);
    }

    /**
     * Apply CSS variables
     */
    applyCSSVariables(variables) {
        if (!variables) return;

        Object.entries(variables).forEach(([key, value]) => {
            document.documentElement.style.setProperty(key, value);
        });
    }

    /**
     * Inject custom CSS
     */
    injectCustomCSS(css) {
        // Remove old custom CSS
        const oldStyle = document.getElementById('theme-custom-css');
        if (oldStyle) {
            oldStyle.remove();
        }

        // Inject new CSS
        if (css) {
            const styleEl = document.createElement('style');
            styleEl.id = 'theme-custom-css';
            styleEl.textContent = css;
            document.head.appendChild(styleEl);
        }
    }

    /**
     * Apply visual effects
     */
    async applyVisualEffects(effects) {
        // Clear existing effects
        this.clearEffects();

        if (!effects) return;

        // Load effect modules dynamically
        if (effects.snow?.enabled) {
            await this.createSnowEffect(effects.snow);
        }

        if (effects.fireworks?.enabled) {
            await this.createFireworksEffect(effects.fireworks);
        }

        if (effects.confetti?.enabled) {
            await this.createConfettiEffect(effects.confetti);
        }

        if (effects.fog?.enabled) {
            await this.createFogEffect(effects.fog);
        }

        if (effects.butterflies?.enabled) {
            await this.createButterfliesEffect(effects.butterflies);
        }

        if (effects.bats?.enabled) {
            await this.createBatsEffect(effects.bats);
        }

        if (effects.lights?.enabled) {
            await this.createLightsEffect(effects.lights);
        }
    }

    /**
     * Create snow effect
     */
    async createSnowEffect(config) {
        const container = this.createEffectContainer('snow-effect');
        const flakeCount = config.flakeCount || 100;

        for (let i = 0; i < flakeCount; i++) {
            const flake = document.createElement('div');
            flake.className = 'snowflake';
            flake.textContent = '❄';
            flake.style.left = Math.random() * 100 + '%';
            flake.style.animationDuration = (Math.random() * 3 + 2) + 's';
            flake.style.animationDelay = Math.random() * 5 + 's';
            flake.style.fontSize = (Math.random() * 10 + 10) + 'px';
            flake.style.opacity = Math.random() * 0.6 + 0.4;
            container.appendChild(flake);
        }

        this.activeEffects.push({ type: 'snow', container });
    }

    /**
     * Create fireworks effect
     */
    async createFireworksEffect(config) {
        const container = this.createEffectContainer('fireworks-effect');
        
        const launchFirework = () => {
            const firework = document.createElement('div');
            firework.className = 'firework';
            firework.style.left = (Math.random() * 80 + 10) + '%';
            firework.style.top = (Math.random() * 50 + 25) + '%';
            
            const colors = config.colors || ['#ff0000', '#00ff00', '#0000ff', '#ffff00'];
            const color = colors[Math.floor(Math.random() * colors.length)];
            
            for (let i = 0; i < 30; i++) {
                const particle = document.createElement('div');
                particle.className = 'firework-particle';
                particle.style.background = color;
                const angle = (Math.PI * 2 * i) / 30;
                particle.style.setProperty('--x', Math.cos(angle) * 100);
                particle.style.setProperty('--y', Math.sin(angle) * 100);
                firework.appendChild(particle);
            }
            
            container.appendChild(firework);
            
            setTimeout(() => firework.remove(), 2000);
        };

        const frequency = config.frequency === 'high' ? 2000 : 
                         config.frequency === 'medium' ? 4000 : 6000;
        
        const interval = setInterval(launchFirework, frequency);
        this.activeEffects.push({ type: 'fireworks', container, interval });
    }

    /**
     * Create confetti effect
     */
    async createConfettiEffect(config) {
        const container = this.createEffectContainer('confetti-effect');
        const colors = config.colors || ['#ff0000', '#00ff00', '#0000ff', '#ffff00', '#ff00ff'];
        const count = config.intensity === 'high' ? 150 : 
                     config.intensity === 'medium' ? 100 : 50;

        for (let i = 0; i < count; i++) {
            const confetti = document.createElement('div');
            confetti.className = 'confetti';
            confetti.style.left = Math.random() * 100 + '%';
            confetti.style.background = colors[Math.floor(Math.random() * colors.length)];
            confetti.style.animationDuration = (Math.random() * 3 + 2) + 's';
            confetti.style.animationDelay = Math.random() * 5 + 's';
            container.appendChild(confetti);
        }

        this.activeEffects.push({ type: 'confetti', container });
    }

    /**
     * Create fog effect
     */
    async createFogEffect(config) {
        const container = this.createEffectContainer('fog-effect');
        
        for (let i = 0; i < 3; i++) {
            const fog = document.createElement('div');
            fog.className = 'fog';
            fog.style.opacity = (config.opacity || 0.3) / 3;
            fog.style.animationDuration = (15 + i * 5) + 's';
            fog.style.animationDelay = (i * 5) + 's';
            container.appendChild(fog);
        }

        this.activeEffects.push({ type: 'fog', container });
    }

    /**
     * Create butterflies effect
     */
    async createButterfliesEffect(config) {
        const container = this.createEffectContainer('butterflies-effect');
        const count = config.count || 10;
        const colors = config.colors || ['pink', 'blue', 'yellow'];

        for (let i = 0; i < count; i++) {
            const butterfly = document.createElement('div');
            butterfly.className = 'butterfly';
            butterfly.textContent = '🦋';
            butterfly.style.left = Math.random() * 100 + '%';
            butterfly.style.top = Math.random() * 100 + '%';
            butterfly.style.animationDuration = (Math.random() * 10 + 10) + 's';
            butterfly.style.animationDelay = Math.random() * 5 + 's';
            container.appendChild(butterfly);
        }

        this.activeEffects.push({ type: 'butterflies', container });
    }

    /**
     * Create bats effect
     */
    async createBatsEffect(config) {
        const container = this.createEffectContainer('bats-effect');
        const count = config.count || 15;

        for (let i = 0; i < count; i++) {
            const bat = document.createElement('div');
            bat.className = 'bat';
            bat.textContent = '🦇';
            bat.style.left = Math.random() * 100 + '%';
            bat.style.top = Math.random() * 50 + '%';
            bat.style.animationDuration = (Math.random() * 8 + 8) + 's';
            bat.style.animationDelay = Math.random() * 5 + 's';
            container.appendChild(bat);
        }

        this.activeEffects.push({ type: 'bats', container });
    }

    /**
     * Create lights effect
     */
    async createLightsEffect(config) {
        const container = this.createEffectContainer('lights-effect');
        const colors = config.colors || ['red', 'green', 'blue', 'yellow'];

        for (let i = 0; i < 20; i++) {
            const light = document.createElement('div');
            light.className = 'light';
            light.style.left = (i * 5) + '%';
            light.style.background = colors[i % colors.length];
            if (config.twinkle) {
                light.style.animationDuration = (Math.random() * 2 + 1) + 's';
            }
            container.appendChild(light);
        }

        this.activeEffects.push({ type: 'lights', container });
    }

    /**
     * Apply decorations
     */
    applyDecorations(decorations, assets) {
        if (!decorations) return;

        // Clear existing decorations
        document.querySelectorAll('.theme-decoration').forEach(el => el.remove());

        // Apply each decoration type
        if (decorations.header) {
            this.createDecoration('header', decorations.header, assets);
        }

        if (decorations.footer) {
            this.createDecoration('footer', decorations.footer, assets);
        }

        if (decorations.corners) {
            this.createDecoration('corners', decorations.corners, assets);
        }

        if (decorations.floating) {
            this.createDecoration('floating', decorations.floating, assets);
        }
    }

    /**
     * Create decoration element
     */
    createDecoration(position, config, assets) {
        const decoration = document.createElement('div');
        decoration.className = `theme-decoration decoration-${position} decoration-${config.type}`;
        
        // Add decoration content based on type
        // This would use actual asset images in production
        decoration.style.opacity = config.opacity || 1;
        
        document.body.appendChild(decoration);
    }

    /**
     * Load sound effects
     */
    loadSoundEffects(sounds) {
        if (!sounds) return;

        // Background music
        if (sounds.music) {
            this.playBackgroundMusic(sounds.music);
        }

        // UI sounds would be triggered by user actions
        if (sounds.ui) {
            this.setupUISounds(sounds.ui);
        }
    }

    /**
     * Play background music
     */
    playBackgroundMusic(config) {
        // Stop existing music
        if (this.backgroundMusic) {
            this.backgroundMusic.pause();
            this.backgroundMusic = null;
        }

        // Create and play new music
        this.backgroundMusic = new Audio(config.file);
        this.backgroundMusic.volume = config.volume || 0.3;
        this.backgroundMusic.loop = config.loop !== false;
        
        // Play with user interaction
        const playMusic = () => {
            this.backgroundMusic.play().catch(e => {
                console.warn('[ThemeLoader] Music autoplay blocked:', e);
            });
            document.removeEventListener('click', playMusic);
        };
        
        document.addEventListener('click', playMusic, { once: true });
    }

    /**
     * Setup UI sounds
     */
    setupUISounds(sounds) {
        // Store UI sounds for use by other components
        this.uiSounds = sounds;
    }

    /**
     * Create effect container
     */
    createEffectContainer(className) {
        const container = document.createElement('div');
        container.className = `theme-effect ${className}`;
        document.body.appendChild(container);
        return container;
    }

    /**
     * Clear all effects
     */
    clearEffects() {
        this.activeEffects.forEach(effect => {
            if (effect.container) {
                effect.container.remove();
            }
            if (effect.interval) {
                clearInterval(effect.interval);
            }
        });
        this.activeEffects = [];

        // Stop background music
        if (this.backgroundMusic) {
            this.backgroundMusic.pause();
            this.backgroundMusic = null;
        }
    }

    /**
     * Clear theme
     */
    clearTheme() {
        this.clearEffects();
        
        // Remove theme classes
        document.body.className = document.body.className
            .replace(/theme-\w+/g, '')
            .trim();

        // Remove custom CSS
        const customCSS = document.getElementById('theme-custom-css');
        if (customCSS) {
            customCSS.remove();
        }

        // Clear decorations
        document.querySelectorAll('.theme-decoration').forEach(el => el.remove());

        this.currentTheme = null;
    }

    /**
     * Setup event listeners
     */
    setupEventListeners() {
        // Listen for manual theme refresh
        window.addEventListener('refreshTheme', () => {
            this.loadCurrentTheme();
        });

        // Listen for preference changes
        window.addEventListener('themePreferencesChanged', (e) => {
            this.applyUserPreferences(e.detail.preferences);
            if (!e.detail.preferences.enabled) {
                this.clearTheme();
            } else {
                this.loadCurrentTheme();
            }
        });
    }

    /**
     * Get current theme
     */
    getCurrentTheme() {
        return this.currentTheme;
    }

    /**
     * Toggle effects
     */
    toggleEffects(enabled) {
        this.effectsEnabled = enabled;
        if (!enabled) {
            this.clearEffects();
        } else if (this.currentTheme) {
            this.applyVisualEffects(this.currentTheme.visual_effects);
        }
    }

    /**
     * Toggle sounds
     */
    toggleSounds(enabled) {
        this.soundsEnabled = enabled;
        if (!enabled && this.backgroundMusic) {
            this.backgroundMusic.pause();
        } else if (enabled && this.backgroundMusic) {
            this.backgroundMusic.play();
        }
    }
}

// Create global instance
window.themeLoader = new ThemeLoader();

// Initialize on page load
if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => {
        window.themeLoader.init();
    });
} else {
    window.themeLoader.init();
}
