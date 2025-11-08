// @ts-nocheck
// Global game state
const GameState = {
    currentPlanet: null,
    planets: [],
    socket: null,
    user: null,
};

// Initialize game
document.addEventListener('DOMContentLoaded', async () => {
    // Check authentication
    const token = localStorage.getItem('token');
    if (!token) {
        window.location.href = '/index.html';
        return;
    }

    // Get user data
    const userData = localStorage.getItem('user');
    if (userData) {
        GameState.user = JSON.parse(userData);
        document.getElementById('username').textContent = GameState.user.username;
    }

    // Initialize Socket.io
    initializeSocket(token);

    // Load planets
    await loadPlanets();

    // Logout button
    document.getElementById('logoutBtn').addEventListener('click', () => {
        localStorage.removeItem('token');
        localStorage.removeItem('user');
        if (GameState.socket) {
            GameState.socket.disconnect();
        }
        window.location.href = '/index.html';
    });

    // Planet selector change
    const planetSelect = document.getElementById('planetSelect');
    planetSelect.addEventListener('change', async (e) => {
        const planetId = parseInt(e.target.value);
        await loadPlanetData(planetId);
    });
});

// Initialize Socket.io connection
function initializeSocket(token) {
    GameState.socket = io({
        auth: {
            token: token
        }
    });
    window.socket = GameState.socket;

    GameState.socket.on('connect', () => {
        console.log('Connected to game server');
    });

    GameState.socket.on('disconnect', () => {
        console.log('Disconnected from game server');
    });

    GameState.socket.on('connect_error', (error) => {
        console.error('Connection error:', error);
    });

    // Listen for resource updates
    GameState.socket.on('resources:update', (data) => {
        updateResourceDisplay(data);
    });

    // Listen for construction events
    GameState.socket.on('construction:complete', (data) => {
        showNotification('Construction Complete', data.message);
        if (GameState.currentPlanet) {
            loadPlanetData(GameState.currentPlanet.id);
        }
    });

    GameState.socket.on('notification:new', (event) => {
        if (window.notificationCenter) {
            window.notificationCenter.handleRealtime(event);
        }
        const title = event?.title || 'Notification';
        const message = event?.message || '';
        if (window.toast) {
            window.toast.info(`${title}: ${message}`, 6000);
        } else {
            showNotification(title, message, 'info');
        }
    });
}

// Load all planets
async function loadPlanets() {
    try {
        const response = await fetch('/api/planets', {
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            }
        });

        if (!response.ok) throw new Error('Failed to load planets');

        GameState.planets = await response.json();

        // Populate planet selector
        const planetSelect = document.getElementById('planetSelect');
        planetSelect.innerHTML = '';

        GameState.planets.forEach(planet => {
            const option = document.createElement('option');
            option.value = planet.id;
            option.textContent = `${planet.name} [${planet.galaxy}:${planet.system}:${planet.position}]`;
            planetSelect.appendChild(option);
        });

        // Load first planet
        if (GameState.planets.length > 0) {
            await loadPlanetData(GameState.planets[0].id);
        }
    } catch (error) {
        console.error('Error loading planets:', error);
        showNotification('Error', 'Failed to load planets', 'error');
    }
}

// Load specific planet data
async function loadPlanetData(planetId) {
    try {
        const response = await fetch(`/api/planets/${planetId}`, {
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            }
        });

        if (!response.ok) throw new Error('Failed to load planet data');

        const data = await response.json();
        let moonData = null;

        try {
            const moonResponse = await fetch(`/api/moons/${planetId}`, {
                headers: {
                    'Authorization': `Bearer ${localStorage.getItem('token')}`
                }
            });

            if (moonResponse.ok) {
                const moonPayload = await moonResponse.json();
                moonData = moonPayload.data;
            }
        } catch (moonError) {
            console.warn('Moon data unavailable', moonError);
        }
        GameState.currentPlanet = data.planet;
        window.currentPlanet = data.planet;
        window.currentPlanetId = data.planet.id;
        window.currentResources = {
            metal: data.planet.metal,
            crystal: data.planet.crystal,
            deuterium: data.planet.deuterium
        };

        // Update resource display
        updateResourceDisplay({
            metal: data.planet.metal,
            crystal: data.planet.crystal,
            deuterium: data.planet.deuterium,
            energy: data.production.energy
        });

        // Subscribe to planet updates via socket
        if (GameState.socket) {
            GameState.socket.emit('subscribe:planet', planetId);
        }

        // Trigger page-specific updates
        if (typeof updatePageData === 'function') {
            updatePageData({
                ...data,
                moonData,
            });
        }
    } catch (error) {
        console.error('Error loading planet data:', error);
        showNotification('Error', 'Failed to load planet data', 'error');
    }
}

// Update resource display
function updateResourceDisplay(resources) {
    document.getElementById('metalDisplay').textContent = formatNumber(Math.floor(resources.metal));
    document.getElementById('crystalDisplay').textContent = formatNumber(Math.floor(resources.crystal));
    document.getElementById('deuteriumDisplay').textContent = formatNumber(Math.floor(resources.deuterium));
    document.getElementById('energyDisplay').textContent = formatNumber(resources.energy);
}

// Format number with thousands separator
function formatNumber(num) {
    return num.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ',');
}

// Format time duration
function formatTime(seconds) {
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = seconds % 60;

    if (hours > 0) {
        return `${hours}h ${minutes}m ${secs}s`;
    } else if (minutes > 0) {
        return `${minutes}m ${secs}s`;
    } else {
        return `${secs}s`;
    }
}

// Show notification
function showNotification(title, message, type = 'info') {
    // Simple alert for now - could be enhanced with a toast system
    console.log(`[${type.toUpperCase()}] ${title}: ${message}`);
}

// Calculate time remaining
function calculateTimeRemaining(endTime) {
    const now = new Date();
    const end = new Date(endTime);
    const diff = Math.max(0, Math.floor((end - now) / 1000));
    return diff;
}
