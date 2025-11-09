/**
 * @module backend/middleware/adminAuth
 *
 * Admin authentication middleware utilities. Verifies admin status,
 * enforces admin-level permissions and provides helpers for admin role
 * and permission checks. Injects admin metadata into the request object.
 */

import { Response, NextFunction } from 'express';
import { pool } from '../config/database';
import { AdminAuthRequest, AdminLevel, ADMIN_PERMISSIONS } from '../types/admin';
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

    // Query admin user data
    const adminResult = await pool.query(
      `SELECT au.*, u.username, u.email 
       FROM admin_users au
       JOIN users u ON au.user_id = u.id
       WHERE au.user_id = $1 AND au.is_active = TRUE`,
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
    req.adminLevel = admin.admin_level as AdminLevel;
    req.adminPermissions = admin.permissions?.length > 0 
      ? admin.permissions 
      : ADMIN_PERMISSIONS[admin.admin_level as AdminLevel];

    next();
  } catch (error) {
    console.error('Admin authentication error:', error);
    res.status(500).json({ error: 'Internal server error' });
  }
};

/**
 * Require Specific Admin Level
 * Ensures admin has at least the specified level
 */
export const requireAdminLevel = (minLevel: AdminLevel) => {
  const levelHierarchy: Record<AdminLevel, number> = {
    support: 1,
    moderator: 2,
    game_admin: 3,
    super_admin: 4,
  };

  return async (
    req: AdminAuthRequest,
    res: Response,
    next: NextFunction
  ): Promise<void> => {
    if (!req.adminLevel) {
      res.status(403).json({ error: 'Admin authentication required' });
      return;
    }

    const userLevel = levelHierarchy[req.adminLevel];
    const requiredLevel = levelHierarchy[minLevel];

    if (userLevel < requiredLevel) {
      res.status(403).json({ 
        error: 'Insufficient admin privileges',
        required: minLevel,
        current: req.adminLevel,
      });
      return;
    }

    next();
  };
};

/**
 * Require Specific Permission
 * Checks if admin has a specific permission
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
 * Require Multiple Permissions (all required)
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
 * Require Any Permission (at least one required)
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
 * Log Admin Action Helper
 * Used throughout admin routes to audit all actions
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
 * Admin Audit Wrapper
 * Wraps route handlers to automatically log actions
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
 * Rate Limiting for Admin Actions
 * Prevents abuse of admin endpoints
 */
const actionCounts = new Map<string, { count: number; resetAt: number }>();

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
