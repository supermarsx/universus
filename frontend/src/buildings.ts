// @ts-nocheck
// Buildings configuration (matches backend)
const BUILDINGS = {
    metal_mine: {
        name: 'Metal Mine',
        description: 'Extracts metal ore from the planet',
        baseCost: { metal: 60, crystal: 15, deuterium: 0 },
        costMultiplier: 1.5,
        image: 'metal-mine-1'
    },
    crystal_mine: {
        name: 'Crystal Mine',
        description: 'Extracts crystal from the planet',
        baseCost: { metal: 48, crystal: 24, deuterium: 0 },
        costMultiplier: 1.6,
        image: 'crystal-mine-1'
    },
    deuterium_synthesizer: {
        name: 'Deuterium Synthesizer',
        description: 'Synthesizes deuterium fuel',
        baseCost: { metal: 225, crystal: 75, deuterium: 0 },
        costMultiplier: 1.5,
        image: 'deuterium-plant'
    },
    solar_plant: {
        name: 'Solar Plant',
        description: 'Provides energy for the planet',
        baseCost: { metal: 75, crystal: 30, deuterium: 0 },
        costMultiplier: 1.5,
        image: 'solar-plant'
    },
    fusion_reactor: {
        name: 'Fusion Reactor',
        description: 'Advanced energy production facility',
        baseCost: { metal: 900, crystal: 360, deuterium: 180 },
        costMultiplier: 1.8,
        image: 'fusion-reactor-1'
    },
    robotics_factory: {
        name: 'Robotics Factory',
        description: 'Reduces construction time for buildings',
        baseCost: { metal: 400, crystal: 120, deuterium: 200 },
        costMultiplier: 2.0,
        image: 'robotics-factory'
    },
    shipyard: {
        name: 'Shipyard',
        description: 'Allows construction of ships and defenses',
        baseCost: { metal: 400, crystal: 200, deuterium: 100 },
        costMultiplier: 2.0,
        image: 'shipyard-1'
    },
    research_lab: {
        name: 'Research Lab',
        description: 'Enables technology research',
        baseCost: { metal: 200, crystal: 400, deuterium: 200 },
        costMultiplier: 2.0,
        image: 'research-lab-basic'
    },
    metal_storage: {
        name: 'Metal Storage',
        description: 'Increases metal storage capacity',
        baseCost: { metal: 1000, crystal: 0, deuterium: 0 },
        costMultiplier: 2.0,
        image: 'metal-mine-1'
    },
    crystal_storage: {
        name: 'Crystal Storage',
        description: 'Increases crystal storage capacity',
        baseCost: { metal: 1000, crystal: 500, deuterium: 0 },
        costMultiplier: 2.0,
        image: 'crystal-mine-1'
    },
    deuterium_tank: {
        name: 'Deuterium Tank',
        description: 'Increases deuterium storage capacity',
        baseCost: { metal: 1000, crystal: 1000, deuterium: 0 },
        costMultiplier: 2.0,
        image: 'deuterium-plant'
    },
    nanite_factory: {
        name: 'Nanite Factory',
        description: 'Drastically reduces construction time',
        baseCost: { metal: 1000000, crystal: 500000, deuterium: 100000 },
        costMultiplier: 2.0,
        image: 'nanite-factory'
    },
};

let currentPlanetData = null;

// Update page with planet data
function updatePageData(data) {
    currentPlanetData = data;
    renderBuildings();
    updateConstructionQueue(data.constructionQueue);
}

// Calculate building cost for next level
function calculateCost(buildingType, currentLevel) {
    const config = BUILDINGS[buildingType];
    if (!config) return null;

    const factor = Math.pow(config.costMultiplier, currentLevel);
    return {
        metal: Math.floor(config.baseCost.metal * factor),
        crystal: Math.floor(config.baseCost.crystal * factor),
        deuterium: Math.floor(config.baseCost.deuterium * factor),
    };
}

// Render buildings grid
function renderBuildings() {
    const grid = document.getElementById('buildingsGrid');
    grid.innerHTML = '';

    const planet = currentPlanetData.planet;
    const resources = {
        metal: planet.metal,
        crystal: planet.crystal,
        deuterium: planet.deuterium,
    };

    const isBuilding = currentPlanetData.constructionQueue && 
                      currentPlanetData.constructionQueue.length > 0;

    for (const [buildingType, config] of Object.entries(BUILDINGS)) {
        const currentLevel = planet[buildingType] || 0;
        const cost = calculateCost(buildingType, currentLevel);
        
        const canAfford = 
            resources.metal >= cost.metal &&
            resources.crystal >= cost.crystal &&
            resources.deuterium >= cost.deuterium;

        const card = document.createElement('div');
        card.className = 'building-card';
        
        card.innerHTML = `
            <img src="/assets/buildings/${config.image}.png" alt="${config.name}" class="building-image" onerror="this.src='/assets/buildings/metal-mine-1.png'">
            <div class="building-card-body">
                <div class="building-header">
                    <span class="building-name">${config.name}</span>
                    <span class="building-level">Level ${currentLevel}</span>
                </div>
                <p class="building-description">${config.description}</p>
                
                <div class="building-cost">
                    <div class="cost-item">
                        <img src="/assets/ui/resource-metal.png" alt="Metal" class="cost-icon">
                        <span class="cost-value ${resources.metal < cost.metal ? 'insufficient' : ''}">
                            ${formatNumber(cost.metal)}
                        </span>
                    </div>
                    <div class="cost-item">
                        <img src="/assets/ui/resource-crystal.png" alt="Crystal" class="cost-icon">
                        <span class="cost-value ${resources.crystal < cost.crystal ? 'insufficient' : ''}">
                            ${formatNumber(cost.crystal)}
                        </span>
                    </div>
                    <div class="cost-item">
                        <img src="/assets/ui/resource-deuterium.png" alt="Deuterium" class="cost-icon">
                        <span class="cost-value ${resources.deuterium < cost.deuterium ? 'insufficient' : ''}">
                            ${formatNumber(cost.deuterium)}
                        </span>
                    </div>
                </div>
                
                <button 
                    class="btn-build" 
                    data-building="${buildingType}"
                    ${!canAfford || isBuilding ? 'disabled' : ''}
                >
                    ${isBuilding ? 'Building in Progress' : canAfford ? `Upgrade to Level ${currentLevel + 1}` : 'Insufficient Resources'}
                </button>
            </div>
        `;

        grid.appendChild(card);

        // Add click handler for build button
        const buildBtn = card.querySelector('.btn-build');
        buildBtn.addEventListener('click', () => startBuilding(buildingType));
    }
}

// Start building construction
async function startBuilding(buildingType) {
    if (!GameState.currentPlanet) return;

    try {
        const response = await fetch(`/api/planets/${GameState.currentPlanet.id}/build`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            },
            body: JSON.stringify({ buildingType })
        });

        const data = await response.json();

        if (!response.ok) {
            throw new Error(data.error || 'Failed to start construction');
        }

        showNotification('Success', `${BUILDINGS[buildingType].name} construction started`, 'success');
        
        // Reload planet data
        await loadPlanetData(GameState.currentPlanet.id);
    } catch (error) {
        showNotification('Error', error.message, 'error');
    }
}

// Update construction queue display
function updateConstructionQueue(queue) {
    const queueDisplay = document.getElementById('constructionQueueDisplay');
    const activeConstruction = document.getElementById('activeConstruction');

    if (queue && queue.length > 0) {
        queueDisplay.style.display = 'block';
        
        const item = queue[0];
        const endTime = new Date(item.end_time);
        const timeRemaining = calculateTimeRemaining(endTime);

        activeConstruction.innerHTML = `
            <div class="active-construction-card">
                <h4>${BUILDINGS[item.building_type].name} (Level ${item.level})</h4>
                <p class="construction-timer">Completes in: <span id="constructionTimer">${formatTime(timeRemaining)}</span></p>
                <button class="btn-secondary" onclick="cancelConstruction(${item.id})">Cancel Construction</button>
            </div>
        `;

        // Start countdown
        startConstructionTimer(endTime);
    } else {
        queueDisplay.style.display = 'none';
    }
}

// Start construction timer
function startConstructionTimer(endTime) {
    const timerInterval = setInterval(() => {
        const remaining = calculateTimeRemaining(endTime);
        const timerElement = document.getElementById('constructionTimer');
        
        if (!timerElement) {
            clearInterval(timerInterval);
            return;
        }

        if (remaining <= 0) {
            clearInterval(timerInterval);
            showNotification('Success', 'Construction completed!', 'success');
            if (GameState.currentPlanet) {
                loadPlanetData(GameState.currentPlanet.id);
            }
        } else {
            timerElement.textContent = formatTime(remaining);
        }
    }, 1000);
}

// Cancel construction
async function cancelConstruction(constructionId) {
    if (!confirm('Are you sure? You will only get 60% of resources back.')) {
        return;
    }

    try {
        const response = await fetch(`/api/planets/construction/${constructionId}`, {
            method: 'DELETE',
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            }
        });

        if (!response.ok) {
            const data = await response.json();
            throw new Error(data.error || 'Failed to cancel construction');
        }

        showNotification('Success', 'Construction cancelled', 'success');
        
        if (GameState.currentPlanet) {
            await loadPlanetData(GameState.currentPlanet.id);
        }
    } catch (error) {
        showNotification('Error', error.message, 'error');
    }
}

// Add CSS for construction queue display
const style = document.createElement('style');
style.textContent = `
    .construction-queue-display {
        background: rgba(20, 25, 45, 0.95);
        border: 2px solid #4a9eff;
        border-radius: 10px;
        padding: 20px;
        margin-bottom: 30px;
    }

    .construction-queue-display h3 {
        color: #4a9eff;
        margin-bottom: 15px;
    }

    .active-construction-card {
        background: rgba(15, 19, 34, 0.7);
        padding: 15px;
        border-radius: 5px;
    }

    .active-construction-card h4 {
        color: #4a9eff;
        margin-bottom: 10px;
    }

    .construction-timer {
        font-size: 18px;
        font-weight: bold;
        color: #22c55e;
        margin: 10px 0;
    }

    .building-description {
        color: #8a9db5;
        font-size: 14px;
        margin: 10px 0;
        line-height: 1.5;
    }
`;
document.head.appendChild(style);
