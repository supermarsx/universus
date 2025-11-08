export interface AnalyticsEventPayload {
  eventType: string;
  userId?: number;
  sessionId?: string;
  properties?: Record<string, any>;
  userAgent?: string;
  ipAddress?: string | null;
}
