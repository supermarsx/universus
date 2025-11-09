// @ts-nocheck
import i18next from 'i18next';
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

const MOON_BUILDINGS = {
    lunar_base: {
        name: 'Lunar Base',
        description: 'Expands available fields on the moon.',
        baseCost: { metal: 20000, crystal: 40000, deuterium: 0 },
        costMultiplier: 2.0,
        image: 'lunar-base',
    },
    moon_robotics_factory: {
        name: 'Moon Robotics Factory',
        description: 'Reduces construction time for moon buildings.',
        baseCost: { metal: 400, crystal: 120, deuterium: 200 },
        costMultiplier: 2.0,
        image: 'robotics-factory',
    },
    moon_shipyard: {
        name: 'Moon Shipyard',
        description: 'Required before defenses or jump gates can be assembled from the moon.',
        baseCost: { metal: 200, crystal: 400, deuterium: 200 },
        costMultiplier: 2.0,
        image: 'shipyard-1',
    },
    moon_nanite_factory: {
        name: 'Moon Nanite Factory',
        description: 'Provides additional speed bonuses for lunar construction.',
        baseCost: { metal: 1000000, crystal: 500000, deuterium: 100000 },
        costMultiplier: 2.0,
        image: 'nanite-factory',
    },
    sensor_phalanx: {
        name: 'Sensor Phalanx',
        description: 'Allows scanning of nearby systems for fleet activity.',
        baseCost: { metal: 20000, crystal: 40000, deuterium: 20000 },
        costMultiplier: 2.0,
        image: 'sensor-phalanx',
    },
    jump_gate: {
        name: 'Jump Gate',
        description: 'Instantly transfers ships between owned moons.',
        baseCost: { metal: 2000000, crystal: 4000000, deuterium: 2000000 },
        costMultiplier: 2.0,
        image: 'jump-gate',
    },
};

let currentPlanetData = null;

function getBuildingConfig(buildingType) {
    return BUILDINGS[buildingType] || MOON_BUILDINGS[buildingType];
}

// Update page with planet data
function updatePageData(data) {
    currentPlanetData = data;
    renderBuildings();
    updateConstructionQueue(data.constructionQueue);
    renderMoonSection();
}

// Calculate building cost for next level
function calculateCost(configMap, buildingType, currentLevel) {
    const config = configMap[buildingType];
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
        const cost = calculateCost(BUILDINGS, buildingType, currentLevel);
        
        const canAfford = 
            resources.metal >= cost.metal &&
            resources.crystal >= cost.crystal &&
            resources.deuterium >= cost.deuterium;

        const card = document.createElement('div');
        card.className = 'building-card';
        
        card.innerHTML = `
            <img src="/assets/buildings/${config.image}.png" alt="${i18next.t(`buildings.list.${buildingType}.name`, { defaultValue: config.name || buildingType })}" class="building-image" onerror="this.src='/assets/buildings/metal-mine-1.png'">
            <div class="building-card-body">
                <div class="building-header">
                    <span class="building-name">${i18next.t(`buildings.list.${buildingType}.name`, { defaultValue: config.name || buildingType })}</span>
                    <span class="building-level">${i18next.t('buildings.level', { defaultValue: 'Level' })} ${currentLevel}</span>
                </div>
                <p class="building-description">${i18next.t(`buildings.list.${buildingType}.description`, { defaultValue: config.description || '' })}</p>
                
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
                    ${isBuilding ? i18next.t('buildings.buildingInProgress', { defaultValue: 'Building in Progress' }) : canAfford ? i18next.t('buildings.upgradeToLevel', { level: currentLevel + 1, defaultValue: `Upgrade to Level ${currentLevel + 1}` }) : i18next.t('buildings.insufficientResources', { defaultValue: 'Insufficient Resources' })}
                </button>
            </div>
        `;

        grid.appendChild(card);

        // Add click handler for build button
        const buildBtn = card.querySelector('.btn-build');
        buildBtn.addEventListener('click', () => startBuilding(buildingType));
    }
}

function renderMoonSection() {
    const section = document.getElementById('moonBuildSection');
    const statusEl = document.getElementById('moonStatus');
    const resourceSummary = document.getElementById('moonResourceSummary');
    const grid = document.getElementById('moonBuildingsGrid');
    const moonQueue = document.getElementById('moonConstructionQueue');

    if (!section || !statusEl || !grid || !moonQueue || !resourceSummary) {
        return;
    }

    const moonData = currentPlanetData?.moonData;
        if (!moonData || !moonData.moon) {
        statusEl.textContent = i18next.t('buildings.noMoonDetected', { defaultValue: 'No moon detected in orbit.' });
        resourceSummary.classList.add('hidden');
        grid.innerHTML = `<div class="moon-empty card-compact">${i18next.t('buildings.generateMoonHint', { defaultValue: 'Generate enough debris in battle to roll for a moon.' })}</div>`;
        moonQueue.style.display = 'none';
        return;
    }

    const moon = moonData.moon;
    statusEl.textContent = `${moon.name} • ${moon.diameter} km • Fields ${moon.used_fields}/${moon.total_fields}`;
    resourceSummary.classList.remove('hidden');
    resourceSummary.innerHTML = `
        <div class="resource-chip"><img src="/assets/ui/resource-metal.png" alt="Metal">${formatNumber(moon.metal)}</div>
        <div class="resource-chip"><img src="/assets/ui/resource-crystal.png" alt="Crystal">${formatNumber(moon.crystal)}</div>
        <div class="resource-chip"><img src="/assets/ui/resource-deuterium.png" alt="Deuterium">${formatNumber(moon.deuterium)}</div>
    `;

    renderMoonBuildingsGrid(moon, moonData);
    updateMoonConstructionQueue(moonData.constructionQueue);
}

function renderMoonBuildingsGrid(moon, moonData) {
    const grid = document.getElementById('moonBuildingsGrid');
    if (!grid) return;

    grid.innerHTML = '';
    const resources = {
        metal: moon.metal,
        crystal: moon.crystal,
        deuterium: moon.deuterium,
    };
    const isBuilding = moonData.constructionQueue && moonData.constructionQueue.length > 0;

    for (const [buildingType, config] of Object.entries(MOON_BUILDINGS)) {
        const currentLevel = moon[buildingType] || 0;
        const cost = calculateCost(MOON_BUILDINGS, buildingType, currentLevel);
        if (!cost) continue;

        const canAfford =
            resources.metal >= cost.metal &&
            resources.crystal >= cost.crystal &&
            resources.deuterium >= cost.deuterium;

        const card = document.createElement('div');
        card.className = 'building-card';
        card.innerHTML = `
            <img src="/assets/buildings/${config.image}.png" alt="${i18next.t(`buildings.list.${buildingType}.name`, { defaultValue: config.name || buildingType })}" class="building-image" onerror="this.src='/assets/buildings/metal-mine-1.png'">
            <div class="building-card-body">
                <div class="building-header">
                    <span class="building-name">${i18next.t(`buildings.list.${buildingType}.name`, { defaultValue: config.name || buildingType })}</span>
                    <span class="building-level">${i18next.t('buildings.level', { defaultValue: 'Level' })} ${currentLevel}</span>
                </div>
                <p class="building-description">${i18next.t(`buildings.list.${buildingType}.description`, { defaultValue: config.description || '' })}</p>
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
                    ${isBuilding ? i18next.t('buildings.moonQueueBusy', { defaultValue: 'Moon Queue Busy' }) : canAfford ? i18next.t('buildings.upgradeToLevel', { level: currentLevel + 1, defaultValue: `Upgrade to Level ${currentLevel + 1}` }) : i18next.t('buildings.insufficientResources', { defaultValue: 'Insufficient Resources' })}
                </button>
            </div>
        `;

        grid.appendChild(card);
        const buildBtn = card.querySelector('.btn-build');
        buildBtn.addEventListener('click', () =>
            startBuilding(buildingType, { locationType: 'moon', moonId: moon.id })
        );
    }
}

// Start building construction
async function startBuilding(buildingType, options = {}) {
    if (!GameState.currentPlanet) return;

    try {
        const response = await fetch(`/api/planets/${GameState.currentPlanet.id}/build`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            },
            body: JSON.stringify({
                buildingType,
                locationType: options.locationType,
                moonId: options.moonId,
            })
        });

        const data = await response.json();

        if (!response.ok) {
            throw new Error(data.error || 'Failed to start construction');
        }

        const config = getBuildingConfig(buildingType);
        const locationLabel = options.locationType === 'moon' ? i18next.t('buildings.moon', { defaultValue: 'Moon' }) : i18next.t('buildings.planet', { defaultValue: 'Planet' });
        showNotification(i18next.t('buildings.constructionStarted', { unit: i18next.t(`buildings.list.${buildingType}.name`, { defaultValue: config?.name || buildingType }), location: locationLabel }), i18next.t('general.success', { defaultValue: 'Success' }));
        
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
        const config = getBuildingConfig(item.building_type);

        activeConstruction.innerHTML = `
            <div class="active-construction-card">
                <h4>${i18next.t(`buildings.list.${item.building_type}.name`, { defaultValue: config?.name || item.building_type })} (${i18next.t('buildings.level', { defaultValue: 'Level' })} ${item.level})</h4>
                <p class="construction-timer">${i18next.t('overview.completesIn', { time: formatTime(timeRemaining), defaultValue: `Completes in: ${formatTime(timeRemaining)}` })}</p>
                <button class="btn-secondary" onclick="cancelConstruction(${item.id})">${i18next.t('buildings.cancelButton', { defaultValue: 'Cancel Construction' })}</button>
            </div>
        `;

        // Start countdown
        startConstructionTimer(endTime, 'constructionTimer');
    } else {
        queueDisplay.style.display = 'none';
    }
}

function updateMoonConstructionQueue(queue) {
    const queueDisplay = document.getElementById('moonConstructionQueue');
    const activeConstruction = document.getElementById('activeMoonConstruction');

    if (!queueDisplay || !activeConstruction) {
        return;
    }

    if (queue && queue.length > 0) {
        queueDisplay.style.display = 'block';

        const item = queue[0];
        const endTime = new Date(item.end_time);
        const timeRemaining = calculateTimeRemaining(endTime);
        const config = getBuildingConfig(item.building_type);

        activeConstruction.innerHTML = `
            <div class="active-construction-card">
                <h4>${i18next.t(`buildings.list.${item.building_type}.name`, { defaultValue: config?.name || item.building_type })} (${i18next.t('buildings.level', { defaultValue: 'Level' })} ${item.level})</h4>
                <p class="construction-timer">${i18next.t('overview.completesIn', { time: formatTime(timeRemaining), defaultValue: `Completes in: ${formatTime(timeRemaining)}` })}</p>
                <button class="btn-secondary" onclick="cancelConstruction(${item.id})">${i18next.t('buildings.cancelButton', { defaultValue: 'Cancel Construction' })}</button>
            </div>
        `;

        startConstructionTimer(endTime, 'moonConstructionTimer');
    } else {
        queueDisplay.style.display = 'none';
    }
}

// Start construction timer
function startConstructionTimer(endTime, timerElementId) {
    const timerInterval = setInterval(() => {
        const remaining = calculateTimeRemaining(endTime);
        const timerElement = document.getElementById(timerElementId);
        
        if (!timerElement) {
            clearInterval(timerInterval);
            return;
        }

        if (remaining <= 0) {
            clearInterval(timerInterval);
            showNotification(i18next.t('buildings.constructionCompleted', { defaultValue: 'Construction completed!' }), i18next.t('general.success', { defaultValue: 'Success' }));
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
    if (!confirm(i18next.t('buildings.cancelConfirm', { defaultValue: 'Are you sure? You will only get 60% of resources back.' }))) {
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

            showNotification(i18next.t('buildings.constructionCancelled', { defaultValue: 'Construction cancelled' }), i18next.t('general.success', { defaultValue: 'Success' }));
        
        if (GameState.currentPlanet) {
            await loadPlanetData(GameState.currentPlanet.id);
        }
    } catch (error) {
        showNotification('Error', error.message, 'error');
    }
}

// Styles moved to global stylesheet
