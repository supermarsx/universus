// @ts-nocheck
// API Configuration
const API_URL = window.location.origin + '/api';

// Get stored token
function getToken() {
    return localStorage.getItem('token');
}

// Set token
function setToken(token) {
    localStorage.setItem('token');
}

// Remove token
function removeToken() {
    localStorage.removeItem('token');
}

// API Request wrapper
async function apiRequest(endpoint, options = {}) {
    const token = getToken();
    const headers = {
        'Content-Type': 'application/json',
        ...options.headers,
    };

    if (token) {
        headers['Authorization'] = `Bearer ${token}`;
    }

    const response = await fetch(`${API_URL}${endpoint}`, {
        ...options,
        headers,
    });

    const data = await response.json();

    if (!response.ok) {
        throw new Error(data.error || 'Request failed');
    }

    return data;
}

// Auth API
const AuthAPI = {
    async register(username, email, password, botChallenge) {
        return apiRequest('/auth/register', {
            method: 'POST',
            body: JSON.stringify({ username, email, password, bot_challenge: botChallenge }),
        });
    },

    async login(username, password, botChallenge) {
        return apiRequest('/auth/login', {
            method: 'POST',
            body: JSON.stringify({ username, password, bot_challenge: botChallenge }),
        });
    },
};

// Planets API
const PlanetsAPI = {
    async getAll() {
        return apiRequest('/planets');
    },

    async getById(id) {
        return apiRequest(`/planets/${id}`);
    },

    async startBuilding(planetId, buildingType) {
        return apiRequest(`/planets/${planetId}/build`, {
            method: 'POST',
            body: JSON.stringify({ buildingType }),
        });
    },

    async cancelConstruction(constructionId) {
        return apiRequest(`/planets/construction/${constructionId}`, {
            method: 'DELETE',
        });
    },
};

// Users API
const UsersAPI = {
    async getMe() {
        return apiRequest('/users/me');
    },

    async getLeaderboard() {
        return apiRequest('/users/leaderboard');
    },
};

// Generic helper (legacy compatibility)
const api = {
    get(endpoint) {
        return apiRequest(endpoint);
    },

    post(endpoint, body = {}) {
        return apiRequest(endpoint, {
            method: 'POST',
            body: body ? JSON.stringify(body) : undefined,
        });
    },

    put(endpoint, body = {}) {
        return apiRequest(endpoint, {
            method: 'PUT',
            body: body ? JSON.stringify(body) : undefined,
        });
    },

    delete(endpoint) {
        return apiRequest(endpoint, {
            method: 'DELETE',
        });
    },
};

window.api = api;

// Export for CommonJS usage in tests/mocks
module.exports = api;

// ES module default export for TypeScript imports
export default api;
