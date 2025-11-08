import { Pool } from 'pg';
import { pool } from '../config/database';
import { AnalyticsEventPayload } from '../types/analytics';
import { analyticsQueue } from './analyticsQueue';

class AnalyticsService {
  private pool: Pool;

  constructor(db: Pool) {
    this.pool = db;
  }

  async trackEvent(input: AnalyticsEventPayload): Promise<void> {
    if (analyticsQueue.isEnabled()) {
      await analyticsQueue.publish(input);
      return;
    }
    await this.persistEvent(input);
  }

  async persistEvent(input: AnalyticsEventPayload): Promise<void> {
    await this.pool.query(
      `INSERT INTO analytics_events (user_id, session_id, event_type, event_properties, user_agent, ip_address)
       VALUES ($1, $2, $3, $4, $5, $6)`,
      [
        input.userId ?? null,
        input.sessionId || null,
        input.eventType,
        input.properties || {},
        input.userAgent || null,
        input.ipAddress || null
      ]
    );
  }

  async getUsageSummary(days: number = 7) {
    const summary = await this.pool.query(
      `SELECT event_type,
              COUNT(*) AS total_events,
              COUNT(DISTINCT COALESCE(user_id::text, session_id::text)) AS unique_sources
         FROM analytics_events
        WHERE created_at >= NOW() - ($1 || ' days')::interval
        GROUP BY event_type
        ORDER BY total_events DESC`,
      [days]
    );

    const daily = await this.pool.query(
      `SELECT DATE(created_at) AS event_date,
              event_type,
              COUNT(*) AS total_events
         FROM analytics_events
        WHERE created_at >= NOW() - ($1 || ' days')::interval
        GROUP BY event_date, event_type
        ORDER BY event_date DESC, event_type`,
      [days]
    );

    return {
      summary: summary.rows,
      daily: daily.rows
    };
  }
}

export const analyticsService = new AnalyticsService(pool);
