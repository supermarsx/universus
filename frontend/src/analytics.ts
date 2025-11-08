const ANALYTICS_ENDPOINT = '/api/analytics/events';
const SESSION_KEY = 'universus:session_id';

function getSessionId(): string | undefined {
    try {
        const stored = localStorage.getItem(SESSION_KEY);
        if (stored) return stored;
        const id =
            (window.crypto?.randomUUID && window.crypto.randomUUID()) ||
            `${Date.now()}-${Math.random().toString(16).slice(2)}`;
        localStorage.setItem(SESSION_KEY, id);
        return id;
    } catch {
        return undefined;
    }
}

function sendAnalyticsEvent(eventType: string, properties?: Record<string, any>) {
    const payload = {
        eventType,
        sessionId: getSessionId(),
        properties: {
            url: window.location.href,
            path: window.location.pathname,
            referrer: document.referrer || undefined,
            ...properties,
        },
    };

    try {
        if (navigator.sendBeacon) {
            const blob = new Blob([JSON.stringify(payload)], { type: 'application/json' });
            navigator.sendBeacon(ANALYTICS_ENDPOINT, blob);
        } else {
            fetch(ANALYTICS_ENDPOINT, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(payload),
                keepalive: true,
            }).catch(() => {
                /* swallow */
            });
        }
    } catch (error) {
        console.warn('Analytics send failed', error);
    }
}

document.addEventListener('DOMContentLoaded', () => {
    sendAnalyticsEvent('page_view', {
        title: document.title,
    });
});

(window as any).UniversusAnalytics = {
    track: sendAnalyticsEvent,
};
