// @ts-nocheck

class NotificationPage {
  constructor() {
    this.list = document.getElementById('notificationPageList');
    if (!this.list) return;

    this.categorySelect = document.getElementById('notificationCategoryFilter');
    this.unreadCheckbox = document.getElementById('notificationUnreadOnly');
    this.refreshBtn = document.getElementById('notificationsRefresh');
    this.markAllBtn = document.getElementById('notificationsMarkAll');
    this.notifications = [];

    this.bindEvents();
    this.fetchNotifications();
  }

  bindEvents() {
    this.categorySelect?.addEventListener('change', () => this.fetchNotifications());
    this.unreadCheckbox?.addEventListener('change', () => this.fetchNotifications());
    this.refreshBtn?.addEventListener('click', () => this.fetchNotifications());
    this.markAllBtn?.addEventListener('click', () => this.markAllRead());
  }

  async fetchNotifications() {
    try {
      const params = new URLSearchParams({
        limit: '100',
        unreadOnly: this.unreadCheckbox?.checked ? 'true' : 'false',
      });
      if (this.categorySelect && this.categorySelect.value !== 'all') {
        params.append('category', this.categorySelect.value);
      }

      const response = await fetch(`/api/realtime/notifications?${params.toString()}`, {
        headers: this.authHeaders(),
      });
      if (!response.ok) throw new Error('Failed to load notifications');
      const data = await response.json();
      this.notifications = data.notifications || [];
      this.renderList();
    } catch (error) {
      console.error('Notification page fetch failed:', error);
      this.list.innerHTML = '<div class="notification-item">Unable to load notifications.</div>';
    }
  }

  renderList() {
    if (!this.notifications.length) {
      this.list.innerHTML = '<div class="notification-item">No notifications found.</div>';
      return;
    }

    this.list.innerHTML = this.notifications
      .map(
        (notif) => `
        <div class="notification-item ${notif.is_read ? '' : 'unread'}" data-id="${notif.id}">
          <div class="notification-icon">${this.iconFor(notif.category)}</div>
          <div class="notification-content">
            <div class="notification-title">${notif.title}</div>
            <div class="notification-message">${notif.message}</div>
            <div class="notification-meta">${this.formatDateTime(notif.created_at)}</div>
            <div class="notification-actions">
              ${
                notif.action_url
                  ? `<a href="${notif.action_url}" class="notification-link">${notif.action_label || 'View'}</a>`
                  : ''
              }
              ${
                notif.is_read
                  ? ''
                  : `<button class="notification-action" data-mark-read="${notif.id}">Mark read</button>`
              }
            </div>
          </div>
        </div>`
      )
      .join('');

    this.list.querySelectorAll('[data-mark-read]').forEach((btn) => {
      btn.addEventListener('click', (e) => {
        e.preventDefault();
        const id = parseInt(btn.getAttribute('data-mark-read'));
        this.markRead(id);
      });
    });
  }

  async markRead(id: number) {
    try {
      await fetch(`/api/realtime/notifications/${id}/read`, {
        method: 'PUT',
        headers: this.authHeaders(),
      });
      this.notifications = this.notifications.map((n) => (n.id === id ? { ...n, is_read: true } : n));
      this.renderList();
      if (window.notificationCenter) {
        window.notificationCenter.fetchNotifications();
      }
    } catch (error) {
      console.error('Failed to mark notification read:', error);
    }
  }

  async markAllRead() {
    try {
      await fetch('/api/realtime/notifications/read/all', {
        method: 'PUT',
        headers: this.authHeaders(),
      });
      this.notifications = this.notifications.map((n) => ({ ...n, is_read: true }));
      this.renderList();
      if (window.notificationCenter) {
        window.notificationCenter.fetchNotifications();
      }
    } catch (error) {
      console.error('Failed to mark all read:', error);
    }
  }

  formatDateTime(value?: string) {
    const date = value ? new Date(value) : new Date();
    const locale = this.getLocale();
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

  getLocale() {
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

  iconFor(category?: string) {
    const icons: Record<string, string> = {
      fleet: '🚀',
      combat: '⚔️',
      resource: '🏗️',
      alliance: '🛡️',
      trade: '💱',
      system: '📢',
      achievement: '🏆',
    };
    return icons[category] || '🔔';
  }

  authHeaders() {
    const token = localStorage.getItem('token');
    const headers: Record<string, string> = { 'Content-Type': 'application/json' };
    if (token) headers['Authorization'] = `Bearer ${token}`;
    return headers;
  }
}

document.addEventListener('DOMContentLoaded', () => {
  if (document.getElementById('notificationPageList')) {
    new NotificationPage();
  }
});
