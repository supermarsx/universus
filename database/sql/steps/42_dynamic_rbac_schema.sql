-- ========================================
-- UNIVERSUS DYNAMIC RBAC SCHEMA MIGRATION
-- ========================================

-- 1. Create roles table
CREATE TABLE IF NOT EXISTS roles (
  id SERIAL PRIMARY KEY,
  name VARCHAR(50) UNIQUE NOT NULL,
  description TEXT,
  protected BOOLEAN DEFAULT FALSE
);

-- 2. Create permissions table
CREATE TABLE IF NOT EXISTS permissions (
  id SERIAL PRIMARY KEY,
  name VARCHAR(100) UNIQUE NOT NULL,
  description TEXT
);

-- 3. Create role_permissions table
CREATE TABLE IF NOT EXISTS role_permissions (
  role_id INTEGER REFERENCES roles(id) ON DELETE CASCADE,
  permission_id INTEGER REFERENCES permissions(id) ON DELETE CASCADE,
  PRIMARY KEY (role_id, permission_id)
);

-- 4. Add role_id to admin_users
ALTER TABLE admin_users ADD COLUMN IF NOT EXISTS role_id INTEGER REFERENCES roles(id);

-- 5. Migrate existing admin_level to roles
DO $$
DECLARE
  r RECORD;
  role_map JSONB := '{"super_admin": "superadmin", "game_admin": "game_master", "moderator": "mod", "support": "support"}';
  role_id INTEGER;
BEGIN
  -- Insert roles if not exist
  INSERT INTO roles (name, description, protected) VALUES
    ('superadmin', 'Superadmin (full access)', TRUE),
    ('super_game_master', 'Super Game Master', FALSE),
    ('game_master', 'Game Master', FALSE),
    ('super_mod', 'Super Moderator', FALSE),
    ('mod', 'Moderator', FALSE),
    ('auditor', 'Auditor (read-only)', FALSE),
    ('support', 'Support', FALSE)
  ON CONFLICT (name) DO NOTHING;

  -- For each admin_user, set role_id based on admin_level
  FOR r IN SELECT id, admin_level FROM admin_users LOOP
    SELECT id INTO role_id FROM roles WHERE name = role_map ->> r.admin_level;
    IF role_id IS NOT NULL THEN
      UPDATE admin_users SET role_id = role_id WHERE id = r.id;
    END IF;
  END LOOP;
END $$;

-- 6. Remove admin_level column (after migration)
ALTER TABLE admin_users DROP COLUMN IF EXISTS admin_level;

-- 7. Add index for role_id
CREATE INDEX IF NOT EXISTS idx_admin_users_role_id ON admin_users(role_id);

-- 8. Add unique constraint for permission name
CREATE UNIQUE INDEX IF NOT EXISTS idx_permissions_name ON permissions(name);

-- ========================================
-- END DYNAMIC RBAC SCHEMA MIGRATION
-- ========================================
