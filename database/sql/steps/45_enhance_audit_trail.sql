-- ENHANCE AUDIT TRAIL FOR RBAC
-- ========================================

-- Add more detailed audit log categories and types
ALTER TABLE admin_audit_logs 
ADD COLUMN IF NOT EXISTS permission_changes JSONB,
ADD COLUMN IF NOT EXISTS old_values JSONB,
ADD COLUMN IF NOT EXISTS new_values JSONB,
ADD COLUMN IF NOT EXISTS admin_user_id INTEGER,
ADD COLUMN IF NOT EXISTS action TEXT,
ADD COLUMN IF NOT EXISTS resource_type TEXT,
ADD COLUMN IF NOT EXISTS resource_id INTEGER,
ADD COLUMN IF NOT EXISTS details JSONB,
ADD COLUMN IF NOT EXISTS created_at TIMESTAMPTZ;

-- RBAC originally used a second audit vocabulary. Normalize it into the
-- canonical Phase 2 columns while retaining the compatibility fields for
-- existing readers and deployments.
CREATE OR REPLACE FUNCTION normalize_admin_audit_log()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.admin_id IS NULL AND COALESCE(NEW.admin_user_id, 0) > 0 THEN
        SELECT COALESCE(au.user_id, u.id)
        INTO NEW.admin_id
        FROM users u
        LEFT JOIN admin_users au ON au.id = NEW.admin_user_id
        WHERE u.id = COALESCE(au.user_id, NEW.admin_user_id)
        LIMIT 1;
    END IF;

    IF NEW.admin_username IS NULL OR BTRIM(NEW.admin_username) = '' THEN
        SELECT username INTO NEW.admin_username
        FROM users WHERE id = NEW.admin_id;
        NEW.admin_username := COALESCE(NEW.admin_username, 'system');
    END IF;

    NEW.action_type := COALESCE(NEW.action_type, NEW.action, 'RBAC_CHANGE');
    NEW.action := COALESCE(NEW.action, NEW.action_type);
    NEW.action_category := COALESCE(NEW.action_category, 'security');
    NEW.target_type := COALESCE(NEW.target_type, NEW.resource_type);
    NEW.resource_type := COALESCE(NEW.resource_type, NEW.target_type);
    NEW.target_id := COALESCE(NEW.target_id, NEW.resource_id);
    NEW.resource_id := COALESCE(NEW.resource_id, NEW.target_id);
    NEW.action_details := COALESCE(NEW.action_details, NEW.details);
    NEW.details := COALESCE(NEW.details, NEW.action_details);
    NEW.timestamp := COALESCE(NEW.timestamp, NEW.created_at, NOW());
    NEW.created_at := COALESCE(NEW.created_at, NEW.timestamp);
    NEW.severity := COALESCE(NEW.severity, 'medium');
    NEW.success := COALESCE(NEW.success, TRUE);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trigger_normalize_admin_audit_log ON admin_audit_logs;
CREATE TRIGGER trigger_normalize_admin_audit_log
    BEFORE INSERT OR UPDATE ON admin_audit_logs
    FOR EACH ROW EXECUTE FUNCTION normalize_admin_audit_log();

-- Create indexes for better audit log querying
CREATE INDEX IF NOT EXISTS idx_admin_audit_logs_permission_changes ON admin_audit_logs USING GIN(permission_changes);
CREATE INDEX IF NOT EXISTS idx_admin_audit_logs_old_values ON admin_audit_logs USING GIN(old_values);
CREATE INDEX IF NOT EXISTS idx_admin_audit_logs_new_values ON admin_audit_logs USING GIN(new_values);

-- Add trigger to automatically log role permission changes
CREATE OR REPLACE FUNCTION log_role_permission_changes()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        INSERT INTO admin_audit_logs (
            admin_user_id, 
            action, 
            resource_type, 
            resource_id, 
            details, 
            permission_changes,
            new_values,
            ip_address,
            created_at
        ) VALUES (
            COALESCE(current_setting('app.current_admin_id', true)::INTEGER, 0),
            'ASSIGN_PERMISSION',
            'ROLE_PERMISSION',
            NEW.role_id,
            JSON_BUILD_OBJECT('permission_id', NEW.permission_id),
            JSON_BUILD_OBJECT('action', 'add', 'permission_id', NEW.permission_id),
            JSON_BUILD_OBJECT('role_id', NEW.role_id, 'permission_id', NEW.permission_id),
            COALESCE(current_setting('app.client_ip', true), 'unknown'),
            NOW()
        );
        RETURN NEW;
    ELSIF TG_OP = 'DELETE' THEN
        INSERT INTO admin_audit_logs (
            admin_user_id, 
            action, 
            resource_type, 
            resource_id, 
            details, 
            permission_changes,
            old_values,
            ip_address,
            created_at
        ) VALUES (
            COALESCE(current_setting('app.current_admin_id', true)::INTEGER, 0),
            'REMOVE_PERMISSION',
            'ROLE_PERMISSION',
            OLD.role_id,
            JSON_BUILD_OBJECT('permission_id', OLD.permission_id),
            JSON_BUILD_OBJECT('action', 'remove', 'permission_id', OLD.permission_id),
            JSON_BUILD_OBJECT('role_id', OLD.role_id, 'permission_id', OLD.permission_id),
            COALESCE(current_setting('app.client_ip', true), 'unknown'),
            NOW()
        );
        RETURN OLD;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- Create trigger for role_permissions changes
DROP TRIGGER IF EXISTS trigger_role_permissions_audit ON role_permissions;
CREATE TRIGGER trigger_role_permissions_audit
    AFTER INSERT OR DELETE ON role_permissions
    FOR EACH ROW EXECUTE FUNCTION log_role_permission_changes();

-- Add function to log role changes
CREATE OR REPLACE FUNCTION log_role_changes()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'UPDATE' THEN
        INSERT INTO admin_audit_logs (
            admin_user_id, 
            action, 
            resource_type, 
            resource_id, 
            details, 
            old_values,
            new_values,
            ip_address,
            created_at
        ) VALUES (
            COALESCE(current_setting('app.current_admin_id', true)::INTEGER, 0),
            'UPDATE',
            'ROLE',
            NEW.id,
            JSON_BUILD_OBJECT('role_name', NEW.name),
            JSON_BUILD_OBJECT('name', OLD.name, 'description', OLD.description),
            JSON_BUILD_OBJECT('name', NEW.name, 'description', NEW.description),
            COALESCE(current_setting('app.client_ip', true), 'unknown'),
            NOW()
        );
        RETURN NEW;
    ELSIF TG_OP = 'DELETE' THEN
        INSERT INTO admin_audit_logs (
            admin_user_id, 
            action, 
            resource_type, 
            resource_id, 
            details, 
            old_values,
            ip_address,
            created_at
        ) VALUES (
            COALESCE(current_setting('app.current_admin_id', true)::INTEGER, 0),
            'DELETE',
            'ROLE',
            OLD.id,
            JSON_BUILD_OBJECT('role_name', OLD.name),
            JSON_BUILD_OBJECT('name', OLD.name, 'description', OLD.description),
            COALESCE(current_setting('app.client_ip', true), 'unknown'),
            NOW()
        );
        RETURN OLD;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- Create trigger for role changes
DROP TRIGGER IF EXISTS trigger_roles_audit ON roles;
CREATE TRIGGER trigger_roles_audit
    AFTER UPDATE OR DELETE ON roles
    FOR EACH ROW EXECUTE FUNCTION log_role_changes();

-- Add function to log admin user role changes
CREATE OR REPLACE FUNCTION log_admin_user_role_changes()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'UPDATE' THEN
        INSERT INTO admin_audit_logs (
            admin_user_id, 
            action, 
            resource_type, 
            resource_id, 
            details, 
            old_values,
            new_values,
            ip_address,
            created_at
        ) VALUES (
            COALESCE(current_setting('app.current_admin_id', true)::INTEGER, 0),
            'CHANGE_ROLE',
            'ADMIN_USER',
            NEW.id,
            JSON_BUILD_OBJECT('user_id', NEW.user_id),
            JSON_BUILD_OBJECT('role_id', OLD.role_id),
            JSON_BUILD_OBJECT('role_id', NEW.role_id),
            COALESCE(current_setting('app.client_ip', true), 'unknown'),
            NOW()
        );
        RETURN NEW;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- Create trigger for admin user role changes
DROP TRIGGER IF EXISTS trigger_admin_users_role_audit ON admin_users;
CREATE TRIGGER trigger_admin_users_role_audit
    AFTER UPDATE OF role_id ON admin_users
    FOR EACH ROW EXECUTE FUNCTION log_admin_user_role_changes();
