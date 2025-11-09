-- ========================================
-- SEED INITIAL ROLES AND PERMISSIONS
-- ========================================

-- 1. Insert permissions
INSERT INTO permissions (name, description) VALUES
  ('*', 'All permissions (superadmin)'),
  ('user:read', 'Read user data'),
  ('user:write', 'Modify user data'),
  ('user:ban', 'Ban users'),
  ('user:tag', 'Tag users'),
  ('game:config', 'Configure game settings'),
  ('game:events', 'Manage game events'),
  ('game:resources', 'Manage game resources'),
  ('monitoring:read', 'Read observability/monitoring data'),
  ('monitoring:write', 'Modify observability/monitoring config'),
  ('reports:read', 'Read reports'),
  ('reports:write', 'Write reports'),
  ('alliance:manage', 'Manage alliances'),
  ('fleet:manage', 'Manage fleets'),
  ('admin:manage', 'Manage admin users/roles'),
  ('content:moderate', 'Moderate content'),
  ('user:mute', 'Mute users'),
  ('user:warn', 'Warn users'),
  ('user:assist', 'Assist users'),
  ('tickets:manage', 'Manage support tickets'),
  ('audit:read', 'Read audit logs')
ON CONFLICT (name) DO NOTHING;

-- 2. Assign permissions to roles
-- Superadmin: all permissions
INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r, permissions p WHERE r.name = 'superadmin';

-- Super Game Master
INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r, permissions p WHERE r.name = 'super_game_master' AND p.name IN (
  'user:read','user:write','user:ban','user:tag','game:config','game:events','game:resources','monitoring:read','monitoring:write','reports:read','reports:write','alliance:manage','fleet:manage','admin:manage'
);

-- Game Master
INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r, permissions p WHERE r.name = 'game_master' AND p.name IN (
  'user:read','user:write','user:ban','user:tag','game:config','game:events','game:resources','monitoring:read','reports:read','alliance:manage','fleet:manage'
);

-- Super Mod
INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r, permissions p WHERE r.name = 'super_mod' AND p.name IN (
  'user:read','user:mute','user:warn','user:tag','content:moderate','reports:read','monitoring:read','reports:write'
);

-- Mod
INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r, permissions p WHERE r.name = 'mod' AND p.name IN (
  'user:read','user:mute','user:warn','user:tag','content:moderate','reports:read','monitoring:read'
);

-- Auditor
INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r, permissions p WHERE r.name = 'auditor' AND p.name IN (
  'monitoring:read','audit:read'
);

-- Support
INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r, permissions p WHERE r.name = 'support' AND p.name IN (
  'user:read','user:assist','reports:read','tickets:manage'
);

-- ========================================
-- END SEED
-- ========================================
