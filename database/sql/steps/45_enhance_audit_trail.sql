-- ENHANCE AUDIT TRAIL FOR RBAC
-- ========================================

-- Add more detailed audit log categories and types
ALTER TABLE admin_audit_logs 
ADD COLUMN IF NOT EXISTS permission_changes JSONB,
ADD COLUMN IF NOT EXISTS old_values JSONB,
ADD COLUMN IF NOT EXISTS new_values JSONB;

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