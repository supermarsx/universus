// @ts-nocheck
import i18next from 'i18next';

const RESEARCH_CATEGORY_LABELS = {
    energy: 'Energy & Power',
    weapons: 'Weapons',
    propulsion: 'Propulsion',
    defense: 'Defensive Systems',
    intelligence: 'Espionage & Intelligence',
    infrastructure: 'Infrastructure',
    advanced: 'Advanced Technologies',
    general: 'General Research'
};

class ResearchManager {
    constructor() {
        this.planet = null;
        this.state = null;
        this.refreshInterval = null;
        this.statusTimer = null;

        this.setupSocketListeners();
        this.startAutoRefresh();
    }

    async onPlanetUpdate(data) {
        this.planet = data.planet;
        window.currentPlanetId = this.planet.id;
        window.currentResources = {
            metal: this.planet.metal,
            crystal: this.planet.crystal,
            deuterium: this.planet.deuterium
        };

        await this.loadResearchData();
    }

    setupSocketListeners() {
        if (window.socket) {
            window.socket.on('researchUpdate', () => this.loadResearchData());
            window.socket.on('researchComplete', (payload) => {
                this.showNotification(
                    i18next.t('research.researchComplete', { tech: this.formatTechName(payload.researchType), level: payload.level, defaultValue: `Research complete: ${this.formatTechName(payload.researchType)} Level ${payload.level}` }),
                    'success'
                );
                this.loadResearchData();
            });
        }
    }

    startAutoRefresh() {
        if (this.refreshInterval) {
            clearInterval(this.refreshInterval);
        }
        this.refreshInterval = setInterval(() => {
            if (this.planet) {
                this.loadResearchData();
            }
        }, 30000);
    }

    async loadResearchData() {
        if (!this.planet) return;
        try {
            const overview = await api.get(`/research?planetId=${this.planet.id}`);
            this.state = overview;
            this.render();
        } catch (error) {
            console.error('Failed to load research data:', error);
            this.showNotification(error.message || i18next.t('research.failedToLoad', { defaultValue: 'Failed to load research data' }), 'error');
        }
    }

    render() {
        this.renderStatus();
        this.renderQueue();
        this.renderCategories();
    }

    renderStatus() {
        const container = document.getElementById('research-status');
        if (!container) return;

        if (!this.state || !this.state.currentResearch) {
            container.innerHTML = `
                <div class="research-status-idle">
                    <h3>${i18next.t('research.idle', { defaultValue: 'Research Laboratory Idle' })}</h3>
                    <p>${i18next.t('research.assignProject', { defaultValue: 'Assign a new research project to keep your scientists busy.' })}</p>
                </div>
            `;
            if (this.statusTimer) {
                clearInterval(this.statusTimer);
            }
            return;
        }

        const current = this.state.currentResearch;
        const endTime = new Date(current.end_time).getTime();
        const startTime = new Date(current.start_time).getTime();
        const total = endTime - startTime;

        container.innerHTML = `
            <div class="research-status-active">
                <h3>${i18next.t('research.researching', { tech: this.formatTechName(current.research_type), level: current.level, defaultValue: `🔬 Researching ${this.formatTechName(current.research_type)} (Level ${current.level})` })}</h3>
                <div class="research-progress">
                    <div class="progress-bar">
                        <div class="progress-fill" id="research-progress-fill"></div>
                    </div>
                    <div class="countdown" id="research-progress-countdown"></div>
                </div>
            </div>
        `;

        if (this.statusTimer) {
            clearInterval(this.statusTimer);
        }

        const updateProgress = () => {
            const now = Date.now();
            const remaining = Math.max(0, endTime - now);
            const elapsed = Math.min(total, now - startTime);
            const percent = Math.min(100, (elapsed / total) * 100);

            const fill = document.getElementById('research-progress-fill');
            const countdown = document.getElementById('research-progress-countdown');

            if (fill) fill.style.width = `${percent}%`;
            if (countdown) countdown.textContent = this.formatTime(Math.floor(remaining / 1000));
        };

        updateProgress();
        this.statusTimer = setInterval(updateProgress, 1000);
    }

    renderQueue() {
        const container = document.getElementById('research-queue');
        if (!container) return;

        if (!this.state || !this.state.queue || this.state.queue.length === 0) {
            container.innerHTML = `
                <div class="research-queue-empty">
                    <p>${i18next.t('research.noPending', { defaultValue: 'No pending research projects.' })}</p>
                </div>
            `;
            return;
        }

        const entries = this.state.queue.map((entry) => `
            <div class="queue-entry">
                <div>
                    <strong>${this.formatTechName(entry.research_type)}</strong>
                    <span>${i18next.t('research.researchLevel', { level: entry.level, defaultValue: `Level ${entry.level}` })}</span>
                </div>
                <div class="queue-actions">
                    <span>${this.formatTime(entry.secondsRemaining)}</span>
                    <button class="btn btn-text" data-queue-id="${entry.id}">
                        ${i18next.t('ui.cancel', { defaultValue: 'Cancel' })}
                    </button>
                </div>
            </div>
        `).join('');

        container.innerHTML = `
            <h3>${i18next.t('research.queue', { defaultValue: 'Queue' })}</h3>
            ${entries}
        `;

        container.querySelectorAll('button[data-queue-id]').forEach((btn) => {
            btn.addEventListener('click', () => this.cancelResearch(btn.dataset.queueId));
        });
    }

    renderCategories() {
        const container = document.getElementById('research-categories');
        if (!container || !this.state) return;

        const grouped = this.groupByCategory(this.state.technologies || []);
        container.innerHTML = '';

        Object.entries(grouped).forEach(([category, techList]) => {
            const section = document.createElement('section');
            section.className = 'tech-category card-enhanced';
            section.innerHTML = `
                <div class="category-header">
                    <h3>${i18next.t(`research.${category}`, { defaultValue: RESEARCH_CATEGORY_LABELS[category] || this.formatTechName(category) })}</h3>
                    <p>${techList.length} technologies</p>
                </div>
                <div class="tech-card-grid"></div>
            `;

            const grid = section.querySelector('.tech-card-grid');
            techList.forEach((tech) => grid.appendChild(this.createTechCard(tech)));
            container.appendChild(section);
        });
    }

    createTechCard(tech) {
        const card = document.createElement('div');
        card.className = 'tech-card card-compact';

        const affordable = this.canAfford(tech.cost);
        const labAvailable = (this.state?.researchLabLevel || 0) > 0;
        const isBusy = !!this.state?.currentResearch;
        const canResearch = tech.requirementsMet && affordable && labAvailable && !isBusy;

        card.innerHTML = `
            <div class="tech-card-header">
                <h4>${tech.name}</h4>
                <span class="tech-level">${i18next.t('research.researchLevel', { level: tech.level, defaultValue: `Level ${tech.level}` })}</span>
            </div>
            <p class="tech-description">${tech.description || i18next.t('research.noDescription', { defaultValue: 'No description available.' })}</p>
            <div class="tech-costs">
                ${this.renderCost(i18next.t('buildings.resources.metal', { defaultValue: 'Metal' }), tech.cost.metal)}
                ${this.renderCost(i18next.t('buildings.resources.crystal', { defaultValue: 'Crystal' }), tech.cost.crystal)}
                ${this.renderCost(i18next.t('buildings.resources.deuterium', { defaultValue: 'Deuterium' }), tech.cost.deuterium)}
            </div>
            ${this.renderRequirements(tech)}
            <button 
                class="btn btn-primary btn-block"
                data-tech="${tech.type}"
                ${canResearch ? '' : 'disabled'}
            >
                ${isBusy ? i18next.t('research.researchInProgress', { defaultValue: 'Research in progress' }) : affordable ? i18next.t('research.researchLevel', { level: tech.nextLevel, defaultValue: `Research Level ${tech.nextLevel}` }) : i18next.t('research.insufficientResources', { defaultValue: 'Insufficient resources' })}
            </button>
        `;

        const btn = card.querySelector('button');
        if (btn) {
            btn.addEventListener('click', () => this.startResearch(tech.type));
        }

        return card;
    }

    renderCost(label, value) {
        return `
            <div class="cost-item">
                <span class="cost-label">${label}</span>
                <span class="cost-value">${this.formatNumber(value)}</span>
            </div>
        `;
    }

    renderRequirements(tech) {
        const requirements = tech.requirements || {};
        const entries = [];

        if (requirements.buildings) {
            for (const [building, level] of Object.entries(requirements.buildings)) {
                const met = (this.planet?.[building] || 0) >= level;
                entries.push(`<div class="requirement ${met ? 'met' : 'unmet'}">${this.formatTechName(building)} ${level}</div>`);
            }
        }

        if (requirements.research) {
            for (const [research, level] of Object.entries(requirements.research)) {
                const met = (this.state?.technologies?.find((t) => t.type === research)?.level || 0) >= level;
                entries.push(`<div class="requirement ${met ? 'met' : 'unmet'}">${this.formatTechName(research)} ${level}</div>`);
            }
        }

        if (entries.length === 0) return '';

        return `
            <div class="tech-requirements">
                <strong>${i18next.t('research.requirements', { defaultValue: 'Requirements' })}</strong>
                ${entries.join('')}
            </div>
        `;
    }

    groupByCategory(technologies) {
        return technologies.reduce((acc, tech) => {
            const key = tech.category || 'general';
            if (!acc[key]) acc[key] = [];
            acc[key].push(tech);
            return acc;
        }, {});
    }

    async startResearch(researchType) {
        if (!this.planet) return;

        try {
            await api.post('/research/start', {
                planetId: this.planet.id,
                researchType,
            });
            this.showNotification(i18next.t('research.researchStarted', { tech: this.formatTechName(researchType), defaultValue: `Research started: ${this.formatTechName(researchType)}` }), 'success');
            await this.loadResearchData();
        } catch (error) {
            console.error('Failed to start research:', error);
            this.showNotification(error.message || i18next.t('research.failedToStart', { defaultValue: 'Failed to start research' }), 'error');
        }
    }

    async cancelResearch(queueId) {
        if (!queueId) return;
        try {
            await api.delete(`/research/queue/${queueId}`);
            this.showNotification(i18next.t('research.researchCancelled', { defaultValue: 'Research cancelled' }), 'info');
            await this.loadResearchData();
        } catch (error) {
            console.error('Failed to cancel research:', error);
            this.showNotification(error.message || i18next.t('research.failedToCancel', { defaultValue: 'Failed to cancel research' }), 'error');
        }
    }

    canAfford(cost) {
        if (!window.currentResources || !cost) return false;
        return (
            window.currentResources.metal >= cost.metal &&
            window.currentResources.crystal >= cost.crystal &&
            window.currentResources.deuterium >= cost.deuterium
        );
    }

    formatTechName(key) {
        if (!key) return '';
        return key
            .split('_')
            .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
            .join(' ');
    }

    formatNumber(num) {
        const locale = this.getLocale();
        if (typeof Intl !== 'undefined' && Intl.NumberFormat) {
            return new Intl.NumberFormat(locale).format(Math.floor(num || 0));
        }
        return Math.floor(num || 0).toLocaleString();
    }

    getLocale() {
        try {
            if (i18next && i18next.language) {
                return i18next.language;
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

    formatTime(seconds) {
        const hrs = Math.floor(seconds / 3600);
        const mins = Math.floor((seconds % 3600) / 60);
        const secs = seconds % 60;
        return `${hrs}h ${mins}m ${secs}s`;
    }

    showNotification(message, type = 'info') {
        const notification = document.getElementById('notification');
        if (!notification) return;
        
        notification.textContent = message;
        notification.className = `notification ${type} show`;
        
        setTimeout(() => {
            notification.classList.remove('show');
        }, 3000);
    }
}

let researchManager = null;

function ensureManager() {
    if (!researchManager) {
        researchManager = new ResearchManager();
    }
    return researchManager;
}

function updatePageData(data) {
    ensureManager().onPlanetUpdate(data);
}

document.addEventListener('DOMContentLoaded', () => {
    ensureManager();
});
