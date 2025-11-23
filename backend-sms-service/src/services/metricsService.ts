type ChannelCounter = Record<string, number>;

class MetricsService {
    private requestCount = 0;
    private successCount = 0;
    private failureCount = 0;
    private perChannelSuccess: ChannelCounter = {};
    private perChannelFailure: ChannelCounter = {};
    private responseTimes: number[] = [];

    recordRequest(): void {
        this.requestCount += 1;
    }

    recordSuccess(channel: string, durationMs: number): void {
        this.successCount += 1;
        this.perChannelSuccess[channel] = (this.perChannelSuccess[channel] || 0) + 1;
        this.responseTimes.push(durationMs);
        if (this.responseTimes.length > 1000) {
            this.responseTimes.shift();
        }
    }

    recordFailure(channel?: string): void {
        this.failureCount += 1;
        if (channel) {
            this.perChannelFailure[channel] = (this.perChannelFailure[channel] || 0) + 1;
        }
    }

    snapshot() {
        return {
            requests: this.requestCount,
            successes: this.successCount,
            failures: this.failureCount,
            perChannelSuccess: this.perChannelSuccess,
            perChannelFailure: this.perChannelFailure,
            avgResponseMs: this.responseTimes.length
                ? this.responseTimes.reduce((sum, value) => sum + value, 0) / this.responseTimes.length
                : 0
        };
    }
}

export const metricsService = new MetricsService();
