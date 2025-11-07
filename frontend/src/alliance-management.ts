// @ts-nocheck
/**
 * ALLIANCE MANAGEMENT - Client-side JavaScript
 * Handles alliance settings, ranks, treasury, and member administration
 */

// Global state
let currentTab = 'settings';
let allMembers = [];
let allRanks = [];
let socket = null;

// Initialize on page load
document.addEventListener('DOMContentLoaded', () => {
    initializeManagement();
    setupSocketListeners();
});

// === INITIALIZATION === //

function initializeManagement() {
    // Load initial data based on default tab
    loadAllianceSettings();
    loadRanks();
    loadTreasuryData();
    loadMembers();
    
    // Connect to Socket.io for real-time updates
    if (typeof io !== 'undefined') {
        socket = io();
    }
}

function setupSocketListeners() {
    if (!socket) return;

    // Listen for alliance events
    socket.on('alliance:settings_updated', async (data) => {
        console.log('Alliance settings updated:', data);
        await loadAllianceSettings();
        showNotification('Alliance settings have been updated', 'info');
    });

    socket.on('alliance:rank_updated', async (data) => {
        console.log('Rank updated:', data);
        await loadRanks();
    });

    socket.on('alliance:treasury_updated', async (data) => {
        console.log('Treasury updated:', data);
        await loadTreasuryData();
    });

    socket.on('alliance:member_role_changed', async (data) => {
        console.log('Member role changed:', data);
        await loadMembers();
    });
}

// === TAB SWITCHING === //

function switchTab(tabName) {
    currentTab = tabName;
    
    // Update tab buttons
    document.querySelectorAll('.tab-btn').forEach(btn => {
        btn.classList.remove('active');
    });
    document.querySelector(`[data-tab="${tabName}"]`)?.classList.add('active');
    
    // Update tab content
    document.querySelectorAll('.tab-content').forEach(content => {
        content.classList.remove('active');
    });
    document.getElementById(`${tabName}-tab`)?.classList.add('active');
    
    // Load data for active tab if needed
    if (tabName === 'treasury') {
        loadTreasuryData();
    } else if (tabName === 'members') {
        loadMembers();
    }
}

// === API CALLS === //

async function loadAllianceSettings() {
    try {
        const response = await fetch('/api/alliance/current', {
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            }
        });

        if (!response.ok) throw new Error('Failed to load alliance settings');

        const data = await response.json();
        populateSettingsForm(data.alliance);
    } catch (error) {
        console.error('Error loading alliance settings:', error);
        showNotification('Failed to load alliance settings', 'error');
    }
}

async function updateAllianceSettings(formData) {
    try {
        const response = await fetch('/api/alliance/current', {
            method: 'PUT',
            headers: {
                'Content-Type': 'application/json',
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            },
            body: JSON.stringify(formData)
        });

        if (!response.ok) {
            const error = await response.json();
            throw new Error(error.message || 'Failed to update settings');
        }

        showNotification('Alliance settings updated successfully', 'success');
    } catch (error) {
        console.error('Error updating alliance settings:', error);
        showNotification(error.message, 'error');
        throw error;
    }
}

async function loadRanks() {
    try {
        const response = await fetch('/api/alliance/ranks', {
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            }
        });

        if (!response.ok) throw new Error('Failed to load ranks');

        const data = await response.json();
        allRanks = data.ranks || [];
        renderRanks(allRanks);
    } catch (error) {
        console.error('Error loading ranks:', error);
        showNotification('Failed to load ranks', 'error');
    }
}

async function loadTreasuryData() {
    try {
        const [treasuryResponse, contributionsResponse, contributorsResponse] = await Promise.all([
            fetch('/api/alliance/treasury', {
                headers: { 'Authorization': `Bearer ${localStorage.getItem('token')}` }
            }),
            fetch('/api/alliance/treasury/contributions/recent', {
                headers: { 'Authorization': `Bearer ${localStorage.getItem('token')}` }
            }),
            fetch('/api/alliance/treasury/contributors/top', {
                headers: { 'Authorization': `Bearer ${localStorage.getItem('token')}` }
            })
        ]);

        const treasury = await treasuryResponse.json();
        const contributions = await contributionsResponse.json();
        const contributors = await contributorsResponse.json();

        updateTreasuryDisplay(treasury);
        renderContributions(contributions.contributions || []);
        renderContributors(contributors.contributors || []);
    } catch (error) {
        console.error('Error loading treasury data:', error);
        showNotification('Failed to load treasury data', 'error');
    }
}

async function loadMembers() {
    try {
        const response = await fetch('/api/alliance/members', {
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            }
        });

        if (!response.ok) throw new Error('Failed to load members');

        const data = await response.json();
        allMembers = data.members || [];
        renderMembersAdmin(allMembers);
    } catch (error) {
        console.error('Error loading members:', error);
        showNotification('Failed to load members', 'error');
    }
}

async function updateMemberRole(memberId, newRole) {
    try {
        const response = await fetch(`/api/alliance/members/${memberId}/role`, {
            method: 'PUT',
            headers: {
                'Content-Type': 'application/json',
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            },
            body: JSON.stringify({ role: newRole })
        });

        if (!response.ok) {
            const error = await response.json();
            throw new Error(error.message || 'Failed to update member role');
        }

        showNotification('Member role updated successfully', 'success');
        await loadMembers();
    } catch (error) {
        console.error('Error updating member role:', error);
        showNotification(error.message, 'error');
    }
}

async function kickMember(memberId) {
    try {
        const response = await fetch(`/api/alliance/members/${memberId}/kick`, {
            method: 'POST',
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            }
        });

        if (!response.ok) {
            const error = await response.json();
            throw new Error(error.message || 'Failed to kick member');
        }

        showNotification('Member kicked successfully', 'success');
        await loadMembers();
        closeModal('memberActionModal');
    } catch (error) {
        console.error('Error kicking member:', error);
        showNotification(error.message, 'error');
    }
}

// === RENDERING === //

function populateSettingsForm(alliance) {
    if (!alliance) return;

    document.getElementById('allianceNameInput').value = alliance.name || '';
    document.getElementById('allianceTagInput').value = alliance.tag || '';
    document.getElementById('allianceDescInput').value = alliance.description || '';
    document.getElementById('allianceImageInput').value = alliance.image_url || '';
    document.getElementById('joinTypeInput').value = alliance.join_type || 'APPROVAL';
    document.getElementById('minRankInput').value = alliance.min_rank_requirement || '';
    document.getElementById('publicVisibleInput').checked = alliance.is_public || false;
}

function renderRanks(ranks) {
    const container = document.getElementById('ranksList');
    if (!container) return;

    if (ranks.length === 0) {
        container.innerHTML = '<p class="empty-message">No custom ranks created yet</p>';
        return;
    }

    container.innerHTML = ranks.map(rank => `
        <div class="rank-item" data-rank-id="${rank.id}">
            <div class="rank-info">
                <div class="rank-name">${escapeHtml(rank.name)}</div>
                <div class="rank-permissions">
                    ${rank.permissions.map(p => `
                        <span class="permission-badge">${p.replace(/_/g, ' ')}</span>
                    `).join('')}
                </div>
            </div>
            <div class="rank-actions">
                <button class="btn btn-sm btn-secondary" onclick="editRank(${rank.id})">
                    Edit
                </button>
                <button class="btn btn-sm btn-danger" onclick="deleteRank(${rank.id})">
                    Delete
                </button>
            </div>
        </div>
    `).join('');
}

function updateTreasuryDisplay(treasury) {
    document.getElementById('treasuryMetal').textContent = formatNumber(treasury.metal || 0);
    document.getElementById('treasuryCrystal').textContent = formatNumber(treasury.crystal || 0);
    document.getElementById('treasuryDeuterium').textContent = formatNumber(treasury.deuterium || 0);
}

function renderContributions(contributions) {
    const container = document.getElementById('contributionsList');
    if (!container) return;

    if (contributions.length === 0) {
        container.innerHTML = '<p class="empty-message">No recent contributions</p>';
        return;
    }

    container.innerHTML = contributions.map(contrib => `
        <div class="contribution-item">
            <div class="contribution-info">
                <div class="contribution-user">${escapeHtml(contrib.username)}</div>
                <div class="contribution-time">${formatTimeAgo(contrib.contributed_at)}</div>
            </div>
            <div class="contribution-resources">
                ${contrib.metal > 0 ? `
                <div class="resource-amount">
                    <span class="label">Metal</span>
                    <span class="value">${formatNumber(contrib.metal)}</span>
                </div>
                ` : ''}
                ${contrib.crystal > 0 ? `
                <div class="resource-amount">
                    <span class="label">Crystal</span>
                    <span class="value">${formatNumber(contrib.crystal)}</span>
                </div>
                ` : ''}
                ${contrib.deuterium > 0 ? `
                <div class="resource-amount">
                    <span class="label">Deuterium</span>
                    <span class="value">${formatNumber(contrib.deuterium)}</span>
                </div>
                ` : ''}
            </div>
        </div>
    `).join('');
}

function renderContributors(contributors) {
    const container = document.getElementById('contributorsList');
    if (!container) return;

    if (contributors.length === 0) {
        container.innerHTML = '<p class="empty-message">No contributors yet</p>';
        return;
    }

    container.innerHTML = contributors.map((contributor, index) => `
        <div class="contributor-item">
            <div class="contributor-info">
                <div class="contributor-name">#${index + 1} ${escapeHtml(contributor.username)}</div>
            </div>
            <div class="contribution-resources">
                <div class="resource-amount">
                    <span class="label">Total Value</span>
                    <span class="value">${formatNumber(contributor.total_value)}</span>
                </div>
            </div>
        </div>
    `).join('');
}

function renderMembersAdmin(members) {
    const container = document.getElementById('membersAdminList');
    if (!container) return;

    if (members.length === 0) {
        container.innerHTML = '<p class="empty-message">No members to display</p>';
        return;
    }

    container.innerHTML = members.map(member => `
        <div class="member-admin-card" data-member-id="${member.user_id}">
            <div class="member-admin-info">
                <div class="member-avatar-small">
                    <img src="${member.avatar_url || '/assets/ui/default-avatar.png'}" alt="${member.username}">
                </div>
                <div class="member-details">
                    <div class="member-username">${escapeHtml(member.username)}</div>
                    <span class="member-role-badge ${member.alliance_role.toLowerCase()}">${member.alliance_role}</span>
                </div>
            </div>
            <div class="member-admin-actions">
                <button class="btn btn-sm btn-secondary" onclick="showMemberActions(${member.user_id})">
                    <span class="css-icon icon-settings"></span> Manage
                </button>
            </div>
        </div>
    `).join('');
}

// === UI INTERACTIONS === //

function resetSettingsForm() {
    loadAllianceSettings();
}

function showCreateRankModal() {
    document.getElementById('rankModalTitle').textContent = 'Create Custom Rank';
    document.getElementById('rankForm').reset();
    openModal('rankModal');
}

function editRank(rankId) {
    const rank = allRanks.find(r => r.id === rankId);
    if (!rank) return;

    document.getElementById('rankModalTitle').textContent = 'Edit Rank';
    document.getElementById('rankNameInput').value = rank.name;
    
    // Check permissions
    document.querySelectorAll('input[name="permission"]').forEach(checkbox => {
        checkbox.checked = rank.permissions.includes(checkbox.value);
    });

    openModal('rankModal');
}

function deleteRank(rankId) {
    if (!confirm('Are you sure you want to delete this rank? Members with this rank will be moved to default Member rank.')) {
        return;
    }

    // API call to delete rank
    fetch(`/api/alliance/ranks/${rankId}`, {
        method: 'DELETE',
        headers: {
            'Authorization': `Bearer ${localStorage.getItem('token')}`
        }
    })
    .then(response => {
        if (!response.ok) throw new Error('Failed to delete rank');
        showNotification('Rank deleted successfully', 'success');
        loadRanks();
    })
    .catch(error => {
        console.error('Error deleting rank:', error);
        showNotification(error.message, 'error');
    });
}

function showMemberActions(memberId) {
    const member = allMembers.find(m => m.user_id === memberId);
    if (!member) return;

    const infoElement = document.getElementById('memberActionInfo');
    const buttonsElement = document.getElementById('memberActionButtons');

    if (infoElement) {
        infoElement.innerHTML = `
            <div class="member-username">${escapeHtml(member.username)}</div>
            <p>Current Role: <strong>${member.alliance_role}</strong></p>
        `;
    }

    if (buttonsElement) {
        buttonsElement.innerHTML = `
            <button class="btn btn-primary" onclick="showPromoteOptions(${memberId})">
                Promote/Demote
            </button>
            <button class="btn btn-danger" onclick="confirmKickMember(${memberId})">
                Kick from Alliance
            </button>
        `;
    }

    openModal('memberActionModal');
}

function showPromoteOptions(memberId) {
    const roles = ['RECRUIT', 'MEMBER', 'OFFICER', 'LEADER'];
    const member = allMembers.find(m => m.user_id === memberId);
    if (!member) return;

    const buttonsElement = document.getElementById('memberActionButtons');
    if (buttonsElement) {
        buttonsElement.innerHTML = `
            <p>Select new role for ${escapeHtml(member.username)}:</p>
            ${roles.map(role => `
                <button class="btn btn-secondary" onclick="updateMemberRole(${memberId}, '${role}')">
                    ${role}
                </button>
            `).join('')}
            <button class="btn btn-sm btn-secondary" onclick="showMemberActions(${memberId})">
                Back
            </button>
        `;
    }
}

function confirmKickMember(memberId) {
    const member = allMembers.find(m => m.user_id === memberId);
    if (!member) return;

    if (confirm(`Are you sure you want to kick ${member.username} from the alliance?`)) {
        kickMember(memberId);
    }
}

// === FORM HANDLERS === //

async function handleUpdateSettings(event) {
    event.preventDefault();

    const formData = {
        name: document.getElementById('allianceNameInput').value,
        description: document.getElementById('allianceDescInput').value,
        image_url: document.getElementById('allianceImageInput').value || null,
        join_type: document.getElementById('joinTypeInput').value,
        min_rank_requirement: parseInt(document.getElementById('minRankInput').value) || null,
        is_public: document.getElementById('publicVisibleInput').checked
    };

    try {
        await updateAllianceSettings(formData);
    } catch (error) {
        // Error already handled in updateAllianceSettings
    }
}

async function handleRankSubmit(event) {
    event.preventDefault();

    const permissions = Array.from(document.querySelectorAll('input[name="permission"]:checked'))
        .map(checkbox => checkbox.value);

    const formData = {
        name: document.getElementById('rankNameInput').value,
        permissions: permissions
    };

    try {
        const response = await fetch('/api/alliance/ranks', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            },
            body: JSON.stringify(formData)
        });

        if (!response.ok) {
            const error = await response.json();
            throw new Error(error.message || 'Failed to create rank');
        }

        showNotification('Rank created successfully', 'success');
        closeModal('rankModal');
        await loadRanks();
    } catch (error) {
        console.error('Error creating rank:', error);
        showNotification(error.message, 'error');
    }
}

// === MODAL MANAGEMENT === //

function openModal(modalId) {
    const modal = document.getElementById(modalId);
    if (modal) {
        modal.classList.add('active');
    }
}

function closeModal(modalId) {
    const modal = document.getElementById(modalId);
    if (modal) {
        modal.classList.remove('active');
    }
}

// Close modal on outside click
document.addEventListener('click', (e) => {
    if (e.target.classList.contains('modal')) {
        e.target.classList.remove('active');
    }
});

// === UTILITY FUNCTIONS === //

function formatNumber(num) {
    if (!num) return '0';
    return num.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ',');
}

function formatTimeAgo(dateString) {
    if (!dateString) return '-';
    const date = new Date(dateString);
    const seconds = Math.floor((new Date() - date) / 1000);

    const intervals = {
        year: 31536000,
        month: 2592000,
        week: 604800,
        day: 86400,
        hour: 3600,
        minute: 60
    };

    for (const [unit, secondsInUnit] of Object.entries(intervals)) {
        const interval = Math.floor(seconds / secondsInUnit);
        if (interval >= 1) {
            return `${interval} ${unit}${interval > 1 ? 's' : ''} ago`;
        }
    }

    return 'just now';
}

function escapeHtml(unsafe) {
    if (!unsafe) return '';
    return unsafe
        .toString()
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#039;');
}

function showNotification(message, type = 'info') {
    // Implementation depends on your notification system
    console.log(`[${type.toUpperCase()}] ${message}`);
    
    // If you have a toast notification system, use it here
    if (typeof showToast !== 'undefined') {
        showToast(message, type);
    } else {
        // Fallback to simple alert for now
        alert(message);
    }
}
