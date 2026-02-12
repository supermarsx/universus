/// Utilities for local (platform) IPC used by worker processes.
///
/// This module provides a small blocking helper to connect to a Unix/Windows
/// local socket created by the worker process and exchange a single-line JSON
/// request/response pair. The function is intentionally small and synchronous
/// so it can be used from both the async worker (in a blocking thread) and
/// from the manager when performing quick control operations.
///
/// Behaviour:
/// - Attempts to connect to `path` repeatedly until `timeout_ms` is
///   exceeded.
/// - When a connection is established, `payload` is written followed by a
///   newline and the function then reads a single newline-terminated response
///   line and returns it as `Ok(String)` (the returned string includes the
///   trailing newline if the peer wrote one).
///
/// Errors are returned using `anyhow::Error` for convenience; callers should
/// map or convert errors to their desired error type when necessary.
///
/// # Examples
/// ```no_run
/// use anyhow::Result;
/// // send a JSON-encoded request to a worker socket and wait up to 500ms
/// let resp: Result<String> = backend_core::ipc_local::call_sync("/tmp/universus_core_default_123.sock".to_string(), "{\"cmd\":\"prewarm\"}", 500);
/// ```
use anyhow::Result;
use interprocess::local_socket::LocalSocketStream;
use std::io::{BufRead, Write};
use std::time::{Duration, Instant};

/// Connect to a local socket and perform a single request/response exchange.
///
/// This is a small blocking helper used to communicate with worker
/// processes that accept single-line JSON requests and reply with a single
/// newline-terminated JSON response.
///
/// # Parameters
/// - `path`: filesystem path or socket name for the worker (platform
///   dependent). This should match the `SOCKET:` value printed by worker
///   processes on startup.
/// - `payload`: UTF-8 payload to send to the worker. The function appends a
///   newline before sending.
/// - `timeout_ms`: maximum time in milliseconds to attempt connecting before
///   returning an error. The function polls the connect call and sleeps 10ms
///   between retries; once the timeout is exceeded a `connect timeout` error
///   is returned.
///
/// # Returns
/// The single response line sent by the worker as a `String`. The returned
/// string contains whatever the worker wrote up to and including the
/// terminating newline (if present).
///
/// # Errors
/// Returns an `anyhow::Error` on connect failure (after timeout), on write
/// or read errors, or on other IO errors.
pub fn call_sync(path: String, payload: &str, timeout_ms: u64) -> Result<String> {
    let start = Instant::now();
    loop {
        match interprocess::local_socket::LocalSocketStream::connect(path.as_str()) {
            Ok(mut s) => {
                s.write_all(payload.as_bytes())?;
                s.write_all(b"\n")?;
                let mut rdr = std::io::BufReader::new(s);
                let mut line = String::new();
                rdr.read_line(&mut line)?;
                return Ok(line);
            }
            Err(e) => {
                if start.elapsed() > Duration::from_millis(timeout_ms) {
                    return Err(anyhow::anyhow!("connect timeout: {}", e));
                }
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
        }
    }
}
