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
    async register(username, email, password) {
        return apiRequest('/auth/register', {
            method: 'POST',
            body: JSON.stringify({ username, email, password }),
        });
    },

    async login(username, password) {
        return apiRequest('/auth/login', {
            method: 'POST',
            body: JSON.stringify({ username, password }),
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
