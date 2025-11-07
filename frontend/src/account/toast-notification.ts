// @ts-nocheck
/**
 * Toast Notification System
 * Professional, non-blocking notification system for user feedback
 */

class ToastNotification {
    constructor() {
        this.container = null;
        this.init();
    }

    /**
     * Initialize toast container
     */
    init() {
        // Create container if it doesn't exist
        if (!document.getElementById('toast-container')) {
            this.container = document.createElement('div');
            this.container.id = 'toast-container';
            this.container.className = 'toast-container';
            document.body.appendChild(this.container);
        } else {
            this.container = document.getElementById('toast-container');
        }
    }

    /**
     * Show toast notification
     */
    show(message, type = 'info', duration = 5000) {
        const toast = document.createElement('div');
        toast.className = `toast toast-${type} toast-enter`;

        const icon = this.getIcon(type);
        
        toast.innerHTML = `
            <div class="toast-icon">${icon}</div>
            <div class="toast-content">
                <div class="toast-message">${message}</div>
            </div>
            <button class="toast-close" aria-label="Close">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor">
                    <line x1="18" y1="6" x2="6" y2="18"></line>
                    <line x1="6" y1="6" x2="18" y2="18"></line>
                </svg>
            </button>
        `;

        this.container.appendChild(toast);

        // Close button handler
        const closeBtn = toast.querySelector('.toast-close');
        closeBtn.addEventListener('click', () => {
            this.remove(toast);
        });

        // Auto-remove after duration
        if (duration > 0) {
            setTimeout(() => {
                this.remove(toast);
            }, duration);
        }

        // Trigger animation
        setTimeout(() => {
            toast.classList.remove('toast-enter');
            toast.classList.add('toast-active');
        }, 10);

        return toast;
    }

    /**
     * Remove toast with animation
     */
    remove(toast) {
        toast.classList.remove('toast-active');
        toast.classList.add('toast-exit');
        
        setTimeout(() => {
            if (toast.parentNode) {
                toast.parentNode.removeChild(toast);
            }
        }, 300);
    }

    /**
     * Get icon for toast type
     */
    getIcon(type) {
        const icons = {
            success: `
                <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path>
                    <polyline points="22 4 12 14.01 9 11.01"></polyline>
                </svg>
            `,
            error: `
                <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <circle cx="12" cy="12" r="10"></circle>
                    <line x1="15" y1="9" x2="9" y2="15"></line>
                    <line x1="9" y1="9" x2="15" y2="15"></line>
                </svg>
            `,
            warning: `
                <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"></path>
                    <line x1="12" y1="9" x2="12" y2="13"></line>
                    <line x1="12" y1="17" x2="12.01" y2="17"></line>
                </svg>
            `,
            info: `
                <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <circle cx="12" cy="12" r="10"></circle>
                    <line x1="12" y1="16" x2="12" y2="12"></line>
                    <line x1="12" y1="8" x2="12.01" y2="8"></line>
                </svg>
            `
        };

        return icons[type] || icons.info;
    }

    /**
     * Convenience methods
     */
    success(message, duration = 5000) {
        return this.show(message, 'success', duration);
    }

    error(message, duration = 7000) {
        return this.show(message, 'error', duration);
    }

    warning(message, duration = 6000) {
        return this.show(message, 'warning', duration);
    }

    info(message, duration = 5000) {
        return this.show(message, 'info', duration);
    }

    /**
     * Clear all toasts
     */
    clearAll() {
        const toasts = this.container.querySelectorAll('.toast');
        toasts.forEach(toast => this.remove(toast));
    }
}

// Create global toast instance
window.toast = new ToastNotification();

// Add CSS styles
const style = document.createElement('style');
style.textContent = `
/* Toast Container */
.toast-container {
    position: fixed;
    top: 20px;
    right: 20px;
    z-index: 10000;
    display: flex;
    flex-direction: column;
    gap: 12px;
    max-width: 400px;
    pointer-events: none;
}

/* Toast Base */
.toast {
    background: var(--surface-color, #1a1a1a);
    border: 1px solid var(--border-color, #333);
    border-radius: 8px;
    padding: 16px;
    display: flex;
    align-items: flex-start;
    gap: 12px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
    pointer-events: auto;
    min-width: 300px;
    max-width: 400px;
}

/* Toast Animations */
.toast-enter {
    opacity: 0;
    transform: translateX(100%);
}

.toast-active {
    opacity: 1;
    transform: translateX(0);
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

.toast-exit {
    opacity: 0;
    transform: translateX(100%);
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

/* Toast Types */
.toast-success {
    border-left: 4px solid #22c55e;
}

.toast-success .toast-icon {
    color: #22c55e;
}

.toast-error {
    border-left: 4px solid #ef4444;
}

.toast-error .toast-icon {
    color: #ef4444;
}

.toast-warning {
    border-left: 4px solid #f59e0b;
}

.toast-warning .toast-icon {
    color: #f59e0b;
}

.toast-info {
    border-left: 4px solid #3b82f6;
}

.toast-info .toast-icon {
    color: #3b82f6;
}

/* Toast Icon */
.toast-icon {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
}

/* Toast Content */
.toast-content {
    flex: 1;
    min-width: 0;
}

.toast-message {
    color: var(--text-color, #fff);
    font-size: 14px;
    line-height: 1.5;
    word-wrap: break-word;
}

/* Toast Close Button */
.toast-close {
    flex-shrink: 0;
    background: none;
    border: none;
    padding: 4px;
    cursor: pointer;
    color: var(--text-muted, #888);
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
    transition: all 0.2s;
}

.toast-close:hover {
    background: var(--surface-hover, #2a2a2a);
    color: var(--text-color, #fff);
}

/* Mobile Responsive */
@media (max-width: 640px) {
    .toast-container {
        left: 12px;
        right: 12px;
        top: 12px;
        max-width: none;
    }

    .toast {
        min-width: 0;
        max-width: none;
    }
}
`;
document.head.appendChild(style);
