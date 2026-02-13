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
| TODO ID | Node route surface | Node source of truth | Crate owner (primary) | Crate owner (supporting) | Concrete cutover checklist |
| --- | --- | --- | --- | --- | --- |
| R-001 | `/api/users` (`GET /me`, `GET /leaderboard`) | `backend/src/routes/users.ts` mounted in `backend/src/index.ts` | `crates/app-api-gateway` | `crates/platform-auth`, `crates/game-leaderboard`, `crates/game-economy`, `crates/platform-db` | [ ] Add `users` route module in `app-api-gateway` with contract-compatible JSON shape.<br>[ ] Implement auth-guarded user context read equivalent to Node `authenticateToken` + `AuthRequest` flow.<br>[ ] Port leaderboard query path (or repository call) and preserve sorting/limit behavior.<br>[ ] Add parity tests for `/api/users/me` and `/api/users/leaderboard` vs Node fixtures.<br>[ ] Switch gateway routing and remove Node `/api/users` mount from production traffic path. |
| R-002 | `/api/acs` (`GET /`, `POST /`, `POST /:id/join`, `DELETE /:id/leave`) | `backend/src/routes/acs.ts` mounted in `backend/src/index.ts` | `crates/app-api-gateway` | `crates/game-fleet`, `crates/game-alliance`, `crates/platform-auth`, `crates/platform-db` | [ ] Add `acs` route module in `app-api-gateway` with route/verb parity.<br>[ ] Port ACS validation rules and error envelope semantics (`success/message`).<br>[ ] Implement DB-backed ACS group CRUD + join/leave operations with transaction safety.<br>[ ] Add auth and permission parity tests for all ACS endpoints.<br>[ ] Cut traffic to Rust route and retire Node `/api/acs` mount. |

## Service-Level Checklist
| TODO ID | Node service/surface | Node source of truth | Crate owner (primary) | Crate owner (supporting) | Concrete cutover checklist |
| --- | --- | --- | --- | --- | --- |
| S-001 | Server-rendered template/page routing (`/`, `/overview`, `/admin/*`, `/account/*`, `/alliance/*`, etc.) currently served by Node templates router | `backend/src/routes/templates.ts` mounted at `/` in `backend/src/index.ts` | `TBD (new crate required)` | `TBD` | [ ] Decide ownership model: new Rust SSR crate vs static frontend hosting + edge/router rewrite.<br>[ ] If Rust-owned, create crate (recommended name: `crates/app-web-frontend`) and define canonical route map for all page endpoints now in `templates.ts`.<br>[ ] Preserve auth gate behavior (`authenticateToken`, `assertAuthenticated`, `requireAdmin`) or migrate auth gating to frontend + API-only backend policy explicitly.<br>[ ] Reproduce Nunjucks page delivery behavior (or approved replacement) and validate all current page URLs.<br>[ ] Remove Node template router from runtime after parity sign-off. |

## Cutover Exit Criteria For This File
- [ ] Every row above has an assigned crate owner (no `TBD`).
- [ ] Rust integration/contract tests exist for each route/service row.
- [ ] Node mounts for these rows are removed from production traffic path.
- [ ] `specification/spec-rust-route-ownership.md` is updated to mark these rows as represented.
