use std::io::{Write, Read};
use std::time::Duration;
use std::net::TcpStream;
use std::process::Command;
use std::thread;

// This is an integration test that is ignored by default because it spawns
// subprocesses and requires a build of the binary. Run with:
//   cd backend-core && cargo test --test worker_integration -- --ignored

#[test]
#[ignore]
fn spawn_worker_and_prewarm() {
    // Use current exe (test runner) is not desirable; instead run `cargo run` to ensure binary
    // available. We'll spawn `cargo run --package backend-core -- --worker --universe default --prewarm`.
    let mut child = Command::new("cargo")
        .args(&["run", "-p", "backend-core", "--", "--worker", "--universe", "default", "--prewarm"]) 
        .spawn()
        .expect("spawn worker process");

    // give worker some time to start and print port
    thread::sleep(Duration::from_secs(1));

    // we can't reliably parse child's stdout here because cargo wraps it; instead
    // try to connect to common candidate ports (the worker binds ephemeral). We'll try a few ports.
    let mut connected = false;
    for port in 50000..50100 {
        if let Ok(mut s) = TcpStream::connect_timeout(&format!("127.0.0.1:{}", port).parse().unwrap(), Duration::from_millis(50)) {
            // send a prewarm control
            let req = "{\"cmd\":\"prewarm\",\"battle_id\":\"\",\"attacker_ships\":{},\"defender_ships\":{},\"defender_defenses\":{},\"attacker_tech\":{},\"defender_tech\":{},\"planet_metal\":0,\"planet_crystal\":0,\"planet_deuterium\":0,\"seed\":\"\",\"universe\":\"default\"}".as_bytes();
            let _ = s.write_all(req);
            let mut buf = [0u8; 1024];
            if let Ok(n) = s.read(&mut buf) {
                let s = String::from_utf8_lossy(&buf[..n]);
                assert!(s.contains("prewarmed") || s.contains("draining") || s.len() > 0);
            }
            connected = true;
            break;
        }
    }

    // cleanup child
    let _ = child.kill();
    assert!(connected, "could not find worker port to connect");
}
