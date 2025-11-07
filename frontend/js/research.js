// Research page functionality
class ResearchManager {
    constructor() {
        this.currentPlanetId = null;
        this.researchData = null;
        this.researchQueue = [];
        this.updateInterval = null;
        
        // Technology categories
        this.techCategories = {
            basic: ['energy_technology', 'laser_technology', 'ion_technology', 'hyperspace_technology'],
            advanced: ['plasma_technology', 'espionage_technology', 'computer_technology', 'astrophysics'],
            combat: ['weapons_technology', 'shielding_technology', 'armor_technology'],
            drive: ['combustion_drive', 'impulse_drive', 'hyperspace_drive']
        };
        
        // Technology display names
        this.techNames = {
            energy_technology: 'Energy Technology',
            laser_technology: 'Laser Technology',
            ion_technology: 'Ion Technology',
            hyperspace_technology: 'Hyperspace Technology',
            plasma_technology: 'Plasma Technology',
            espionage_technology: 'Espionage Technology',
            computer_technology: 'Computer Technology',
            astrophysics: 'Astrophysics',
            weapons_technology: 'Weapons Technology',
            shielding_technology: 'Shielding Technology',
            armor_technology: 'Armor Technology',
            combustion_drive: 'Combustion Drive',
            impulse_drive: 'Impulse Drive',
            hyperspace_drive: 'Hyperspace Drive'
        };
        
        // Technology descriptions
        this.techDescriptions = {
            energy_technology: 'Increases energy production and enables advanced technologies',
            laser_technology: 'Required for laser weapons and advanced ship systems',
            ion_technology: 'Enables ion weapons with shield-piercing capabilities',
            hyperspace_technology: 'Allows faster space travel and advanced research',
            plasma_technology: 'Most powerful weapons technology',
            espionage_technology: 'Improves intelligence gathering capabilities',
            computer_technology: 'Increases fleet slot capacity',
            astrophysics: 'Enables colonization of additional planets',
            weapons_technology: 'Increases all weapon damage output',
            shielding_technology: 'Improves shield strength for ships and defenses',
            armor_technology: 'Increases structural integrity of ships',
            combustion_drive: 'Basic propulsion for small ships',
            impulse_drive: 'Advanced drive for medium ships',
            hyperspace_drive: 'Fastest drive for capital ships'
        };
        
        this.init();
    }
    
    init() {
        this.setupEventListeners();
        this.loadResearchData();
        this.startResourceUpdates();
    }
    
    setupEventListeners() {
        const logoutBtn = document.getElementById('logout');
        if (logoutBtn) {
            logoutBtn.addEventListener('click', (e) => {
                e.preventDefault();
                localStorage.removeItem('token');
                window.location.href = 'index.html';
            });
        }
        
        // Listen for Socket.io events
        if (window.socket) {
            window.socket.on('researchUpdate', (data) => {
                this.handleResearchUpdate(data);
            });
            
            window.socket.on('researchComplete', (data) => {
                this.handleResearchComplete(data);
            });
        }
    }
    
    async loadResearchData() {
        try {
            const response = await api.get(`/research/${window.currentPlanetId}`);
            this.researchData = response.data;
            this.renderResearch();
            this.updateResearchStatus();
        } catch (error) {
            console.error('Failed to load research data:', error);
            this.showNotification('Failed to load research data', 'error');
        }
    }
    
    renderResearch() {
        if (!this.researchData) return;
        
        // Render each category
        Object.keys(this.techCategories).forEach(category => {
            const container = document.getElementById(`${category}-tech`);
            if (!container) return;
            
            container.innerHTML = '';
            
            this.techCategories[category].forEach(techKey => {
                const level = this.researchData.technologies[techKey] || 0;
                const techCard = this.createTechCard(techKey, level);
                container.appendChild(techCard);
            });
        });
    }
    
    createTechCard(techKey, currentLevel) {
        const card = document.createElement('div');
        card.className = 'tech-card';
        
        const name = this.techNames[techKey] || techKey;
        const description = this.techDescriptions[techKey] || '';
        const nextLevel = currentLevel + 1;
        
        // Calculate costs for next level
        const baseCost = this.getBaseCost(techKey);
        const cost = this.calculateCost(baseCost, nextLevel);
        
        // Check if requirements are met
        const requirements = this.getRequirements(techKey);
        const canResearch = this.checkRequirements(requirements);
        const hasResources = this.checkResources(cost);
        const isResearching = this.researchData.currentResearch?.technology === techKey;
        
        card.innerHTML = `
            <div class="tech-header">
                <h3>${name}</h3>
                <div class="tech-level">Level ${currentLevel}</div>
            </div>
            <div class="tech-description">${description}</div>
            <div class="tech-cost">
                <div class="cost-item">
                    <span class="resource-icon">🔩</span> ${this.formatNumber(cost.metal)}
                </div>
                <div class="cost-item">
                    <span class="resource-icon">💎</span> ${this.formatNumber(cost.crystal)}
                </div>
                <div class="cost-item">
                    <span class="resource-icon">⚡</span> ${this.formatNumber(cost.deuterium)}
                </div>
            </div>
            ${requirements.length > 0 ? `
                <div class="tech-requirements">
                    <strong>Requirements:</strong>
                    ${requirements.map(req => {
                        const reqName = this.techNames[req.tech] || req.tech;
                        const reqMet = (this.researchData.technologies[req.tech] || 0) >= req.level;
                        return `<div class="requirement ${reqMet ? 'met' : 'unmet'}">${reqName} ${req.level}</div>`;
                    }).join('')}
                </div>
            ` : ''}
            <div class="tech-time">
                Research time: ${this.formatTime(this.calculateResearchTime(baseCost, nextLevel))}
            </div>
            ${isResearching ? `
                <div class="research-progress">
                    <div class="progress-bar">
                        <div class="progress-fill" id="progress-${techKey}"></div>
                    </div>
                    <div class="countdown" id="countdown-${techKey}"></div>
                </div>
            ` : `
                <button 
                    class="btn ${canResearch && hasResources && !this.researchData.currentResearch ? 'btn-primary' : 'btn-disabled'}" 
                    data-tech="${techKey}"
                    ${!canResearch || !hasResources || this.researchData.currentResearch ? 'disabled' : ''}
                >
                    ${!canResearch ? 'Requirements not met' : 
                      !hasResources ? 'Insufficient resources' : 
                      this.researchData.currentResearch ? 'Lab busy' : 
                      'Research'}
                </button>
            `}
        `;
        
        const button = card.querySelector('button');
        if (button && !button.disabled) {
            button.addEventListener('click', () => this.startResearch(techKey));
        }
        
        return card;
    }
    
    getBaseCost(techKey) {
        // Base costs for each technology (metal, crystal, deuterium)
        const costs = {
            energy_technology: { metal: 0, crystal: 800, deuterium: 400 },
            laser_technology: { metal: 200, crystal: 100, deuterium: 0 },
            ion_technology: { metal: 1000, crystal: 300, deuterium: 100 },
            hyperspace_technology: { metal: 0, crystal: 4000, deuterium: 2000 },
            plasma_technology: { metal: 2000, crystal: 4000, deuterium: 1000 },
            espionage_technology: { metal: 200, crystal: 1000, deuterium: 200 },
            computer_technology: { metal: 0, crystal: 400, deuterium: 600 },
            astrophysics: { metal: 4000, crystal: 8000, deuterium: 4000 },
            weapons_technology: { metal: 800, crystal: 200, deuterium: 0 },
            shielding_technology: { metal: 200, crystal: 600, deuterium: 0 },
            armor_technology: { metal: 1000, crystal: 0, deuterium: 0 },
            combustion_drive: { metal: 400, crystal: 0, deuterium: 600 },
            impulse_drive: { metal: 2000, crystal: 4000, deuterium: 600 },
            hyperspace_drive: { metal: 10000, crystal: 20000, deuterium: 6000 }
        };
        return costs[techKey] || { metal: 0, crystal: 0, deuterium: 0 };
    }
    
    getRequirements(techKey) {
        const reqs = {
            ion_technology: [{ tech: 'laser_technology', level: 5 }, { tech: 'energy_technology', level: 4 }],
            hyperspace_technology: [{ tech: 'energy_technology', level: 5 }, { tech: 'shielding_technology', level: 5 }],
            plasma_technology: [{ tech: 'energy_technology', level: 8 }, { tech: 'laser_technology', level: 10 }, { tech: 'ion_technology', level: 5 }],
            computer_technology: [{ tech: 'laser_technology', level: 2 }],
            astrophysics: [{ tech: 'espionage_technology', level: 4 }, { tech: 'impulse_drive', level: 3 }],
            impulse_drive: [{ tech: 'energy_technology', level: 1 }],
            hyperspace_drive: [{ tech: 'hyperspace_technology', level: 3 }]
        };
        return reqs[techKey] || [];
    }
    
    checkRequirements(requirements) {
        if (!this.researchData) return false;
        return requirements.every(req => {
            const currentLevel = this.researchData.technologies[req.tech] || 0;
            return currentLevel >= req.level;
        });
    }
    
    checkResources(cost) {
        const resources = window.currentResources || {};
        return resources.metal >= cost.metal &&
               resources.crystal >= cost.crystal &&
               resources.deuterium >= cost.deuterium;
    }
    
    calculateCost(baseCost, level) {
        return {
            metal: Math.floor(baseCost.metal * Math.pow(2, level - 1)),
            crystal: Math.floor(baseCost.crystal * Math.pow(2, level - 1)),
            deuterium: Math.floor(baseCost.deuterium * Math.pow(2, level - 1))
        };
    }
    
    calculateResearchTime(baseCost, level) {
        const cost = this.calculateCost(baseCost, level);
        const totalCost = cost.metal + cost.crystal;
        const labLevel = this.researchData.researchLabLevel || 1;
        return Math.floor((totalCost / (1000 * (1 + labLevel))) * 3600);
    }
    
    async startResearch(techKey) {
        try {
            await api.post(`/research/${window.currentPlanetId}`, {
                technology: techKey
            });
            this.showNotification(`Started researching ${this.techNames[techKey]}`, 'success');
            await this.loadResearchData();
        } catch (error) {
            console.error('Failed to start research:', error);
            this.showNotification(error.response?.data?.error || 'Failed to start research', 'error');
        }
    }
    
    updateResearchStatus() {
        const statusDiv = document.getElementById('research-status');
        if (!statusDiv) return;
        
        if (this.researchData.currentResearch) {
            const tech = this.researchData.currentResearch;
            const name = this.techNames[tech.technology] || tech.technology;
            const endTime = new Date(tech.end_time).getTime();
            const now = Date.now();
            const remaining = Math.max(0, endTime - now);
            
            statusDiv.innerHTML = `
                <div class="active-research">
                    <h3>🔬 Currently Researching: ${name} (Level ${tech.current_level} → ${tech.current_level + 1})</h3>
                    <div class="progress-bar">
                        <div class="progress-fill" id="current-research-progress"></div>
                    </div>
                    <div class="countdown" id="current-research-countdown"></div>
                </div>
            `;
            
            this.updateCountdown(tech.technology, endTime);
        } else {
            statusDiv.innerHTML = '<div class="idle-status">🔬 Research Laboratory idle</div>';
        }
    }
    
    updateCountdown(techKey, endTime) {
        const update = () => {
            const now = Date.now();
            const remaining = Math.max(0, endTime - now);
            
            if (remaining === 0) {
                return;
            }
            
            const countdownEl = document.getElementById(`countdown-${techKey}`) || 
                               document.getElementById('current-research-countdown');
            const progressEl = document.getElementById(`progress-${techKey}`) || 
                              document.getElementById('current-research-progress');
            
            if (countdownEl) {
                countdownEl.textContent = this.formatTime(Math.floor(remaining / 1000));
            }
            
            if (progressEl && this.researchData.currentResearch) {
                const startTime = new Date(this.researchData.currentResearch.start_time).getTime();
                const total = endTime - startTime;
                const elapsed = now - startTime;
                const progress = Math.min(100, (elapsed / total) * 100);
                progressEl.style.width = `${progress}%`;
            }
        };
        
        update();
        setInterval(update, 1000);
    }
    
    handleResearchUpdate(data) {
        if (data.planetId === window.currentPlanetId) {
            this.loadResearchData();
        }
    }
    
    handleResearchComplete(data) {
        if (data.planetId === window.currentPlanetId) {
            this.showNotification(`Research complete: ${this.techNames[data.technology]} Level ${data.level}`, 'success');
            this.loadResearchData();
        }
    }
    
    startResourceUpdates() {
        if (this.updateInterval) {
            clearInterval(this.updateInterval);
        }
        
        this.updateInterval = setInterval(() => {
            this.loadResearchData();
        }, 30000); // Update every 30 seconds
    }
    
    formatNumber(num) {
        return new Intl.NumberFormat('en-US').format(Math.floor(num));
    }
    
    formatTime(seconds) {
        const hours = Math.floor(seconds / 3600);
        const minutes = Math.floor((seconds % 3600) / 60);
        const secs = seconds % 60;
        return `${hours}h ${minutes}m ${secs}s`;
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

// Initialize when page loads
document.addEventListener('DOMContentLoaded', () => {
    new ResearchManager();
});
