// @ts-nocheck
// Fleet management functionality
class FleetManager {
    constructor() {
        this.currentPlanetId = null;
        this.planetShips = {};
        this.activeFleets = [];
        this.selectedShips = {};
        this.updateInterval = null;
        
        // Ship display names
        this.shipNames = {
            small_cargo: 'Small Cargo',
            large_cargo: 'Large Cargo',
            light_fighter: 'Light Fighter',
            heavy_fighter: 'Heavy Fighter',
            cruiser: 'Cruiser',
            battleship: 'Battleship',
            colony_ship: 'Colony Ship',
            recycler: 'Recycler',
            espionage_probe: 'Espionage Probe',
            bomber: 'Bomber',
            destroyer: 'Destroyer',
            deathstar: 'Deathstar'
        };
        
        // Ship cargo capacities
        this.shipCargo = {
            small_cargo: 5000,
            large_cargo: 25000,
            light_fighter: 50,
            heavy_fighter: 100,
            cruiser: 800,
            battleship: 1500,
            colony_ship: 7500,
            recycler: 20000,
            espionage_probe: 5,
            bomber: 500,
            destroyer: 2000,
            deathstar: 1000000
        };
        
        // Base fuel consumption (deuterium per 10 million distance)
        this.baseFuelConsumption = {
            small_cargo: 10,
            large_cargo: 50,
            light_fighter: 20,
            heavy_fighter: 75,
            cruiser: 300,
            battleship: 500,
            colony_ship: 1000,
            recycler: 300,
            espionage_probe: 1,
            bomber: 1000,
            destroyer: 1000,
            deathstar: 1
        };
        
        this.init();
    }
    
    init() {
        this.setupEventListeners();
        this.loadFleetData();
        this.startFleetUpdates();
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
        
        // Mission type change
        const missionType = document.getElementById('mission-type');
        if (missionType) {
            missionType.addEventListener('change', () => {
                this.updateCargoSection();
            });
        }
        
        // Dispatch button
        const dispatchBtn = document.getElementById('dispatch-btn');
        if (dispatchBtn) {
            dispatchBtn.addEventListener('click', () => this.dispatchFleet());
        }
        
        // Reset button
        const resetBtn = document.getElementById('reset-btn');
        if (resetBtn) {
            resetBtn.addEventListener('click', () => this.resetFleetForm());
        }
        
        // Listen for Socket.io events
        if (window.socket) {
            window.socket.on('fleetUpdate', (data) => {
                this.handleFleetUpdate(data);
            });
            
            window.socket.on('fleetArrival', (data) => {
                this.handleFleetArrival(data);
            });
        }
    }
    
    async loadFleetData() {
        try {
            // Load planet ships
            const shipsResponse = await api.get(`/planets/${window.currentPlanetId}`);
            this.planetShips = shipsResponse.data.ships || {};
            
            // Load active fleets
            const fleetsResponse = await api.get(`/fleet/${window.currentPlanetId}`);
            this.activeFleets = fleetsResponse.data.fleets || [];
            
            this.renderShipSelection();
            this.renderPlanetShips();
            this.renderActiveFleets();
        } catch (error) {
            console.error('Failed to load fleet data:', error);
            this.showNotification('Failed to load fleet data', 'error');
        }
    }
    
    renderShipSelection() {
        const container = document.getElementById('ship-selection');
        if (!container) return;
        
        container.innerHTML = '';
        
        Object.keys(this.shipNames).forEach(shipKey => {
            const available = this.planetShips[shipKey] || 0;
            if (available === 0) return;
            
            const shipCard = document.createElement('div');
            shipCard.className = 'ship-select-card';
            shipCard.innerHTML = `
                <div class="ship-name">${this.shipNames[shipKey]}</div>
                <div class="ship-available">Available: ${available}</div>
                <div class="ship-input">
                    <input 
                        type="number" 
                        class="ship-count-input" 
                        data-ship="${shipKey}" 
                        min="0" 
                        max="${available}" 
                        value="0"
                        placeholder="0"
                    >
                    <button class="btn-small" data-ship="${shipKey}" data-action="all">All</button>
                </div>
            `;
            
            const input = shipCard.querySelector('input');
            const allBtn = shipCard.querySelector('button');
            
            input.addEventListener('change', () => this.updateShipSelection());
            allBtn.addEventListener('click', () => {
                input.value = available;
                this.updateShipSelection();
            });
            
            container.appendChild(shipCard);
        });
    }
    
    updateShipSelection() {
        this.selectedShips = {};
        
        document.querySelectorAll('.ship-count-input').forEach(input => {
            const shipKey = input.dataset.ship;
            const count = parseInt(input.value) || 0;
            if (count > 0) {
                this.selectedShips[shipKey] = count;
            }
        });
        
        this.updateCargoInfo();
    }
    
    updateCargoInfo() {
        let totalCargo = 0;
        
        Object.keys(this.selectedShips).forEach(shipKey => {
            const count = this.selectedShips[shipKey];
            const cargoPerShip = this.shipCargo[shipKey] || 0;
            totalCargo += count * cargoPerShip;
        });
        
        const cargoDisplay = document.getElementById('available-cargo');
        if (cargoDisplay) {
            cargoDisplay.textContent = this.formatNumber(totalCargo);
        }
        
        // Calculate fuel consumption (simplified)
        const distance = this.calculateDistance();
        let totalFuel = 0;
        
        Object.keys(this.selectedShips).forEach(shipKey => {
            const count = this.selectedShips[shipKey];
            const fuelPerShip = this.baseFuelConsumption[shipKey] || 0;
            totalFuel += count * fuelPerShip * (distance / 10000000);
        });
        
        const fuelDisplay = document.getElementById('fuel-consumption');
        if (fuelDisplay) {
            fuelDisplay.textContent = this.formatNumber(Math.ceil(totalFuel));
        }
    }
    
    calculateDistance() {
        // Get current planet coordinates from window.currentPlanet
        const from = window.currentPlanet || { galaxy: 1, system: 1, position: 1 };
        
        const toGalaxy = parseInt(document.getElementById('target-galaxy')?.value) || 1;
        const toSystem = parseInt(document.getElementById('target-system')?.value) || 1;
        const toPosition = parseInt(document.getElementById('target-position')?.value) || 1;
        
        // Simplified distance calculation
        if (from.galaxy !== toGalaxy) {
            return 20000 * Math.abs(from.galaxy - toGalaxy) * 1000000;
        } else if (from.system !== toSystem) {
            return 2700 + 95 * Math.abs(from.system - toSystem) * 1000;
        } else {
            return 1000 + 5 * Math.abs(from.position - toPosition) * 100;
        }
    }
    
    updateCargoSection() {
        const missionType = document.getElementById('mission-type')?.value;
        const cargoSection = document.getElementById('cargo-section');
        
        if (cargoSection) {
            cargoSection.style.display = (missionType === 'transport' || missionType === 'deploy') ? 'block' : 'none';
        }
    }
    
    async dispatchFleet() {
        if (Object.keys(this.selectedShips).length === 0) {
            this.showNotification('Please select ships to dispatch', 'error');
            return;
        }
        
        const missionType = document.getElementById('mission-type')?.value;
        const targetGalaxy = parseInt(document.getElementById('target-galaxy')?.value) || 1;
        const targetSystem = parseInt(document.getElementById('target-system')?.value) || 1;
        const targetPosition = parseInt(document.getElementById('target-position')?.value) || 1;
        
        const cargo = {
            metal: parseInt(document.getElementById('cargo-metal')?.value) || 0,
            crystal: parseInt(document.getElementById('cargo-crystal')?.value) || 0,
            deuterium: parseInt(document.getElementById('cargo-deuterium')?.value) || 0
        };
        
        try {
            await api.post(`/fleet/${window.currentPlanetId}/dispatch`, {
                ships: this.selectedShips,
                target: {
                    galaxy: targetGalaxy,
                    system: targetSystem,
                    position: targetPosition
                },
                mission: missionType,
                cargo: cargo
            });
            
            this.showNotification('Fleet dispatched successfully!', 'success');
            this.resetFleetForm();
            await this.loadFleetData();
        } catch (error) {
            console.error('Failed to dispatch fleet:', error);
            this.showNotification(error.response?.data?.error || 'Failed to dispatch fleet', 'error');
        }
    }
    
    resetFleetForm() {
        this.selectedShips = {};
        document.querySelectorAll('.ship-count-input').forEach(input => {
            input.value = 0;
        });
        document.getElementById('cargo-metal').value = 0;
        document.getElementById('cargo-crystal').value = 0;
        document.getElementById('cargo-deuterium').value = 0;
        this.updateCargoInfo();
    }
    
    renderPlanetShips() {
        const container = document.getElementById('planet-ships');
        if (!container) return;
        
        const hasShips = Object.values(this.planetShips).some(count => count > 0);
        
        if (!hasShips) {
            container.innerHTML = '<div class="empty-state">No ships available on this planet</div>';
            return;
        }
        
        container.innerHTML = '';
        
        Object.keys(this.shipNames).forEach(shipKey => {
            const count = this.planetShips[shipKey] || 0;
            if (count === 0) return;
            
            const shipItem = document.createElement('div');
            shipItem.className = 'ship-item';
            shipItem.innerHTML = `
                <div class="ship-info">
                    <strong>${this.shipNames[shipKey]}</strong>
                    <span>Quantity: ${this.formatNumber(count)}</span>
                    <span>Cargo: ${this.formatNumber(this.shipCargo[shipKey])} per ship</span>
                </div>
            `;
            container.appendChild(shipItem);
        });
    }
    
    renderActiveFleets() {
        const container = document.getElementById('active-fleets');
        if (!container) return;
        
        if (this.activeFleets.length === 0) {
            container.innerHTML = '<div class="empty-state">No active fleet missions</div>';
            return;
        }
        
        container.innerHTML = '';
        
        this.activeFleets.forEach(fleet => {
            const fleetCard = this.createFleetCard(fleet);
            container.appendChild(fleetCard);
        });
    }
    
    createFleetCard(fleet) {
        const card = document.createElement('div');
        card.className = 'fleet-card';
        
        const arrivalTime = new Date(fleet.arrival_time).getTime();
        const now = Date.now();
        const isReturning = fleet.mission === 'return';
        
        // Parse ships from JSON if needed
        const ships = typeof fleet.ships === 'string' ? JSON.parse(fleet.ships) : fleet.ships;
        
        card.innerHTML = `
            <div class="fleet-header">
                <h3>${isReturning ? '↩️ Returning' : '🚀'} ${this.getMissionName(fleet.mission)}</h3>
                <div class="fleet-destination">
                    ${fleet.target_galaxy}:${fleet.target_system}:${fleet.target_position}
                </div>
            </div>
            <div class="fleet-ships">
                ${Object.keys(ships).map(shipKey => 
                    `<span>${this.shipNames[shipKey]}: ${ships[shipKey]}</span>`
                ).join(', ')}
            </div>
            <div class="fleet-arrival">
                <div class="countdown" id="fleet-countdown-${fleet.id}"></div>
            </div>
        `;
        
        // Start countdown
        this.updateFleetCountdown(fleet.id, arrivalTime);
        
        return card;
    }
    
    getMissionName(mission) {
        const names = {
            transport: 'Transport Mission',
            attack: 'Attack Mission',
            deploy: 'Deployment',
            return: 'Return'
        };
        return names[mission] || mission;
    }
    
    updateFleetCountdown(fleetId, arrivalTime) {
        const update = () => {
            const now = Date.now();
            const remaining = Math.max(0, arrivalTime - now);
            
            const countdownEl = document.getElementById(`fleet-countdown-${fleetId}`);
            if (!countdownEl) return;
            
            if (remaining === 0) {
                countdownEl.textContent = 'Arriving...';
                return;
            }
            
            countdownEl.textContent = `Arrives in: ${this.formatTime(Math.floor(remaining / 1000))}`;
        };
        
        update();
        setInterval(update, 1000);
    }
    
    handleFleetUpdate(data) {
        if (data.planetId === window.currentPlanetId) {
            this.loadFleetData();
        }
    }
    
    handleFleetArrival(data) {
        if (data.planetId === window.currentPlanetId || data.originPlanetId === window.currentPlanetId) {
            this.showNotification(`Fleet mission completed: ${this.getMissionName(data.mission)}`, 'success');
            this.loadFleetData();
        }
    }
    
    startFleetUpdates() {
        if (this.updateInterval) {
            clearInterval(this.updateInterval);
        }
        
        this.updateInterval = setInterval(() => {
            this.loadFleetData();
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
    new FleetManager();
});
