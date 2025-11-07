// @ts-nocheck
// Shipyard configuration (matches backend)
const SHIPS = {
    small_cargo: {
        name: 'Small Cargo Ship',
        description: 'Small transporter for resources',
        cost: { metal: 2000, crystal: 2000, deuterium: 0 },
        cargo: 5000,
        speed: 5000,
        image: 'support-cargo-freighter'
    },
    large_cargo: {
        name: 'Large Cargo Ship',
        description: 'Large transporter for resources',
        cost: { metal: 6000, crystal: 6000, deuterium: 0 },
        cargo: 25000,
        speed: 7500,
        image: 'support-cargo-freighter'
    },
    light_fighter: {
        name: 'Light Fighter',
        description: 'Fast attack ship',
        cost: { metal: 3000, crystal: 1000, deuterium: 0 },
        cargo: 50,
        speed: 12500,
        image: 'fighter-interceptor'
    },
    heavy_fighter: {
        name: 'Heavy Fighter',
        description: 'Powerful combat ship',
        cost: { metal: 6000, crystal: 4000, deuterium: 0 },
        cargo: 100,
        speed: 10000,
        image: 'fighter-assault'
    },
    cruiser: {
        name: 'Cruiser',
        description: 'Versatile warship',
        cost: { metal: 20000, crystal: 7000, deuterium: 2000 },
        cargo: 800,
        speed: 15000,
        image: 'cruiser-medium'
    },
    battleship: {
        name: 'Battleship',
        description: 'Heavy warship',
        cost: { metal: 45000, crystal: 15000, deuterium: 0 },
        cargo: 1500,
        speed: 10000,
        image: 'battleship-dreadnought'
    },
    colony_ship: {
        name: 'Colony Ship',
        description: 'Colonize new planets',
        cost: { metal: 10000, crystal: 20000, deuterium: 10000 },
        cargo: 7500,
        speed: 2500,
        image: 'support-colony-ship'
    },
    recycler: {
        name: 'Recycler',
        description: 'Harvest debris fields',
        cost: { metal: 10000, crystal: 6000, deuterium: 2000 },
        cargo: 20000,
        speed: 2000,
        image: 'miner-industrial'
    }
};

const DEFENSES = {
    rocket_launcher: {
        name: 'Rocket Launcher',
        description: 'Basic defense',
        cost: { metal: 2000, crystal: 0, deuterium: 0 },
        image: 'missile-battery'
    },
    light_laser: {
        name: 'Light Laser',
        description: 'Light laser defense',
        cost: { metal: 1500, crystal: 500, deuterium: 0 },
        image: 'defense-turret'
    },
    heavy_laser: {
        name: 'Heavy Laser',
        description: 'Heavy laser defense',
        cost: { metal: 6000, crystal: 2000, deuterium: 0 },
        image: 'plasma-turret'
    },
    gauss_cannon: {
        name: 'Gauss Cannon',
        description: 'Powerful magnetic weapon',
        cost: { metal: 20000, crystal: 15000, deuterium: 2000 },
        image: 'plasma-turret'
    },
    ion_cannon: {
        name: 'Ion Cannon',
        description: 'Advanced ion weapon',
        cost: { metal: 2000, crystal: 6000, deuterium: 0 },
        image: 'ion-cannon'
    },
    plasma_turret: {
        name: 'Plasma Turret',
        description: 'Ultimate defense weapon',
        cost: { metal: 50000, crystal: 50000, deuterium: 30000 },
        image: 'plasma-turret'
    }
};

let currentPlanetData = null;
let currentTab = 'ships';

// Tab switching
document.addEventListener('DOMContentLoaded', () => {
    const tabButtons = document.querySelectorAll('.tab-button');
    tabButtons.forEach(button => {
        button.addEventListener('click', () => {
            currentTab = button.dataset.tab;
            
            tabButtons.forEach(btn => btn.classList.remove('active'));
            button.classList.add('active');

            document.querySelectorAll('.tab-content').forEach(content => {
                content.classList.remove('active');
            });
            
            document.getElementById(currentTab + 'Tab').classList.add('active');
        });
    });
});

// Update page with planet data
function updatePageData(data) {
    currentPlanetData = data;
    renderUnits();
    updateProductionQueue();
}

// Render ships or defenses
function renderUnits() {
    if (currentTab === 'ships') {
        renderShips();
    } else {
        renderDefense();
    }
}

function renderShips() {
    const grid = document.getElementById('shipsGrid');
    grid.innerHTML = '';

    const planet = currentPlanetData.planet;
    const resources = {
        metal: planet.metal,
        crystal: planet.crystal,
        deuterium: planet.deuterium
    };

    // Check if shipyard exists
    if (planet.shipyard === 0) {
        grid.innerHTML = '<p class="text-muted">Shipyard required to build ships</p>';
        return;
    }

    for (const [shipType, config] of Object.entries(SHIPS)) {
        const currentCount = planet[shipType] || 0;
        
        const canAfford = 
            resources.metal >= config.cost.metal &&
            resources.crystal >= config.cost.crystal &&
            resources.deuterium >= config.cost.deuterium;

        const card = document.createElement('div');
        card.className = 'ship-card';
        
        card.innerHTML = `
            <img src="/assets/ships/${config.image}.png" alt="${config.name}" class="ship-image" onerror="this.src='/assets/ships/fighter-interceptor.png'">
            <div class="ship-card-body">
                <div class="building-header">
                    <span class="ship-name">${config.name}</span>
                    <span class="building-level">Available: ${currentCount}</span>
                </div>
                <p class="ship-description">${config.description}</p>
                
                <div class="building-cost">
                    <div class="cost-item">
                        <img src="/assets/ui/resource-metal.png" alt="Metal" class="cost-icon">
                        <span class="cost-value ${resources.metal < config.cost.metal ? 'insufficient' : ''}">
                            ${formatNumber(config.cost.metal)}
                        </span>
                    </div>
                    <div class="cost-item">
                        <img src="/assets/ui/resource-crystal.png" alt="Crystal" class="cost-icon">
                        <span class="cost-value ${resources.crystal < config.cost.crystal ? 'insufficient' : ''}">
                            ${formatNumber(config.cost.crystal)}
                        </span>
                    </div>
                    <div class="cost-item">
                        <img src="/assets/ui/resource-deuterium.png" alt="Deuterium" class="cost-icon">
                        <span class="cost-value ${resources.deuterium < config.cost.deuterium ? 'insufficient' : ''}">
                            ${formatNumber(config.cost.deuterium)}
                        </span>
                    </div>
                </div>
                
                <div style="display: flex; gap: 10px; align-items: center; margin-top: 15px;">
                    <input type="number" class="quantity-input" value="1" min="1" max="1000" style="width: 80px; padding: 8px; background: #0f1322; border: 2px solid #2a3f5f; border-radius: 5px; color: #e0e0e0;">
                    <button 
                        class="btn-build" 
                        data-unit="${shipType}"
                        ${!canAfford ? 'disabled' : ''}
                        style="flex: 1;"
                    >
                        ${canAfford ? 'Build' : 'Insufficient Resources'}
                    </button>
                </div>
            </div>
        `;

        grid.appendChild(card);

        const buildBtn = card.querySelector('.btn-build');
        const quantityInput = card.querySelector('.quantity-input');
        buildBtn.addEventListener('click', () => {
            const quantity = parseInt(quantityInput.value) || 1;
            startProduction(shipType, quantity);
        });
    }
}

function renderDefense() {
    const grid = document.getElementById('defenseGrid');
    grid.innerHTML = '';

    const planet = currentPlanetData.planet;
    const resources = {
        metal: planet.metal,
        crystal: planet.crystal,
        deuterium: planet.deuterium
    };

    if (planet.shipyard === 0) {
        grid.innerHTML = '<p class="text-muted">Shipyard required to build defenses</p>';
        return;
    }

    for (const [defenseType, config] of Object.entries(DEFENSES)) {
        const currentCount = planet[defenseType] || 0;
        
        const canAfford = 
            resources.metal >= config.cost.metal &&
            resources.crystal >= config.cost.crystal &&
            resources.deuterium >= config.cost.deuterium;

        const card = document.createElement('div');
        card.className = 'building-card';
        
        card.innerHTML = `
            <img src="/assets/buildings/${config.image}.png" alt="${config.name}" class="building-image" onerror="this.src='/assets/buildings/defense-turret.png'">
            <div class="building-card-body">
                <div class="building-header">
                    <span class="building-name">${config.name}</span>
                    <span class="building-level">Available: ${currentCount}</span>
                </div>
                <p class="building-description">${config.description}</p>
                
                <div class="building-cost">
                    <div class="cost-item">
                        <img src="/assets/ui/resource-metal.png" alt="Metal" class="cost-icon">
                        <span class="cost-value ${resources.metal < config.cost.metal ? 'insufficient' : ''}">
                            ${formatNumber(config.cost.metal)}
                        </span>
                    </div>
                    <div class="cost-item">
                        <img src="/assets/ui/resource-crystal.png" alt="Crystal" class="cost-icon">
                        <span class="cost-value ${resources.crystal < config.cost.crystal ? 'insufficient' : ''}">
                            ${formatNumber(config.cost.crystal)}
                        </span>
                    </div>
                    <div class="cost-item">
                        <img src="/assets/ui/resource-deuterium.png" alt="Deuterium" class="cost-icon">
                        <span class="cost-value ${resources.deuterium < config.cost.deuterium ? 'insufficient' : ''}">
                            ${formatNumber(config.cost.deuterium)}
                        </span>
                    </div>
                </div>
                
                <div style="display: flex; gap: 10px; align-items: center; margin-top: 15px;">
                    <input type="number" class="quantity-input" value="1" min="1" max="1000" style="width: 80px; padding: 8px; background: #0f1322; border: 2px solid #2a3f5f; border-radius: 5px; color: #e0e0e0;">
                    <button 
                        class="btn-build" 
                        data-unit="${defenseType}"
                        ${!canAfford ? 'disabled' : ''}
                        style="flex: 1;"
                    >
                        ${canAfford ? 'Build' : 'Insufficient Resources'}
                    </button>
                </div>
            </div>
        `;

        grid.appendChild(card);

        const buildBtn = card.querySelector('.btn-build');
        const quantityInput = card.querySelector('.quantity-input');
        buildBtn.addEventListener('click', () => {
            const quantity = parseInt(quantityInput.value) || 1;
            startProduction(defenseType, quantity);
        });
    }
}

// Start production
async function startProduction(unitType, quantity) {
    if (!GameState.currentPlanet) return;

    try {
        const response = await fetch(`/api/shipyard/${GameState.currentPlanet.id}/build`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            },
            body: JSON.stringify({ unitType, quantity })
        });

        const data = await response.json();

        if (!response.ok) {
            throw new Error(data.error || 'Failed to start production');
        }

        showNotification('Success', `Production started: ${quantity}x ${unitType}`, 'success');
        
        // Reload planet data
        await loadPlanetData(GameState.currentPlanet.id);
    } catch (error) {
        showNotification('Error', error.message, 'error');
    }
}

// Update production queue display
async function updateProductionQueue() {
    if (!GameState.currentPlanet) return;

    try {
        const response = await fetch(`/api/shipyard/${GameState.currentPlanet.id}/queue`, {
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            }
        });

        const queue = await response.json();

        const queueDisplay = document.getElementById('productionQueueDisplay');
        const activeProduction = document.getElementById('activeProduction');

        if (queue && queue.length > 0) {
            queueDisplay.style.display = 'block';
            
            const item = queue[0];
            const endTime = new Date(item.end_time);
            const timeRemaining = calculateTimeRemaining(endTime);

            activeProduction.innerHTML = `
                <div class="active-construction-card">
                    <h4>${item.quantity}x ${formatUnitName(item.unit_type)}</h4>
                    <p class="construction-timer">Completes in: <span id="productionTimer">${formatTime(timeRemaining)}</span></p>
                    <button class="btn-secondary" onclick="cancelProduction(${item.id})">Cancel Production</button>
                </div>
            `;

            // Start countdown
            startProductionTimer(endTime);
        } else {
            queueDisplay.style.display = 'none';
        }
    } catch (error) {
        console.error('Error fetching production queue:', error);
    }
}

// Start production timer
function startProductionTimer(endTime) {
    const timerInterval = setInterval(() => {
        const remaining = calculateTimeRemaining(endTime);
        const timerElement = document.getElementById('productionTimer');
        
        if (!timerElement) {
            clearInterval(timerInterval);
            return;
        }

        if (remaining <= 0) {
            clearInterval(timerInterval);
            showNotification('Success', 'Production completed!', 'success');
            if (GameState.currentPlanet) {
                loadPlanetData(GameState.currentPlanet.id);
            }
        } else {
            timerElement.textContent = formatTime(remaining);
        }
    }, 1000);
}

// Cancel production
async function cancelProduction(queueId) {
    if (!confirm('Are you sure? You will only get 60% of resources back.')) {
        return;
    }

    try {
        const response = await fetch(`/api/shipyard/queue/${queueId}`, {
            method: 'DELETE',
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            }
        });

        if (!response.ok) {
            const data = await response.json();
            throw new Error(data.error || 'Failed to cancel production');
        }

        showNotification('Success', 'Production cancelled', 'success');
        
        if (GameState.currentPlanet) {
            await loadPlanetData(GameState.currentPlanet.id);
        }
    } catch (error) {
        showNotification('Error', error.message, 'error');
    }
}

// Format unit name for display
function formatUnitName(unitType) {
    return unitType
        .split('_')
        .map(word => word.charAt(0).toUpperCase() + word.slice(1))
        .join(' ');
}

// Add CSS for tab content
const style = document.createElement('style');
style.textContent = `
    .tab-content {
        display: none;
    }
    .tab-content.active {
        display: block;
    }
    .quantity-input:focus {
        outline: none;
        border-color: #4a9eff;
    }
`;
document.head.appendChild(style);
