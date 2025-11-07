// Account Security Dashboard JavaScript
// Manages security overview, sessions, and security events

(function() {
    'use strict';

    const API_BASE = '/api/account';
    let currentUser = null;
    let securitySummary = null;
    let activeSessions = [];

    // Initialize on page load
    document.addEventListener('DOMContentLoaded', async () => {
        await loadSecuritySummary();
        await loadActiveSessions();
        await loadSecurityEvents();
        setupEventListeners();
        
        // Refresh data every 30 seconds
        setInterval(() => {
            loadSecuritySummary();
            loadActiveSessions();
        }, 30000);
    });

    // Load security summary
    async function loadSecuritySummary() {
        try {
            const response = await fetch(`${API_BASE}/security/summary`, {
                headers: {
                    'Authorization': `Bearer ${getAuthToken()}`
                }
            });

            if (!response.ok) throw new Error('Failed to load security summary');

            const data = await response.json();
            securitySummary = data.summary;
            renderSecuritySummary(securitySummary);
        } catch (error) {
            console.error('Error loading security summary:', error);
            showError('Failed to load security summary');
        }
    }

    // Render security summary
    function renderSecuritySummary(summary) {
        // Account status
        const statusBadge = document.getElementById('security-status-badge');
        const accountStatus = document.getElementById('account-status');
        const accountDetail = document.getElementById('account-detail');
        
        if (summary.is_locked) {
            statusBadge.textContent = 'Locked';
            statusBadge.className = 'status-badge status-locked';
            accountStatus.textContent = 'Locked';
            accountDetail.textContent = 'Account is temporarily locked';
        } else if (summary.account_status === 'suspended') {
            statusBadge.textContent = 'Suspended';
            statusBadge.className = 'status-badge status-suspended';
            accountStatus.textContent = 'Suspended';
            accountDetail.textContent = 'Account access is suspended';
        } else {
            statusBadge.textContent = 'Active';
            statusBadge.className = 'status-badge status-active';
            accountStatus.textContent = 'Active';
            accountDetail.textContent = 'All systems operational';
        }

        // Risk level
        const riskLevel = document.getElementById('risk-level');
        riskLevel.textContent = `Risk Level: ${summary.risk_level.toUpperCase()}`;
        riskLevel.className = `risk-level risk-${summary.risk_level}`;

        // Email verification
        const emailVerified = document.getElementById('email-verified');
        emailVerified.textContent = summary.email_verified ? 'Verified' : 'Not Verified';
        emailVerified.className = summary.email_verified ? 'card-value verified' : 'card-value not-verified';

        // 2FA status
        const twoFAStatus = document.getElementById('2fa-status');
        twoFAStatus.textContent = summary.has_2fa ? 'Enabled' : 'Disabled';
        twoFAStatus.className = summary.has_2fa ? 'card-value enabled' : 'card-value disabled';

        // Active sessions count
        const sessionsCount = document.getElementById('active-sessions-count');
        sessionsCount.textContent = summary.active_sessions;

        // Show risk assessment if needed
        if (summary.risk_level !== 'low') {
            showRiskAssessment(summary);
        }
    }

    // Show risk assessment and recommendations
    function showRiskAssessment(summary) {
        const section = document.getElementById('risk-assessment-section');
        const list = document.getElementById('recommendations-list');
        section.style.display = 'block';

        const recommendations = [];

        if (!summary.has_2fa) {
            recommendations.push({
                icon: 'shield',
                title: 'Enable Two-Factor Authentication',
                description: 'Add an extra layer of security to your account',
                action: 'Setup 2FA',
                link: '/account/2fa-setup'
            });
        }

        if (!summary.email_verified) {
            recommendations.push({
                icon: 'mail',
                title: 'Verify Your Email',
                description: 'Verify your email address for account recovery',
                action: 'Verify Email',
                link: '/account/email-verification'
            });
        }

        if (summary.active_sessions > 5) {
            recommendations.push({
                icon: 'alert',
                title: 'Multiple Active Sessions Detected',
                description: `You have ${summary.active_sessions} active sessions. Review and terminate any suspicious sessions.`,
                action: 'Review Sessions',
                link: '/account/sessions'
            });
        }

        if (summary.recent_security_events > 5) {
            recommendations.push({
                icon: 'alert-triangle',
                title: 'Unusual Security Activity',
                description: 'Multiple security events detected in the last 30 days',
                action: 'View Activity',
                link: '/account/activity-log'
            });
        }

        list.innerHTML = recommendations.map(rec => `
            <div class="recommendation-item">
                <div class="recommendation-icon">
                    ${getIconSVG(rec.icon)}
                </div>
                <div class="recommendation-content">
                    <h3>${rec.title}</h3>
                    <p>${rec.description}</p>
                    <a href="${rec.link}" class="btn btn-primary btn-sm">${rec.action}</a>
                </div>
            </div>
        `).join('');
    }

    // Load active sessions
    async function loadActiveSessions() {
        try {
            const response = await fetch(`${API_BASE}/sessions`, {
                headers: {
                    'Authorization': `Bearer ${getAuthToken()}`
                }
            });

            if (!response.ok) throw new Error('Failed to load sessions');

            const data = await response.json();
            activeSessions = data.sessions;
            renderActiveSessions(activeSessions);
        } catch (error) {
            console.error('Error loading sessions:', error);
            showError('Failed to load active sessions');
        }
    }

    // Render active sessions
    function renderActiveSessions(sessions) {
        const list = document.getElementById('sessions-list');
        
        if (sessions.length === 0) {
            list.innerHTML = '<p class="empty-state">No active sessions</p>';
            return;
        }

        list.innerHTML = sessions.slice(0, 3).map(session => `
            <div class="session-item">
                <div class="session-icon">
                    ${getDeviceIcon(session.device_type)}
                </div>
                <div class="session-info">
                    <div class="session-device">
                        <strong>${session.device_name || 'Unknown Device'}</strong>
                        ${session.is_trusted ? '<span class="trusted-badge">Trusted</span>' : ''}
                    </div>
                    <div class="session-details">
                        <span>${session.browser} on ${session.os}</span>
                        <span>${session.ip_address}</span>
                        ${session.location ? `<span>${session.location}</span>` : ''}
                    </div>
                    <div class="session-time">
                        Last active: ${formatTimeAgo(session.last_activity)}
                    </div>
                </div>
                <div class="session-actions">
                    <button class="btn btn-sm btn-danger" onclick="window.accountSecurity.terminateSession(${session.id})">
                        Terminate
                    </button>
                </div>
            </div>
        `).join('');
    }

    // Load recent security events
    async function loadSecurityEvents() {
        try {
            const response = await fetch(`${API_BASE}/security/logs?limit=5`, {
                headers: {
                    'Authorization': `Bearer ${getAuthToken()}`
                }
            });

            if (!response.ok) throw new Error('Failed to load security events');

            const data = await response.json();
            renderSecurityEvents(data.logs);
        } catch (error) {
            console.error('Error loading security events:', error);
            showError('Failed to load security events');
        }
    }

    // Render security events
    function renderSecurityEvents(events) {
        const list = document.getElementById('security-events-list');
        
        if (events.length === 0) {
            list.innerHTML = '<p class="empty-state">No recent events</p>';
            return;
        }

        list.innerHTML = events.map(event => `
            <div class="event-item severity-${event.severity}">
                <div class="event-icon">
                    ${getSeverityIcon(event.severity)}
                </div>
                <div class="event-content">
                    <div class="event-type">${formatEventType(event.event_type)}</div>
                    <div class="event-description">${event.event_description}</div>
                    <div class="event-meta">
                        <span>${formatDateTime(event.created_at)}</span>
                        ${event.ip_address ? `<span>IP: ${event.ip_address}</span>` : ''}
                    </div>
                </div>
            </div>
        `).join('');
    }

    // Setup event listeners
    function setupEventListeners() {
        const terminateAllBtn = document.getElementById('terminate-all-btn');
        if (terminateAllBtn) {
            terminateAllBtn.addEventListener('click', terminateAllSessions);
        }
    }

    // Terminate all other sessions
    async function terminateAllSessions() {
        if (!confirm('Are you sure you want to terminate all other sessions? You will remain logged in on this device.')) {
            return;
        }

        try {
            const response = await fetch(`${API_BASE}/sessions`, {
                method: 'DELETE',
                headers: {
                    'Authorization': `Bearer ${getAuthToken()}`,
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({ except_current: true })
            });

            if (!response.ok) throw new Error('Failed to terminate sessions');

            showSuccess('All other sessions have been terminated');
            await loadActiveSessions();
            await loadSecuritySummary();
        } catch (error) {
            console.error('Error terminating sessions:', error);
            showError('Failed to terminate sessions');
        }
    }

    // Terminate specific session
    async function terminateSession(sessionId) {
        if (!confirm('Are you sure you want to terminate this session?')) {
            return;
        }

        try {
            const response = await fetch(`${API_BASE}/sessions/${sessionId}`, {
                method: 'DELETE',
                headers: {
                    'Authorization': `Bearer ${getAuthToken()}`
                }
            });

            if (!response.ok) throw new Error('Failed to terminate session');

            showSuccess('Session terminated successfully');
            await loadActiveSessions();
            await loadSecuritySummary();
        } catch (error) {
            console.error('Error terminating session:', error);
            showError('Failed to terminate session');
        }
    }

    // Utility functions
    function getAuthToken() {
        return localStorage.getItem('auth_token') || sessionStorage.getItem('auth_token');
    }

    function formatTimeAgo(dateString) {
        const date = new Date(dateString);
        const now = new Date();
        const diff = now - date;
        
        const minutes = Math.floor(diff / 60000);
        const hours = Math.floor(diff / 3600000);
        const days = Math.floor(diff / 86400000);
        
        if (minutes < 1) return 'Just now';
        if (minutes < 60) return `${minutes} minute${minutes > 1 ? 's' : ''} ago`;
        if (hours < 24) return `${hours} hour${hours > 1 ? 's' : ''} ago`;
        return `${days} day${days > 1 ? 's' : ''} ago`;
    }

    function formatDateTime(dateString) {
        const date = new Date(dateString);
        return date.toLocaleString();
    }

    function formatEventType(type) {
        return type.split('_').map(word => 
            word.charAt(0).toUpperCase() + word.slice(1).toLowerCase()
        ).join(' ');
    }

    function getDeviceIcon(deviceType) {
        const icons = {
            desktop: '<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="3" width="20" height="14" rx="2" ry="2"></rect><line x1="8" y1="21" x2="16" y2="21"></line><line x1="12" y1="17" x2="12" y2="21"></line></svg>',
            mobile: '<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="5" y="2" width="14" height="20" rx="2" ry="2"></rect><line x1="12" y1="18" x2="12" y2="18"></line></svg>',
            tablet: '<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="4" y="2" width="16" height="20" rx="2" ry="2"></rect><line x1="12" y1="18" x2="12" y2="18"></line></svg>'
        };
        return icons[deviceType] || icons.desktop;
    }

    function getSeverityIcon(severity) {
        const colors = {
            critical: '#dc2626',
            high: '#ea580c',
            medium: '#eab308',
            low: '#3b82f6',
            info: '#6b7280'
        };
        const color = colors[severity] || colors.info;
        return `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="${color}" stroke-width="2"><circle cx="12" cy="12" r="10"></circle><line x1="12" y1="8" x2="12" y2="12"></line><line x1="12" y1="16" x2="12" y2="16"></line></svg>`;
    }

    function getIconSVG(name) {
        const icons = {
            shield: '<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"></path></svg>',
            mail: '<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"></path><polyline points="22,6 12,13 2,6"></polyline></svg>',
            alert: '<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"></circle><line x1="12" y1="8" x2="12" y2="12"></line><line x1="12" y1="16" x2="12" y2="16"></line></svg>',
            'alert-triangle': '<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"></path><line x1="12" y1="9" x2="12" y2="13"></line><line x1="12" y1="17" x2="12" y2="17"></line></svg>'
        };
        return icons[name] || icons.alert;
    }

    function showSuccess(message) {
        // Implement toast notification
        console.log('Success:', message);
        if (window.toast) { window.toast.success(message); } else { window.toast ? window.toast.success(message) : alert(message); } // Replace with proper toast
    }

    function showError(message) {
        // Implement toast notification
        console.error('Error:', message);
        if (window.toast) { window.toast.success(message); } else { window.toast ? window.toast.success(message) : alert(message); } // Replace with proper toast
    }

    // Expose public API
    window.accountSecurity = {
        terminateSession,
        terminateAllSessions,
        refresh: () => {
            loadSecuritySummary();
            loadActiveSessions();
            loadSecurityEvents();
        }
    };
})();
