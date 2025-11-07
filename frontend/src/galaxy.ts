// @ts-nocheck
// Galaxy view functionality
class GalaxyViewer {
    constructor() {
        this.currentGalaxy = 1;
        this.currentSystem = 1;
        this.galaxyData = [];
        this.selectedPlanet = null;
        this.updateInterval = null;
        this.planetGenerator = new PlanetImageGenerator();
        
        this.init();
    }
    
    init() {
        this.setupEventListeners();
        this.loadInitialView();
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
        
        // View system button
        const viewBtn = document.getElementById('view-system-btn');
        if (viewBtn) {
            viewBtn.addEventListener('click', () => this.viewSystem());
        }
        
        // Galaxy and system selectors
        const galaxySelect = document.getElementById('galaxy-select');
        const systemInput = document.getElementById('system-input');
        
        if (galaxySelect) {
            galaxySelect.addEventListener('change', (e) => {
                this.currentGalaxy = parseInt(e.target.value);
                this.viewSystem();
            });
        }
        
        if (systemInput) {
            systemInput.addEventListener('keypress', (e) => {
                if (e.key === 'Enter') {
                    this.viewSystem();
                }
            });
        }
        
        // Modal close
        const closeModal = document.querySelector('.close-modal');
        if (closeModal) {
            closeModal.addEventListener('click', () => {
                this.closeModal();
            });
        }
        
        // Close modal on outside click
        const modal = document.getElementById('planet-details');
        if (modal) {
            modal.addEventListener('click', (e) => {
                if (e.target === modal) {
                    this.closeModal();
                }
            });
        }
        
        // Listen for Socket.io events
        if (window.socket) {
            window.socket.on('galaxyUpdate', (data) => {
                this.handleGalaxyUpdate(data);
            });
        }
    }
    
    async loadInitialView() {
        // Load current planet location
        if (window.currentPlanet) {
            this.currentGalaxy = window.currentPlanet.galaxy || 1;
            this.currentSystem = window.currentPlanet.system || 1;
            
            const galaxySelect = document.getElementById('galaxy-select');
            const systemInput = document.getElementById('system-input');
            
            if (galaxySelect) galaxySelect.value = this.currentGalaxy;
            if (systemInput) systemInput.value = this.currentSystem;
        }
        
        await this.viewSystem();
    }
    
    async viewSystem() {
        const systemInput = document.getElementById('system-input');
        if (systemInput) {
            this.currentSystem = parseInt(systemInput.value) || 1;
            // Clamp to valid range
            this.currentSystem = Math.max(1, Math.min(499, this.currentSystem));
            systemInput.value = this.currentSystem;
        }
        
        try {
            const response = await api.get(`/galaxy?galaxy=${this.currentGalaxy}&system=${this.currentSystem}`);
            this.galaxyData = response.data.planets || [];
            this.renderGalaxyView();
        } catch (error) {
            console.error('Failed to load galaxy data:', error);
            this.showNotification('Failed to load galaxy data', 'error');
        }
    }
    
    renderGalaxyView() {
        const container = document.getElementById('galaxy-view');
        const title = document.getElementById('system-title');
        
        if (title) {
            title.textContent = `Galaxy ${this.currentGalaxy}, System ${this.currentSystem}`;
        }
        
        if (!container) return;
        
        container.innerHTML = '';
        
        // Create 15 positions
        for (let position = 1; position <= 15; position++) {
            const planet = this.galaxyData.find(p => p.position === position);
            const positionCard = this.createPositionCard(position, planet);
            container.appendChild(positionCard);
        }
    }
    
    createPositionCard(position, planet) {
        const card = document.createElement('div');
        card.className = 'galaxy-position';
        
        if (!planet) {
            // Empty position
            card.classList.add('empty');
            card.innerHTML = `
                <div class="position-header">
                    <span class="position-number">${position}</span>
                </div>
                <div class="position-content">
                    <div class="empty-slot">Empty</div>
                </div>
            `;
            return card;
        }
        
        // Check if it's the current player's planet
        const isOwnPlanet = planet.user_id === window.currentUserId;
        const isCurrentPlanet = planet.id === window.currentPlanetId;
        
        if (isOwnPlanet) {
            card.classList.add('own-planet');
        }
        
        if (isCurrentPlanet) {
            card.classList.add('current-planet');
        }
        
        // Generate planet image
        const planetImageUrl = this.planetGenerator.generate({
            galaxy: planet.galaxy,
            system: planet.system,
            position: planet.position,
            type: planet.planet_type || 'terrestrial',
            temperature: planet.temperature || 0
        }, 120);
        
        card.innerHTML = `
            <div class="position-header">
                <span class="position-number">${position}</span>
                ${isCurrentPlanet ? '<span class="badge current">Current</span>' : ''}
                ${isOwnPlanet && !isCurrentPlanet ? '<span class="badge own">Your Planet</span>' : ''}
            </div>
            <div class="position-content">
                <img src="${planetImageUrl}" alt="${planet.name || 'Planet'}" class="planet-image" style="width: 120px; height: 120px; border-radius: 50%; margin: 10px auto; display: block;">
                <div class="planet-name">${planet.name || 'Planet'}</div>
                <div class="planet-owner">${planet.username || 'Unknown'}</div>
                ${planet.alliance_tag ? `<div class="planet-alliance">[${planet.alliance_tag}]</div>` : ''}
            </div>
            <div class="position-actions">
                <button class="btn-small" data-action="view" data-planet-id="${planet.id}">View</button>
                ${!isOwnPlanet ? `<button class="btn-small btn-attack" data-action="attack" data-planet-id="${planet.id}">Attack</button>` : ''}
            </div>
        `;
        
        // Add event listeners to buttons
        const viewBtn = card.querySelector('[data-action="view"]');
        const attackBtn = card.querySelector('[data-action="attack"]');
        
        if (viewBtn) {
            viewBtn.addEventListener('click', () => this.viewPlanetDetails(planet));
        }
        
        if (attackBtn) {
            attackBtn.addEventListener('click', () => this.prepareAttack(planet));
        }
        
        return card;
    }
    
    viewPlanetDetails(planet) {
        const modal = document.getElementById('planet-details');
        const content = document.getElementById('planet-details-content');
        
        if (!modal || !content) return;
        
        const isOwnPlanet = planet.user_id === window.currentUserId;
        
        content.innerHTML = `
            <h2>🪐 ${planet.name || 'Planet'}</h2>
            <div class="planet-info">
                <div class="info-row">
                    <span class="label">Coordinates:</span>
                    <span class="value">${planet.galaxy}:${planet.system}:${planet.position}</span>
                </div>
                <div class="info-row">
                    <span class="label">Owner:</span>
                    <span class="value">${planet.username || 'Unknown'}</span>
                </div>
                ${planet.alliance_tag ? `
                    <div class="info-row">
                        <span class="label">Alliance:</span>
                        <span class="value">[${planet.alliance_tag}] ${planet.alliance_name || ''}</span>
                    </div>
                ` : ''}
                ${isOwnPlanet ? `
                    <div class="info-row">
                        <span class="label">Metal:</span>
                        <span class="value">${this.formatNumber(planet.metal || 0)}</span>
                    </div>
                    <div class="info-row">
                        <span class="label">Crystal:</span>
                        <span class="value">${this.formatNumber(planet.crystal || 0)}</span>
                    </div>
                    <div class="info-row">
                        <span class="label">Deuterium:</span>
                        <span class="value">${this.formatNumber(planet.deuterium || 0)}</span>
                    </div>
                ` : ''}
            </div>
            ${!isOwnPlanet ? `
                <div class="modal-actions">
                    <button class="btn btn-primary" onclick="galaxyViewer.prepareAttack(${JSON.stringify(planet).replace(/"/g, '&quot;')})">
                        Send Fleet
                    </button>
                    <button class="btn btn-secondary" onclick="galaxyViewer.espionage(${planet.id})">
                        Espionage
                    </button>
                </div>
            ` : `
                <div class="modal-actions">
                    <button class="btn btn-primary" onclick="window.location.href='overview.html?planet=${planet.id}'">
                        Go to Planet
                    </button>
                </div>
            `}
        `;
        
        modal.style.display = 'block';
        this.selectedPlanet = planet;
    }
    
    prepareAttack(planet) {
        // Redirect to fleet page with pre-filled target coordinates
        const url = `fleet.html?target=${planet.galaxy}:${planet.system}:${planet.position}`;
        window.location.href = url;
    }
    
    async espionage(planetId) {
        try {
            // This would require an espionage probe dispatch
            this.showNotification('Espionage feature coming soon!', 'info');
            this.closeModal();
        } catch (error) {
            console.error('Failed to send espionage probe:', error);
            this.showNotification('Failed to send espionage probe', 'error');
        }
    }
    
    closeModal() {
        const modal = document.getElementById('planet-details');
        if (modal) {
            modal.style.display = 'none';
        }
        this.selectedPlanet = null;
    }
    
    handleGalaxyUpdate(data) {
        // Reload if update affects current view
        if (data.galaxy === this.currentGalaxy && data.system === this.currentSystem) {
            this.viewSystem();
        }
    }
    
    formatNumber(num) {
        return new Intl.NumberFormat('en-US').format(Math.floor(num));
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
let galaxyViewer;
document.addEventListener('DOMContentLoaded', () => {
    galaxyViewer = new GalaxyViewer();
});
