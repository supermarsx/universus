// =====================================================
// Phase 8: Seasonal Theme System - Scheduler Service
// =====================================================

import { ThemeService } from './themeService';

/**
 * ThemeScheduler
 * Automatically checks and activates scheduled themes
 */
export class ThemeScheduler {
    private intervalId: NodeJS.Timeout | null = null;
    private checkIntervalMs: number;
    private isRunning: boolean = false;

    constructor(checkIntervalMs: number = 60000) { // Default: 1 minute
        this.checkIntervalMs = checkIntervalMs;
    }

    /**
     * Start the scheduler
     */
    start(): void {
        if (this.isRunning) {
            console.log('[ThemeScheduler] Already running');
            return;
        }

        // Prevent automatic scheduling during tests or when SKIP_SERVER_START is set
        if (process.env.NODE_ENV === 'test' || process.env.SKIP_SERVER_START === 'true') {
            console.log('[ThemeScheduler] Start skipped (test mode or SKIP_SERVER_START)');
            return;
        }

        console.log(`[ThemeScheduler] Starting theme scheduler (interval: ${this.checkIntervalMs}ms)`);
        
        // Run immediately on start
        this.checkSchedules();

        // Then run on interval
        this.intervalId = setInterval(() => {
            this.checkSchedules();
        }, this.checkIntervalMs);

        this.isRunning = true;
    }

    /**
     * Stop the scheduler
     */
    stop(): void {
        if (this.intervalId) {
            clearInterval(this.intervalId);
            this.intervalId = null;
            this.isRunning = false;
            console.log('[ThemeScheduler] Theme scheduler stopped');
        }
    }

    /**
     * Check schedules and activate if needed
     */
    private async checkSchedules(): Promise<void> {
        try {
            const result = await ThemeService.checkScheduledThemes();

            if (result.activated && result.theme) {
                console.log(`[ThemeScheduler] Theme activated: ${result.theme.name} (${result.theme.theme_key})`);
            }
        } catch (error) {
            console.error('[ThemeScheduler] Error checking schedules:', error);
        }
    }

    /**
     * Manually trigger a check
     */
    async triggerCheck(): Promise<void> {
        console.log('[ThemeScheduler] Manual check triggered');
        await this.checkSchedules();
    }

    /**
     * Check if scheduler is running
     */
    isSchedulerRunning(): boolean {
        return this.isRunning;
    }

    /**
     * Update check interval
     */
    updateInterval(intervalMs: number): void {
        this.checkIntervalMs = intervalMs;
        
        if (this.isRunning) {
            this.stop();
            this.start();
            console.log(`[ThemeScheduler] Interval updated to ${intervalMs}ms`);
        }
    }
}

// Singleton instance
export const themeScheduler = new ThemeScheduler();
