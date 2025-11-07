/**
 * ALLIANCE DIPLOMACY - Client-side JavaScript
 * Handles diplomatic relations, treaty proposals, and real-time updates
 */

// Global state
let currentRelationId = null;
let allRelations = [];
let allProposals = [];
let socket = null;

// Initialize on page load
document.addEventListener('DOMContentLoaded', () => {
    initializeDiplomacy();
    setupSocketListeners();
});

// === INITIALIZATION === //

function initializeDiplomacy() {
    loadDiplomaticRelations();
    loadPendingProposals();
    loadDiplomaticHistory();
    
    // Connect to Socket.io for real-time updates
    if (typeof io !== 'undefined') {
        socket = io();
    }
}

function setupSocketListeners() {
    if (!socket) return;

    // Listen for diplomatic events
    socket.on('diplomacy:treaty_proposed', async (data) => {
        console.log('Treaty proposed:', data);
        await loadPendingProposals();
        showNotification('New diplomatic proposal received', 'info');
    });

    socket.on('diplomacy:treaty_accepted', async (data) => {
        console.log('Treaty accepted:', data);
        await loadDiplomaticRelations();
        await loadPendingProposals();
        await loadDiplomaticHistory();
        showNotification('Diplomatic treaty has been accepted', 'success');
    });

    socket.on('diplomacy:treaty_rejected', async (data) => {
        console.log('Treaty rejected:', data);
        await loadPendingProposals();
        showNotification('Diplomatic proposal was rejected', 'warning');
    });

    socket.on('diplomacy:relation_changed', async (data) => {
        console.log('Diplomatic relation changed:', data);
        await loadDiplomaticRelations();
        await loadDiplomaticHistory();
        showNotification('Diplomatic relation has been updated', 'info');
    });

    socket.on('diplomacy:treaty_broken', async (data) => {
        console.log('Treaty broken:', data);
        await loadDiplomaticRelations();
        await loadDiplomaticHistory();
        showNotification('Diplomatic treaty has been broken', 'error');
    });
}

// === API CALLS === //

async function loadDiplomaticRelations() {
    try {
        const response = await fetch('/api/alliance/diplomacy/relations', {
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            }
        });

        if (!response.ok) throw new Error('Failed to load relations');

        const data = await response.json();
        allRelations = data.relations || [];
        renderRelations(allRelations);
    } catch (error) {
        console.error('Error loading diplomatic relations:', error);
        showNotification('Failed to load diplomatic relations', 'error');
    }
}

async function loadPendingProposals() {
    try {
        const response = await fetch('/api/alliance/diplomacy/proposals/pending', {
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            }
        });

        if (!response.ok) throw new Error('Failed to load proposals');

        const data = await response.json();
        allProposals = data.proposals || [];
        renderProposals(allProposals);
    } catch (error) {
        console.error('Error loading pending proposals:', error);
        showNotification('Failed to load pending proposals', 'error');
    }
}

async function loadDiplomaticHistory() {
    try {
        const response = await fetch('/api/alliance/diplomacy/history', {
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            }
        });

        if (!response.ok) throw new Error('Failed to load history');

        const data = await response.json();
        renderHistory(data.history || []);
    } catch (error) {
        console.error('Error loading diplomatic history:', error);
    }
}

async function proposeTreaty(formData) {
    try {
        const response = await fetch('/api/alliance/diplomacy/propose', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            },
            body: JSON.stringify(formData)
        });

        if (!response.ok) {
            const error = await response.json();
            throw new Error(error.message || 'Failed to propose treaty');
        }

        const data = await response.json();
        showNotification('Treaty proposal sent successfully', 'success');
        await loadPendingProposals();
        return data;
    } catch (error) {
        console.error('Error proposing treaty:', error);
        showNotification(error.message, 'error');
        throw error;
    }
}

async function acceptProposal(proposalId) {
    try {
        const response = await fetch(`/api/alliance/diplomacy/proposals/${proposalId}/respond`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            },
            body: JSON.stringify({ action: 'accept' })
        });

        if (!response.ok) {
            const error = await response.json();
            throw new Error(error.message || 'Failed to accept proposal');
        }

        showNotification('Treaty proposal accepted', 'success');
        await loadDiplomaticRelations();
        await loadPendingProposals();
        await loadDiplomaticHistory();
    } catch (error) {
        console.error('Error accepting proposal:', error);
        showNotification(error.message, 'error');
    }
}

async function rejectProposal(proposalId) {
    try {
        const response = await fetch(`/api/alliance/diplomacy/proposals/${proposalId}/respond`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            },
            body: JSON.stringify({ action: 'reject' })
        });

        if (!response.ok) {
            const error = await response.json();
            throw new Error(error.message || 'Failed to reject proposal');
        }

        showNotification('Treaty proposal rejected', 'success');
        await loadPendingProposals();
        await loadDiplomaticHistory();
    } catch (error) {
        console.error('Error rejecting proposal:', error);
        showNotification(error.message, 'error');
    }
}

async function cancelProposal(proposalId) {
    try {
        const response = await fetch(`/api/alliance/diplomacy/proposals/${proposalId}`, {
            method: 'DELETE',
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            }
        });

        if (!response.ok) {
            const error = await response.json();
            throw new Error(error.message || 'Failed to cancel proposal');
        }

        showNotification('Treaty proposal cancelled', 'success');
        await loadPendingProposals();
    } catch (error) {
        console.error('Error cancelling proposal:', error);
        showNotification(error.message, 'error');
    }
}

async function breakTreaty(relationId, reason) {
    try {
        const response = await fetch(`/api/alliance/diplomacy/terminate/${relationId}`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            },
            body: JSON.stringify({ reason: reason || '' })
        });

        if (!response.ok) {
            const error = await response.json();
            throw new Error(error.message || 'Failed to break treaty');
        }

        showNotification('Diplomatic treaty broken', 'success');
        await loadDiplomaticRelations();
        await loadDiplomaticHistory();
        closeModal('breakTreatyModal');
    } catch (error) {
        console.error('Error breaking treaty:', error);
        showNotification(error.message, 'error');
    }
}

// === RENDERING === //

function renderRelations(relations) {
    const container = document.getElementById('relationsGrid');
    if (!container) return;

    if (relations.length === 0) {
        container.innerHTML = `
            <div class="empty-state">
                <span class="css-icon icon-handshake"></span>
                <p>No diplomatic relations established yet</p>
            </div>
        `;
        return;
    }

    container.innerHTML = relations.map(relation => `
        <div class="relation-card relation-type-${relation.type.toLowerCase()}" data-relation-id="${relation.id}">
            <div class="relation-header">
                <div class="relation-alliance">
                    <div class="alliance-tag">${escapeHtml(relation.target_alliance_tag)}</div>
                    <div class="alliance-name">${escapeHtml(relation.target_alliance_name)}</div>
                </div>
                <span class="relation-type-badge ${relation.type.toLowerCase()}">${relation.type}</span>
            </div>
            
            <div class="relation-info">
                <div class="info-row">
                    <span class="info-label">Established:</span>
                    <span class="info-value">${formatDate(relation.established_at)}</span>
                </div>
                ${relation.expires_at ? `
                <div class="info-row">
                    <span class="info-label">Expires:</span>
                    <span class="info-value">${formatDate(relation.expires_at)}</span>
                </div>
                ` : ''}
                <div class="info-row">
                    <span class="info-label">Proposed by:</span>
                    <span class="info-value">${escapeHtml(relation.proposed_by_name)}</span>
                </div>
            </div>

            ${relation.terms ? `
            <div class="relation-terms">
                <div class="terms-label">Treaty Terms:</div>
                <p class="terms-text">${escapeHtml(relation.terms)}</p>
            </div>
            ` : ''}

            <div class="relation-actions">
                <button class="btn btn-sm btn-secondary" onclick="viewRelationDetails(${relation.id})">
                    <span class="css-icon icon-view"></span> View Details
                </button>
                <button class="btn btn-sm btn-danger" onclick="showBreakTreatyConfirm(${relation.id})">
                    Break Treaty
                </button>
            </div>
        </div>
    `).join('');
}

function renderProposals(proposals) {
    const container = document.getElementById('proposalsList');
    if (!container) return;

    if (proposals.length === 0) {
        container.innerHTML = '<div class="empty-state-small"><p>No pending proposals</p></div>';
        return;
    }

    container.innerHTML = proposals.map(proposal => `
        <div class="proposal-card" data-proposal-id="${proposal.id}">
            <div class="proposal-header">
                <div class="proposal-type">
                    <span class="relation-type-badge ${proposal.type.toLowerCase()}">${proposal.type}</span>
                </div>
                <div class="proposal-direction">
                    ${proposal.is_incoming 
                        ? '<span class="direction-badge incoming">Incoming</span>'
                        : '<span class="direction-badge outgoing">Outgoing</span>'}
                </div>
            </div>

            <div class="proposal-content">
                <div class="proposal-alliance">
                    <strong>${proposal.is_incoming ? 'From' : 'To'}:</strong>
                    <span class="alliance-tag">${escapeHtml(proposal.target_alliance_tag)}</span>
                    ${escapeHtml(proposal.target_alliance_name)}
                </div>
                
                <div class="proposal-info">
                    <div class="info-item">
                        <span class="info-label">Proposed:</span>
                        <span class="info-value">${formatTimeAgo(proposal.proposed_at)}</span>
                    </div>
                    ${proposal.duration_days ? `
                    <div class="info-item">
                        <span class="info-label">Duration:</span>
                        <span class="info-value">${proposal.duration_days} days</span>
                    </div>
                    ` : ''}
                </div>

                ${proposal.terms ? `
                <div class="proposal-terms">
                    <div class="terms-label">Proposed Terms:</div>
                    <p class="terms-text">${escapeHtml(proposal.terms)}</p>
                </div>
                ` : ''}
            </div>

            <div class="proposal-actions">
                ${proposal.is_incoming ? `
                    <button class="btn btn-sm btn-success" onclick="acceptProposal(${proposal.id})">
                        <span class="css-icon icon-check"></span> Accept
                    </button>
                    <button class="btn btn-sm btn-danger" onclick="rejectProposal(${proposal.id})">
                        <span class="css-icon icon-close"></span> Reject
                    </button>
                ` : `
                    <button class="btn btn-sm btn-secondary" onclick="cancelProposal(${proposal.id})">
                        Cancel Proposal
                    </button>
                `}
            </div>
        </div>
    `).join('');
}

function renderHistory(history) {
    const container = document.getElementById('historyTimeline');
    if (!container) return;

    if (history.length === 0) {
        container.innerHTML = '<div class="empty-state-small"><p>No diplomatic history to display</p></div>';
        return;
    }

    container.innerHTML = history.map(event => `
        <div class="history-event event-type-${event.event_type.toLowerCase()}">
            <div class="event-marker"></div>
            <div class="event-content">
                <div class="event-time">${formatTimeAgo(event.created_at)}</div>
                <div class="event-message">${event.message}</div>
                ${event.details ? `<div class="event-details">${escapeHtml(event.details)}</div>` : ''}
            </div>
        </div>
    `).join('');
}

// === UI INTERACTIONS === //

function filterRelations() {
    const filterValue = document.getElementById('relationTypeFilter')?.value;
    if (!filterValue) {
        renderRelations(allRelations);
        return;
    }

    const filtered = allRelations.filter(r => r.type === filterValue);
    renderRelations(filtered);
}

function refreshRelations() {
    loadDiplomaticRelations();
    showNotification('Relations refreshed', 'info');
}

function showProposeTreatyModal() {
    openModal('proposeTreatyModal');
}

function viewRelationDetails(relationId) {
    const relation = allRelations.find(r => r.id === relationId);
    if (!relation) return;

    // Populate modal with detailed information
    const content = document.getElementById('relationDetailsContent');
    if (content) {
        content.innerHTML = `
            <div class="relation-details">
                <h3>${escapeHtml(relation.target_alliance_name)}</h3>
                <p><strong>Type:</strong> ${relation.type}</p>
                <p><strong>Established:</strong> ${formatDate(relation.established_at)}</p>
                ${relation.expires_at ? `<p><strong>Expires:</strong> ${formatDate(relation.expires_at)}</p>` : ''}
                <p><strong>Terms:</strong></p>
                <p>${escapeHtml(relation.terms || 'No terms specified')}</p>
            </div>
        `;
    }
    openModal('relationDetailsModal');
}

function showBreakTreatyConfirm(relationId) {
    currentRelationId = relationId;
    openModal('breakTreatyModal');
}

function confirmBreakTreaty() {
    if (!currentRelationId) return;
    const reason = document.getElementById('breakReasonInput')?.value || '';
    breakTreaty(currentRelationId, reason);
    currentRelationId = null;
}

function updateRelationDescription() {
    const select = document.getElementById('relationTypeInput');
    const descElement = document.getElementById('relationDescription');
    if (!select || !descElement) return;

    const descriptions = {
        'ALLIED': 'Full alliance with shared defense, resource sharing benefits, and coordinated strategies.',
        'NAP': 'Non-aggression pact - parties agree not to attack each other for the duration.',
        'TRADE': 'Trade agreement allowing resource exchanges with reduced taxes and priority access.',
        'DEFENSE': 'Mutual defense pact - both alliances commit to defending each other if attacked.'
    };

    descElement.textContent = descriptions[select.value] || '';
}

// === FORM HANDLERS === //

async function handleProposeTreaty(event) {
    event.preventDefault();
    
    const formData = {
        targetAlliance: document.getElementById('targetAllianceInput').value,
        relationType: document.getElementById('relationTypeInput').value,
        duration: document.getElementById('durationInput').value || null,
        terms: document.getElementById('termsInput').value
    };

    try {
        await proposeTreaty(formData);
        closeModal('proposeTreatyModal');
        document.getElementById('proposeTreatyForm').reset();
    } catch (error) {
        // Error already handled in proposeTreaty
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

function formatDate(dateString) {
    if (!dateString) return '-';
    const date = new Date(dateString);
    return date.toLocaleDateString('en-US', { 
        year: 'numeric', 
        month: 'short', 
        day: 'numeric' 
    });
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
