Status feature notes

- SQL migration: `database/sql/steps/46_status_page.sql` should be applied to the admin database (migrations follow the ordered `steps/` convention).
- Adds public endpoint: `GET /status` returning overall_status, incidents, maintenance, last_updated.
- Admin endpoints (require admin auth and permissions):
  - `GET /api/admin/status/incidents`
  - `POST /api/admin/status/incidents`
  - `PUT /api/admin/status/incidents/:id`
  - `GET /api/admin/status/maintenance`
  - `POST /api/admin/status/maintenance`

- Admins creating incidents or maintenance windows will have actions logged in `admin_audit_logs` via `logAdminAction`.
- Frontend should implement `/status` page to fetch `/status` for public view and use admin API routes for admin controls.
