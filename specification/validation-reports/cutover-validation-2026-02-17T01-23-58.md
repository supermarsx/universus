# Rust Cutover Validation Report

Timestamp: 2026-02-17T01:24:09.6941406+00:00
Status: **PASS**

| Step | Status | Duration (s) |
| --- | --- | --- |
| workspace-check | PASS | 4.56 |
| web-frontend-routes | PASS | 1.62 |
| realtime-chat-moderation | PASS | 1.15 |
| api-notifications-load | PASS | 1.34 |
| api-sharding-churn | PASS | 1.23 |
| scheduler-key-dedupe | PASS | 0.95 |

## workspace-check

Command: `cargo check --workspace`

```text
warning: field `created_at_unix` is never read
  --> crates\app-api-gateway\src\state.rs:95:5
   |
80 | struct MarketplaceListingRecord {
   |        ------------------------ field in this struct
...
95 |     created_at_unix: i64,
   |     ^^^^^^^^^^^^^^^
   |
   = note: `MarketplaceListingRecord` has a derived impl for the trait `Clone`, but this is intentionally ignored during dead code analysis
   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `app-api-gateway` (lib) generated 1 warning
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

warning: `backend-core` (bin "backend-core") generated 6 warnings (run `cargo fix --bin "backend-core" -p backend-core` to apply 3 suggestions)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.87s
```

## web-frontend-routes

Command: `cargo test -p app-web-frontend all_template_routes_have_expected_auth_gating_and_render -- --nocapture`

```text
Finished `test` profile [unoptimized + debuginfo] target(s) in 0.37s
     Running unittests src\lib.rs (target\debug\deps\app_web_frontend-0ec900c58eec7f1c.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\main.rs (target\debug\deps\app_web_frontend-f06ba0bcc6473cdd.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\routes.rs (target\debug\deps\routes-19b7be77a961c540.exe)

running 1 test
test all_template_routes_have_expected_auth_gating_and_render ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 15 filtered out; finished in 0.46s
```

## realtime-chat-moderation

Command: `cargo test -p app-realtime-gateway chat_message_moderation_endpoints_update_state -- --nocapture`

```text
Finished `test` profile [unoptimized + debuginfo] target(s) in 0.47s
     Running unittests src\lib.rs (target\debug\deps\app_realtime_gateway-c3145b9142ac6141.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\main.rs (target\debug\deps\app_realtime_gateway-1420df895e16ae1a.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\realtime_gateway_api.rs (target\debug\deps\realtime_gateway_api-8ea1703694bda608.exe)

running 1 test
test chat_message_moderation_endpoints_update_state ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 15 filtered out; finished in 0.01s
```

## api-notifications-load

Command: `cargo test -p app-api-gateway notifications_high_volume_create_flow_stays_consistent -- --nocapture`

```text
warning: field `created_at_unix` is never read
  --> crates\app-api-gateway\src\state.rs:95:5
   |
80 | struct MarketplaceListingRecord {
   |        ------------------------ field in this struct
...
95 |     created_at_unix: i64,
   |     ^^^^^^^^^^^^^^^
   |
   = note: `MarketplaceListingRecord` has a derived impl for the trait `Clone`, but this is intentionally ignored during dead code analysis
   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `app-api-gateway` (lib) generated 1 warning
warning: `app-api-gateway` (lib test) generated 1 warning (1 duplicate)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.53s
     Running unittests src\lib.rs (target\debug\deps\app_api_gateway-cab6c354196dee31.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\main.rs (target\debug\deps\app_api_gateway-d6d4fb053ea5f52e.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\routes.rs (target\debug\deps\routes-ab414d3ac21781d0.exe)

running 1 test
test notifications_high_volume_create_flow_stays_consistent ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 53 filtered out; finished in 0.10s
```

## api-sharding-churn

Command: `cargo test -p app-api-gateway sharding_registration_churn_keeps_routing_stats_coherent -- --nocapture`

```text
warning: field `created_at_unix` is never read
  --> crates\app-api-gateway\src\state.rs:95:5
   |
80 | struct MarketplaceListingRecord {
   |        ------------------------ field in this struct
...
95 |     created_at_unix: i64,
   |     ^^^^^^^^^^^^^^^
   |
   = note: `MarketplaceListingRecord` has a derived impl for the trait `Clone`, but this is intentionally ignored during dead code analysis
   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `app-api-gateway` (lib) generated 1 warning
warning: `app-api-gateway` (lib test) generated 1 warning (1 duplicate)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.47s
     Running unittests src\lib.rs (target\debug\deps\app_api_gateway-cab6c354196dee31.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\main.rs (target\debug\deps\app_api_gateway-d6d4fb053ea5f52e.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\routes.rs (target\debug\deps\routes-ab414d3ac21781d0.exe)

running 1 test
test sharding_registration_churn_keeps_routing_stats_coherent ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 53 filtered out; finished in 0.06s
```

## scheduler-key-dedupe

Command: `cargo test -p app-scheduler-worker -- --nocapture`

```text
Finished `test` profile [unoptimized + debuginfo] target(s) in 0.41s
     Running unittests src\main.rs (target\debug\deps\app_scheduler_worker-d6eda701838ff356.exe)

running 2 tests
test tests::scheduler_task_key_changes_for_next_bucket ... ok
test tests::scheduler_task_key_is_stable_for_same_bucket ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

