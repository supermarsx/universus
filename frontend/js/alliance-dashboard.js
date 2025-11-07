/**
 * ALLIANCE DASHBOARD JAVASCRIPT
 * Handles all alliance dashboard interactions and API calls
 */

(function() {
    'use strict';

    // State management
    let currentAlliance = null;
    let currentMember = null;
    let socket = null;

    // API endpoints
    const API_BASE = '/api/alliances';

    /**
     * Initialize the dashboard
     */
    async function init() {
        console.log('[Alliance Dashboard] Initializing...');
        
        // Load alliance data
        await loadAllianceData();
        
        // Setup event listeners
        setupEventListeners();
        
        // Initialize real-time updates via Socket.io
        initializeSocketConnection();
        
        console.log('[Alliance Dashboard] Initialization complete');
    }

    /**
     * Load alliance data from API
     */
    async function loadAllianceData() {
        try {
            const token = localStorage.getItem('token');
            if (!token) {
                console.error('[Alliance Dashboard] No authentication token found');
                return;
            }

            // Get user's current alliance
            const response = await fetch(`${API_BASE}/my-alliance`, {
                headers: {
                    'Authorization': `Bearer ${token}`,
                    'Content-Type': 'application/json'
                }
            });

            if (response.ok) {
                currentAlliance = await response.json();
                console.log('[Alliance Dashboard] Alliance data loaded:', currentAlliance);
                updateUI();
            } else if (response.status === 404) {
                console.log('[Alliance Dashboard] User is not in an alliance');
                currentAlliance = null;
                updateUI();
            } else {
                console.error('[Alliance Dashboard] Failed to load alliance data:', response.statusText);
            }
        } catch (error) {
            console.error('[Alliance Dashboard] Error loading alliance data:', error);
            showNotification('Failed to load alliance data', 'error');
        }
    }

    /**
     * Update UI with current alliance data
     */
    function updateUI() {
        if (!currentAlliance) {
            console.log('[Alliance Dashboard] No alliance data to display');
            return;
        }

        // Update alliance header
        updateElement('allianceTag', currentAlliance.tag);
        updateElement('allianceName', currentAlliance.name);
        updateElement('allianceDescription', currentAlliance.description || 'No description provided');
        updateElement('founderName', currentAlliance.founder_name);
        updateElement('foundedDate', formatDate(currentAlliance.founded_at));

        // Update statistics
        updateElement('statTotalMembers', currentAlliance.total_members || 0);
        updateElement('statAlliancePower', formatNumber(currentAlliance.total_power || 0));
        updateElement('statAllianceRank', currentAlliance.rank || '-');
        updateElement('statWarPoints', currentAlliance.war_points || 0);
        updateElement('statTerritories', currentAlliance.territories_count || 0);
        updateElement('statDiplomacy', currentAlliance.diplomatic_relations_count || 0);

        // Update members list
        if (currentAlliance.members && currentAlliance.members.length > 0) {
            renderMembersList(currentAlliance.members);
        }

        // Update activity feed
        if (currentAlliance.recent_activity && currentAlliance.recent_activity.length > 0) {
            renderActivityFeed(currentAlliance.recent_activity);
        }

        // Update announcements
        if (currentAlliance.announcements && currentAlliance.announcements.length > 0) {
            renderAnnouncements(currentAlliance.announcements);
        }

        // Update action buttons visibility based on permissions
        updateActionButtons();
    }

    /**
     * Render members list
     */
    function renderMembersList(members) {
        const membersList = document.getElementById('membersList');
        if (!membersList) return;

        membersList.innerHTML = '';

        members.forEach(member => {
            const memberCard = createMemberCard(member);
            membersList.appendChild(memberCard);
        });
    }

    /**
     * Create member card element
     */
    function createMemberCard(member) {
        const card = document.createElement('div');
        card.className = 'member-card';
        card.dataset.memberId = member.user_id;

        card.innerHTML = `
            <div class="member-avatar">
                <img src="${member.avatar_url || '/assets/ui/default-avatar.png'}" alt="${member.username}">
                <span class="member-status-indicator ${member.is_online ? 'status-online' : 'status-offline'}"></span>
            </div>
            <div class="member-info">
                <div class="member-name">${escapeHtml(member.username)}</div>
                <div class="member-meta">
                    <span class="member-rank rank-${member.alliance_role.toLowerCase()}">${member.alliance_role}</span>
                    <span class="member-joined">Joined ${formatTimeAgo(member.joined_at)}</span>
                </div>
            </div>
            <div class="member-stats">
                <div class="member-stat">
                    <span class="stat-label">Power</span>
                    <span class="stat-value">${formatNumber(member.power || 0)}</span>
                </div>
                <div class="member-stat">
                    <span class="stat-label">Contribution</span>
                    <span class="stat-value">${formatNumber(member.contribution_points || 0)}</span>
                </div>
            </div>
            <div class="member-actions">
                <button class="btn-icon" onclick="viewMemberProfile(${member.user_id})" title="View Profile">
                    <span class="css-icon icon-view"></span>
                </button>
                ${canManageMembers() ? `
                <button class="btn-icon" onclick="showMemberManageMenu(${member.user_id})" title="Manage">
                    <span class="css-icon icon-settings"></span>
                </button>
                ` : ''}
            </div>
        `;

        return card;
    }

    /**
     * Render activity feed
     */
    function renderActivityFeed(activities) {
        const activityFeed = document.getElementById('activityFeed');
        if (!activityFeed) return;

        activityFeed.innerHTML = '';

        activities.forEach(activity => {
            const activityItem = createActivityItem(activity);
            activityFeed.appendChild(activityItem);
        });
    }

    /**
     * Create activity item element
     */
    function createActivityItem(activity) {
        const item = document.createElement('div');
        item.className = `activity-item activity-type-${activity.type}`;

        item.innerHTML = `
            <div class="activity-icon">
                <span class="css-icon icon-${activity.icon || 'activity'}"></span>
            </div>
            <div class="activity-content">
                <div class="activity-message">${activity.message}</div>
                <div class="activity-time">${formatTimeAgo(activity.timestamp)}</div>
            </div>
        `;

        return item;
    }

    /**
     * Render announcements
     */
    function renderAnnouncements(announcements) {
        const announcementsList = document.getElementById('announcementsList');
        if (!announcementsList) return;

        announcementsList.innerHTML = '';

        announcements.forEach(announcement => {
            const announcementCard = createAnnouncementCard(announcement);
            announcementsList.appendChild(announcementCard);
        });
    }

    /**
     * Create announcement card element
     */
    function createAnnouncementCard(announcement) {
        const card = document.createElement('div');
        card.className = 'announcement-card';

        card.innerHTML = `
            <div class="announcement-header">
                <div class="announcement-author">
                    <strong>${escapeHtml(announcement.author_name)}</strong>
                    <span class="announcement-role">${announcement.author_role}</span>
                </div>
                <div class="announcement-time">${formatTimeAgo(announcement.created_at)}</div>
            </div>
            <div class="announcement-content">
                <h3 class="announcement-title">${escapeHtml(announcement.title)}</h3>
                <div class="announcement-message">${escapeHtml(announcement.message)}</div>
            </div>
            ${announcement.is_pinned ? `
            <div class="announcement-pinned-badge">
                <span class="css-icon icon-pin"></span> Pinned
            </div>
            ` : ''}
        `;

        return card;
    }

    /**
     * Setup event listeners
     */
    function setupEventListeners() {
        // Member search
        const memberSearch = document.getElementById('memberSearch');
        if (memberSearch) {
            memberSearch.addEventListener('input', handleMemberSearch);
        }

        // Member role filter
        const memberRoleFilter = document.getElementById('memberRoleFilter');
        if (memberRoleFilter) {
            memberRoleFilter.addEventListener('change', handleMemberFilter);
        }

        // Modal close on background click
        document.querySelectorAll('.modal').forEach(modal => {
            modal.addEventListener('click', (e) => {
                if (e.target === modal) {
                    closeModal(modal.id);
                }
            });
        });
    }

    /**
     * Handle member search
     */
    function handleMemberSearch(event) {
        const searchTerm = event.target.value.toLowerCase();
        const memberCards = document.querySelectorAll('.member-card');

        memberCards.forEach(card => {
            const memberName = card.querySelector('.member-name').textContent.toLowerCase();
            if (memberName.includes(searchTerm)) {
                card.style.display = 'flex';
            } else {
                card.style.display = 'none';
            }
        });
    }

    /**
     * Handle member filter
     */
    function handleMemberFilter(event) {
        const selectedRole = event.target.value;
        const memberCards = document.querySelectorAll('.member-card');

        memberCards.forEach(card => {
            if (!selectedRole) {
                card.style.display = 'flex';
            } else {
                const memberRank = card.querySelector('.member-rank').textContent;
                if (memberRank === selectedRole) {
                    card.style.display = 'flex';
                } else {
                    card.style.display = 'none';
                }
            }
        });
    }

    /**
     * Show modal
     */
    window.showModal = function(modalId) {
        const modal = document.getElementById(modalId);
        if (modal) {
            modal.classList.add('active');
        }
    };

    /**
     * Close modal
     */
    window.closeModal = function(modalId) {
        const modal = document.getElementById(modalId);
        if (modal) {
            modal.classList.remove('active');
        }
    };

    /**
     * Show create alliance modal
     */
    window.showCreateAllianceModal = function() {
        showModal('createAllianceModal');
    };

    /**
     * Show invite modal
     */
    window.showInviteModal = function() {
        showModal('inviteMemberModal');
    };

    /**
     * Show announcement modal
     */
    window.showAnnouncementModal = function() {
        showModal('announcementModal');
    };

    /**
     * Handle create alliance form submission
     */
    window.handleCreateAlliance = async function(event) {
        event.preventDefault();

        const form = event.target;
        const formData = new FormData(form);
        const data = {
            tag: formData.get('tag'),
            name: formData.get('name'),
            description: formData.get('description')
        };

        try {
            const token = localStorage.getItem('token');
            const response = await fetch(`${API_BASE}/create`, {
                method: 'POST',
                headers: {
                    'Authorization': `Bearer ${token}`,
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify(data)
            });

            if (response.ok) {
                const result = await response.json();
                showNotification('Alliance created successfully!', 'success');
                closeModal('createAllianceModal');
                form.reset();
                
                // Reload page to show new alliance
                setTimeout(() => window.location.reload(), 1000);
            } else {
                const error = await response.json();
                showNotification(error.message || 'Failed to create alliance', 'error');
            }
        } catch (error) {
            console.error('[Alliance Dashboard] Error creating alliance:', error);
            showNotification('Failed to create alliance', 'error');
        }
    };

    /**
     * Handle invite member form submission
     */
    window.handleInviteMember = async function(event) {
        event.preventDefault();

        const form = event.target;
        const formData = new FormData(form);
        const data = {
            username: formData.get('username'),
            message: formData.get('message')
        };

        try {
            const token = localStorage.getItem('token');
            const response = await fetch(`${API_BASE}/${currentAlliance.alliance_id}/invite`, {
                method: 'POST',
                headers: {
                    'Authorization': `Bearer ${token}`,
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify(data)
            });

            if (response.ok) {
                showNotification('Invitation sent successfully!', 'success');
                closeModal('inviteMemberModal');
                form.reset();
            } else {
                const error = await response.json();
                showNotification(error.message || 'Failed to send invitation', 'error');
            }
        } catch (error) {
            console.error('[Alliance Dashboard] Error sending invitation:', error);
            showNotification('Failed to send invitation', 'error');
        }
    };

    /**
     * Handle create announcement form submission
     */
    window.handleCreateAnnouncement = async function(event) {
        event.preventDefault();

        const form = event.target;
        const formData = new FormData(form);
        const data = {
            title: formData.get('title'),
            message: formData.get('message'),
            is_pinned: formData.get('is_pinned') === 'on'
        };

        try {
            const token = localStorage.getItem('token');
            const response = await fetch(`${API_BASE}/${currentAlliance.alliance_id}/announcements`, {
                method: 'POST',
                headers: {
                    'Authorization': `Bearer ${token}`,
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify(data)
            });

            if (response.ok) {
                showNotification('Announcement posted successfully!', 'success');
                closeModal('announcementModal');
                form.reset();
                
                // Reload announcements
                await loadAllianceData();
            } else {
                const error = await response.json();
                showNotification(error.message || 'Failed to post announcement', 'error');
            }
        } catch (error) {
            console.error('[Alliance Dashboard] Error posting announcement:', error);
            showNotification('Failed to post announcement', 'error');
        }
    };

    /**
     * View member profile
     */
    window.viewMemberProfile = function(userId) {
        window.location.href = `/profile/${userId}`;
    };

    /**
     * Show member manage menu
     */
    window.showMemberManageMenu = function(userId) {
        // TODO: Implement member management dropdown
        console.log('[Alliance Dashboard] Manage member:', userId);
        showNotification('Member management coming soon', 'info');
    };

    /**
     * Refresh activity feed
     */
    window.refreshActivity = async function() {
        await loadAllianceData();
        showNotification('Activity refreshed', 'success');
    };

    /**
     * Update action buttons based on permissions
     */
    function updateActionButtons() {
        if (!currentAlliance || !currentAlliance.current_member_role) return;

        const role = currentAlliance.current_member_role;
        const canInvite = ['FOUNDER', 'LEADER', 'OFFICER'].includes(role);
        const canAnnounce = ['FOUNDER', 'LEADER', 'OFFICER'].includes(role);
        const canManage = ['FOUNDER', 'LEADER'].includes(role);

        const btnInvite = document.getElementById('btnInvite');
        const btnAnnounce = document.getElementById('btnAnnounce');
        const btnManage = document.getElementById('btnManage');

        if (btnInvite) btnInvite.style.display = canInvite ? 'flex' : 'none';
        if (btnAnnounce) btnAnnounce.style.display = canAnnounce ? 'flex' : 'none';
        if (btnManage) btnManage.style.display = canManage ? 'flex' : 'none';
    }

    /**
     * Check if current user can manage members
     */
    function canManageMembers() {
        if (!currentAlliance || !currentAlliance.current_member_role) return false;
        return ['FOUNDER', 'LEADER', 'OFFICER'].includes(currentAlliance.current_member_role);
    }

    /**
     * Initialize Socket.io connection for real-time updates
     */
    function initializeSocketConnection() {
        if (typeof io === 'undefined') {
            console.warn('[Alliance Dashboard] Socket.io not available');
            return;
        }

        socket = io();

        socket.on('connect', () => {
            console.log('[Alliance Dashboard] Socket.io connected');
            
            // Join alliance room for updates
            if (currentAlliance) {
                socket.emit('join-alliance-room', currentAlliance.alliance_id);
            }
        });

        socket.on('disconnect', () => {
            console.log('[Alliance Dashboard] Socket.io disconnected');
        });

        // Listen for alliance updates
        socket.on('alliance-update', (data) => {
            console.log('[Alliance Dashboard] Alliance update received:', data);
            loadAllianceData();
        });

        socket.on('alliance-member-joined', (data) => {
            console.log('[Alliance Dashboard] Member joined:', data);
            showNotification(`${data.username} joined the alliance!`, 'success');
            loadAllianceData();
        });

        socket.on('alliance-member-left', (data) => {
            console.log('[Alliance Dashboard] Member left:', data);
            showNotification(`${data.username} left the alliance`, 'info');
            loadAllianceData();
        });

        socket.on('alliance-announcement', (data) => {
            console.log('[Alliance Dashboard] New announcement:', data);
            showNotification(`New announcement: ${data.title}`, 'info');
            loadAllianceData();
        });
    }

    /**
     * Utility: Update element text content
     */
    function updateElement(id, value) {
        const element = document.getElementById(id);
        if (element) {
            element.textContent = value;
        }
    }

    /**
     * Utility: Format number with commas
     */
    function formatNumber(num) {
        return num.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ',');
    }

    /**
     * Utility: Format date
     */
    function formatDate(dateString) {
        const date = new Date(dateString);
        return date.toLocaleDateString('en-US', { 
            year: 'numeric', 
            month: 'long', 
            day: 'numeric' 
        });
    }

    /**
     * Utility: Format time ago
     */
    function formatTimeAgo(dateString) {
        const date = new Date(dateString);
        const now = new Date();
        const seconds = Math.floor((now - date) / 1000);

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

    /**
     * Utility: Escape HTML
     */
    function escapeHtml(text) {
        const div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
    }

    /**
     * Utility: Show notification
     */
    function showNotification(message, type = 'info') {
        // Use existing notification system if available
        if (typeof window.showToast === 'function') {
            window.showToast(message, type);
            return;
        }

        // Fallback to console
        console.log(`[Alliance Dashboard] ${type.toUpperCase()}: ${message}`);
        alert(message);
    }

    // Initialize on page load
    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', init);
    } else {
        init();
    }

})();
