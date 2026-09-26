//! Minimal HTTP health probe over a raw TCP socket — no reqwest dependency.
//! PixivFlow exposes `GET /api/health` (returns 200 + `{"status":"ok"}`) and
//! `GET /health`, both of which are auth-exempt.

use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream, ToSocketAddrs};
use std::time::Duration;

/// Normalizes a display host (`localhost`) to `127.0.0.1` for probing while
/// keeping real IP/host values intact.
fn probe_host(host: &str) -> &str {
    if host == "localhost" {
        "127.0.0.1"
    } else {
        host
    }
}

/// Resolves `host:port` to a socket address so we can use `connect_timeout`.
fn resolve(host: &str, port: u16) -> Option<std::net::SocketAddr> {
    format!("{}:{}", host, port)
        .to_socket_addrs()
        .ok()?
        .next()
}

/// Returns true if the backend answered `HTTP/1.x 200` on `/api/health`.
pub fn health_ok(host: &str, port: u16, timeout: Duration) -> bool {
    let host = probe_host(host);
    let Some(addr) = resolve(host, port) else {
        return false;
    };
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, timeout) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));

    let req = format!(
        "GET /api/health HTTP/1.1\r\nHost: {}:{}\r\nConnection: close\r\nUser-Agent: pixivflow-desktop\r\n\r\n",
        host, port
    );
    if stream.write_all(req.as_bytes()).is_err() {
        return false;
    }
    let _ = stream.flush();

    let mut raw_head = Vec::new();
    let mut buf = [0u8; 512];
    // Read just the status line.
    while raw_head.len() < 2048 {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                raw_head.extend_from_slice(&buf[..n]);
                if raw_head.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    let _ = stream.shutdown(Shutdown::Both);

    let head = String::from_utf8_lossy(&raw_head);
    head.starts_with("HTTP/1.1 200") || head.starts_with("HTTP/1.0 200")
}

/// Polls health until OK or `deadline` elapses.
pub fn wait_healthy(host: &str, port: u16, total_timeout: Duration) -> bool {
    let start = std::time::Instant::now();
    let poll = Duration::from_millis(400);
    while start.elapsed() < total_timeout {
        if health_ok(host, port, Duration::from_secs(2)) {
            return true;
        }
        std::thread::sleep(poll);
    }
    health_ok(host, port, Duration::from_secs(2))
}