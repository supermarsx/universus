# Rust Final Cutover Checklist (Unrepresented Node Surfaces)

Updated: 2026-02-13

## Audit Scope
This checklist covers Node routes/services that are still active in entrypoints but do not yet have explicit Rust crate representation.

Audit inputs:
- `backend/src/index.ts`
- `backend/src/routes/*.ts`
- `backend-admin-service/src/index.ts`
- `backend-bot-service/src/index.ts`
- `backend-sms-service/src/index.ts`
- `email-delivery-service/src/index.ts`
- `crates/*`

Excluded from this file:
- Node surfaces already represented by existing Rust crates (API/admin/bot/sms/realtime/email/analytics/core families).

## Route-Level Checklist
All previously unrepresented mounted route surfaces (`/api/users`, `/api/acs`) are now represented in `crates/app-api-gateway` with parity tests. Remaining route-level work is behavior-depth hardening (DB parity), tracked in `specification/spec-rust-route-ownership.md`.

## Service-Level Checklist
| TODO ID | Node service/surface | Node source of truth | Crate owner (primary) | Crate owner (supporting) | Concrete cutover checklist |
| --- | --- | --- | --- | --- | --- |
| S-001 | Server-rendered template/page routing (`/`, `/overview`, `/admin/*`, `/account/*`, `/alliance/*`, etc.) currently served by Node templates router | `backend/src/routes/templates.ts` mounted at `/` in `backend/src/index.ts` | `TBD (new crate required)` | `TBD` | [ ] Decide ownership model: new Rust SSR crate vs static frontend hosting + edge/router rewrite.<br>[ ] If Rust-owned, create crate (recommended name: `crates/app-web-frontend`) and define canonical route map for all page endpoints now in `templates.ts`.<br>[ ] Preserve auth gate behavior (`authenticateToken`, `assertAuthenticated`, `requireAdmin`) or migrate auth gating to frontend + API-only backend policy explicitly.<br>[ ] Reproduce Nunjucks page delivery behavior (or approved replacement) and validate all current page URLs.<br>[ ] Remove Node template router from runtime after parity sign-off. |

## Cutover Exit Criteria For This File
- [ ] Every row above has an assigned crate owner (no `TBD`).
- [ ] Rust integration/contract tests exist for each row.
- [ ] Node mounts for these rows are removed from production traffic path.
- [ ] `specification/spec-rust-route-ownership.md` reflects final ownership status.
