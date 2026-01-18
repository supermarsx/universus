// @ts-nocheck
/**
 * Messages Page JavaScript
 * Handles inbox, sent messages, and real-time message notifications
 */

const API_BASE_URL = 'http://localhost:3000/api';

// State management
let currentFolder = 'inbox';
let messages = [];
let selectedMessageId = null;
let socket = null;
let currentUserId = null;

/**
 * Initialize the messages page
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
    await loadMessages('inbox');
    await updateUnreadCount();

    // Setup auto-refresh for unread count
    setInterval(updateUnreadCount, 30000);
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
    // Folder tabs
    document.querySelectorAll('.tab-item').forEach(button => {
        button.addEventListener('click', () => {
            const folder = button.getAttribute('data-tab');
            switchFolder(folder);
        });
    });

    // Compose button
    document.getElementById('composeBtn').addEventListener('click', openComposeModal);
    document.getElementById('composeModalClose').addEventListener('click', closeComposeModal);
    document.getElementById('composeCancelBtn').addEventListener('click', closeComposeModal);

    // Compose form
    document.getElementById('composeForm').addEventListener('submit', handleCompose);

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

    socket.on('newMessage', async (data) => {
        console.log('New message received:', data);
        
        // Show notification
        showNotification(`New message from ${data.senderUsername || 'System'}`);
        
        // Refresh if viewing relevant folder
        if (currentFolder === 'inbox' || 
            (currentFolder === 'combat' && data.messageType === 'combat_report') ||
            (currentFolder === 'espionage' && data.messageType === 'espionage_report') ||
            (currentFolder === 'alliance' && data.messageType === 'alliance_circular')) {
            await loadMessages(currentFolder);
        }
        
        // Update unread count
        await updateUnreadCount();
    });

    socket.on('disconnect', () => {
        console.log('Disconnected from server');
    });
}

/**
 * Switch between message folders
 */
async function switchFolder(folder) {
    currentFolder = folder;
    selectedMessageId = null;

    // Update tab buttons
    document.querySelectorAll('.tab-item').forEach(button => {
        if (button.getAttribute('data-tab') === folder) {
            button.classList.add('active');
        } else {
            button.classList.remove('active');
        }
    });

    // Update folder title
    const titles = {
        inbox: i18n.t('messages.folder.inbox', { defaultValue: 'Inbox' }),
        sent: i18n.t('messages.folder.sent', { defaultValue: 'Sent Messages' }),
        combat: i18n.t('messages.folder.combat', { defaultValue: 'Combat Reports' }),
        espionage: i18n.t('messages.folder.espionage', { defaultValue: 'Espionage Reports' }),
        alliance: i18n.t('messages.folder.alliance', { defaultValue: 'Alliance Messages' })
    };
    document.getElementById('currentFolderTitle').textContent = titles[folder] || 'Messages';

    // Load messages for this folder
    await loadMessages(folder);
}

/**
 * Load messages for a specific folder
 */
async function loadMessages(folder) {
    const loadingEl = document.getElementById('messagesLoading');
    const listEl = document.getElementById('messagesList');
    const emptyEl = document.getElementById('emptyState');
    const viewerEl = document.getElementById('messageViewer');

    try {
        loadingEl.style.display = 'block';
        listEl.innerHTML = '';
        emptyEl.style.display = 'none';
        viewerEl.style.display = 'none';

        let endpoint = '';
        let messageType = null;

        if (folder === 'inbox') {
            endpoint = `${API_BASE_URL}/messages/inbox`;
        } else if (folder === 'sent') {
            endpoint = `${API_BASE_URL}/messages/sent`;
        } else if (folder === 'combat') {
            endpoint = `${API_BASE_URL}/messages/inbox`;
            messageType = 'combat_report';
        } else if (folder === 'espionage') {
            endpoint = `${API_BASE_URL}/messages/inbox`;
            messageType = 'espionage_report';
        } else if (folder === 'alliance') {
            endpoint = `${API_BASE_URL}/messages/inbox`;
            messageType = 'alliance_circular';
        }

        const response = await fetch(endpoint, {
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            }
        });

        if (!response.ok) {
            throw new Error('Failed to fetch messages');
        }

        let fetchedMessages = await response.json();

        // Filter by message type if needed
        if (messageType) {
            fetchedMessages = fetchedMessages.filter(msg => msg.messageType === messageType);
        }

        messages = fetchedMessages;
        loadingEl.style.display = 'none';

    if (messages.length === 0) {
            emptyEl.style.display = 'block';
            emptyEl.textContent = i18n.t('messages.noMessages', { defaultValue: 'No messages yet.' });
            return;
        }

        // Render message list
        renderMessageList();

    } catch (error) {
        console.error('Error loading messages:', error);
        loadingEl.style.display = 'none';
        emptyEl.style.display = 'block';
        emptyEl.textContent = i18n.t('messages.errorLoading', { defaultValue: 'Error loading messages' });
    }
}

/**
 * Render message list
 */
function renderMessageList() {
    const listEl = document.getElementById('messagesList');

    listEl.innerHTML = messages.map(msg => {
        const isUnread = !msg.isRead;
        const isSelected = msg.id === selectedMessageId;
        const senderName = msg.fromUsername || 'System';
        const date = formatDateTime(msg.createdAt);
        const preview = msg.content.substring(0, 100);

        const typeBadge = getMessageTypeBadge(msg.messageType);

        return `
            <div class="message-item ${isUnread ? 'unread' : ''} ${isSelected ? 'selected' : ''}" 
                 data-message-id="${msg.id}"
                 onclick="viewMessage(${msg.id})">
                <div class="message-item-header">
                    <span class="message-sender">${escapeHtml(senderName)}${typeBadge}</span>
                    <span class="message-date">${date}</span>
                </div>
                <div class="message-subject">${escapeHtml(msg.subject)}</div>
                <div class="message-preview">${escapeHtml(preview)}...</div>
            </div>
        `;
    }).join('');
}

/**
 * Get message type badge HTML
 */
function getMessageTypeBadge(type) {
    const badges = {
        combat_report: '<span class="message-type-badge type-combat">' + i18n.t('messages.type.combat', { defaultValue: 'COMBAT' }) + '</span>',
        espionage_report: '<span class="message-type-badge type-espionage">' + i18n.t('messages.type.espionage', { defaultValue: 'SPY' }) + '</span>',
        alliance_circular: '<span class="message-type-badge type-alliance">' + i18n.t('messages.type.alliance', { defaultValue: 'ALLIANCE' }) + '</span>',
        system_notification: '<span class="message-type-badge type-system">' + i18n.t('messages.type.system', { defaultValue: 'SYSTEM' }) + '</span>',
        player_message: '<span class="message-type-badge type-player">' + i18n.t('messages.type.player', { defaultValue: 'PLAYER' }) + '</span>',
    };
    return badges[type] || '';
}

/**
 * View a specific message
 */
async function viewMessage(messageId) {
    selectedMessageId = messageId;
    const message = messages.find(m => m.id === messageId);

    if (!message) return;

    // Update selection in list
    renderMessageList();

    // Render message viewer
    const viewerEl = document.getElementById('messageViewer');
    const listEl = document.getElementById('messagesList');

    const senderName = message.fromUsername || 'System';
    const date = formatDateTime(message.createdAt);

    let bodyContent = escapeHtml(message.content);

    // Special rendering for combat reports
    if (message.messageType === 'combat_report' && message.metadata) {
        bodyContent += renderCombatReport(message.metadata);
    }

    // Special rendering for espionage reports
    if (message.messageType === 'espionage_report' && message.metadata) {
        bodyContent += renderEspionageReport(message.metadata);
    }

    viewerEl.innerHTML = `
        <div class="message-viewer">
            <div class="message-viewer-header">
                <h3 class="message-viewer-title">${escapeHtml(message.subject)}</h3>
                <div class="message-viewer-meta">
                    <span>${i18n.t('messages.meta.from', { defaultValue: 'From:' })} ${escapeHtml(senderName)}</span>
                    <span>${i18n.t('messages.meta.date', { defaultValue: 'Date:' })} ${date}</span>
                    <span>${getMessageTypeBadge(message.messageType)}</span>
                </div>
            </div>
            <div class="message-viewer-body">${bodyContent}</div>
            <div class="message-actions">
                ${!message.isRead && currentFolder === 'inbox' ? 
                    '<button class="btn-mark-read" onclick="markAsRead(' + message.id + ')">' + i18n.t('messages.markAsRead', { defaultValue: 'Mark as Read' }) + '</button>' : ''}
                ${message.fromUserId && currentFolder === 'inbox' ? 
                    '<button class="btn-reply" onclick="replyToMessage(' + message.id + ')">' + i18n.t('messages.reply', { defaultValue: 'Reply' }) + '</button>' : ''}
                '<button class="btn-delete" onclick="deleteMessage(${message.id})">' + i18n.t('messages.delete', { defaultValue: 'Delete' }) + '</button>'
                + '<button class="btn-secondary" onclick="closeMessageViewer()">' + i18n.t('messages.close', { defaultValue: 'Close' }) + '</button>'
            </div>
        </div>
    `;

    viewerEl.style.display = 'block';
    listEl.style.display = 'none';

    // Mark as read automatically after viewing
    if (!message.isRead && currentFolder === 'inbox') {
        setTimeout(() => markAsRead(messageId, false), 1000);
    }
}

/**
 * Render combat report details
 */
function renderCombatReport(metadata) {
    if (!metadata || !metadata.attacker || !metadata.defender) {
        return '';
    }

    return `
        <div style="margin-top: 20px;">
            <h4 style="color: #4fc3f7;">${i18n.t('messages.combat.detailsTitle', { defaultValue: 'Combat Details' })}</h4>
            <table class="combat-report-table">
                <tr>
                    <th>${i18n.t('messages.combat.side', { defaultValue: 'Side' })}</th>
                    <th>${i18n.t('messages.combat.player', { defaultValue: 'Player' })}</th>
                    <th>${i18n.t('messages.combat.shipsLost', { defaultValue: 'Ships Lost' })}</th>
                    <th>${i18n.t('messages.combat.defenseLost', { defaultValue: 'Defense Lost' })}</th>
                </tr>
                <tr>
                    <td><strong>${i18n.t('messages.combat.attacker', { defaultValue: 'Attacker' })}</strong></td>
                    <td>${escapeHtml(metadata.attacker.username || i18n.t('messages.unknown', { defaultValue: 'Unknown' }))}</td>
                    <td>${metadata.attacker.shipsLost || 0}</td>
                    <td>-</td>
                </tr>
                <tr>
                    <td><strong>${i18n.t('messages.combat.defender', { defaultValue: 'Defender' })}</strong></td>
                    <td>${escapeHtml(metadata.defender.username || i18n.t('messages.unknown', { defaultValue: 'Unknown' }))}</td>
                    <td>${metadata.defender.shipsLost || 0}</td>
                    <td>${metadata.defender.defenseLost || 0}</td>
                </tr>
            </table>
            <p style="margin-top: 15px;">
                <strong>${i18n.t('messages.combat.resultLabel', { defaultValue: 'Result:' })}</strong> ${metadata.winner === 'attacker' ? i18n.t('messages.combat.attackerVictory', { defaultValue: 'Attacker Victory' }) : i18n.t('messages.combat.defenderVictory', { defaultValue: 'Defender Victory' })}
            </p>
            ${metadata.loot ? `
                <p><strong>${i18n.t('messages.combat.lootLabel', { defaultValue: 'Loot:' })}</strong> 
                ${i18n.t('messages.combat.metal', { defaultValue: 'Metal' })}: ${formatNumber(metadata.loot.metal || 0)}, 
                ${i18n.t('messages.combat.crystal', { defaultValue: 'Crystal' })}: ${formatNumber(metadata.loot.crystal || 0)}, 
                ${i18n.t('messages.combat.deuterium', { defaultValue: 'Deuterium' })}: ${formatNumber(metadata.loot.deuterium || 0)}
                </p>
            ` : ''}
        </div>
    `;
}

/**
 * Render espionage report details
 */
function renderEspionageReport(metadata) {
    if (!metadata || !metadata.target) {
        return '';
    }

    return `
        <div style="margin-top: 20px;">
            <h4 style="color: #4fc3f7;">${i18n.t('messages.espionage.title', { defaultValue: 'Espionage Report' })}</h4>
            <p><strong>${i18n.t('messages.espionage.targetLabel', { defaultValue: 'Target:' })}</strong> ${escapeHtml(metadata.target.username || i18n.t('messages.unknown', { defaultValue: 'Unknown' }))}</p>
            <p><strong>${i18n.t('messages.espionage.planetLabel', { defaultValue: 'Planet:' })}</strong> ${metadata.target.planet || i18n.t('messages.unknown', { defaultValue: 'Unknown' })}</p>
            ${metadata.resources ? `
                <h5 style="color: #4fc3f7; margin-top: 15px;">${i18n.t('messages.espionage.resourcesTitle', { defaultValue: 'Resources' })}</h5>
                <p>
                    ${i18n.t('messages.espionage.metal', { defaultValue: 'Metal' })}: ${formatNumber(metadata.resources.metal || 0)}<br>
                    ${i18n.t('messages.espionage.crystal', { defaultValue: 'Crystal' })}: ${formatNumber(metadata.resources.crystal || 0)}<br>
                    ${i18n.t('messages.espionage.deuterium', { defaultValue: 'Deuterium' })}: ${formatNumber(metadata.resources.deuterium || 0)}
                </p>
            ` : ''}
            ${metadata.fleet ? `
                <h5 style="color: #4fc3f7; margin-top: 15px;">${i18n.t('messages.espionage.fleetTitle', { defaultValue: 'Fleet' })}</h5>
                <p>${JSON.stringify(metadata.fleet, null, 2)}</p>
            ` : ''}
            ${metadata.defense ? `
                <h5 style="color: #4fc3f7; margin-top: 15px;">${i18n.t('messages.espionage.defenseTitle', { defaultValue: 'Defense' })}</h5>
                <p>${JSON.stringify(metadata.defense, null, 2)}</p>
            ` : ''}
        </div>
    `;
}

/**
 * Close message viewer and show list
 */
function closeMessageViewer() {
    document.getElementById('messageViewer').style.display = 'none';
    document.getElementById('messagesList').style.display = 'flex';
    selectedMessageId = null;
    renderMessageList();
}

/**
 * Mark message as read
 */
async function markAsRead(messageId, updateUI = true) {
    try {
        const response = await fetch(`${API_BASE_URL}/messages/${messageId}/read`, {
            method: 'PUT',
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            }
        });

        if (!response.ok) {
            throw new Error('Failed to mark message as read');
        }

        // Update local state
        const message = messages.find(m => m.id === messageId);
        if (message) {
            message.isRead = true;
        }

        if (updateUI) {
            await loadMessages(currentFolder);
            await updateUnreadCount();
        }

    } catch (error) {
        console.error('Error marking message as read:', error);
    }
}

/**
 * Delete a message
 */
async function deleteMessage(messageId) {
    if (!confirm(i18n.t('messages.areYouSureDelete', { defaultValue: 'Are you sure you want to delete this message?' }))) {
        return;
    }

    try {
        const response = await fetch(`${API_BASE_URL}/messages/${messageId}`, {
            method: 'DELETE',
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            }
        });

        if (!response.ok) {
            throw new Error('Failed to delete message');
        }

        // Reload messages
        closeMessageViewer();
        await loadMessages(currentFolder);
        await updateUnreadCount();

        showNotification(i18n.t('messages.messageDeleted', { defaultValue: 'Message deleted successfully' }));

    } catch (error) {
        console.error('Error deleting message:', error);
            alert(i18n.t('messages.failedToDelete', { defaultValue: 'Failed to delete message' }));
    }
}

/**
 * Reply to a message
 */
function replyToMessage(messageId) {
    const message = messages.find(m => m.id === messageId);
    if (!message || !message.fromUsername) return;

    openComposeModal();
    document.getElementById('recipientUsername').value = message.fromUsername;
    document.getElementById('messageSubject').value = 'RE: ' + message.subject;
}

/**
 * Open compose message modal
 */
function openComposeModal() {
    document.getElementById('composeModal').classList.add('active');
}

/**
 * Close compose message modal
 */
function closeComposeModal() {
    document.getElementById('composeModal').classList.remove('active');
    document.getElementById('composeForm').reset();
}

/**
 * Handle message composition
 */
async function handleCompose(e) {
    e.preventDefault();

    const recipientInput = document.getElementById('recipientUsername').value;
    const subject = document.getElementById('messageSubject').value;
    const content = document.getElementById('messageContent').value;

    try {
        // Try to parse as user ID first, otherwise treat as username
        let toUserId = null;
        if (/^\d+$/.test(recipientInput)) {
            toUserId = parseInt(recipientInput);
        } else {
            // We need to look up the user by username
            // For now, we'll just show an error - ideally we'd have an endpoint for this
            alert(i18n.t('messages.pleaseEnterNumericId', { defaultValue: 'Please enter a numeric user ID. Username lookup not yet implemented.' }));
            return;
        }

        const response = await fetch(`${API_BASE_URL}/messages/send`, {
            method: 'POST',
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`,
                'Content-Type': 'application/json'
            },
            body: JSON.stringify({
                toUserId,
                subject,
                content,
                messageType: 'player_message'
            })
        });

        if (!response.ok) {
            throw new Error('Failed to send message');
        }

        closeComposeModal();
        showNotification(i18n.t('messages.messageSent', { defaultValue: 'Message sent successfully' }));

        // Refresh sent folder if viewing it
        if (currentFolder === 'sent') {
            await loadMessages('sent');
        }

    } catch (error) {
        console.error('Error sending message:', error);
            alert(i18n.t('messages.failedToSend', { error: error.message, defaultValue: 'Failed to send message: ' + error.message }));
    }
}

/**
 * Update unread message count
 */
async function updateUnreadCount() {
    try {
        const response = await fetch(`${API_BASE_URL}/messages/unread-count`, {
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            }
        });

        if (!response.ok) {
            throw new Error('Failed to fetch unread count');
        }

        const data = await response.json();
        const count = data.count || 0;

        // Update badge
        const badge = document.getElementById('inboxUnread');
        if (count > 0) {
            badge.textContent = count;
            badge.style.display = 'inline-block';
        } else {
            badge.style.display = 'none';
        }

    } catch (error) {
        console.error('Error fetching unread count:', error);
    }
}

/**
 * Show notification
 */
function showNotification(message) {
    // Simple alert for now - could be enhanced with toast notifications
    console.log('Notification:', message);
    
    // Future: implement a toast notification system here
    // For now, we'll just log it
}

/**
 * Format number with commas
 */
function formatNumber(num) {
    if (typeof num !== 'number') return '0';
    const locale = getLocale();
    if (typeof Intl !== 'undefined' && Intl.NumberFormat) {
        return new Intl.NumberFormat(locale).format(num);
    }
    return num.toLocaleString();
}

function formatDateTime(value) {
    const date = value ? new Date(value) : new Date();
    const locale = getLocale();
    if (typeof Intl !== 'undefined' && Intl.DateTimeFormat) {
        return new Intl.DateTimeFormat(locale, {
            year: 'numeric',
            month: 'short',
            day: 'numeric',
            hour: '2-digit',
            minute: '2-digit',
        }).format(date);
    }
    return date.toLocaleString();
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
