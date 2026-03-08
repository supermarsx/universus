# Rust Cutover Validation Report

Timestamp: 2026-03-08T23:16:03.0518844+00:00
Status: **PASS**

| Step | Status | Duration (s) |
| --- | --- | --- |
| workspace-check | PASS | 19.41 |
| web-frontend-routes | PASS | 1.03 |
| realtime-chat-moderation | PASS | 19.42 |
| api-notifications-load | PASS | 6.52 |
| api-sharding-churn | PASS | 0.83 |
| scheduler-key-dedupe | PASS | 14.37 |

## workspace-check

Command: `cargo check --workspace`

```text
Blocking waiting for file lock on package cache
    Blocking waiting for file lock on package cache
    Blocking waiting for file lock on package cache
    Checking adapter-db v0.1.0 (C:\Projects\universus\crates\adapter-db)
    Checking backend-core v0.1.0 (C:\Projects\universus\crates\backend-core)
    Checking app-web-frontend v0.1.0 (C:\Projects\universus\crates\app-web-frontend)
warning: unused import: `interprocess::local_socket::LocalSocketStream`
  --> crates\backend-core\src\ipc_local.rs:27:5
   |
27 | use interprocess::local_socket::LocalSocketStream;
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

warning: variable does not need to be mutable
   --> crates\backend-core\src\main.rs:446:24
    |
446 |                     Ok(mut s) => {
    |                        ----^
    |                        |
    |                        help: remove this `mut`
    |
    = note: `#[warn(unused_mut)]` (part of `#[warn(unused)]`) on by default

warning: variable does not need to be mutable
   --> crates\backend-core\src\main.rs:321:13
    |
321 |         let mut worker = match self.manager.pick_worker_or_spawn(&universe).await {
    |             ----^^^^^^
    |             |
    |             help: remove this `mut`

warning: field `child` is never read
  --> crates\backend-core\src\main.rs:88:5
   |
85 | struct WorkerHandle {
   |        ------------ field in this struct
...
88 |     child: Arc<tokio::sync::Mutex<Option<std::process::Child>>>,
   |     ^^^^^
   |
   = note: `WorkerHandle` has a derived impl for the trait `Clone`, but this is intentionally ignored during dead code analysis
   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: field `spawn_backoff_secs` is never read
  --> crates\backend-core\src\main.rs:96:5
   |
93 | struct Manager {
   |        ------- field in this struct
...
96 |     spawn_backoff_secs: u64,
   |     ^^^^^^^^^^^^^^^^^^

warning: function `load_default_ships` is never used
  --> crates\backend-core\src\ships.rs:20:8
   |
20 | pub fn load_default_ships() -> HashMap<String, ShipDef> {
   |        ^^^^^^^^^^^^^^^^^^

    Checking platform-migrations v0.1.0 (C:\Projects\universus\crates\platform-migrations)
    Checking platform-adapter v0.1.0 (C:\Projects\universus\crates\platform-adapter)
warning: `backend-core` (bin "backend-core") generated 6 warnings (run `cargo fix --bin "backend-core" -p backend-core` to apply 3 suggestions)
    Checking app-admin-api v0.1.0 (C:\Projects\universus\crates\app-admin-api)
warning: function `app_router` is never used
   --> crates\app-admin-api\src\main.rs:247:4
    |
247 | fn app_router() -> Router {
    |    ^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `app-admin-api` (bin "app-admin-api") generated 1 warning
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 18.98s
```

## web-frontend-routes

Command: `cargo test -p app-web-frontend all_template_routes_have_expected_auth_gating_and_render -- --nocapture`

```text
Finished `test` profile [unoptimized + debuginfo] target(s) in 0.31s
     Running unittests src\lib.rs (target\debug\deps\app_web_frontend-9f011dd6428f34c1.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 40 filtered out; finished in 0.00s

     Running unittests src\main.rs (target\debug\deps\app_web_frontend-eeb434d85363d752.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\integration.rs (target\debug\deps\integration-29a046107f56df17.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s

     Running tests\routes.rs (target\debug\deps\routes-1e812f8f4b2fc598.exe)

running 1 test
test all_template_routes_have_expected_auth_gating_and_render ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 15 filtered out; finished in 0.26s
```

## realtime-chat-moderation

Command: `cargo test -p app-realtime-gateway chat_message_moderation_endpoints_update_state -- --nocapture`

```text
Compiling parking_lot_core v0.9.12
   Compiling serde v1.0.228
   Compiling tokio v1.48.0
   Compiling axum-core v0.3.4
   Compiling parking_lot v0.12.5
   Compiling serde_urlencoded v0.7.1
   Compiling tokio-util v0.7.18
   Compiling deadpool-runtime v0.1.4
   Compiling tower v0.4.13
   Compiling hyper v0.14.32
   Compiling deadpool v0.12.3
   Compiling tokio-postgres v0.7.16
   Compiling axum v0.6.20
   Compiling deadpool-postgres v0.14.1
   Compiling platform-db v0.1.0 (C:\Projects\universus\crates\platform-db)
   Compiling app-realtime-gateway v0.1.0 (C:\Projects\universus\crates\app-realtime-gateway)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 18.73s
     Running unittests src\lib.rs (target\debug\deps\app_realtime_gateway-c3145b9142ac6141.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\main.rs (target\debug\deps\app_realtime_gateway-1420df895e16ae1a.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\realtime_gateway_api.rs (target\debug\deps\realtime_gateway_api-8ea1703694bda608.exe)

running 1 test
test chat_message_moderation_endpoints_update_state ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 15 filtered out; finished in 0.00s
```

## api-notifications-load

Command: `cargo test -p app-api-gateway notifications_high_volume_create_flow_stays_consistent -- --nocapture`

```text
Compiling app-api-gateway v0.1.0 (C:\Projects\universus\crates\app-api-gateway)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 4.90s
     Running unittests src\lib.rs (target\debug\deps\app_api_gateway-efdeb621e769c4cf.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\main.rs (target\debug\deps\app_api_gateway-ced4ae939a44b3ef.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\integration.rs (target\debug\deps\integration-15e6799c0ffc57c3.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.00s

     Running tests\routes.rs (target\debug\deps\routes-85d40b3b90c474d3.exe)

running 1 test
test notifications_high_volume_create_flow_stays_consistent ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 56 filtered out; finished in 0.06s

     Running tests\simulated_flow.rs (target\debug\deps\simulated_flow-c9445951c199b4ab.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s
```

## api-sharding-churn

Command: `cargo test -p app-api-gateway sharding_registration_churn_keeps_routing_stats_coherent -- --nocapture`

```text
Finished `test` profile [unoptimized + debuginfo] target(s) in 0.39s
     Running unittests src\lib.rs (target\debug\deps\app_api_gateway-efdeb621e769c4cf.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\main.rs (target\debug\deps\app_api_gateway-ced4ae939a44b3ef.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\integration.rs (target\debug\deps\integration-15e6799c0ffc57c3.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.00s

     Running tests\routes.rs (target\debug\deps\routes-85d40b3b90c474d3.exe)

running 1 test
test sharding_registration_churn_keeps_routing_stats_coherent ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 56 filtered out; finished in 0.05s

     Running tests\simulated_flow.rs (target\debug\deps\simulated_flow-c9445951c199b4ab.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s
```

## scheduler-key-dedupe

Command: `cargo test -p app-scheduler-worker -- --nocapture`

```text
Compiling serde_core v1.0.228
   Compiling tracing v0.1.41
   Compiling tokio v1.48.0
   Compiling tracing-subscriber v0.3.20
   Compiling serde_json v1.0.145
   Compiling serde v1.0.228
   Compiling serde_urlencoded v0.7.1
   Compiling platform-consensus v0.1.0 (C:\Projects\universus\crates\platform-consensus)
   Compiling hyper v1.8.1
   Compiling tower v0.5.3
   Compiling tokio-util v0.7.18
   Compiling deadpool-runtime v0.1.4
   Compiling tokio-rustls v0.26.4
   Compiling deadpool v0.12.3
   Compiling platform-sharding v0.1.0 (C:\Projects\universus\crates\platform-sharding)
   Compiling tower-http v0.6.8
   Compiling platform-tenancy v0.1.0 (C:\Projects\universus\crates\platform-tenancy)
   Compiling postgres-types v0.2.12
   Compiling platform-observability v0.1.0 (C:\Projects\universus\crates\platform-observability)
   Compiling hyper-util v0.1.20
   Compiling tokio-postgres v0.7.16
   Compiling platform-tenant-routing v0.1.0 (C:\Projects\universus\crates\platform-tenant-routing)
   Compiling platform-worker-runtime v0.1.0 (C:\Projects\universus\crates\platform-worker-runtime)
   Compiling platform-scheduler v0.1.0 (C:\Projects\universus\crates\platform-scheduler)
   Compiling hyper-rustls v0.27.7
   Compiling reqwest v0.12.28
   Compiling deadpool-postgres v0.14.1
   Compiling platform-events v0.1.0 (C:\Projects\universus\crates\platform-events)
   Compiling platform-db v0.1.0 (C:\Projects\universus\crates\platform-db)
   Compiling app-scheduler-worker v0.1.0 (C:\Projects\universus\crates\app-scheduler-worker)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 13.98s
     Running unittests src\main.rs (target\debug\deps\app_scheduler_worker-e9359c77233ae2a5.exe)

running 2 tests
test tests::scheduler_task_key_is_stable_for_same_bucket ... ok
test tests::scheduler_task_key_changes_for_next_bucket ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

