// @ts-nocheck
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
    let playerPlanets = [];
    let depotSessions = [];
    let acsGroups = [];

    // API endpoints
    const API_BASE = '/api/alliances';

    /**
     * Initialize the dashboard
     */
    async function init() {
        console.log('[Alliance Dashboard] Initializing...');
        
        // Load alliance data
        await loadAllianceData();
        await loadPlayerPlanets();
        await loadDepotSessions();
        await loadAcsGroups();

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

            const payload = await response.json();

            if (response.ok && payload.success) {
                currentAlliance = payload.data;
                console.log('[Alliance Dashboard] Alliance data loaded:', currentAlliance);
            } else if (response.status === 404) {
                console.log('[Alliance Dashboard] User is not in an alliance');
                currentAlliance = null;
            } else {
                console.error('[Alliance Dashboard] Failed to load alliance data:', payload?.error || response.statusText);
                currentAlliance = null;
                showNotification(payload?.error || 'Failed to load alliance data', 'error');
            }

            updateUI();
        } catch (error) {
            console.error('[Alliance Dashboard] Error loading alliance data:', error);
            showNotification('Failed to load alliance data', 'error');
        }
    }

    async function loadPlayerPlanets() {
        try {
            const response = await fetch('/api/planets', {
                headers: { 'Authorization': `Bearer ${localStorage.getItem('token')}` }
            });

            if (!response.ok) throw new Error('Failed to load planets');

            const data = await response.json();
            playerPlanets = Array.isArray(data) ? data : data.planets || [];
            populatePlanetSelects();
        } catch (error) {
            console.error('[Alliance Dashboard] Error loading planets:', error);
            playerPlanets = [];
        }
    }

    /**
     * Update UI with current alliance data
     */
    function updateUI() {
        if (!currentAlliance) {
            const emptyState = document.getElementById('membersList');
            if (emptyState) {
                emptyState.innerHTML = `
                    <div class="empty-state">
                        <p>You are not currently a member of an alliance.</p>
                        <button class="btn btn-primary" onclick="window.location.href='/alliance/manage'">
                            Manage Alliances
                        </button>
                    </div>
                `;
            }
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
        renderAnnouncements(currentAlliance.announcements || []);

        populatePlanetSelects();

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
                <button class="btn-icon" onclick="showMemberManageMenu(${member.user_id}, event)" title="Manage">
                    <span class="css-icon icon-settings"></span>
                </button>
                ` : ''}
            </div>
        `;

        return card;
    }

    function populatePlanetSelects() {
        const depotHostSelect = document.getElementById('hostPlanetSelect');
        const depotGuestSelect = document.getElementById('guestPlanetSelect');
        const transportSelect = document.getElementById('transportPlanetSelect');

        if (!depotHostSelect && !depotGuestSelect && !transportSelect) {
            return;
        }

        if (!playerPlanets || playerPlanets.length === 0) {
            [depotHostSelect, depotGuestSelect, transportSelect].forEach((select) => {
                if (select) {
                    select.innerHTML = '<option value="">No planets available</option>';
                    select.disabled = true;
                }
            });
            return;
        }

        if (depotHostSelect) {
            const hostOptions = playerPlanets
                .filter((planet) => (planet.alliance_depot || 0) > 0)
                .map(
                    (planet) =>
                        `<option value="${planet.id}">${escapeHtml(planet.name)} — Depot Lv ${planet.alliance_depot || 0}</option>`
                )
                .join('');
            depotHostSelect.innerHTML =
                hostOptions || '<option value="">No depot-enabled planets</option>';
            depotHostSelect.disabled = !hostOptions;
        }

        const defaultOptions = playerPlanets
            .map(
                (planet) =>
                    `<option value="${planet.id}">${escapeHtml(planet.name)} [${planet.galaxy}:${planet.system}:${planet.position}]</option>`
            )
            .join('');

        [depotGuestSelect, transportSelect].forEach((select) => {
            if (select) {
                select.innerHTML = defaultOptions;
                select.disabled = false;
            }
        });
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

        if (!announcements || announcements.length === 0) {
            announcementsList.innerHTML = `
                <div class="empty-state">
                    <span class="css-icon icon-broadcast"></span>
                    <p>No announcements yet</p>
                </div>
            `;
            return;
        }

        announcements.forEach(announcement => {
            const announcementCard = createAnnouncementCard(announcement);
            announcementsList.appendChild(announcementCard);
        });
    }

    async function loadDepotSessions() {
        if (!currentAlliance || !currentAlliance.alliance_id) {
            return;
        }
        try {
            const response = await fetch(
                `${API_BASE}/${currentAlliance.alliance_id}/depot/sessions`,
                {
                    headers: {
                        Authorization: `Bearer ${localStorage.getItem('token')}`,
                    },
                }
            );

            if (!response.ok) {
                throw new Error('Failed to load depot sessions');
            }

            const payload = await response.json();
            depotSessions = payload.data || [];
            renderDepotSessions();
        } catch (error) {
            console.error('[Alliance Dashboard] Error loading depot sessions:', error);
        }
    }

    function renderDepotSessions() {
        const container = document.getElementById('depotSessionsList');
        if (!container) return;

        if (!depotSessions || depotSessions.length === 0) {
            container.innerHTML = `
                <div class="empty-state">
                    <span class="css-icon icon-fuel"></span>
                    <p>No depot requests yet</p>
                </div>
            `;
            return;
        }

        container.innerHTML = depotSessions
            .map((session) => {
                const metadata = session.metadata || {};
                const requested = metadata.requestedDeuterium || metadata.requested_deuterium || 0;
                const statusClass = session.status?.toLowerCase() || 'pending';
                const canApprove = hasAlliancePermission('manage_resources');

                const actions =
                    statusClass === 'pending' && canApprove
                        ? `
                            <div class="logistics-session-actions">
                                <button class="btn btn-sm btn-success" onclick="approveDepotSession(${session.id})">Approve</button>
                                <button class="btn btn-sm btn-danger" onclick="cancelDepotSession(${session.id})">Cancel</button>
                            </div>
                        `
                        : '';

                return `
                    <div class="logistics-session ${statusClass}">
                        <div class="logistics-session-header">
                            <div>
                                <strong>${escapeHtml(session.guest_username || 'Member')}</strong>
                                <span class="logistics-session-meta">needs ${formatNumber(requested)} deut</span>
                            </div>
                            <span class="status-badge status-${statusClass}">${statusClass}</span>
                        </div>
                        <div class="logistics-session-meta">
                            Host: ${escapeHtml(session.host_username || 'N/A')} (${session.host_galaxy}:${session.host_system}:${session.host_position})
                        </div>
                        ${actions}
                    </div>
                `;
            })
            .join('');
    }

    async function loadAcsGroups() {
        try {
            const response = await fetch('/api/acs', {
                headers: {
                    Authorization: `Bearer ${localStorage.getItem('token')}`,
                },
            });

            if (!response.ok) {
                throw new Error('Failed to load ACS groups');
            }

            const payload = await response.json();
            acsGroups = payload.groups || [];
            renderAcsGroups();
        } catch (error) {
            console.error('[Alliance Dashboard] Error loading ACS groups:', error);
            acsGroups = [];
            renderAcsGroups();
        }
    }

    function renderAcsGroups() {
        const container = document.getElementById('acsGroupList');
        if (!container) return;

        if (!acsGroups || acsGroups.length === 0) {
            container.innerHTML = `
                <div class="empty-state">
                    <span class="css-icon icon-target"></span>
                    <p>No active ACS groups</p>
                </div>
            `;
            return;
        }

        container.innerHTML = acsGroups
            .map((group) => {
                const coord = `[${group.target_galaxy}:${group.target_system}:${group.target_position}]`;
                const windowStart = group.departure_window_start
                    ? formatDateTime(group.departure_window_start)
                    : '';
                const windowEnd = group.departure_window_end
                    ? formatDateTime(group.departure_window_end)
                    : '';
                return `
                    <div class="acs-group-card">
                        <div class="acs-group-header">
                            <div>
                                <strong>${group.mission_type?.toUpperCase() || 'ATTACK'}</strong>
                                <span class="acs-group-meta">${coord}</span>
                            </div>
                            <span class="status-badge">${group.member_count || 0} members</span>
                        </div>
                        <div class="acs-group-meta">
                            <span>Window: ${windowStart || 'now'} - ${windowEnd || 'soon'}</span>
                        </div>
                        ${group.notes ? `<div class="acs-group-notes">${escapeHtml(group.notes)}</div>` : ''}
                        <div class="acs-group-actions">
                            <button class="btn btn-sm btn-primary" onclick="joinAcsGroup(${group.id})">Join</button>
                            <button class="btn btn-sm btn-secondary" onclick="leaveAcsGroup(${group.id})">Leave</button>
                        </div>
                    </div>
                `;
            })
            .join('');
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
                    <strong>${escapeHtml(announcement.author_name || announcement.created_by_username || 'Alliance Command')}</strong>
                    ${announcement.author_role ? `<span class="announcement-role">${escapeHtml(announcement.author_role)}</span>` : ''}
                </div>
                <div class="announcement-time">${formatTimeAgo(announcement.created_at)}</div>
            </div>
            <div class="announcement-content">
                <h3 class="announcement-title">${escapeHtml(announcement.title)}</h3>
                <div class="announcement-message">${escapeHtml(announcement.content || announcement.message || '')}</div>
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

        const depotRequestForm = document.getElementById('depotRequestForm');
        if (depotRequestForm) {
            depotRequestForm.addEventListener('submit', handleDepotRequest);
        }

        const sharedTransportForm = document.getElementById('sharedTransportForm');
        if (sharedTransportForm) {
            sharedTransportForm.addEventListener('submit', handleSharedTransport);
        }

        const targetTypeSelect = document.getElementById('transportTargetType');
        if (targetTypeSelect) {
            targetTypeSelect.addEventListener('change', toggleSharedTransportTarget);
            toggleSharedTransportTarget(targetTypeSelect.value);
        }

        const acsForm = document.getElementById('acsCreateForm');
        if (acsForm) {
            acsForm.addEventListener('submit', handleCreateAcsGroup);
        }

        document.addEventListener('click', handleMemberMenuOutsideClick);
        document.addEventListener('keydown', handleMemberMenuKeydown);
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
            content: formData.get('message'),
            is_pinned: formData.get('is_pinned') === 'on',
            broadcast: true
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
    window.showMemberManageMenu = function(userId, event) {
        const member = (currentAlliance?.members || []).find((entry) => entry.user_id === userId);
        if (!member) {
            showNotification('Member not found', 'error');
            return;
        }
        currentMember = member;

        const menu = getMemberManageMenu();
        renderMemberManageMenu(menu, member);

        const anchor = event?.currentTarget || event?.target;
        positionMemberManageMenu(menu, anchor, userId);
        menu.classList.add('active');
    };

    /**
     * Refresh activity feed
     */
    window.refreshActivity = async function() {
        await loadAllianceData();
        showNotification('Activity refreshed', 'success');
    };

    window.loadDepotSessions = loadDepotSessions;
    window.approveDepotSession = approveDepotSession;
    window.cancelDepotSession = cancelDepotSession;
    window.joinAcsGroup = joinAcsGroup;
    window.leaveAcsGroup = leaveAcsGroup;
    window.loadAcsGroups = loadAcsGroups;

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

    function getMemberRank(member) {
        const raw = member?.rank || member?.alliance_role || member?.alliance_role_display || '';
        return String(raw || '').toLowerCase();
    }

    function getRankOptions() {
        return [
            { value: 'leader', label: 'Leader' },
            { value: 'officer', label: 'Officer' },
            { value: 'member', label: 'Member' },
            { value: 'recruit', label: 'Recruit' }
        ];
    }

    function getMemberManageMenu() {
        let menu = document.getElementById('memberManageMenu');
        if (!menu) {
            menu = document.createElement('div');
            menu.id = 'memberManageMenu';
            menu.className = 'member-manage-menu';
            document.body.appendChild(menu);
        }
        return menu;
    }

    function renderMemberManageMenu(menu, member) {
        const canKick = hasAlliancePermission('kick_members');
        const canManageRanks = hasAlliancePermission('manage_ranks');
        const rankOptions = getRankOptions()
            .map((option) => `<option value="${option.value}">${option.label}</option>`)
            .join('');
        const displayName = escapeHtml(member.username || 'Member');
        const currentRank = getMemberRank(member);

        menu.innerHTML = `
            <div class="menu-header">Manage ${displayName}</div>
            ${canManageRanks ? `
                <div class="menu-section">
                    <label class="menu-label">Rank</label>
                    <select id="memberRankSelect" class="menu-select">${rankOptions}</select>
                    <button class="btn btn-sm btn-primary menu-action" data-action="apply-rank">Apply Rank</button>
                </div>
            ` : ''}
            ${canKick ? `
                <div class="menu-section">
                    <button class="btn btn-sm btn-danger menu-action" data-action="kick">Kick Member</button>
                </div>
            ` : ''}
            <div class="menu-section">
                <button class="btn btn-sm btn-secondary menu-action" data-action="close">Close</button>
            </div>
        `;

        const select = menu.querySelector('#memberRankSelect');
        if (select) {
            const option = Array.from(select.options).find((opt) => opt.value === currentRank);
            if (option) option.selected = true;
        }

        menu.querySelectorAll('.menu-action').forEach((button) => {
            button.addEventListener('click', async (e) => {
                const action = e.currentTarget?.getAttribute('data-action');
                if (action === 'apply-rank') {
                    await submitMemberRankChange(member);
                } else if (action === 'kick') {
                    await submitKickMember(member);
                } else {
                    closeMemberManageMenu();
                }
            });
        });
    }

    function positionMemberManageMenu(menu, anchor, userId) {
        const anchorEl = anchor || document.querySelector(`.member-card[data-member-id="${userId}"] .btn-icon`);
        if (!anchorEl) {
            menu.style.top = '30%';
            menu.style.left = '50%';
            menu.style.transform = 'translate(-50%, -30%)';
            return;
        }
        const rect = anchorEl.getBoundingClientRect();
        menu.style.transform = 'none';
        menu.style.top = `${window.scrollY + rect.bottom + 8}px`;
        menu.style.left = `${window.scrollX + rect.left - 120}px`;
    }

    function closeMemberManageMenu() {
        const menu = document.getElementById('memberManageMenu');
        if (menu) {
            menu.classList.remove('active');
        }
    }

    function handleMemberMenuOutsideClick(event) {
        const menu = document.getElementById('memberManageMenu');
        if (!menu || !menu.classList.contains('active')) return;
        if (menu.contains(event.target)) return;
        if (event.target.closest('.member-actions')) return;
        closeMemberManageMenu();
    }

    function handleMemberMenuKeydown(event) {
        if (event.key !== 'Escape') return;
        const menu = document.getElementById('memberManageMenu');
        if (menu?.classList.contains('active')) {
            closeMemberManageMenu();
        }
    }

    async function submitMemberRankChange(member) {
        if (!currentAlliance) return;
        const select = document.getElementById('memberRankSelect');
        const newRank = select?.value;
        const currentRank = getMemberRank(member);
        if (!newRank || newRank === currentRank) {
            showNotification('Member is already at that rank', 'info');
            return;
        }

        const rankOrder = ['recruit', 'member', 'officer', 'leader', 'founder'];
        const action =
            rankOrder.indexOf(newRank) > rankOrder.indexOf(currentRank) ? 'promote' : 'demote';

        try {
            const response = await fetch(`${API_BASE}/${currentAlliance.alliance_id}/members/manage`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    Authorization: `Bearer ${localStorage.getItem('token')}`,
                },
                body: JSON.stringify({
                    user_id: member.user_id,
                    action,
                    new_rank: newRank,
                }),
            });

            const payload = await response.json();
            if (!response.ok) {
                throw new Error(payload?.error?.message || payload?.message || 'Failed to update rank');
            }

            showNotification('Member rank updated', 'success');
            closeMemberManageMenu();
            await loadAllianceData();
        } catch (error) {
            console.error('[Alliance Dashboard] Rank update failed:', error);
            showNotification(error.message || 'Failed to update rank', 'error');
        }
    }

    async function submitKickMember(member) {
        if (!currentAlliance) return;
        const confirmKick = confirm(`Kick ${member.username}?`);
        if (!confirmKick) return;

        try {
            const response = await fetch(`${API_BASE}/${currentAlliance.alliance_id}/members/manage`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    Authorization: `Bearer ${localStorage.getItem('token')}`,
                },
                body: JSON.stringify({
                    user_id: member.user_id,
                    action: 'kick',
                }),
            });

            const payload = await response.json();
            if (!response.ok) {
                throw new Error(payload?.error?.message || payload?.message || 'Failed to kick member');
            }

            showNotification('Member removed', 'success');
            closeMemberManageMenu();
            await loadAllianceData();
        } catch (error) {
            console.error('[Alliance Dashboard] Kick failed:', error);
            showNotification(error.message || 'Failed to kick member', 'error');
        }
    }

    function hasAlliancePermission(permission) {
        if (!currentAlliance || !currentAlliance.user_permissions) return false;
        return currentAlliance.user_permissions.includes(permission);
    }

    async function handleDepotRequest(event) {
        event.preventDefault();
        if (!currentAlliance || !currentAlliance.alliance_id) return;

        const hostPlanetId = parseInt(document.getElementById('hostPlanetSelect')?.value || '0', 10);
        const guestPlanetId = parseInt(document.getElementById('guestPlanetSelect')?.value || '0', 10);
        const requestedDeuterium = parseInt(document.getElementById('depotRequestAmount')?.value || '0', 10);
        const notes = document.getElementById('depotRequestNotes')?.value || '';

        if (!hostPlanetId || !guestPlanetId || requestedDeuterium <= 0) {
            showNotification('Please complete all depot fields', 'error');
            return;
        }

        try {
            const response = await fetch(`${API_BASE}/${currentAlliance.alliance_id}/depot/request`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    Authorization: `Bearer ${localStorage.getItem('token')}`,
                },
                body: JSON.stringify({
                    hostPlanetId,
                    guestPlanetId,
                    requestedDeuterium,
                    notes,
                }),
            });

            if (!response.ok) {
                const error = await response.json();
                throw new Error(error?.error?.message || error?.message || 'Failed to submit request');
            }

            showNotification('Depot request submitted', 'success');
            (event.target as HTMLFormElement).reset();
            populatePlanetSelects();
            await loadDepotSessions();
        } catch (error) {
            console.error('[Alliance Dashboard] Depot request failed:', error);
            showNotification(error.message || 'Failed to submit depot request', 'error');
        }
    }

    async function handleSharedTransport(event) {
        event.preventDefault();
        if (!currentAlliance || !currentAlliance.alliance_id) return;

        const originPlanetId = parseInt(document.getElementById('transportPlanetSelect')?.value || '0', 10);
        const targetType = (document.getElementById('transportTargetType') as HTMLSelectElement)?.value || 'treasury';
        const resourceType = (document.getElementById('transportResourceType') as HTMLSelectElement)?.value || 'metal';
        const amount = parseInt(document.getElementById('transportAmount')?.value || '0', 10);
        const notes = document.getElementById('transportNotes')?.value || '';
        const targetPlanetId =
            targetType === 'member'
                ? parseInt(document.getElementById('transportTargetPlanet')?.value || '0', 10)
                : null;

        if (!originPlanetId || amount <= 0) {
            showNotification('Please provide origin planet and amount', 'error');
            return;
        }

        if (targetType === 'member' && !targetPlanetId) {
            showNotification('Please provide member planet ID', 'error');
            return;
        }

        try {
            const response = await fetch(`${API_BASE}/${currentAlliance.alliance_id}/shared-transport`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    Authorization: `Bearer ${localStorage.getItem('token')}`,
                },
                body: JSON.stringify({
                    fromPlanetId: originPlanetId,
                    targetType,
                    targetPlanetId,
                    resourceType,
                    amount,
                    notes,
                }),
            });

            if (!response.ok) {
                const error = await response.json();
                throw new Error(error?.error?.message || error?.message || 'Failed to send transport');
            }

            showNotification('Shared transport dispatched', 'success');
            (event.target as HTMLFormElement).reset();
            populatePlanetSelects();
        } catch (error) {
            console.error('[Alliance Dashboard] Shared transport failed:', error);
            showNotification(error.message || 'Failed to send shared transport', 'error');
        }
    }

    function toggleSharedTransportTarget(eventOrValue) {
        const container = document.getElementById('transportTargetPlanetGroup');
        if (!container) return;
        const value =
            typeof eventOrValue === 'string'
                ? eventOrValue
                : eventOrValue?.target?.value || 'treasury';
        container.style.display = value === 'member' ? 'block' : 'none';
    }

    async function approveDepotSession(sessionId) {
        if (!currentAlliance || !currentAlliance.alliance_id) return;
        const session = depotSessions.find((s) => s.id === sessionId);
        const defaultAmount =
            session?.metadata?.approved_amount ||
            session?.metadata?.requestedDeuterium ||
            session?.metadata?.requested_deuterium ||
            '';
        const input = prompt('Approve amount of deuterium to transfer', defaultAmount);
        if (input === null) return;
        const amount = Number(input);
        if (!amount || amount <= 0) {
            showNotification('Please enter a valid amount', 'error');
            return;
        }
        try {
            const response = await fetch(
                `${API_BASE}/${currentAlliance.alliance_id}/depot/${sessionId}/approve`,
                {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json',
                        Authorization: `Bearer ${localStorage.getItem('token')}`,
                    },
                    body: JSON.stringify({ amount }),
                }
            );

            if (!response.ok) {
                const error = await response.json();
                throw new Error(error?.error?.message || error?.message || 'Failed to approve request');
            }

            showNotification('Depot request approved', 'success');
            await loadDepotSessions();
        } catch (error) {
            console.error('[Alliance Dashboard] Approve depot failed:', error);
            showNotification(error.message || 'Failed to approve request', 'error');
        }
    }

    async function cancelDepotSession(sessionId) {
        if (!currentAlliance || !currentAlliance.alliance_id) return;
        try {
            const response = await fetch(
                `${API_BASE}/${currentAlliance.alliance_id}/depot/${sessionId}/cancel`,
                {
                    method: 'POST',
                    headers: {
                        Authorization: `Bearer ${localStorage.getItem('token')}`,
                    },
                }
            );

            if (!response.ok) {
                const error = await response.json();
                throw new Error(error?.error?.message || error?.message || 'Failed to cancel request');
            }

            showNotification('Depot request cancelled', 'info');
            await loadDepotSessions();
        } catch (error) {
            console.error('[Alliance Dashboard] Cancel depot failed:', error);
            showNotification(error.message || 'Failed to cancel request', 'error');
        }
    }

    async function handleCreateAcsGroup(event) {
        event.preventDefault();
        try {
            const payload = {
                missionType: (document.getElementById('acsMissionType') as HTMLSelectElement)?.value || 'attack',
                targetGalaxy: parseInt(document.getElementById('acsGalaxy')?.value || '1', 10),
                targetSystem: parseInt(document.getElementById('acsSystem')?.value || '1', 10),
                targetPosition: parseInt(document.getElementById('acsPosition')?.value || '1', 10),
                departureWindowStart: (document.getElementById('acsWindowStart') as HTMLInputElement)?.value || undefined,
                departureWindowEnd: (document.getElementById('acsWindowEnd') as HTMLInputElement)?.value || undefined,
                notes: document.getElementById('acsNotes')?.value || '',
            };

            const response = await fetch('/api/acs', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    Authorization: `Bearer ${localStorage.getItem('token')}`,
                },
                body: JSON.stringify(payload),
            });

            if (!response.ok) {
                const error = await response.json();
                throw new Error(error?.message || 'Failed to create ACS group');
            }

            showNotification('ACS group created', 'success');
            (event.target as HTMLFormElement).reset();
            loadAcsGroups();
        } catch (error) {
            console.error('[Alliance Dashboard] Create ACS failed:', error);
            showNotification(error.message || 'Failed to create ACS group', 'error');
        }
    }

    async function joinAcsGroup(groupId) {
        try {
            const defaultPlanetId = playerPlanets[0]?.id || '';
            const planetInput = prompt('Enter planet ID to dispatch from', defaultPlanetId);
            if (planetInput === null) return;
            const planetId = Number(planetInput);
            if (!planetId) {
                showNotification('Invalid planet ID', 'error');
                return;
            }

            const response = await fetch(`/api/acs/${groupId}/join`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    Authorization: `Bearer ${localStorage.getItem('token')}`,
                },
                body: JSON.stringify({ planetId }),
            });

            if (!response.ok) {
                const error = await response.json();
                throw new Error(error?.message || 'Failed to join ACS group');
            }

            showNotification('Joined ACS group', 'success');
            loadAcsGroups();
        } catch (error) {
            console.error('[Alliance Dashboard] Join ACS failed:', error);
            showNotification(error.message || 'Failed to join ACS group', 'error');
        }
    }

    async function leaveAcsGroup(groupId) {
        try {
            const response = await fetch(`/api/acs/${groupId}/leave`, {
                method: 'DELETE',
                headers: {
                    Authorization: `Bearer ${localStorage.getItem('token')}`,
                },
            });

            if (!response.ok) {
                const error = await response.json();
                throw new Error(error?.message || 'Failed to leave ACS group');
            }

            showNotification('Left ACS group', 'info');
            loadAcsGroups();
        } catch (error) {
            console.error('[Alliance Dashboard] Leave ACS failed:', error);
            showNotification(error.message || 'Failed to leave ACS group', 'error');
        }
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
        const locale = getLocale();
        if (typeof Intl !== 'undefined' && Intl.NumberFormat) {
            return new Intl.NumberFormat(locale).format(num);
        }
        return num.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ',');
    }

    /**
     * Utility: Format date
     */
    function formatDate(dateString) {
        const date = new Date(dateString);
        const locale = getLocale();
        if (typeof Intl !== 'undefined' && Intl.DateTimeFormat) {
            return new Intl.DateTimeFormat(locale, {
                year: 'numeric',
                month: 'long',
                day: 'numeric'
            }).format(date);
        }
        return date.toLocaleDateString(locale || 'en-US', {
            year: 'numeric',
            month: 'long',
            day: 'numeric'
        });
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
