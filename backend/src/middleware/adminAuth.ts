/**
 * @module backend/middleware/adminAuth
 *
 * Admin authentication middleware utilities. Verifies admin status,
 * enforces admin-level permissions and provides helpers for admin role
 * and permission checks. Injects admin metadata into the request object.
 */

import { Response, NextFunction } from 'express';
import { pool } from '../config/database';
import { AdminAuthRequest } from '../types/admin';
/**
 * Ensure the current request is performed by an active admin user.
 *
 * - Verifies that the request contains an authenticated user.
 * - Loads admin metadata from `admin_users` and injects `req.admin`,
 *   `req.adminLevel` and `req.adminPermissions` for downstream handlers.
 * - Optionally enforces an IP whitelist defined on the admin record.
 *
 * On failure the middleware will send an appropriate HTTP response and
 * will not call `next()`.
 *
 * @param req - Express request augmented with typed admin fields
 * @param res - Express response used to send failure responses
 * @param next - Express next function to continue the middleware chain
 */
export const requireAdmin = async (
  req: AdminAuthRequest,
  res: Response,
  next: NextFunction
): Promise<void> => {
  try {
    if (!req.user) {
      res.status(401).json({ error: 'Authentication required' });
      return;
    }

    // Query admin user data, role, and permissions
    const adminResult = await pool.query(
      `SELECT 
         au.*, 
         u.username, 
         u.email, 
         r.id as role_id, 
         r.name as role_name, 
         ARRAY_REMOVE(ARRAY_AGG(p.name), NULL) as permissions
       FROM admin_users au
       JOIN users u ON au.user_id = u.id
       JOIN roles r ON au.role_id = r.id
       LEFT JOIN role_permissions rp ON r.id = rp.role_id
       LEFT JOIN permissions p ON rp.permission_id = p.id
       WHERE au.user_id = $1 AND au.is_active = TRUE
       GROUP BY au.id, u.username, u.email, r.id, r.name`,
      [req.user.id]
    );

    if (adminResult.rows.length === 0) {
      res.status(403).json({ error: 'Admin access required' });
      return;
    }

    const admin = adminResult.rows[0];

    // Check IP whitelist if configured
    if (admin.ip_whitelist && admin.ip_whitelist.length > 0) {
      const clientIp = req.ip || req.connection.remoteAddress || '';
      if (!admin.ip_whitelist.includes(clientIp)) {
        await logAdminAction(
          req.user.id,
          req.user.username,
          'access_denied_ip',
          'security',
          null,
          null,
          { attempted_ip: clientIp },
          'high',
          false
        );
        res.status(403).json({ error: 'IP address not whitelisted' });
        return;
      }
    }

    // Update last login
    await pool.query(
      'UPDATE admin_users SET last_login = NOW() WHERE id = $1',
      [admin.id]
    );

    // Inject admin data into request
    req.admin = admin;
    req.adminRole = admin.role_name;
    req.adminPermissions = Array.isArray(admin.permissions) ? admin.permissions : [];

    next();
  } catch (error) {
    console.error('Admin authentication error:', error);
    res.status(500).json({ error: 'Internal server error' });
  }
};

/**
 * Factory middleware to require a specific admin permission.
 *
 * Permission format is a colon-delimited string (e.g. 'user:read'). Admin
 * permission entries may include wildcards such as 'user:*' or '*' to grant
 * broader access. Super-admins (permissions include '*') bypass checks.
 *
 * @param permission - Permission string required to proceed
 * @returns Express middleware that enforces the permission
 */
export const requirePermission = (permission: string) => {
  return async (
    req: AdminAuthRequest,
    res: Response,
    next: NextFunction
  ): Promise<void> => {
    if (!req.adminPermissions) {
      res.status(403).json({ error: 'Admin authentication required' });
      return;
    }

    // Super admins have all permissions (*)
    if (req.adminPermissions.includes('*')) {
      next();
      return;
    }

    // Check for specific permission or wildcard match
    const hasPermission = req.adminPermissions.some((perm) => {
      if (perm === permission) return true;
      
      // Check wildcard permissions (e.g., 'user:*' matches 'user:read')
      if (perm.endsWith(':*')) {
        const prefix = perm.slice(0, -2);
        return permission.startsWith(prefix + ':');
      }
      
      return false;
    });

    if (!hasPermission) {
      res.status(403).json({ 
        error: 'Permission denied',
        required: permission,
        available: req.adminPermissions,
      });
      return;
    }

    next();
  };
};

/**
 * Require multiple permissions — all listed permissions must be present.
 *
 * Super-admins bypass permission enforcement. Wildcard permission entries
 * (e.g. 'user:*') match any permission with the same prefix.
 *
 * @param permissions - Array of permission strings required
 * @returns Express middleware enforcing that all permissions exist
 */
export const requirePermissions = (permissions: string[]) => {
  return async (
    req: AdminAuthRequest,
    res: Response,
    next: NextFunction
  ): Promise<void> => {
    if (!req.adminPermissions) {
      res.status(403).json({ error: 'Admin authentication required' });
      return;
    }

    // Super admins bypass permission checks
    if (req.adminPermissions.includes('*')) {
      next();
      return;
    }

    const missingPermissions = permissions.filter((perm) => {
      return !req.adminPermissions!.some((adminPerm) => {
        if (adminPerm === perm) return true;
        if (adminPerm.endsWith(':*')) {
          const prefix = adminPerm.slice(0, -2);
          return perm.startsWith(prefix + ':');
        }
        return false;
      });
    });

    if (missingPermissions.length > 0) {
      res.status(403).json({ 
        error: 'Insufficient permissions',
        missing: missingPermissions,
      });
      return;
    }

    next();
  };
};

/**
 * Require any of the provided permissions — succeeds if the admin has at
 * least one of them.
 *
 * Super-admins bypass the check.
 *
 * @param permissions - Array of permission strings (at least one required)
 * @returns Express middleware that requires at least one permission
 */
export const requireAnyPermission = (permissions: string[]) => {
  return async (
    req: AdminAuthRequest,
    res: Response,
    next: NextFunction
  ): Promise<void> => {
    if (!req.adminPermissions) {
      res.status(403).json({ error: 'Admin authentication required' });
      return;
    }

    // Super admins bypass permission checks
    if (req.adminPermissions.includes('*')) {
      next();
      return;
    }

    const hasAnyPermission = permissions.some((perm) =>
      req.adminPermissions!.some((adminPerm) => {
        if (adminPerm === perm) return true;
        if (adminPerm.endsWith(':*')) {
          const prefix = adminPerm.slice(0, -2);
          return perm.startsWith(prefix + ':');
        }
        return false;
      })
    );

    if (!hasAnyPermission) {
      res.status(403).json({ 
        error: 'Permission denied',
        required_any: permissions,
      });
      return;
    }

    next();
  };
};

/**
 * Persist an admin audit log row.
 *
 * Inserts a record into `admin_audit_logs` and returns the new row id.
 * This helper swallows errors and returns `-1` on failure to avoid
 * disrupting the caller's flow.
 *
 * @param adminId - Numeric id of the admin performing the action (optional)
 * @param adminUsername - Admin username (for readability in logs)
 * @param actionType - Short action identifier (e.g. 'user:ban')
 * @param actionCategory - High level category (e.g. 'security')
 * @param targetType - Optional target entity type (e.g. 'user')
 * @param targetId - Optional numeric id of the target entity
 * @param actionDetails - Optional free-form details/metadata (will be JSON-stringified)
 * @param severity - Severity level (e.g. 'low', 'medium', 'high')
 * @param success - Whether the action completed successfully
 * @param errorMessage - Optional error message if the action failed
 * @param beforeState - Optional snapshot of resource before the action
 * @param afterState - Optional snapshot of resource after the action
 * @returns Newly created audit log id, or -1 on failure
 */
export const logAdminAction = async (
  adminId: number | undefined,
  adminUsername: string,
  actionType: string,
  actionCategory: string,
  targetType: string | null = null,
  targetId: number | null = null,
  actionDetails: Record<string, any> | null = null,
  severity: string = 'medium',
  success: boolean = true,
  errorMessage: string | null = null,
  beforeState: Record<string, any> | null = null,
  afterState: Record<string, any> | null = null
): Promise<number> => {
  try {
    const result = await pool.query(
      `INSERT INTO admin_audit_logs (
        admin_id, admin_username, action_type, action_category,
        target_type, target_id, action_details, severity,
        success, error_message, before_state, after_state
      ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
      RETURNING id`,
      [
        adminId || null,
        adminUsername,
        actionType,
        actionCategory,
        targetType,
        targetId,
        actionDetails ? JSON.stringify(actionDetails) : null,
        severity,
        success,
        errorMessage,
        beforeState ? JSON.stringify(beforeState) : null,
        afterState ? JSON.stringify(afterState) : null,
      ]
    );

    return result.rows[0].id;
  } catch (error) {
    console.error('Error logging admin action:', error);
    return -1;
  }
};

/**
 * Higher-order wrapper for admin route handlers that automatically logs the
 * action after the handler completes (or fails). Useful to ensure audit
 * entries are always recorded with duration and basic request context.
 *
 * @param actionType - Short action identifier used in logs
 * @param actionCategory - High-level category for the action
 * @param getTarget - Optional function to extract a target { type, id } from the request
 * @returns A function that wraps an async route handler
 */
export const withAudit = (
  actionType: string,
  actionCategory: string,
  getTarget?: (req: AdminAuthRequest) => { type: string; id: number } | null
) => {
  return (handler: Function) => {
    return async (
      req: AdminAuthRequest,
      res: Response,
      next: NextFunction
    ): Promise<void> => {
      const startTime = Date.now();
      let success = true;
      let errorMessage: string | null = null;

      try {
        await handler(req, res, next);
      } catch (error: any) {
        success = false;
        errorMessage = error.message || 'Unknown error';
        throw error;
      } finally {
        // Log action after handler completes
        const target = getTarget ? getTarget(req) : null;
        const duration = Date.now() - startTime;

        await logAdminAction(
          req.user?.id,
          req.user?.username || 'unknown',
          actionType,
          actionCategory,
          target?.type || null,
          target?.id || null,
          {
            method: req.method,
            path: req.path,
            query: req.query,
            duration_ms: duration,
          },
          success ? 'low' : 'high',
          success,
          errorMessage
        );
      }
    };
  };
};

/**
 * Rate limiting helper for admin endpoints.
 *
 * This basic in-memory limiter tracks counts per `userId:path` key and is
 * intended for low-volume admin use. It is NOT suitable for distributed
 * deployments — use a Redis-backed limiter for production-scaled rate
 * limiting.
 */
const actionCounts = new Map<string, { count: number; resetAt: number }>();

/**
 * Create a middleware enforcing a maximum number of actions per window.
 *
 * @param maxActions - Maximum allowed actions in the sliding window
 * @param windowMs - Window duration in milliseconds
 * @returns Express middleware enforcing the rate limit
 */
export const rateLimit = (maxActions: number, windowMs: number) => {
  return (req: AdminAuthRequest, res: Response, next: NextFunction): void => {
    if (!req.user) {
      res.status(401).json({ error: 'Authentication required' });
      return;
    }

    const key = `${req.user.id}:${req.path}`;
    const now = Date.now();
    const record = actionCounts.get(key);

    if (!record || now > record.resetAt) {
      actionCounts.set(key, { count: 1, resetAt: now + windowMs });
      next();
      return;
    }

    if (record.count >= maxActions) {
      res.status(429).json({ 
        error: 'Rate limit exceeded',
        retryAfter: Math.ceil((record.resetAt - now) / 1000),
      });
      return;
    }

    record.count++;
    next();
  };
};

/**
 * Check if User is Blocked
 * Middleware to prevent blocked users from performing actions
 */
/**
 * Middleware to check whether the current user account is blocked.
 *
 * Calls the `is_user_blocked` DB helper and, when blocked, returns a 403
 * with block metadata. Otherwise calls `next()`.
 *
 * @param req - AdminAuthRequest (must include authenticated `req.user`)
 * @param res - Express response used to send block details on failure
 * @param next - Express next function to continue the middleware chain
 */
export const checkUserBlocked = async (
  req: AdminAuthRequest,
  res: Response,
  next: NextFunction
): Promise<void> => {
  if (!req.user) {
    res.status(401).json({ error: 'Authentication required' });
    return;
  }

  try {
    const blockResult = await pool.query(
      `SELECT * FROM is_user_blocked($1)`,
      [req.user.id]
    );

    const blockInfo = blockResult.rows[0];

    if (blockInfo.is_blocked) {
      res.status(403).json({
        error: 'Account blocked',
        block_type: blockInfo.block_type,
        reason: blockInfo.reason,
        end_time: blockInfo.end_time,
      });
      return;
    }

    next();
  } catch (error) {
    console.error('Error checking user block status:', error);
    res.status(500).json({ error: 'Internal server error' });
  }
};
