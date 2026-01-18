# TODO - Spec compliance for Universus

Status legend: [done] implemented and verified, [partial] present but needs audit/gaps, [missing] not implemented or not found.

## Main game spec (spec.md)

### Real-time multiplayer
- [partial] WebSocket stack present (`backend/src/socket/index.ts`, `backend/src/routes/realtimeRoutes.ts`), but event-level validation/authorization and payload schema validation need audit.
- [partial] Scheduling services exist (`backend/src/services/gameLoopService.ts`, `backend/src/services/fleetScheduler.ts`), but event persistence/recovery across restarts and multi-node reconciliation are not verified.
- [partial] Authoritative server anti-cheat rules and rate limiting exist (`backend/src/services/botProtectionService.ts`, `backend/src/services/authThrottleService.ts`), but coverage across all realtime actions is not verified.

### Planet and fleet management
- [partial] Planet/building/shipyard services exist (`backend/src/services/planetService.ts`, `backend/src/services/buildingService.ts`, `backend/src/services/shipyardService.ts`), but colonization flow, slot limits, and coordinate uniqueness need audit.
- [partial] Fleet missions service exists (`backend/src/services/fleetService.ts`) but jump-to-moon path is stubbed (`moveFleetToMoon` returns true) and mission coverage vs spec (recall rules, transport/harvest edge cases) needs audit + tests.
- [partial] Fleet dispatch and overview UI exist (`frontend/views/pages/fleet.njk`, `frontend/src/galaxy.ts`), but mission coverage and realtime updates need validation.

### Resource gathering and economy
- [partial] Resource production, lazy accrual, and storage caps need verification against spec; confirm last-updated timestamps and cap enforcement.
- [partial] Trade/marketplace endpoints exist (`backend/src/routes/realtimeRoutes.ts`) and accept flow now transfers resources, but planet selection rules and transactional tests need review.
- [missing] Premium currency purchase flow and payment gateway integration not found; rewarded ads and ad placement hooks not implemented.

### Building construction and technology trees
- [partial] Building and research scaffolding exists (`backend/src/services/buildingService.ts`, `backend/src/services/researchService.ts`) but tech tree/prerequisite config (JSON/DAG) not located; enforce single active research and prerequisites.
- [partial] Speedups (Robotics/Nanite/IRN) and their time calculations not verified.

### Combat simulation
- [partial] Combat engine present (`backend/src/services/combatService.ts`) but OGame rules (6 rounds, rapid fire, shield regen, 70% defense rebuild, debris/loot formulas) not verified; tests missing.
- [partial] Combat reports UI exists (`frontend/views/pages/fleet.njk`, `frontend/src/messages.ts`) but report payload/content from backend needs audit.

### Alliance system
- [partial] Alliance services exist (`backend/src/services/allianceService.ts`, `backend/src/services/allianceDiplomacyService.ts`, `backend/src/services/allianceWarService.ts`, `backend/src/services/acsService.ts`) but ACS coordination in combat flow, diplomacy rules, circulars, and alliance depot/refuel are not validated.
- [partial] Alliance UI exists (`frontend/views/pages/alliance-*.njk`) but member management dropdown is TODO (`frontend/src/alliance-dashboard.ts`).

### Player accounts and security
- [partial] Auth/account services exist (`backend/src/services/authService.ts`, `backend/src/services/accountSecurityService.ts`, `backend/src/services/twoFactorAuthService.ts`) but brute-force protection, CAPTCHA, and endpoint coverage need audit.
- [partial] Email/SMS verification and GDPR flows exist (`backend/src/services/emailVerificationService.ts`, `backend/src/services/smsVerificationService.ts`, `backend/src/services/gdprComplianceService.ts`) with export email notification wired; delivery verification pending.
- [partial] Account transfer exists (`backend/src/services/accountTransferService.ts`) with completion email wired; delivery verification pending.

### Messaging and notifications
- [partial] Messaging service/UI exists (`backend/src/services/messagingService.ts`, `frontend/src/messages.ts`) and chat PM send is wired; username lookup now resolves via `backend/src/routes/messages.ts` but UX audit needed.
- [partial] Notification service exists (`backend/src/services/notificationService.ts`), but email/offline notifications and alert channels not verified.

### In-game shop and monetization
- [partial] Shop/enhanced shop scaffolding exists (`backend/src/services/shopService.ts`, `backend/src/services/enhancedShopService.ts`) but payment gateway, anti-fraud, and ad integrations are not implemented.
- [partial] Gift purchase flow exists in UI (`frontend/views/pages/matrix-shop.njk`) with email notification wired; delivery verification pending.

### Leaderboards and rankings
- [partial] Leaderboard services exist (`backend/src/services/leaderboardService.ts`, `backend/src/services/globalLeaderboardService.ts`) but scoring formula and update triggers vs spec need audit; tests missing.

### Administration and moderation
- [partial] Admin UI exists (`frontend/views/pages/admin/*.njk`, `frontend/src/admin.ts`) but audit logging, impersonation, and moderation tooling need verification.
- [partial] Admin monitoring service exists (`backend/src/services/adminMonitoringService.ts`), but KPIs and alerting per spec not validated.

### Analytics, observability, and ops
- [partial] Analytics services exist (`backend/src/services/analyticsService.ts`, `backend/src/services/analyticsQueue.ts`) but pipeline destinations and dashboards are not documented.
- [partial] Observability service exists (`observability-service/`) but health checks, queue depth tracking, and alerting are not verified.

### Frontend requirements
- [partial] Galaxy view and fleet dispatch UI exist (`frontend/src/galaxy.ts`) but visual galaxy map, battle viewer, and animations from spec are not confirmed.
- [missing] Custom CSS sanitization/scoping rules not found; risk of UI abuse if user CSS is enabled.
- [partial] Responsive layout and cross-browser testing not verified.

### Deployment and scalability
- [partial] Docker compose exists (`docker-compose.yml`), but CI workflows are missing (`.github/workflows` empty).
- [missing] Load-testing scripts, CDN pipeline, multi-region deployment, and sticky-session load balancer configs not found.
- [partial] Redis adapter/cluster configuration for Socket.io and multi-node game servers not validated in code/docs.

## Moon mechanics spec (spec-moon-mechanics.md)
- [partial] Debris -> moon chance and diameter tunables exist (`backend/src/config/moonConfig.ts`) and are invoked in `fleetService`, but formula validation vs spec is pending.
- [partial] Moon entity and Lunar Base gating present (`backend/src/services/moonService.ts`, `backend/src/services/moonFieldService.ts`).
- [partial] Sensor Phalanx implemented (`backend/src/services/phalanxService.ts`) but missing spec items: time jitter, rate limits/daily caps, and filtering to hide moon legs (queries planet legs only by coords; moon-origin/dest missions are still enumerable).
- [partial] Jump Gate: cooldown and ownership checks exist (`backend/src/services/jumpGateService.ts`), but fleet movement is stubbed (`FleetService.moveFleetToMoon`), no resource strip/capacity checks, no per-gate cooldown on destination, and no clearing of fleet orders.
- [partial] Moon destruction: simplified formula + per-ship loss loop (`backend/src/services/destroyMoonService.ts`) diverge from spec math; no cleanup of queues/ships/defenses or debris/result logging.
- [partial] Moon visibility/galaxy map indicators and moon-only build roster enforcement not validated in UI/API.
- [partial] Phalanx counters (moon legs invisible, jitter, cooldown spam control) and RIP spam controls not enforced.
- [partial] API surface from spec (`/moons/{id}/scan`, `/jump`) lacks contract tests and response schemas; moon info is available via `GET /api/moons/id/:moonId` (spec path still not matched).

## i18n and a11y spec (spec-i18n-a11y.md)
- [done] Locale files exist (`frontend/locales/*.json`), i18next configured (`frontend/src/i18n.ts`), templates use `| t`, and language switcher UI present (`frontend/views/partials/nav.njk`).
- [partial] Language preference now persists via localStorage and updates i18next (`frontend/src/i18n.ts`, `frontend/views/partials/nav.njk`, `frontend/src/account/account-settings.ts`), but server-side persistence/cookies and remaining hardcoded labels need alignment.
- [partial] Locale-aware number/date helpers added across key screens; direct `toLocaleString`/`toLocaleDateString` usage replaced in UI render paths, with fallback calls retained inside helpers.
- [partial] Translation workflow for new locales and translator docs absent; admin locale editor (`frontend/views/pages/admin/locales.njk`) lacks backend/API hooks verification.
- [partial] Accessibility: jest-axe covers snippets only; no page-level tests for key flows, no focus-trap/keyboard tests for modals, contrast/focus indicators not validated.
- [missing] CI workflows are missing, so i18n/a11y tests are not enforced automatically.
- [missing] RTL support, screen-reader live regions, skip links, and accessible notifications (attack/build complete) not found.

## Pending TODOs in code
- None found in tracked source files.

## Validation / next steps
- Add contract + integration tests for moon APIs (phalanx, jump gate, destroy) and fleet mission paths involving moons; backfill coverage for combat rules and debris handling.
- Wire jump gate fleet moves, gate cooldown persistence, and resource stripping; align destruction formula + cleanup with spec.
- Enforce phalanx limits (range for moon legs, jitter, cooldowns) and hide moon-origin/destination legs from scans.
- Persist language preference (cookie/localStorage + account setting), audit templates for untranslated strings/ARIA labels, and add locale-aware formatting utilities.
- Expand jest-axe to rendered pages/components, add keyboard/focus regression tests, and introduce CI workflow to run i18n/a11y + unit suites.
- Add transactional integrity tests for trade acceptance resource transfer flow.
