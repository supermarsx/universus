### Removed Node-Era References
The files listed below were referenced historically but have been deleted from this workspace because the Rust backend is the only supported stack from now on:

- `ADMIN_SYSTEM_QUICK_START.md`
- `ASSET_INVENTORY.md`
- `BOT_SYSTEM_QUICK_REFERENCE.md`
- `COMPLETE_TESTING_DEPLOYMENT_GUIDE.md`
- `DEBRIS_SYSTEM_QUICK_REFERENCE.md`
- `DEPLOYMENT_AND_TESTING_GUIDE.md`
- `FINAL_DEPLOYMENT_GUIDE.md`
- `i18n_a11y_audit.md`
- `INTEGRATION_BACKEND_CORE.md`
- `load-testing.md`
- `OBSERVABILITY_API.md`
- `PHASE5`–`PHASE9` guides (deployment, sharding, quick references) and all related seasonal/implementation notes
- `PRODUCTION_DEPLOYMENT_GUIDE.md`
- `QUICKSTART_NEW_FEATURES.md`
- `STRIPE_INTEGRATION_GUIDE.md`
- `VERIFICATION_AND_TESTING_GUIDE.md`

# Legacy Node-Era Documentation (Do Not Edit)

These guides reference the original Node/Express backend, deployment playbooks, and late-stage phase documents. They remain in the tree purely for historical context; all operational guidance for the Rust backend is now under `docs/rust-backend-plan.md`, `docs/architecture.md`, and the platform-specific specs.

## Archive Index
| Document | Notes |
| --- | --- |
| `ADMIN_SYSTEM_QUICK_START.md` | Node admin service quick start (superseded by `app-admin-api`). |
| `BOT_SYSTEM_QUICK_REFERENCE.md` | Legacy bot reference now captured in `docs/rust-backend-plan.md` where the Rust workers are described. |
| `DEPLOYMENT*.md` (multiple) | Phase-based Node deployment guides. Archived because the Rust cutover plan now controls deployments. |
| `PHASE5_*` — `PHASE9_*` | Phase-specific sharding, implementation, and testing notes. Refer to `specification/spec-rust-backend.md` for current multi-tenant/sharding architecture. |
| `OBSERVABILITY_API.md`, `LOAD-TESTING.md`, `VERIFICATION_AND_TESTING_GUIDE.md` | Historical telemetry and load-testing guidance; new observability notes live in `docs/architecture.md`. |
| `COMPLETE_TESTING_DEPLOYMENT_GUIDE.md`, `FINAL_DEPLOYMENT_GUIDE.md`, `PRODUCTION_DEPLOYMENT_GUIDE.md` | Archived as the Rust deployment plan has a different topology and validation flow.
| `README.md`, `QUICKSTART*.md`, `PHASE*_.md` | Refer to the short-form "Rust backend plan" entry point (`docs/QUICK_START.md` and `docs/rust-backend-plan.md`), which link to the active docs.

## What's supported
- **Rust-first documentation**: `docs/rust-backend-plan.md` (crate partitioning/testing plan), `docs/architecture.md` (infra overview), and `specification/spec-rust-backend.md` (detailed runtime/spec references).
- **Validation reports**: `specification/validation-reports/` now captures benchmarks, migration validations, and consensus/sharding proofs.
- **TODO backlog**: `TODO.md` tracks the remaining integration steps; legacy docs are not part of the backlog beyond their archival status.

Reminder: Please do not edit the archived Node-era files directly. Instead, update the Rust docs listed above, and add new content to `docs/`, `specification/`, or `TODO.md` as appropriate.
