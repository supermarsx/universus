// @ts-nocheck

class AdminMonitoringDashboard {
    token: string | null = null;
    refreshHandle: number | null = null;

    async init() {
        this.token = localStorage.getItem('token');
        if (!this.token) {
            window.location.href = '/login.html';
            return;
        }

        this.bindControls();
        await this.loadMetrics();
        this.refreshHandle = window.setInterval(() => this.loadMetrics(), 15000);
    }

    bindControls() {
        document.getElementById('refreshScalingMetrics')?.addEventListener('click', () => this.loadMetrics());
        document.getElementById('triggerLeaderboardRebuild')?.addEventListener('click', () => this.triggerLeaderboardRebuild());
        document.getElementById('backToAdminDash')?.addEventListener('click', () => {
            window.location.href = '/admin/dashboard';
        });
    }

    async loadMetrics() {
        try {
            const response = await fetch('/api/admin/monitoring/scaling', {
                headers: {
                    Authorization: `Bearer ${this.token}`,
                },
            });

            if (!response.ok) {
                throw new Error('Failed to load scaling metrics');
            }

            const data = await response.json();
            this.renderMetrics(data);
        } catch (error: any) {
            console.error('Scaling metrics error:', error);
            this.showToast(error.message || 'Unable to load metrics', 'error');
        }
    }

    renderMetrics(data) {
        if (data.process) {
            const uptimeEl = document.getElementById('processUptime');
            if (uptimeEl) {
                uptimeEl.textContent = this.formatDuration(data.process.uptimeSeconds || 0);
            }

            const loadAvg = data.process.loadAverage || [];
            document.getElementById('loadOne')!.textContent = (loadAvg[0] || 0).toFixed(2);
            document.getElementById('loadFive')!.textContent = (loadAvg[1] || 0).toFixed(2);
            document.getElementById('loadFifteen')!.textContent = (loadAvg[2] || 0).toFixed(2);

            document.getElementById('memoryRss')!.textContent = this.formatBytes(data.process.memory?.rss || 0);
            document.getElementById('memoryHeapUsed')!.textContent = this.formatBytes(data.process.memory?.heapUsed || 0);
        }

        if (data.sockets) {
            document.getElementById('socketClients')!.textContent = this.formatNumber(data.sockets.connectedClients || 0);
            document.getElementById('socketRooms')!.textContent = this.formatNumber(data.sockets.rooms || 0);
            document.getElementById('socketNamespaces')!.textContent = this.formatNumber(data.sockets.namespaces || 0);
            document.getElementById('socketAdapter')!.textContent = data.sockets.adapterName || 'n/a';
        }

        if (data.redis) {
            document.getElementById('redisStatus')!.textContent = data.redis.status || 'unknown';
            document.getElementById('redisLatency')!.textContent = data.redis.latencyMs != null
                ? `${data.redis.latencyMs} ms`
                : 'n/a';
        }

        if (data.leaderboard) {
            const badge = document.getElementById('leaderboardStatus');
            if (badge) {
                badge.textContent = data.leaderboard.running ? 'RUNNING' : 'PAUSED';
                badge.className = `status-badge ${data.leaderboard.running ? 'status-active' : 'status-warning'}`;
            }

            const lastRunEl = document.getElementById('leaderboardLastRun');
            if (lastRunEl) {
                lastRunEl.textContent = data.leaderboard.lastRun
                    ? this.formatDateTime(data.leaderboard.lastRun)
                    : 'Never';
            }

            const intervalEl = document.getElementById('leaderboardInterval');
            if (intervalEl) {
                intervalEl.textContent = data.leaderboard.intervalMs
                    ? this.formatDuration(data.leaderboard.intervalMs / 1000)
                    : 'Not scheduled';
            }
        }
    }

    async triggerLeaderboardRebuild() {
        try {
            const response = await fetch('/api/admin/monitoring/leaderboard/rebuild', {
                method: 'POST',
                headers: {
                    Authorization: `Bearer ${this.token}`,
                },
            });

            const payload = await response.json();
            if (!response.ok) {
                throw new Error(payload.error || 'Failed to trigger rebuild');
            }

            this.showToast(payload.message || 'Leaderboard rebuild triggered', 'success');
            await this.loadMetrics();
        } catch (error: any) {
            console.error('Leaderboard rebuild error:', error);
            this.showToast(error.message || 'Unable to rebuild leaderboard', 'error');
        }
    }

    formatDuration(seconds: number) {
        const hours = Math.floor(seconds / 3600);
        const minutes = Math.floor((seconds % 3600) / 60);
        const secs = Math.floor(seconds % 60);

        if (hours > 0) {
            return `${hours}h ${minutes}m`;
        }

        if (minutes > 0) {
            return `${minutes}m ${secs}s`;
        }

        return `${secs}s`;
    }

    formatBytes(bytes: number) {
        if (!bytes) return '0 B';
        const units = ['B', 'KB', 'MB', 'GB'];
        const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
        const value = bytes / Math.pow(1024, index);
        return `${value.toFixed(1)} ${units[index]}`;
    }

    formatNumber(value: number) {
        const locale = this.getLocale();
        if (typeof Intl !== 'undefined' && Intl.NumberFormat) {
            return new Intl.NumberFormat(locale).format(value || 0);
        }
        return (value || 0).toLocaleString();
    }

    formatDateTime(value: string) {
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

    showToast(message: string, type: 'success' | 'error' | 'info' = 'info') {
        const container = document.getElementById('monitoringToast');
        if (!container) return;

        container.textContent = message;
        container.className = `monitoring-toast ${type} show`;

        setTimeout(() => {
            container.classList.remove('show');
        }, 3200);
    }
}

document.addEventListener('DOMContentLoaded', () => {
    const dashboard = new AdminMonitoringDashboard();
    dashboard.init();
});
