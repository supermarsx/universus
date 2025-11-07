/**
 * Leaderboard Page JavaScript
 * Handles player and alliance ranking display with real-time updates
 */

const API_BASE_URL = 'http://localhost:3000/api';

// State management
let currentTab = 'players';
let playerPage = 0;
let alliancePage = 0;
const PAGE_SIZE = 50;
let socket = null;
let currentUserId = null;

/**
 * Initialize the leaderboard page
 */
async function init() {
    // Check authentication
    const token = localStorage.getItem('token');
    if (!token) {
        window.location.href = '/login.html';
        return;
    }

    // Get user info
    await fetchUserInfo();

    // Setup event listeners
    setupEventListeners();

    // Setup socket connection
    setupSocket();

    // Load initial data
    await loadPlayersLeaderboard();
    await fetchMyRank();

    // Setup auto-refresh
    setInterval(() => {
        if (currentTab === 'players') {
            loadPlayersLeaderboard();
        } else if (currentTab === 'alliances') {
            loadAlliancesLeaderboard();
        } else if (currentTab === 'myrank') {
            loadMyRankingDetail();
        }
    }, 30000); // Refresh every 30 seconds
}

/**
 * Fetch current user information
 */
async function fetchUserInfo() {
    try {
        const response = await fetch(`${API_BASE_URL}/users/me`, {
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            }
        });

        if (!response.ok) {
            throw new Error('Failed to fetch user info');
        }

        const data = await response.json();
        currentUserId = data.id;
        document.getElementById('username').textContent = data.username;
    } catch (error) {
        console.error('Error fetching user info:', error);
    }
}

/**
 * Setup event listeners
 */
function setupEventListeners() {
    // Tab switching
    document.querySelectorAll('.tab-button').forEach(button => {
        button.addEventListener('click', () => {
            const tab = button.getAttribute('data-tab');
            switchTab(tab);
        });
    });

    // Pagination - Players
    document.getElementById('playersPrevBtn').addEventListener('click', () => {
        if (playerPage > 0) {
            playerPage--;
            loadPlayersLeaderboard();
        }
    });

    document.getElementById('playersNextBtn').addEventListener('click', () => {
        playerPage++;
        loadPlayersLeaderboard();
    });

    // Pagination - Alliances
    document.getElementById('alliancesPrevBtn').addEventListener('click', () => {
        if (alliancePage > 0) {
            alliancePage--;
            loadAlliancesLeaderboard();
        }
    });

    document.getElementById('alliancesNextBtn').addEventListener('click', () => {
        alliancePage++;
        loadAlliancesLeaderboard();
    });

    // Logout
    document.getElementById('logoutBtn').addEventListener('click', logout);
}

/**
 * Setup WebSocket connection for real-time updates
 */
function setupSocket() {
    socket = io('http://localhost:3000', {
        auth: {
            token: localStorage.getItem('token')
        }
    });

    socket.on('connect', () => {
        console.log('Connected to server');
    });

    socket.on('leaderboard:updated', () => {
        console.log('Leaderboard updated, refreshing...');
        if (currentTab === 'players') {
            loadPlayersLeaderboard();
        } else if (currentTab === 'alliances') {
            loadAlliancesLeaderboard();
        }
    });

    socket.on('disconnect', () => {
        console.log('Disconnected from server');
    });
}

/**
 * Switch between tabs
 */
function switchTab(tab) {
    currentTab = tab;

    // Update tab buttons
    document.querySelectorAll('.tab-button').forEach(button => {
        if (button.getAttribute('data-tab') === tab) {
            button.classList.add('active');
        } else {
            button.classList.remove('active');
        }
    });

    // Show/hide tab content
    document.getElementById('playersTab').style.display = tab === 'players' ? 'block' : 'none';
    document.getElementById('alliancesTab').style.display = tab === 'alliances' ? 'block' : 'none';
    document.getElementById('myrankTab').style.display = tab === 'myrank' ? 'block' : 'none';

    // Load data for the new tab
    if (tab === 'players') {
        loadPlayersLeaderboard();
    } else if (tab === 'alliances') {
        loadAlliancesLeaderboard();
    } else if (tab === 'myrank') {
        loadMyRankingDetail();
    }
}

/**
 * Load players leaderboard
 */
async function loadPlayersLeaderboard() {
    const loadingEl = document.getElementById('playersLoading');
    const tableEl = document.getElementById('playersTable');
    const emptyEl = document.getElementById('playersEmpty');
    const paginationEl = document.getElementById('playersPagination');
    const tbody = document.getElementById('playersTableBody');

    try {
        loadingEl.style.display = 'block';
        tableEl.style.display = 'none';
        emptyEl.style.display = 'none';
        paginationEl.style.display = 'none';

        const response = await fetch(
            `${API_BASE_URL}/leaderboard/players?limit=${PAGE_SIZE}&offset=${playerPage * PAGE_SIZE}`,
            {
                headers: {
                    'Authorization': `Bearer ${localStorage.getItem('token')}`
                }
            }
        );

        if (!response.ok) {
            throw new Error('Failed to fetch players leaderboard');
        }

        const players = await response.json();

        loadingEl.style.display = 'none';

        if (players.length === 0) {
            emptyEl.style.display = 'block';
            return;
        }

        // Render table
        tbody.innerHTML = players.map(player => {
            const isCurrentPlayer = player.userId === currentUserId;
            const rankBadgeClass = getRankBadgeClass(player.rank);

            return `
                <tr class="${isCurrentPlayer ? 'current-player' : ''}">
                    <td>
                        <div class="rank-badge ${rankBadgeClass}">${player.rank}</div>
                    </td>
                    <td>
                        <span class="player-name">${escapeHtml(player.username)}</span>
                    </td>
                    <td>
                        ${player.allianceTag ? `<span class="alliance-tag">${escapeHtml(player.allianceTag)}</span>` : '-'}
                    </td>
                    <td>
                        <div class="score-value">${formatNumber(player.totalScore)}</div>
                    </td>
                    <td>${formatNumber(player.buildingScore || 0)}</td>
                    <td>${formatNumber(player.researchScore || 0)}</td>
                    <td>${formatNumber(player.fleetScore || 0)}</td>
                    <td>${formatNumber(player.defenseScore || 0)}</td>
                </tr>
            `;
        }).join('');

        tableEl.style.display = 'table';
        paginationEl.style.display = 'flex';

        // Update pagination
        document.getElementById('playersCurrentPage').textContent = playerPage + 1;
        document.getElementById('playersPrevBtn').disabled = playerPage === 0;
        document.getElementById('playersNextBtn').disabled = players.length < PAGE_SIZE;

        // Update total players count
        if (players.length > 0) {
            document.getElementById('totalPlayers').textContent = players[players.length - 1].rank + '+';
        }

    } catch (error) {
        console.error('Error loading players leaderboard:', error);
        loadingEl.style.display = 'none';
        emptyEl.style.display = 'block';
        emptyEl.textContent = 'Error loading leaderboard';
    }
}

/**
 * Load alliances leaderboard
 */
async function loadAlliancesLeaderboard() {
    const loadingEl = document.getElementById('alliancesLoading');
    const tableEl = document.getElementById('alliancesTable');
    const emptyEl = document.getElementById('alliancesEmpty');
    const paginationEl = document.getElementById('alliancesPagination');
    const tbody = document.getElementById('alliancesTableBody');

    try {
        loadingEl.style.display = 'block';
        tableEl.style.display = 'none';
        emptyEl.style.display = 'none';
        paginationEl.style.display = 'none';

        const response = await fetch(
            `${API_BASE_URL}/leaderboard/alliances?limit=${PAGE_SIZE}&offset=${alliancePage * PAGE_SIZE}`,
            {
                headers: {
                    'Authorization': `Bearer ${localStorage.getItem('token')}`
                }
            }
        );

        if (!response.ok) {
            throw new Error('Failed to fetch alliances leaderboard');
        }

        const alliances = await response.json();

        loadingEl.style.display = 'none';

        if (alliances.length === 0) {
            emptyEl.style.display = 'block';
            return;
        }

        // Render table
        tbody.innerHTML = alliances.map(alliance => {
            const rankBadgeClass = getRankBadgeClass(alliance.rank);

            return `
                <tr>
                    <td>
                        <div class="rank-badge ${rankBadgeClass}">${alliance.rank}</div>
                    </td>
                    <td>
                        <span class="player-name">${escapeHtml(alliance.allianceName)}</span>
                    </td>
                    <td>
                        <span class="alliance-tag">${escapeHtml(alliance.allianceTag)}</span>
                    </td>
                    <td>
                        <div class="score-value">${formatNumber(alliance.totalScore)}</div>
                    </td>
                    <td>${alliance.memberCount}</td>
                    <td>${formatNumber(alliance.averageScore)}</td>
                </tr>
            `;
        }).join('');

        tableEl.style.display = 'table';
        paginationEl.style.display = 'flex';

        // Update pagination
        document.getElementById('alliancesCurrentPage').textContent = alliancePage + 1;
        document.getElementById('alliancesPrevBtn').disabled = alliancePage === 0;
        document.getElementById('alliancesNextBtn').disabled = alliances.length < PAGE_SIZE;

        // Update total alliances count
        if (alliances.length > 0) {
            document.getElementById('totalAlliances').textContent = alliances[alliances.length - 1].rank + '+';
        }

    } catch (error) {
        console.error('Error loading alliances leaderboard:', error);
        loadingEl.style.display = 'none';
        emptyEl.style.display = 'block';
        emptyEl.textContent = 'Error loading leaderboard';
    }
}

/**
 * Fetch and display current user's rank
 */
async function fetchMyRank() {
    try {
        const response = await fetch(`${API_BASE_URL}/leaderboard/me`, {
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            }
        });

        if (!response.ok) {
            throw new Error('Failed to fetch my rank');
        }

        const data = await response.json();

        document.getElementById('myRank').textContent = data.player.rank || '-';
        document.getElementById('myTotalScore').textContent = formatNumber(data.player.totalScore || 0);

    } catch (error) {
        console.error('Error fetching my rank:', error);
    }
}

/**
 * Load detailed ranking information for current user
 */
async function loadMyRankingDetail() {
    try {
        const response = await fetch(`${API_BASE_URL}/leaderboard/me`, {
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            }
        });

        if (!response.ok) {
            throw new Error('Failed to fetch my ranking details');
        }

        const data = await response.json();

        // Update detailed stats
        document.getElementById('detailMyRank').textContent = data.player.rank || '-';
        document.getElementById('detailTotalScore').textContent = formatNumber(data.player.totalScore || 0);
        document.getElementById('detailBuildingScore').textContent = formatNumber(data.player.buildingScore || 0);
        document.getElementById('detailResearchScore').textContent = formatNumber(data.player.researchScore || 0);
        document.getElementById('detailFleetScore').textContent = formatNumber(data.player.fleetScore || 0);
        document.getElementById('detailDefenseScore').textContent = formatNumber(data.player.defenseScore || 0);

        // Render nearby players
        const tbody = document.getElementById('nearbyPlayersTableBody');
        tbody.innerHTML = data.neighbors.map(player => {
            const isCurrentPlayer = player.userId === currentUserId;
            const rankBadgeClass = getRankBadgeClass(player.rank);

            return `
                <tr class="${isCurrentPlayer ? 'current-player' : ''}">
                    <td>
                        <div class="rank-badge ${rankBadgeClass}">${player.rank}</div>
                    </td>
                    <td>
                        <span class="player-name">${escapeHtml(player.username)}</span>
                    </td>
                    <td>
                        ${player.allianceTag ? `<span class="alliance-tag">${escapeHtml(player.allianceTag)}</span>` : '-'}
                    </td>
                    <td>
                        <div class="score-value">${formatNumber(player.totalScore)}</div>
                    </td>
                </tr>
            `;
        }).join('');

    } catch (error) {
        console.error('Error loading my ranking detail:', error);
    }
}

/**
 * Get CSS class for rank badge based on rank number
 */
function getRankBadgeClass(rank) {
    if (rank === 1) return 'rank-1';
    if (rank === 2) return 'rank-2';
    if (rank === 3) return 'rank-3';
    return 'rank-other';
}

/**
 * Format number with commas
 */
function formatNumber(num) {
    if (typeof num !== 'number') return '0';
    return num.toLocaleString();
}

/**
 * Escape HTML to prevent XSS
 */
function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

/**
 * Logout function
 */
function logout() {
    localStorage.removeItem('token');
    if (socket) {
        socket.disconnect();
    }
    window.location.href = '/login.html';
}

// Initialize on page load
document.addEventListener('DOMContentLoaded', init);
