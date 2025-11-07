// @ts-nocheck
/**
 * Admin Panel JavaScript
 * Handles server administration, user management, and monitoring
 */

const API_BASE_URL = 'http://localhost:3000/api';

// State management
let currentSection = 'dashboard';
let socket = null;
let users = [];
let serverStats = null;

/**
 * Initialize admin panel
 */
async function init() {
    // Check authentication and admin status
    const token = localStorage.getItem('token');
    if (!token) {
        window.location.href = '/login.html';
        return;
    }

    // Verify admin access
    await verifyAdminAccess();

    // Setup event listeners
    setupEventListeners();

    // Setup socket connection
    setupSocket();

    // Load initial data
    await loadDashboard();

    // Start auto-refresh
    setInterval(() => {
        if (currentSection === 'dashboard') {
            loadDashboard();
        } else if (currentSection === 'server') {
            loadServerStatus();
        }
    }, 30000); // Refresh every 30 seconds
}

/**
 * Verify user has admin access
 */
async function verifyAdminAccess() {
    try {
        const response = await fetch(`${API_BASE_URL}/users/me`, {
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            }
        });

        if (!response.ok) {
            throw new Error('Failed to fetch user info');
        }

        const user = await response.json();
        
        // Check if user is admin
        if (!user.is_admin && !user.role !== 'admin') {
            alert('Access denied. Admin privileges required.');
            window.location.href = '/overview.html';
            return;
        }

        document.getElementById('adminUsername').textContent = user.username;

    } catch (error) {
        console.error('Error verifying admin access:', error);
        window.location.href = '/overview.html';
    }
}

/**
 * Setup event listeners
 */
function setupEventListeners() {
    // Navigation
    document.querySelectorAll('.admin-nav-item').forEach(button => {
        button.addEventListener('click', () => {
            const section = button.getAttribute('data-section');
            switchSection(section);
        });
    });

    // Buttons
    document.getElementById('refreshStatsBtn').addEventListener('click', loadDashboard);
    document.getElementById('refreshServerBtn').addEventListener('click', loadServerStatus);
    document.getElementById('backToGameBtn').addEventListener('click', () => {
        window.location.href = '/overview.html';
    });
    document.getElementById('logoutBtn').addEventListener('click', logout);
    document.getElementById('closeUserModal').addEventListener('click', closeUserModal);

    // User search and filter
    document.getElementById('userSearch').addEventListener('input', filterUsers);
    document.getElementById('userFilter').addEventListener('change', filterUsers);

    // Log filter
    document.getElementById('logLevelFilter').addEventListener('change', loadLogs);

    // Settings
    document.getElementById('saveSettingsBtn').addEventListener('click', saveSettings);
}

/**
 * Setup WebSocket connection
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

    socket.on('admin:stats_update', (data) => {
        console.log('Stats updated:', data);
        if (currentSection === 'dashboard') {
            updateDashboardStats(data);
        }
    });

    socket.on('disconnect', () => {
        console.log('Disconnected from server');
    });
}

/**
 * Switch between admin sections
 */
function switchSection(section) {
    currentSection = section;

    // Update navigation
    document.querySelectorAll('.admin-nav-item').forEach(button => {
        if (button.getAttribute('data-section') === section) {
            button.classList.add('active');
        } else {
            button.classList.remove('active');
        }
    });

    // Hide all sections
    document.querySelectorAll('.admin-section').forEach(el => {
        el.style.display = 'none';
    });

    // Show selected section
    const sectionMap = {
        dashboard: 'dashboardSection',
        users: 'usersSection',
        server: 'serverSection',
        logs: 'logsSection',
        database: 'databaseSection',
        settings: 'settingsSection'
    };

    const sectionId = sectionMap[section];
    if (sectionId) {
        document.getElementById(sectionId).style.display = 'block';
    }

    // Load section data
    loadSectionData(section);
}

/**
 * Load data for current section
 */
async function loadSectionData(section) {
    switch (section) {
        case 'dashboard':
            await loadDashboard();
            break;
        case 'users':
            await loadUsers();
            break;
        case 'server':
            await loadServerStatus();
            break;
        case 'logs':
            await loadLogs();
            break;
        case 'database':
            await loadDatabaseStats();
            break;
        case 'settings':
            await loadSettings();
            break;
    }
}

/**
 * Load dashboard statistics
 */
async function loadDashboard() {
    try {
        const response = await fetch(`${API_BASE_URL}/admin/stats`, {
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            }
        });

        if (!response.ok) {
            throw new Error('Failed to fetch stats');
        }

        const stats = await response.json();
        updateDashboardStats(stats);

    } catch (error) {
        console.error('Error loading dashboard:', error);
        // Use mock data for development
        const mockStats = {
            totalUsers: 1523,
            activePlayers: 342,
            totalPlanets: 4521,
            serverUptime: 168,
            activeCombats: 23,
            dbSize: 256,
            usersToday: 45,
            recentActivity: [
                { type: 'user_registered', username: 'NewPlayer123', timestamp: new Date().toISOString() },
                { type: 'combat_completed', details: 'Galaxy 1:234:5', timestamp: new Date().toISOString() }
            ]
        };
        updateDashboardStats(mockStats);
    }
}

/**
 * Update dashboard stats display
 */
function updateDashboardStats(stats) {
    document.getElementById('totalUsers').textContent = formatNumber(stats.totalUsers || 0);
    document.getElementById('activePlayers').textContent = formatNumber(stats.activePlayers || 0);
    document.getElementById('totalPlanets').textContent = formatNumber(stats.totalPlanets || 0);
    document.getElementById('serverUptime').textContent = `${stats.serverUptime || 0}h`;
    document.getElementById('activeCombats').textContent = formatNumber(stats.activeCombats || 0);
    document.getElementById('dbSize').textContent = `${stats.dbSize || 0} MB`;

    const usersChange = document.getElementById('usersChange');
    if (stats.usersToday) {
        usersChange.textContent = `+${stats.usersToday} today`;
        usersChange.className = 'stat-change positive';
    }

    // Render recent activity
    if (stats.recentActivity) {
        const container = document.getElementById('recentActivity');
        container.innerHTML = stats.recentActivity.map(activity => `
            <div class="log-entry log-level-info">
                <div class="log-timestamp">${new Date(activity.timestamp).toLocaleString()}</div>
                <div class="log-message">${activity.type}: ${activity.username || activity.details || ''}</div>
            </div>
        `).join('');
    }
}

/**
 * Load users list
 */
async function loadUsers() {
    try {
        const response = await fetch(`${API_BASE_URL}/admin/users`, {
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            }
        });

        if (!response.ok) {
            throw new Error('Failed to fetch users');
        }

        users = await response.json();
        renderUsers(users);

    } catch (error) {
        console.error('Error loading users:', error);
        // Use mock data
        users = [
            { id: 1, username: 'player1', email: 'player1@example.com', status: 'active', last_login: new Date().toISOString(), is_admin: false },
            { id: 2, username: 'player2', email: 'player2@example.com', status: 'active', last_login: new Date().toISOString(), is_admin: false },
            { id: 3, username: 'admin', email: 'admin@example.com', status: 'active', last_login: new Date().toISOString(), is_admin: true }
        ];
        renderUsers(users);
    }
}

/**
 * Render users table
 */
function renderUsers(usersToRender) {
    const tbody = document.getElementById('usersTableBody');
    
    tbody.innerHTML = usersToRender.map(user => `
        <tr>
            <td>${user.id}</td>
            <td>${escapeHtml(user.username)}</td>
            <td>${escapeHtml(user.email)}</td>
            <td>
                <span class="status-badge status-${user.status || 'active'}">
                    ${user.status || 'active'}
                </span>
                ${user.is_admin ? '<span class="status-badge" style="background: #9c27b0;">ADMIN</span>' : ''}
            </td>
            <td>${user.last_login ? new Date(user.last_login).toLocaleString() : 'Never'}</td>
            <td>
                <button class="action-btn btn-view" onclick="viewUser(${user.id})">View</button>
                ${user.status === 'active' && !user.is_admin ? 
                    `<button class="action-btn btn-ban" onclick="banUser(${user.id})">Ban</button>` :
                    user.status === 'banned' ? 
                    `<button class="action-btn btn-unban" onclick="unbanUser(${user.id})">Unban</button>` : ''
                }
            </td>
        </tr>
    `).join('');
}

/**
 * Filter users based on search and filter
 */
function filterUsers() {
    const searchTerm = document.getElementById('userSearch').value.toLowerCase();
    const filter = document.getElementById('userFilter').value;

    let filtered = users;

    // Apply search filter
    if (searchTerm) {
        filtered = filtered.filter(user => 
            user.username.toLowerCase().includes(searchTerm) ||
            user.email.toLowerCase().includes(searchTerm)
        );
    }

    // Apply status filter
    if (filter !== 'all') {
        if (filter === 'admin') {
            filtered = filtered.filter(user => user.is_admin);
        } else {
            filtered = filtered.filter(user => user.status === filter);
        }
    }

    renderUsers(filtered);
}

/**
 * View user details
 */
async function viewUser(userId) {
    try {
        const response = await fetch(`${API_BASE_URL}/admin/users/${userId}`, {
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            }
        });

        if (!response.ok) {
            throw new Error('Failed to fetch user details');
        }

        const user = await response.json();
        showUserDetail(user);

    } catch (error) {
        console.error('Error loading user details:', error);
        const user = users.find(u => u.id === userId);
        if (user) {
            showUserDetail(user);
        }
    }
}

/**
 * Show user detail modal
 */
function showUserDetail(user) {
    const content = document.getElementById('userDetailContent');
    
    content.innerHTML = `
        <div class="form-group">
            <label>Username:</label>
            <div style="color: #fff; padding: 10px; background: #2a2a3e; border-radius: 6px;">${escapeHtml(user.username)}</div>
        </div>
        <div class="form-group">
            <label>Email:</label>
            <div style="color: #fff; padding: 10px; background: #2a2a3e; border-radius: 6px;">${escapeHtml(user.email)}</div>
        </div>
        <div class="form-group">
            <label>User ID:</label>
            <div style="color: #fff; padding: 10px; background: #2a2a3e; border-radius: 6px;">${user.id}</div>
        </div>
        <div class="form-group">
            <label>Account Created:</label>
            <div style="color: #fff; padding: 10px; background: #2a2a3e; border-radius: 6px;">${new Date(user.created_at).toLocaleString()}</div>
        </div>
        <div class="form-group">
            <label>Last Login:</label>
            <div style="color: #fff; padding: 10px; background: #2a2a3e; border-radius: 6px;">${user.last_login ? new Date(user.last_login).toLocaleString() : 'Never'}</div>
        </div>
        <div class="form-group">
            <label>Total Planets:</label>
            <div style="color: #fff; padding: 10px; background: #2a2a3e; border-radius: 6px;">${user.planet_count || 0}</div>
        </div>
        <div class="form-group">
            <label>Total Score:</label>
            <div style="color: #fff; padding: 10px; background: #2a2a3e; border-radius: 6px;">${formatNumber(user.total_score || 0)}</div>
        </div>
    `;

    document.getElementById('userDetailModal').classList.add('active');
}

/**
 * Close user detail modal
 */
function closeUserModal() {
    document.getElementById('userDetailModal').classList.remove('active');
}

/**
 * Ban user
 */
async function banUser(userId) {
    if (!confirm('Are you sure you want to ban this user?')) {
        return;
    }

    try {
        const response = await fetch(`${API_BASE_URL}/admin/users/${userId}/ban`, {
            method: 'POST',
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            }
        });

        if (!response.ok) {
            throw new Error('Failed to ban user');
        }

        alert('User banned successfully');
        await loadUsers();

    } catch (error) {
        console.error('Error banning user:', error);
        alert('Failed to ban user');
    }
}

/**
 * Unban user
 */
async function unbanUser(userId) {
    try {
        const response = await fetch(`${API_BASE_URL}/admin/users/${userId}/unban`, {
            method: 'POST',
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            }
        });

        if (!response.ok) {
            throw new Error('Failed to unban user');
        }

        alert('User unbanned successfully');
        await loadUsers();

    } catch (error) {
        console.error('Error unbanning user:', error);
        alert('Failed to unban user');
    }
}

/**
 * Load server status
 */
async function loadServerStatus() {
    try {
        const response = await fetch(`${API_BASE_URL}/admin/server-status`, {
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            }
        });

        if (!response.ok) {
            throw new Error('Failed to fetch server status');
        }

        serverStats = await response.json();
        updateServerStatus(serverStats);

    } catch (error) {
        console.error('Error loading server status:', error);
        // Mock data
        const mockStatus = {
            cpu: 45.2,
            memory: 512,
            connections: 142,
            requestsPerMin: 324,
            services: [
                { name: 'PostgreSQL', status: 'running', uptime: 168 },
                { name: 'Redis', status: 'running', uptime: 168 },
                { name: 'WebSocket', status: 'running', uptime: 168 }
            ]
        };
        updateServerStatus(mockStatus);
    }
}

/**
 * Update server status display
 */
function updateServerStatus(status) {
    document.getElementById('cpuUsage').textContent = `${status.cpu || 0}%`;
    document.getElementById('memoryUsage').textContent = `${status.memory || 0} MB`;
    document.getElementById('activeConnections').textContent = formatNumber(status.connections || 0);
    document.getElementById('requestsPerMin').textContent = formatNumber(status.requestsPerMin || 0);

    if (status.services) {
        const container = document.getElementById('serviceStatus');
        container.innerHTML = status.services.map(service => `
            <div class="log-entry log-level-${service.status === 'running' ? 'info' : 'error'}">
                <div style="display: flex; justify-content: space-between; align-items: center;">
                    <div>
                        <div style="color: #fff; font-weight: 600;">${service.name}</div>
                        <div style="color: #888; font-size: 12px;">Uptime: ${service.uptime}h</div>
                    </div>
                    <span class="status-badge status-${service.status === 'running' ? 'active' : 'banned'}">
                        ${service.status}
                    </span>
                </div>
            </div>
        `).join('');
    }
}

/**
 * Load system logs
 */
async function loadLogs() {
    const logLevel = document.getElementById('logLevelFilter').value;
    
    try {
        const response = await fetch(`${API_BASE_URL}/admin/logs?level=${logLevel}`, {
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            }
        });

        if (!response.ok) {
            throw new Error('Failed to fetch logs');
        }

        const logs = await response.json();
        renderLogs(logs);

    } catch (error) {
        console.error('Error loading logs:', error);
        // Mock logs
        const mockLogs = [
            { level: 'info', message: 'Server started successfully', timestamp: new Date().toISOString() },
            { level: 'warn', message: 'High memory usage detected', timestamp: new Date().toISOString() },
            { level: 'error', message: 'Failed to connect to external API', timestamp: new Date().toISOString() }
        ];
        renderLogs(mockLogs);
    }
}

/**
 * Render logs
 */
function renderLogs(logs) {
    const container = document.getElementById('logsContainer');
    
    container.innerHTML = logs.map(log => `
        <div class="log-entry log-level-${log.level}">
            <div class="log-timestamp">${new Date(log.timestamp).toLocaleString()}</div>
            <div class="log-message">[${log.level.toUpperCase()}] ${escapeHtml(log.message)}</div>
        </div>
    `).join('');
}

/**
 * Load database statistics
 */
async function loadDatabaseStats() {
    try {
        const response = await fetch(`${API_BASE_URL}/admin/database-stats`, {
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            }
        });

        if (!response.ok) {
            throw new Error('Failed to fetch database stats');
        }

        const tables = await response.json();
        renderDatabaseStats(tables);

    } catch (error) {
        console.error('Error loading database stats:', error);
        // Mock data
        const mockTables = [
            { table_name: 'users', row_count: 1523, size: '2.4 MB', last_modified: new Date().toISOString() },
            { table_name: 'planets', row_count: 4521, size: '8.7 MB', last_modified: new Date().toISOString() },
            { table_name: 'fleets', row_count: 8234, size: '12.3 MB', last_modified: new Date().toISOString() }
        ];
        renderDatabaseStats(mockTables);
    }
}

/**
 * Render database stats table
 */
function renderDatabaseStats(tables) {
    const tbody = document.getElementById('dbStatsTableBody');
    
    tbody.innerHTML = tables.map(table => `
        <tr>
            <td>${escapeHtml(table.table_name)}</td>
            <td>${formatNumber(table.row_count || 0)}</td>
            <td>${table.size || '0 KB'}</td>
            <td>${table.last_modified ? new Date(table.last_modified).toLocaleString() : 'N/A'}</td>
        </tr>
    `).join('');
}

/**
 * Load settings
 */
async function loadSettings() {
    try {
        const response = await fetch(`${API_BASE_URL}/admin/settings`, {
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            }
        });

        if (!response.ok) {
            throw new Error('Failed to fetch settings');
        }

        const settings = await response.json();
        updateSettingsForm(settings);

    } catch (error) {
        console.error('Error loading settings:', error);
    }
}

/**
 * Update settings form
 */
function updateSettingsForm(settings) {
    document.getElementById('maintenanceMode').value = settings.maintenanceMode || 'false';
    document.getElementById('registrationEnabled').value = settings.registrationEnabled || 'true';
    document.getElementById('maxPlayers').value = settings.maxPlayers || 10000;
    document.getElementById('motd').value = settings.motd || '';
}

/**
 * Save settings
 */
async function saveSettings() {
    const settings = {
        maintenanceMode: document.getElementById('maintenanceMode').value === 'true',
        registrationEnabled: document.getElementById('registrationEnabled').value === 'true',
        maxPlayers: parseInt(document.getElementById('maxPlayers').value),
        motd: document.getElementById('motd').value
    };

    try {
        const response = await fetch(`${API_BASE_URL}/admin/settings`, {
            method: 'PUT',
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`,
                'Content-Type': 'application/json'
            },
            body: JSON.stringify(settings)
        });

        if (!response.ok) {
            throw new Error('Failed to save settings');
        }

        alert('Settings saved successfully');

    } catch (error) {
        console.error('Error saving settings:', error);
        alert('Failed to save settings');
    }
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
