// @ts-nocheck

interface UINotification {
  id: number;
  title: string;
  message: string;
  category?: string;
  type_name?: string;
  icon?: string;
  is_read?: boolean;
  created_at?: string;
  action_url?: string;
  action_label?: string;
}

class NotificationCenter {
  private dropdown: HTMLElement | null;
  private bell: HTMLElement | null;
  private badge: HTMLElement | null;
  private list: HTMLElement | null;
  private markAllBtn: HTMLElement | null;
  private notifications: UINotification[] = [];
  private unreadCount = 0;

  constructor() {
    this.dropdown = document.getElementById('notificationDropdown');
    this.bell = document.getElementById('notificationBell');
    this.badge = document.getElementById('notificationBadge');
    this.list = document.getElementById('notificationList');
    this.markAllBtn = document.getElementById('markAllReadBtn');

    if (!this.dropdown || !this.bell || !this.list) {
      return;
    }

    this.bindEvents();
    this.fetchNotifications();
  }

  bindEvents() {
    this.bell?.addEventListener('click', (e) => {
      e.stopPropagation();
      this.dropdown?.classList.toggle('show');
      if (this.dropdown?.classList.contains('show')) {
        this.fetchNotifications();
      }
    });

    document.addEventListener('click', () => {
      this.dropdown?.classList.remove('show');
    });

    this.dropdown?.addEventListener('click', (e) => e.stopPropagation());

    this.markAllBtn?.addEventListener('click', () => this.markAllRead());
  }

  async fetchNotifications() {
    try {
      const response = await fetch('/api/realtime/notifications?limit=10', {
        headers: this.authHeaders(),
      });
      if (!response.ok) throw new Error('Failed to load notifications');
      const data = await response.json();
      this.notifications = data.notifications || [];
      this.unreadCount = data.unreadCount || 0;
      this.updateBadge();
      this.renderList();
    } catch (error) {
      console.error('Notifications fetch failed:', error);
      if (this.list) {
        this.list.innerHTML = '<div class="notification-item">Unable to load notifications.</div>';
      }
    }
  }

  renderList() {
    if (!this.list) return;

    if (this.notifications.length === 0) {
      this.list.innerHTML = '<div class="notification-item">No notifications yet.</div>';
      return;
    }

    this.list.innerHTML = this.notifications
      .map(
        (notif) => `
        <div class="notification-item ${notif.is_read ? '' : 'unread'}" data-id="${notif.id}">
          <div class="notification-icon">
            <span>${this.iconFor(notif.category)}</span>
          </div>
          <div class="notification-content">
            <div class="notification-title">${notif.title}</div>
            <div class="notification-message">${notif.message}</div>
            <div class="notification-meta">${new Date(notif.created_at || Date.now()).toLocaleString()}</div>
            <div class="notification-actions">
              ${notif.action_url ? `<a href="${notif.action_url}" class="notification-link">${notif.action_label || 'View'}</a>` : ''}
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
      this.unreadCount = Math.max(0, this.unreadCount - 1);
      this.updateBadge();
      this.renderList();
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
      this.unreadCount = 0;
      this.updateBadge();
      this.renderList();
    } catch (error) {
      console.error('Failed to mark all notifications read:', error);
    }
  }

  handleRealtime(event: any) {
    this.unreadCount += 1;
    this.notifications = [
      {
        id: event.notificationId,
        title: event.title,
        message: event.message,
        category: event.category,
        icon: event.icon,
        is_read: false,
        created_at: event.timestamp,
        action_url: event.actionUrl,
        action_label: event.actionLabel,
      },
      ...this.notifications,
    ].slice(0, 10);

    this.updateBadge();
    this.renderList();
  }

  updateBadge() {
    if (!this.badge) return;
    if (this.unreadCount > 0) {
      this.badge.textContent = String(this.unreadCount);
      this.badge.classList.remove('hidden');
    } else {
      this.badge.classList.add('hidden');
    }
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
  window.notificationCenter = new NotificationCenter();
});
