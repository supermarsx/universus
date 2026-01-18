// @ts-nocheck
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
let sparklineIdCounter = 0;
let selectedAllianceId: number | null = null;
let allianceMembersOffset = 0;
const ALLIANCE_MEMBERS_LIMIT = 25;

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
    await loadLeaderboardStatus();

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

    setInterval(() => {
        loadLeaderboardStatus();
    }, 60000);
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

    document.getElementById('alliancesTableBody').addEventListener('click', (event) => {
        const row = event.target.closest('tr');
        if (!row) return;
        const allianceId = parseInt(row.getAttribute('data-alliance-id'), 10);
        if (Number.isNaN(allianceId)) return;
        selectedAllianceId = allianceId;
        allianceMembersOffset = 0;
        loadAllianceDetail();
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

        const payload = await response.json();
        const players = Array.isArray(payload) ? payload : payload.data || [];
        const paginationTotal = (!Array.isArray(payload) && payload.pagination) ? payload.pagination.total : null;
        const playerTotal = typeof paginationTotal === 'number'
            ? paginationTotal
            : ((playerPage * PAGE_SIZE) + players.length);
        document.getElementById('totalPlayers').textContent = playerTotal.toString();

        loadingEl.style.display = 'none';

        if (!players.length) {
            emptyEl.style.display = 'block';
            return;
        }

        // Render table
        tbody.innerHTML = players.map(player => {
            const isCurrentPlayer = player.userId === currentUserId;
            const rankBadgeClass = getRankBadgeClass(player.rank);
            const sparkline = renderSparkline(player.scoreTrend);
            const rankDelta = renderRankDelta(player.weeklyRankChange);

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
                    <td>${sparkline}</td>
                    <td>${rankDelta}</td>
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

        const alliancePayload = await response.json();
        const alliances = Array.isArray(alliancePayload) ? alliancePayload : alliancePayload.data || [];

        loadingEl.style.display = 'none';

        if (!alliances.length) {
            emptyEl.style.display = 'block';
            return;
        }

        // Render table
        tbody.innerHTML = alliances.map(alliance => {
            const rankBadgeClass = getRankBadgeClass(alliance.rank);
            const sparkline = renderSparkline(alliance.scoreTrend);
            const rankDelta = renderRankDelta(alliance.weeklyRankChange);

            return `
                <tr data-alliance-id="${alliance.allianceId}">
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
                    <td>${sparkline}</td>
                    <td>${rankDelta}</td>
                </tr>
            `;
        }).join('');

        tableEl.style.display = 'table';
        paginationEl.style.display = alliances.length === PAGE_SIZE ? 'flex' : 'none';

        // Update pagination
        document.getElementById('alliancesCurrentPage').textContent = alliancePage + 1;
        document.getElementById('alliancesPrevBtn').disabled = alliancePage === 0;
        document.getElementById('alliancesNextBtn').disabled = alliances.length < PAGE_SIZE;

        if (alliances.length) {
            if (!selectedAllianceId || !alliances.some((a) => a.allianceId === selectedAllianceId)) {
                selectedAllianceId = alliances[0].allianceId;
                allianceMembersOffset = 0;
            }
            loadAllianceDetail(true);
        } else {
            document.getElementById('allianceDetailPanel').style.display = 'none';
        }

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

async function loadAllianceDetail(silent?: boolean) {
    if (!selectedAllianceId) {
        document.getElementById('allianceDetailPanel').style.display = 'none';
        return;
    }

    const panel = document.getElementById('allianceDetailPanel');
    const tbody = document.getElementById('allianceMembersBody');

    if (!silent) {
        tbody.innerHTML = '<tr><td colspan="7" style="color:#94a3b8;">Loading alliance members…</td></tr>';
    }

    try {
        const response = await fetch(
            `${API_BASE_URL}/leaderboard/alliances/${selectedAllianceId}/details?limit=${ALLIANCE_MEMBERS_LIMIT}&offset=${allianceMembersOffset}`,
            {
                headers: {
                    'Authorization': `Bearer ${localStorage.getItem('token')}`
                }
            }
        );
        if (!response.ok) throw new Error('Failed to load alliance details');

        const payload = await response.json();
        const alliance = payload.data?.alliance;
        const members = payload.data?.members || [];
        if (!alliance) {
            panel.style.display = 'none';
            return;
        }

        panel.style.display = 'block';
        document.getElementById('detailAllianceName').textContent = alliance.allianceName;
        document.getElementById('detailAllianceTag').textContent = alliance.allianceTag || '-';
        document.getElementById('detailAllianceScore').textContent = formatNumber(alliance.totalScore);
        document.getElementById('detailAllianceMembers').textContent = alliance.memberCount || members.length;
        document.getElementById('detailAllianceAverage').textContent = formatNumber(alliance.averageScore || 0);
        document.getElementById('detailAllianceRank').textContent = alliance.rank;

        tbody.innerHTML = members.map(member => `
            <tr>
                <td>${member.rank}</td>
                <td>${escapeHtml(member.username)}</td>
                <td>${formatNumber(member.totalScore)}</td>
                <td>${formatNumber(member.buildingScore || 0)}</td>
                <td>${formatNumber(member.researchScore || 0)}</td>
                <td>${formatNumber(member.fleetScore || 0)}</td>
                <td>${formatNumber(member.defenseScore || 0)}</td>
            </tr>
        `).join('');
    } catch (error) {
        console.error('Error loading alliance detail', error);
        panel.style.display = 'none';
    }
}

async function loadLeaderboardStatus() {
    try {
        const response = await fetch(`${API_BASE_URL}/leaderboard/cache/meta`, {
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            }
        });
        if (!response.ok) throw new Error('Failed to load cache metadata');
        const payload = await response.json();
        const players = payload.cache?.players;
        const alliances = payload.cache?.alliances;
        const scheduler = payload.scheduler;

        document.getElementById('playersCacheStatus').textContent = players?.lastBuild
            ? formatTime(players.lastBuild)
            : 'pending';
        document.getElementById('alliancesCacheStatus').textContent = alliances?.lastBuild
            ? formatTime(alliances.lastBuild)
            : 'pending';

        document.getElementById('playersCacheTTL').textContent = `TTL: ${formatTTL(players?.ttlSeconds)}`;
        document.getElementById('alliancesCacheTTL').textContent = `TTL: ${formatTTL(alliances?.ttlSeconds)}`;

        document.getElementById('schedulerStatus').textContent = scheduler?.running ? 'Running' : 'Idle';
        if (scheduler?.lastRun && scheduler?.intervalMs) {
            const nextRun = new Date(new Date(scheduler.lastRun).getTime() + scheduler.intervalMs);
            document.getElementById('schedulerNextRun').textContent = `Next run: ${formatTime(nextRun)}`;
        } else {
            document.getElementById('schedulerNextRun').textContent = 'Next run: --';
        }
    } catch (error) {
        console.error('Failed to load leaderboard status', error);
    }
}

function formatTTL(ttl?: number) {
    if (typeof ttl !== 'number' || ttl < 0) return 'expired';
    if (ttl > 3600) {
        return `${Math.floor(ttl / 3600)}h`;
    }
    const minutes = Math.floor(ttl / 60);
    const seconds = ttl % 60;
    return `${minutes}m ${seconds}s`;
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
    if (typeof num !== 'number') num = Number(num) || 0;
    const locale = getLocale();
    if (typeof Intl !== 'undefined' && Intl.NumberFormat) {
        return new Intl.NumberFormat(locale).format(num);
    }
    return num.toLocaleString();
}

function formatTime(value) {
    const date = value instanceof Date ? value : new Date(value);
    const locale = getLocale();
    if (typeof Intl !== 'undefined' && Intl.DateTimeFormat) {
        return new Intl.DateTimeFormat(locale, {
            hour: '2-digit',
            minute: '2-digit',
            second: '2-digit',
        }).format(date);
    }
    return date.toLocaleTimeString();
}

function getLocale() {
    try {
        if (window.i18next && window.i18next.language) {
            return window.i18next.language;
        }
    } catch (error) {
        // ignore i18next access errors
    }
    try {
        const stored = localStorage.getItem('preferredLanguage');
        if (stored) return stored;
    } catch (error) {
        // ignore storage access errors
    }
    return navigator.language || 'en-US';
}

function renderSparkline(trend = []) {
    if (!trend || trend.length === 0) {
        return '<div class="sparkline sparkline-empty">–</div>';
    }

    const width = 80;
    const height = 24;
    const scores = trend.map(point => Number(point.score) || 0);
    const min = Math.min(...scores);
    const max = Math.max(...scores);
    const range = max - min || 1;
    const steps = trend.length - 1 || 1;
    const sparkId = `spark-${sparklineIdCounter++}`;

    const pointPairs = scores
        .map((score, idx) => {
            const x = (idx / steps) * width;
            const y = height - ((score - min) / range) * height;
            return { x: Number(x.toFixed(2)), y: Number(y.toFixed(2)) };
        })
        .map(point => `${point.x},${point.y}`);

    const linePoints = pointPairs.join(' ');

    const areaPathPoints = [...pointPairs, `${width},${height}`, `0,${height}`].join(' ');

    return `
        <div class="sparkline">
            <svg viewBox="0 0 ${width} ${height}" preserveAspectRatio="none">
                <defs>
                    <linearGradient id="${sparkId}-gradient" x1="0%" y1="0%" x2="0%" y2="100%">
                        <stop offset="0%" stop-color="#4a9eff" stop-opacity="0.35" />
                        <stop offset="100%" stop-color="#4a9eff" stop-opacity="0" />
                    </linearGradient>
                </defs>
                <polygon class="sparkline-area" points="${areaPathPoints}" fill="url(#${sparkId}-gradient)" />
                <polyline class="sparkline-line" points="${linePoints}" />
            </svg>
        </div>
    `;
}

function renderRankDelta(delta) {
    if (delta === null || delta === undefined) {
        return '<span class="rank-delta neutral">–</span>';
    }
    if (delta > 0) {
        return `<span class="rank-delta up">▲ ${delta}</span>`;
    }
    if (delta < 0) {
        return `<span class="rank-delta down">▼ ${Math.abs(delta)}</span>`;
    }
    return '<span class="rank-delta neutral">0</span>';
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
