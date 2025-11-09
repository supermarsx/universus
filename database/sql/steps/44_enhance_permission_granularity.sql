-- ENHANCE PERMISSION GRANULARITY
-- ========================================

-- Add more granular permissions for game configuration
INSERT INTO permissions (name, description) VALUES
  ('game:config:read', 'Read game configuration'),
  ('game:config:write', 'Modify game configuration'),
  ('game:config:economy', 'Configure economy settings'),
  ('game:config:combat', 'Configure combat settings'),
  ('game:config:universe', 'Configure universe settings'),
  ('game:config:features', 'Configure feature flags'),
  ('shop:analytics', 'View shop analytics data'),
  ('shop:manage', 'Manage shop items and orders'),
  ('shop:config', 'Configure shop settings'),
  ('universe:manage', 'Manage universe configuration'),
  ('universe:generate_bots', 'Generate bot players'),
  ('bot:manage', 'Manage bot service'),
  ('bot:config', 'Configure bot behavior'),
  ('analytics:view', 'View analytics data'),
  ('analytics:export', 'Export analytics data'),
  ('config:read', 'Read configuration settings'),
  ('config:write', 'Modify configuration settings'),
  ('config:templates', 'Manage configuration templates')
ON CONFLICT (name) DO NOTHING;

-- Update role permissions with more granular permissions

-- Super Game Master - Add granular permissions
INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r, permissions p 
WHERE r.name = 'super_game_master' AND p.name IN (
  'game:config:read', 'game:config:write', 'game:config:economy', 
  'game:config:combat', 'game:config:universe', 'game:config:features',
  'shop:analytics', 'shop:manage', 'shop:config',
  'universe:manage', 'universe:generate_bots',
  'bot:manage', 'bot:config',
  'analytics:view', 'analytics:export',
  'config:read', 'config:write', 'config:templates'
)
ON CONFLICT DO NOTHING;

-- Game Master - Add granular permissions (read-only for some)
INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r, permissions p 
WHERE r.name = 'game_master' AND p.name IN (
  'game:config:read', 'game:config:economy', 'game:config:combat', 
  'game:config:universe', 'game:config:features',
  'shop:analytics', 'shop:manage',
  'universe:manage',
  'analytics:view',
  'config:read', 'config:templates'
)
ON CONFLICT DO NOTHING;

-- Super Mod - Add analytics permissions
INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r, permissions p 
WHERE r.name = 'super_mod' AND p.name IN (
  'analytics:view', 'analytics:export'
)
ON CONFLICT DO NOTHING;

-- Auditor - Add read permissions
INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r, permissions p 
WHERE r.name = 'auditor' AND p.name IN (
  'game:config:read', 'shop:analytics', 'analytics:view', 'config:read'
)
ON CONFLICT DO NOTHING;

-- Support - Add basic permissions
INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r, permissions p 
WHERE r.name = 'support' AND p.name IN (
  'analytics:view'
)
ON CONFLICT DO NOTHING;