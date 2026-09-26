//! BackendManager — owns the local PixivFlow backend process (F1).
//!
//! Responsibilities (Rust owns the system capability):
//!   - start(): idempotent; refuses to spawn if already running or if the target
//!     port is taken; never assumes a fixed path (BackendCommand override).
//!   - stop(): asks the process to exit gracefully via SIGTERM (Unix) and waits
//!     for it — never `kill -9`.
//!   - restart(): stop then start.
//!   - health_check(): minimal `GET {port}/api/health` probe using std TcpStream
//!     (no HTTP client dependency).

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::process::{Child, Command};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::config::AppConfig;

const GRACEFUL_TIMEOUT: Duration = Duration::from_secs(6);
const HEALTH_TIMEOUT: Duration = Duration::from_millis(1500);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusSnapshot {
    /// one of: running | starting | stopped | error
    pub state: String,
    pub mode: String,
    pub port: u16,
    pub pid: Option<u32>,
    pub healthy: bool,
    pub message: Option<String>,
}

pub struct BackendManager {
    config: AppConfig,
    /// Adapter-injected command (from discovery). Preferred over config/mock.
    command_override: Option<(String, Vec<String>)>,
    child: Mutex<Option<Child>>,
    /// Last healthy probe result, kept in sync by the caller's poll loop.
    healthy: Mutex<Option<bool>>,
    last_error: Mutex<Option<String>>,
}

impl BackendManager {
    pub fn new(config: AppConfig) -> Self {
        Self {
            config,
            command_override: None,
            child: Mutex::new(None),
            healthy: Mutex::new(None),
            last_error: Mutex::new(None),
        }
    }

    pub fn is_running(&self) -> bool {
        match self.child.lock().unwrap().as_mut() {
            Some(c) => matches!(c.try_wait(), Ok(None)),
            None => false,
        }
    }

    pub fn pid(&self) -> Option<u32> {
        self.child.lock().unwrap().as_ref().map(|c| c.id())
    }

    pub fn port(&self) -> u16 {
        self.config.port()
    }

    pub fn mode(&self) -> &str {
        &self.config.mode
    }

    pub fn config_clone(&self) -> AppConfig {
        self.config.clone()
    }

    /// Inject the command chosen by the discovery adapter. `None` restores the
    /// default resolution (config `backend.command`, else the bundled mock).
    /// This is an adapter wiring point only — it does NOT change the lifecycle.
    pub fn set_command_override(&mut self, command: Option<(String, Vec<String>)>) {
        self.command_override = command;
    }

    pub fn command_override(&self) -> Option<(String, Vec<String>)> {
        self.command_override.clone()
    }

    pub fn report_err(&self, e: &str) {
        *self.last_error.lock().unwrap() = Some(e.to_string());
    }

    pub fn last_error(&self) -> Option<String> {
        self.last_error.lock().unwrap().clone()
    }

    pub fn set_healthy(&self, healthy: bool) {
        *self.healthy.lock().unwrap() = Some(healthy);
    }

    pub fn healthy(&self) -> Option<bool> {
        *self.healthy.lock().unwrap()
    }

    /// Spawn the backend if not already running.
    pub fn start(&mut self) -> Result<u32, String> {
        if self.is_running() {
            return Ok(self.pid().unwrap_or(0));
        }
        let port = self.config.port();
        if port_is_open(port) {
            let err = format!(
                "端口 {port} 已被占用，无法启动 backend（改 small 配置中的 port 或确认无旧进程）"
            );
            *self.last_error.lock().unwrap() = Some(err.clone());
            return Err(err);
        }
        let (cmd, args) = self.resolve_command()?;
        let mut c = Command::new(&cmd);
        c.args(&args)
            .env("PORT", port.to_string())
            .env("HOST", "127.0.0.1");
        let child = c.spawn().map_err(|e| {
            *self.last_error.lock().unwrap() = Some(format!("spawn backend 失败: {e}"));
            format!("spawn backend 失败: {e}")
        })?;
        let pid = child.id();
        *self.child.lock().unwrap() = Some(child);
        *self.healthy.lock().unwrap() = None;
        *self.last_error.lock().unwrap() = None;
        Ok(pid)
    }

    /// Graceful stop: SIGTERM (Unix) then wait for exit. Never SIGKILL.
    pub fn stop(&mut self) -> Result<(), String> {
        let mut guard = self.child.lock().unwrap();
        let Some(mut child) = guard.take() else {
            return Ok(());
        };
        let pid = child.id();
        #[cfg(unix)]
        {
            let rc = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
            if rc != 0 && port_is_open(self.config.port()) {
                // Process may have exited already; fall through to reap.
            }
        }
        #[cfg(windows)]
        {
            // Windows: TerminateProcess (there is no portable SIGTERM).
            let _ = child.kill();
        }

        let deadline = Instant::now() + GRACEFUL_TIMEOUT;
        loop {
            match child.try_wait() {
                Ok(Some(_)) => {
                    *self.healthy.lock().unwrap() = None;
                    return Ok(());
                }
                Ok(None) => {}
                Err(e) => {
                    *guard = Some(child);
                    return Err(format!("等待 backend 退出失败: {e}"));
                }
            }
            if Instant::now() >= deadline {
                *guard = Some(child);
                let msg = format!("backend(pid {pid}) 未在超时内优雅退出（SIGTERM 已发送）");
                *self.last_error.lock().unwrap() = Some(msg.clone());
                return Err(msg);
            }
            std::thread::sleep(Duration::from_millis(120));
        }
    }

    pub fn restart(&mut self) -> Result<u32, String> {
        let _ = self.stop(); // best-effort; start() re-checks the port
        self.start()
    }

    /// Probe `GET {port}/api/health`. Returns connection/protocol failures as Err,
    /// and `Ok(false)` when not running.
    pub fn health_check(&self) -> Result<bool, String> {
        if !self.is_running() {
            return Ok(false);
        }
        let status_line = http_get("127.0.0.1", self.config.port(), "/api/health")?;
        Ok(status_line.contains(" 200 "))
    }

    fn resolve_command(&self) -> Result<(String, Vec<String>), String> {
        if let Some((cmd, args)) = &self.command_override {
            if !cmd.is_empty() {
                return Ok((cmd.clone(), args.clone()));
            }
        }
        if !self.config.backend.command.trim().is_empty() {
            return Ok((self.config.backend.command.clone(), self.config.backend.args.clone()));
        }
        // Dev default: the bundled mock backend (health-probe stub). F2 replaces
        // this with the real PixivFlow via BackendCommand.
        let candidates = [
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("resources/mock-backend.mjs"),
            std::path::PathBuf::from("src-tauri/resources/mock-backend.mjs"),
        ];
        for c in &candidates {
            if c.exists() {
                return Ok(("node".into(), vec![c.display().to_string()]));
            }
        }
        Err("未配置 backend.command 且未找到默认 mock backend（F1 仅演示用）".into())
    }
}

fn port_is_open(port: u16) -> bool {
    TcpStream::connect(("127.0.0.1", port)).is_ok()
}

/// Minimal blocking HTTP GET returning the response status line
/// (e.g. "HTTP/1.1 200 OK"). No external HTTP client used.
fn http_get(host: &str, port: u16, path: &str) -> Result<String, String> {
    let addr = (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("resolve {host}:{port}: {e}"))?
        .next()
        .ok_or_else(|| format!("no addr for {host}:{port}"))?;
    let mut s = TcpStream::connect(addr).map_err(|e| format!("connect {host}:{port}: {e}"))?;
    s.set_read_timeout(Some(HEALTH_TIMEOUT)).ok();
    s.set_write_timeout(Some(HEALTH_TIMEOUT)).ok();
    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n"
    );
    s.write_all(req.as_bytes()).map_err(|e| format!("send: {e}"))?;
    let mut buf = String::new();
    s.read_to_string(&mut buf).map_err(|e| format!("recv: {e}"))?;
    Ok(buf.lines().next().unwrap_or("").to_string())
}