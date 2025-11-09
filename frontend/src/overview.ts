// @ts-nocheck
// Overview Page Logic

import i18next from 'i18next';
import './i18n';

// i18next is now initialized centrally in `frontend/src/i18n.ts`
// Example: set welcome message (safe to call since the initializer runs on import)
const welcomeEl = document.getElementById('welcomeMessage');
if (welcomeEl) welcomeEl.textContent = i18next.t('overview.welcome');
const descEl = document.getElementById('overviewDescription');
if (descEl) descEl.textContent = i18next.t('overview.description');
const startBtn = document.getElementById('startButton');
if (startBtn) startBtn.textContent = i18next.t('overview.startButton');

// Update page with planet data
function updatePageData(data) {
    const planet = data.planet;
    const production = data.production;
    const constructionQueue = data.constructionQueue;

    // Update planet information
    document.getElementById('planetName').textContent = planet.name;
    document.getElementById('planetCoords').textContent = 
        `[${planet.galaxy}:${planet.system}:${planet.position}]`;
    document.getElementById('planetTemp').textContent = `${planet.temperature}°C`;
    document.getElementById('planetDiameter').textContent = `${formatNumber(planet.diameter)} km`;

    // Update production information
    document.getElementById('metalProd').textContent = formatNumber(production.metal);
    document.getElementById('crystalProd').textContent = formatNumber(production.crystal);
    document.getElementById('deuteriumProd').textContent = formatNumber(production.deuterium);
    document.getElementById('energyProd').textContent = production.energy;

    // Update construction queue
    const queueElement = document.getElementById('constructionQueue');
    if (constructionQueue && constructionQueue.length > 0) {
        queueElement.innerHTML = '';
        constructionQueue.forEach(item => {
            const queueItem = document.createElement('div');
            queueItem.className = 'queue-item';
            
            const endTime = new Date(item.end_time);
            const timeRemaining = calculateTimeRemaining(endTime);
            
            queueItem.innerHTML = `
                <p><strong>${formatBuildingName(item.building_type)}</strong> (Level ${item.level})</p>
                <p class="text-muted">Completes in: ${formatTime(timeRemaining)}</p>
            `;
            
            queueElement.appendChild(queueItem);

            // Start countdown timer
            startCountdown(queueItem, endTime);
        });
    } else {
        queueElement.innerHTML = '<p class="text-muted">No construction in progress</p>';
    }

    // Update quick stats
    document.getElementById('totalPlanets').textContent = GameState.planets.length;
}

// Start countdown timer
function startCountdown(element, endTime) {
    const timerInterval = setInterval(() => {
        const remaining = calculateTimeRemaining(endTime);
        
        if (remaining <= 0) {
            clearInterval(timerInterval);
            // Reload planet data when construction finishes
            if (GameState.currentPlanet) {
                loadPlanetData(GameState.currentPlanet.id);
            }
        } else {
            const timeElement = element.querySelector('.text-muted');
            if (timeElement) {
                timeElement.textContent = `Completes in: ${formatTime(remaining)}`;
            }
        }
    }, 1000);
}

// Format building name for display
function formatBuildingName(buildingType) {
    return buildingType
        .split('_')
        .map(word => word.charAt(0).toUpperCase() + word.slice(1))
        .join(' ');
}
