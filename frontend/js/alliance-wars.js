/**
 * ALLIANCE WARS JAVASCRIPT
 * Handles war management, battle recording, and real-time updates
 */

(function() {
    'use strict';

    // State management
    let currentAllianceId = null;
    let activeWars = [];
    let pendingWars = [];
    let warHistory = [];
    let socket = null;

    // API endpoints
    const API_BASE = '/api/alliances';

    /**
     * Initialize wars dashboard
     */
    async function init() {
        console.log('[Alliance Wars] Initializing...');
        
        // Load alliance ID
        await loadCurrentAlliance();
        
        // Load war data
        await loadAllWars();
        
        // Setup event listeners
        setupEventListeners();
        
        // Initialize Socket.io
        initializeSocket();
        
        console.log('[Alliance Wars] Initialization complete');
    }

    /**
     * Load current alliance
     */
    async function loadCurrentAlliance() {
        try {
            const token = localStorage.getItem('token');
            const response = await fetch(`${API_BASE}/my-alliance`, {
                headers: {
                    'Authorization': `Bearer ${token}`,
                    'Content-Type': 'application/json'
                }
            });

            if (response.ok) {
                const alliance = await response.json();
                currentAllianceId = alliance.alliance_id;
                console.log('[Alliance Wars] Current alliance ID:', currentAllianceId);
            }
        } catch (error) {
            console.error('[Alliance Wars] Error loading alliance:', error);
        }
    }

    /**
     * Load all wars data
     */
    async function loadAllWars() {
        if (!currentAllianceId) return;

        try {
            const token = localStorage.getItem('token');
            const response = await fetch(`${API_BASE}/${currentAllianceId}/wars`, {
                headers: {
                    'Authorization': `Bearer ${token}`,
                    'Content-Type': 'application/json'
                }
            });

            if (response.ok) {
                const data = await response.json();
                activeWars = data.active || [];
                pendingWars = data.pending || [];
                warHistory = data.history || [];
                
                updateWarStats();
                updateUI();
                console.log('[Alliance Wars] Wars loaded:', { active: activeWars.length, pending: pendingWars.length, history: warHistory.length });
            }
        } catch (error) {
            console.error('[Alliance Wars] Error loading wars:', error);
        }
    }

    /**
     * Update war statistics
     */
    function updateWarStats() {
        const stats = {
            activeWars: activeWars.length,
            victories: warHistory.filter(w => w.outcome === 'VICTORY').length,
            defeats: warHistory.filter(w => w.outcome === 'DEFEAT').length,
            warPoints: activeWars.reduce((sum, w) => sum + (w.our_score || 0), 0)
        };

        updateElement('statActiveWars', stats.activeWars);
        updateElement('statVictories', stats.victories);
        updateElement('statDefeats', stats.defeats);
        updateElement('statWarPoints', stats.warPoints);
    }

    /**
     * Update UI with war data
     */
    function updateUI() {
        // Active wars would be rendered by template, but we can update counts
        const activeTab = document.querySelector('.war-tab[data-tab="active"]');
        if (activeTab) {
            activeTab.textContent = `Active Wars (${activeWars.length})`;
        }

        const pendingTab = document.querySelector('.war-tab[data-tab="pending"]');
        if (pendingTab) {
            pendingTab.textContent = `Pending (${pendingWars.length})`;
        }
    }

    /**
     * Setup event listeners
     */
    function setupEventListeners() {
        // Modal background click
        document.querySelectorAll('.modal').forEach(modal => {
            modal.addEventListener('click', (e) => {
                if (e.target === modal) {
                    closeModal(modal.id);
                }
            });
        });
    }

    /**
     * Switch war tab
     */
    window.switchWarTab = function(tabName) {
        // Update tab buttons
        document.querySelectorAll('.war-tab').forEach(tab => {
            tab.classList.remove('active');
        });
        document.querySelector(`.war-tab[data-tab="${tabName}"]`)?.classList.add('active');

        // Update content
        document.querySelectorAll('.war-tab-content').forEach(content => {
            content.classList.remove('active');
        });
        document.getElementById(`tab${capitalize(tabName)}`)?.classList.add('active');
    };

    /**
     * Show declare war modal
     */
    window.showDeclareWarModal = function() {
        showModal('declareWarModal');
    };

    /**
     * Handle declare war form
     */
    window.handleDeclareWar = async function(event) {
        event.preventDefault();

        const form = event.target;
        const formData = new FormData(form);
        const data = {
            targetAlliance: formData.get('targetAlliance'),
            warType: formData.get('warType'),
            objective: formData.get('objective') || null,
            declaration: formData.get('declaration')
        };

        try {
            const token = localStorage.getItem('token');
            const response = await fetch(`${API_BASE}/${currentAllianceId}/wars/declare`, {
                method: 'POST',
                headers: {
                    'Authorization': `Bearer ${token}`,
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify(data)
            });

            if (response.ok) {
                showNotification('War declared successfully!', 'success');
                closeModal('declareWarModal');
                form.reset();
                await loadAllWars();
            } else {
                const error = await response.json();
                showNotification(error.message || 'Failed to declare war', 'error');
            }
        } catch (error) {
            console.error('[Alliance Wars] Error declaring war:', error);
            showNotification('Failed to declare war', 'error');
        }
    };

    /**
     * View war details
     */
    window.viewWarDetails = function(warId) {
        window.location.href = `/alliance/wars/${warId}`;
    };

    /**
     * Show record battle modal
     */
    window.showRecordBattleModal = function(warId) {
        document.getElementById('battleWarId').value = warId;
        showModal('recordBattleModal');
    };

    /**
     * Handle record battle form
     */
    window.handleRecordBattle = async function(event) {
        event.preventDefault();

        const form = event.target;
        const formData = new FormData(form);
        const warId = formData.get('warId');
        const data = {
            battleType: formData.get('battleType'),
            outcome: formData.get('outcome'),
            points: parseInt(formData.get('points')),
            participants: formData.get('participants')?.split(',').map(p => p.trim()).filter(p => p) || [],
            notes: formData.get('notes')
        };

        try {
            const token = localStorage.getItem('token');
            const response = await fetch(`${API_BASE}/wars/${warId}/battles`, {
                method: 'POST',
                headers: {
                    'Authorization': `Bearer ${token}`,
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify(data)
            });

            if (response.ok) {
                showNotification('Battle recorded successfully!', 'success');
                closeModal('recordBattleModal');
                form.reset();
                await loadAllWars();
            } else {
                const error = await response.json();
                showNotification(error.message || 'Failed to record battle', 'error');
            }
        } catch (error) {
            console.error('[Alliance Wars] Error recording battle:', error);
            showNotification('Failed to record battle', 'error');
        }
    };

    /**
     * Show peace terms modal
     */
    window.showPeaceTermsModal = function(warId) {
        document.getElementById('peaceWarId').value = warId;
        showModal('peaceTermsModal');
    };

    /**
     * Handle propose peace form
     */
    window.handleProposePeace = async function(event) {
        event.preventDefault();

        const form = event.target;
        const formData = new FormData(form);
        const warId = formData.get('warId');
        const data = {
            terms: formData.get('terms'),
            unconditional: formData.get('unconditional') === 'on'
        };

        try {
            const token = localStorage.getItem('token');
            const response = await fetch(`${API_BASE}/wars/${warId}/ceasefire`, {
                method: 'POST',
                headers: {
                    'Authorization': `Bearer ${token}`,
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify(data)
            });

            if (response.ok) {
                showNotification('Peace terms proposed!', 'success');
                closeModal('peaceTermsModal');
                form.reset();
                await loadAllWars();
            } else {
                const error = await response.json();
                showNotification(error.message || 'Failed to propose peace', 'error');
            }
        } catch (error) {
            console.error('[Alliance Wars] Error proposing peace:', error);
            showNotification('Failed to propose peace', 'error');
        }
    };

    /**
     * Accept war declaration
     */
    window.acceptWarDeclaration = async function(warId) {
        if (!confirm('Are you sure you want to accept this war declaration?')) return;

        try {
            const token = localStorage.getItem('token');
            const response = await fetch(`${API_BASE}/wars/${warId}/accept`, {
                method: 'POST',
                headers: {
                    'Authorization': `Bearer ${token}`,
                    'Content-Type': 'application/json'
                }
            });

            if (response.ok) {
                showNotification('War declaration accepted!', 'success');
                await loadAllWars();
            } else {
                const error = await response.json();
                showNotification(error.message || 'Failed to accept war', 'error');
            }
        } catch (error) {
            console.error('[Alliance Wars] Error accepting war:', error);
            showNotification('Failed to accept war', 'error');
        }
    };

    /**
     * Reject war declaration
     */
    window.rejectWarDeclaration = async function(warId) {
        if (!confirm('Are you sure you want to reject this war declaration?')) return;

        try {
            const token = localStorage.getItem('token');
            const response = await fetch(`${API_BASE}/wars/${warId}/reject`, {
                method: 'POST',
                headers: {
                    'Authorization': `Bearer ${token}`,
                    'Content-Type': 'application/json'
                }
            });

            if (response.ok) {
                showNotification('War declaration rejected', 'success');
                await loadAllWars();
            } else {
                const error = await response.json();
                showNotification(error.message || 'Failed to reject war', 'error');
            }
        } catch (error) {
            console.error('[Alliance Wars] Error rejecting war:', error);
            showNotification('Failed to reject war', 'error');
        }
    };

    /**
     * Cancel war declaration
     */
    window.cancelWarDeclaration = async function(warId) {
        if (!confirm('Are you sure you want to cancel this war declaration?')) return;

        try {
            const token = localStorage.getItem('token');
            const response = await fetch(`${API_BASE}/wars/${warId}`, {
                method: 'DELETE',
                headers: {
                    'Authorization': `Bearer ${token}`,
                    'Content-Type': 'application/json'
                }
            });

            if (response.ok) {
                showNotification('War declaration cancelled', 'success');
                await loadAllWars();
            } else {
                const error = await response.json();
                showNotification(error.message || 'Failed to cancel war', 'error');
            }
        } catch (error) {
            console.error('[Alliance Wars] Error cancelling war:', error);
            showNotification('Failed to cancel war', 'error');
        }
    };

    /**
     * View alliance profile
     */
    window.viewAllianceProfile = function(allianceId) {
        window.location.href = `/alliance/profile/${allianceId}`;
    };

    /**
     * View war report
     */
    window.viewWarReport = function(warId) {
        window.location.href = `/alliance/wars/${warId}/report`;
    };

    /**
     * Filter war history
     */
    window.filterWarHistory = function() {
        const filter = document.getElementById('historyFilter')?.value;
        const cards = document.querySelectorAll('.history-war-card');

        cards.forEach(card => {
            const outcome = card.querySelector('.war-outcome').classList;
            if (filter === 'all') {
                card.style.display = 'block';
            } else if (filter === 'victories' && outcome.contains('victory')) {
                card.style.display = 'block';
            } else if (filter === 'defeats' && outcome.contains('defeat')) {
                card.style.display = 'block';
            } else if (filter === 'draws' && outcome.contains('draw')) {
                card.style.display = 'block';
            } else {
                card.style.display = 'none';
            }
        });
    };

    /**
     * Search war history
     */
    window.searchWarHistory = function() {
        const searchTerm = document.getElementById('historySearch')?.value.toLowerCase();
        const cards = document.querySelectorAll('.history-war-card');

        cards.forEach(card => {
            const text = card.querySelector('.war-participants-compact').textContent.toLowerCase();
            card.style.display = text.includes(searchTerm) ? 'block' : 'none';
        });
    };

    /**
     * Initialize Socket.io
     */
    function initializeSocket() {
        if (typeof io === 'undefined') {
            console.warn('[Alliance Wars] Socket.io not available');
            return;
        }

        socket = io();

        socket.on('connect', () => {
            console.log('[Alliance Wars] Socket.io connected');
            if (currentAllianceId) {
                socket.emit('join-alliance-room', currentAllianceId);
            }
        });

        socket.on('war-declared', (data) => {
            console.log('[Alliance Wars] War declared:', data);
            showNotification(`War declared against ${data.enemyName}!`, 'warning');
            loadAllWars();
        });

        socket.on('war-accepted', (data) => {
            console.log('[Alliance Wars] War accepted:', data);
            showNotification(`${data.allianceName} accepted the war!`, 'success');
            loadAllWars();
        });

        socket.on('battle-recorded', (data) => {
            console.log('[Alliance Wars] Battle recorded:', data);
            showNotification(`New battle recorded in war against ${data.enemyName}`, 'info');
            loadAllWars();
        });

        socket.on('war-ended', (data) => {
            console.log('[Alliance Wars] War ended:', data);
            showNotification(`War with ${data.enemyName} has ended!`, 'success');
            loadAllWars();
        });
    }

    /**
     * Modal functions
     */
    window.showModal = function(modalId) {
        const modal = document.getElementById(modalId);
        if (modal) {
            modal.classList.add('active');
        }
    };

    window.closeModal = function(modalId) {
        const modal = document.getElementById(modalId);
        if (modal) {
            modal.classList.remove('active');
        }
    };

    /**
     * Utility functions
     */
    function updateElement(id, value) {
        const element = document.getElementById(id);
        if (element) {
            element.textContent = value;
        }
    }

    function capitalize(str) {
        return str.charAt(0).toUpperCase() + str.slice(1);
    }

    function showNotification(message, type = 'info') {
        if (typeof window.showToast === 'function') {
            window.showToast(message, type);
            return;
        }
        console.log(`[Alliance Wars] ${type.toUpperCase()}: ${message}`);
        alert(message);
    }

    // Initialize on page load
    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', init);
    } else {
        init();
    }

})();
