# TODO — Spec compliance for Universus

Status legend: ✅ done, ⚠️ partial/gap, ⏳ not implemented/found.

## Main game spec (browser MMO)
- ✅ Auth / accounts / JWT flows present (`backend/src/services/authService.ts`, README API examples).
- ✅ Planet + resource model with build queues (`backend/src/services/planetService.ts`, `buildingService.ts`; README lists planet/build endpoints).
- ✅ Research scaffolding and UI wiring (`backend/src/services/researchService.ts`, `frontend/js/research.js`).
- ✅ Real-time + Redis adapter wired (`backend/README.md`, Socket.IO config).
- ⚠️ Fleet missions: service exists (`backend/src/services/fleetService.ts`) but jump-to-moon path is stubbed (`moveFleetToMoon` returns true) and mission coverage vs spec (recall rules, ACS timing, transport/harvest edge cases) needs audit + tests.
- ⚠️ Combat engine present (`backend/src/services/combatService.ts`, used by `fleetService.processFleetArrival`) but unverified against spec (6 rounds, rapid-fire rules, defense rebuild, debris handling tests missing).
- ⚠️ Alliance system services present (`backend/src/services/alliance*.ts`) but UI/API parity with spec (ACS coordination, diplomacy states, circulars) not validated.
- ⚠️ Leaderboards exist (`globalLeaderboardService.ts`, `leaderboardScheduler.ts`) — confirm scoring formula matches spec and updates on all state changes.
- ⚠️ Shop/monetization scaffolding (`shopService.ts`, `enhancedShopService.ts`) — payment gateway, anti-fraud, ad placement, and premium currency flows not implemented in repo.
- ⚠️ Admin panel/UI shipped (`frontend/views/pages/admin/*.njk`) but moderation tooling, audit logs, and impersonation are minimal; align with spec checklist.
- ⚠️ Observability configs exist (`observability-service/`) — need health KPIs per spec (queue depths, event lag) and alerting verification.
- ⏳ Visual battle viewer, galaxy map enhancements, rewarded ads, notification channels, and deployment/load-test scripts from spec not found.

## Moon mechanics spec
- ✅ Debris → moon chance + diameter tunables match spec (`backend/src/config/moonConfig.ts`; invoked in `fleetService` after combat).
- ✅ Moon entity + fields and Lunar Base gating present (`moonService.ts`, `moonFieldService.ts`).
- ⚠️ Sensor Phalanx implemented (`phalanxService.ts`) but missing spec items: no time jitter, no rate limits/daily caps, and no filtering to hide moon legs (queries planet legs only by coords; moon-origin/dest missions are still enumerable).
- ⚠️ Jump Gate: cooldown + ownership checks exist (`jumpGateService.ts`), but fleet movement is stubbed (`FleetService.moveFleetToMoon`), no resource strip/capacity checks, no per-gate cooldown on destination, and no clearing of fleet orders.
- ⚠️ Moon destruction: simplified formula + per-ship loss loop (`destroyMoonService.ts`) diverge from spec math; no cleanup of queues/ships/defenses or debris/result logging.
- ⚠️ Moon visibility/galaxy map indicators and moon-only build roster enforcement not validated in UI/API.
- ⚠️ Phalanx counters (moon legs invisible, jitter, cooldown spam control) and RIP spam controls not enforced.
- ⏳ API surface from spec (`/moons/{id}/scan`, `/jump`) lacks contract tests and response schemas; no public moon info endpoint (`GET /moons/{id}`) found.

## i18n & a11y spec
- ✅ Locale files exist (`frontend/locales/*.json`), i18next configured with fallback (`frontend/js/i18n.js`), templates use `| t` (`frontend/views/...`), and language switcher UI present (`views/partials/nav.njk`).
- ✅ Basic a11y form + jest-axe smoke tests exist (`frontend/__tests__/a11y-*.test.ts`); WCAG target documented (`frontend/README.md`).
- ⚠️ Language preference is not persisted (nav switcher only reloads; no cookie/localStorage/user setting wiring) and some labels remain hardcoded (e.g., `aria-label="Select language"` in `nav.njk`).
- ⚠️ Date/number/currency formatting and pluralization helpers not implemented; audit frontend code for raw literals.
- ⚠️ Translation workflow for new locales and translator docs absent; admin locale editor (`views/pages/admin/locales.njk`) lacks backend/API hooks verification.
- ⚠️ Accessibility: jest-axe covers snippets only; no page-level tests for key flows, no focus-trap/keyboard tests for modals, contrast/focus indicators not validated.
- ⚠️ No CI workflows detected (`.github/workflows` missing), so i18n/a11y tests are not enforced automatically.
- ⏳ RTL support, screen-reader live regions, skip links, and accessible notifications (attack/build complete) not found.

## Validation / next steps
- Add contract + integration tests for moon APIs (phalanx, jump gate, destroy) and fleet mission paths involving moons; backfill coverage for combat rules and debris handling.
- Wire jump gate fleet moves, gate cooldown persistence, and resource stripping; align destruction formula + cleanup with spec.
- Enforce phalanx limits (range for moon legs, jitter, cooldowns) and hide moon-origin/destination legs from scans.
- Persist language preference (cookie/localStorage + account setting), audit templates for untranslated strings/ARIA labels, and add locale-aware formatting utilities.
- Expand jest-axe to rendered pages/components, add keyboard/focus regression tests, and introduce CI workflow to run i18n/a11y + unit suites.
