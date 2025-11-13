use anyhow::Result;
use interprocess::local_socket::{LocalSocketStream};
use std::io::{BufRead, Write};
use std::time::{Duration, Instant};

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
