/**
 * @module services/adminStatusService
 *
 * AdminStatusService
 * ------------------
 * Service responsible for creating, updating and querying status incidents
 * and maintenance windows. Designed to be used by the admin panel and by
 * public status endpoints. All actions performed by admins are logged using
 * the existing `logAdminAction` audit helper so they appear in the
 * `admin_audit_logs` table.
 *
 * This module provides lightweight, well-documented functions and data
 * structures intended to be small and robust for both automated and manual
 * workflows (alerts -> auto-incident creation, and manual admin updates).
 */

import { pool } from '../config/database';
import { logAdminAction } from '../middleware/adminAuth';

/**
 * Allowed severity levels for incidents.
 * @typedef {"low"|"medium"|"high"|"critical"} IncidentSeverity
 */
export type IncidentSeverity = 'low' | 'medium' | 'high' | 'critical';

/**
 * Incident record returned by the service.
 *
 * @interface Incident
 * @property {number} id - Unique incident identifier (DB sequence)
 * @property {string} title - Short human readable incident title
 * @property {(string|null)} description - Longer description / updates
 * @property {string} status - Lifecycle status (detected|investigating|identified|monitoring|resolved)
 * @property {IncidentSeverity} severity - Severity level
 * @property {string[]} affected_components - List of affected logical components
 * @property {string} start_time - ISO timestamp when incident started
 * @property {(string|null)} end_time - ISO timestamp when incident ended (or null)
 * @property {(number|null)} created_by - Admin user id who created the incident (null for auto)
 * @property {(string|null)} created_by_username - Admin username or null
 * @property {string} created_at - ISO timestamp when DB row was created
 * @property {string} updated_at - ISO timestamp when DB row was last updated
 */
export interface Incident {
  id: number;
  title: string;
  description: string | null;
  status: string;
  severity: IncidentSeverity;
  affected_components: string[];
  start_time: string;
  end_time: string | null;
  created_by: number | null;
  created_by_username: string | null;
  created_at: string;
  updated_at: string;
}

/**
 * AdminStatusService
 * Provides CRUD helpers for incidents and maintenance windows.
 */
export class AdminStatusService {
  /**
   * Create a new incident record in the database and log the admin action.
   *
   * If called with no admin information this will create an incident attributed
   * to the system (useful for automated incident creation from alerting).
   *
   * @async
   * @param {Partial<Incident>} payload - Partial incident fields (title required)
   * @param {number} [adminId] - Optional admin user id creating the incident
   * @param {string} [adminUsername] - Optional admin username creating the incident
   * @returns {Promise<Incident>} Newly created incident, converted to the Incident interface
   * @throws {Error} If database insertion fails or required fields are missing
   * @example
   * await AdminStatusService.createIncident({ title: 'Database latency', severity: 'high' }, 12, 'alice');
   * @see {@link logAdminAction} for audit log behavior
   */
  static async createIncident(
    payload: Partial<Incident>,
    adminId?: number,
    adminUsername?: string
  ): Promise<Incident> {
    const result = await pool.query(
      `INSERT INTO status_incidents (
        title, description, status, severity, affected_components,
        start_time, end_time, created_by, created_by_username
      ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9) RETURNING *`,
      [
        payload.title,
        payload.description || null,
        payload.status || 'detected',
        payload.severity || 'medium',
        payload.affected_components ? JSON.stringify(payload.affected_components) : JSON.stringify([]),
        payload.start_time || new Date().toISOString(),
        payload.end_time || null,
        adminId || null,
        adminUsername || null,
      ]
    );

    const incident = result.rows[0];

    // Audit log for admin action
    await logAdminAction(
      adminId ?? undefined,
      adminUsername || (adminId ? 'admin' : 'system') || 'system',
      'create_incident',
      'status',
      'incident',
      incident.id,
      payload,
      (incident.severity as IncidentSeverity) || 'medium'
    );

    return this.rowToIncident(incident);
  }

  /**
   * Update an existing incident and record an audit entry.
   * Only provided fields are updated; other fields are left unchanged.
   *
   * @async
   * @param {number} id - Incident id to update
   * @param {Partial<Incident>} updates - Fields to update
   * @param {number} [adminId] - Admin id performing the update
   * @param {string} [adminUsername] - Admin username performing the update
   * @returns {Promise<Incident>} The updated incident
   * @throws {Error} If the incident does not exist or the DB operation fails
   * @example
   * await AdminStatusService.updateIncident(42, { status: 'resolved', end_time: new Date().toISOString() }, 5, 'bob');
   * @see {@link logAdminAction} for audit logging of before/after state
   */
  static async updateIncident(
    id: number,
    updates: Partial<Incident>,
    adminId?: number,
    adminUsername?: string
  ): Promise<Incident> {
    // Fetch before state for audit logging
    const before = await pool.query('SELECT * FROM status_incidents WHERE id = $1', [id]);
    if (before.rows.length === 0) throw new Error('Incident not found');

    const result = await pool.query(
      `UPDATE status_incidents SET
         title = COALESCE($1, title),
         description = COALESCE($2, description),
         status = COALESCE($3, status),
         severity = COALESCE($4, severity),
         affected_components = COALESCE($5, affected_components),
         start_time = COALESCE($6, start_time),
         end_time = COALESCE($7, end_time),
         updated_at = NOW()
       WHERE id = $8 RETURNING *`,
      [
        updates.title || null,
        updates.description || null,
        updates.status || null,
        updates.severity || null,
        updates.affected_components ? JSON.stringify(updates.affected_components) : null,
        updates.start_time || null,
        updates.end_time || null,
        id,
      ]
    );

    const after = result.rows[0];

    // Audit log including before/after state for traceability
    await logAdminAction(
      adminId ?? undefined,
      adminUsername || 'admin',
      'update_incident',
      'status',
      'incident',
      id,
      updates,
      (after.severity as IncidentSeverity) || 'medium',
      true,
      null,
      before.rows[0],
      after
    );

    return this.rowToIncident(after);
  }

  /**
   * Get a list of incidents. This method currently returns the newest incidents
   * ordered by `start_time` descending up to the given limit. Optional `since`
   * parameter is reserved for future filtering but currently ignored in the
   * simplified implementation.
   *
   * @param {number} [limit=50] - Maximum number of incidents to return
   * @param {string} [since] - Optional ISO timestamp filter (future use)
   * @returns {Promise<Incident[]>} Array of incidents
   * @example
   * const recent = await AdminStatusService.getIncidents(25);
   */
  static async getIncidents(limit = 50, since?: string): Promise<Incident[]> {
    // Note: `since` parameter kept for API parity; current simple implementation ignores it.
    const res = await pool.query('SELECT * FROM status_incidents ORDER BY start_time DESC LIMIT $1', [limit]);
    return res.rows.map(this.rowToIncident);
  }

  /**
   * Create a maintenance window. Admin-created maintenance windows should be
   * visible on the public status page and are logged in the audit trail.
   *
   * @param {Object} payload - Maintenance window details
   * @param {string} payload.name - Short name for maintenance
   * @param {string} [payload.description] - Optional description
   * @param {string} payload.start_time - ISO start timestamp
   * @param {string} payload.end_time - ISO end timestamp
   * @param {number} [payload.created_by] - Admin id creating it
   * @param {string} [payload.created_by_username] - Admin username
   * @returns {Promise<any>} Inserted DB row for the maintenance window
   * @throws {Error} On DB insertion failure
   * @example
   * await AdminStatusService.createMaintenanceWindow({ name: 'Weekly Patch', start_time: '2025-11-14T02:00:00Z', end_time: '2025-11-14T04:00:00Z' }, 5, 'admin');
   */
  static async createMaintenanceWindow(
    payload: {
      name: string;
      description?: string;
      start_time: string;
      end_time: string;
      created_by?: number;
      created_by_username?: string;
    }
  ): Promise<any> {
    const result = await pool.query(
      `INSERT INTO status_maintenance_windows (
        name, description, start_time, end_time, created_by, created_by_username
      ) VALUES ($1,$2,$3,$4,$5,$6) RETURNING *`,
      [
        payload.name,
        payload.description || null,
        payload.start_time,
        payload.end_time,
        payload.created_by || null,
        payload.created_by_username || null,
      ]
    );

    const row = result.rows[0];

    await logAdminAction(
      payload.created_by ?? undefined,
      payload.created_by_username || 'admin',
      'create_maintenance',
      'status',
      'maintenance',
      row.id,
      payload,
      'low'
    );

    return row;
  }

  /**
   * Retrieve recent maintenance windows ordered by start time (desc).
   *
   * @param {number} [limit=50] - Maximum number of windows to return
   * @returns {Promise<any[]>} Array of DB rows representing maintenance windows
   * @example
   * const windows = await AdminStatusService.getMaintenanceWindows(10);
   */
  static async getMaintenanceWindows(limit = 50): Promise<any[]> {
    const res = await pool.query('SELECT * FROM status_maintenance_windows ORDER BY start_time DESC LIMIT $1', [limit]);
    return res.rows;
  }

  /**
   * Build a public-facing status snapshot used by the `/status` endpoint.
   * Logic: if any active non-resolved incidents or active maintenance windows
   * exist, overall status becomes `degraded`, otherwise `good`.
   *
   * @async
   * @returns {Promise<Object>} Public status snapshot containing `overall_status`, `incidents`, `maintenance`, and `last_updated`.
   * @example
   * const status = await AdminStatusService.getPublicStatus();
   */
  static async getPublicStatus(): Promise<any> {
    const incidentsRes = await pool.query(
      `SELECT * FROM status_incidents WHERE (end_time IS NULL OR end_time > NOW()) AND status != 'resolved' ORDER BY severity DESC, start_time DESC`
    );

    const maintenanceRes = await pool.query(
      `SELECT * FROM status_maintenance_windows WHERE end_time > NOW() ORDER BY start_time DESC`
    );

    const overall = incidentsRes.rows.length > 0 || maintenanceRes.rows.length > 0 ? 'degraded' : 'good';

    return {
      overall_status: overall,
      incidents: incidentsRes.rows.map(this.rowToIncident),
      maintenance: maintenanceRes.rows,
      last_updated: new Date().toISOString(),
    };
  }

  /**
   * Convert a DB row into the typed Incident interface.
   * This handles both JSONB and plain text fields and ensures consistent
   * JavaScript types for callers.
   *
   * @private
   * @param {any} row - Database row from `status_incidents`
   * @returns {Incident}
   */
  private static rowToIncident(row: any): Incident {
    return {
      id: row.id,
      title: row.title,
      description: row.description,
      status: row.status,
      severity: row.severity,
      affected_components: row.affected_components ? JSON.parse(row.affected_components) : [],
      start_time: row.start_time,
      end_time: row.end_time,
      created_by: row.created_by,
      created_by_username: row.created_by_username,
      created_at: row.created_at,
      updated_at: row.updated_at,
    };
  }
}
