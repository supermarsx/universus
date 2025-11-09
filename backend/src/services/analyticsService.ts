/**
 * @module backend/services/analyticsService
 *
 * AnalyticsService accepts tracking events and persists or publishes them to
 * a processing queue depending on configuration. It also exposes reporting
 * helpers used by admin dashboards and exports.
 */
import { Pool } from 'pg';
import { pool } from '../config/database';
import { AnalyticsEventPayload } from '../types/analytics';
import { analyticsQueue } from './analyticsQueue';

class AnalyticsService {
  private pool: Pool;

  /**
   * Construct the service with a PG connection pool.
   *
   * @param db - PostgreSQL Pool used for inserts and analytics queries.
   */
  constructor(db: Pool) {
    this.pool = db;
  }

  /**
   * Track an analytics event. When an async analytics queue is enabled the
   * event will be published to the queue; otherwise it will be persisted
   * synchronously into the analytics_events table.
   *
   * @param input - AnalyticsEventPayload describing the event.
   */
  async trackEvent(input: AnalyticsEventPayload): Promise<void> {
    if (analyticsQueue.isEnabled()) {
      await analyticsQueue.publish(input);
      return;
    }
    await this.persistEvent(input);
  }

  /**
   * Persist an analytics event directly into the database.
   *
   * @param input - AnalyticsEventPayload to persist.
   */
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

  /**
   * Produce usage summary and daily breakdown for the given number of days.
   *
   * @param days - Number of days to include in the report (default 7).
   * @returns Object with `summary` and `daily` arrays describing event counts.
   */
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
