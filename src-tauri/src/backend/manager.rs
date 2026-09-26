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

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::process::{Child, Command};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::backend::discovery;
use crate::backend::proxy::{self, ProxyInjection};
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

/// An adapter-injected launch configuration (from discovery).
///
/// Additive only: carries the argv plus any extra env (e.g. `STATIC_PATH` for
/// 方案 A WebUI hosting). The core lifecycle (spawn / stop / restart / health)
/// is unchanged — this is purely how the command is wired in.
#[derive(Debug, Clone, Default)]
pub struct LaunchSpec {
    /// Full argv (`argv[0]` = executable).
    pub command: Vec<String>,
    /// Extra environment variables to inject at spawn.
    pub env: BTreeMap<String, String>,
    /// Working directory for the spawned process. None inherits the app's CWD.
    pub cwd: Option<std::path::PathBuf>,
}

pub struct BackendManager {
    config: AppConfig,
    /// Adapter-injected launch spec (from discovery). Preferred over config/mock.
    command_override: Option<LaunchSpec>,
    child: Mutex<Option<Child>>,
    /// Pid of a backend we did NOT spawn but adopted (orphan of a previous run).
    /// We hold no `Child` handle for it, so stop() signals it by pid instead.
    adopted_pid: Mutex<Option<u32>>,
    /// Last healthy probe result, kept in sync by the caller's poll loop.
    healthy: Mutex<Option<bool>>,
    last_error: Mutex<Option<String>>,
    /// What the last spawn did about the system proxy (for the caller's log).
    last_proxy: Mutex<ProxyInjection>,
}

impl BackendManager {
    pub fn new(config: AppConfig) -> Self {
        Self {
            config,
            command_override: None,
            child: Mutex::new(None),
            adopted_pid: Mutex::new(None),
            healthy: Mutex::new(None),
            last_error: Mutex::new(None),
            last_proxy: Mutex::new(ProxyInjection::Unavailable),
        }
    }

    pub fn is_running(&self) -> bool {
        if let Some(c) = self.child.lock().unwrap().as_mut() {
            if matches!(c.try_wait(), Ok(None)) {
                return true;
            }
        }
        // An adopted backend is alive as long as it still holds the port.
        self.adopted_pid.lock().unwrap().is_some() && port_is_open(self.config.port())
    }

    pub fn pid(&self) -> Option<u32> {
        if let Some(pid) = self.child.lock().unwrap().as_ref().map(|c| c.id()) {
            return Some(pid);
        }
        *self.adopted_pid.lock().unwrap()
    }

    /// True when the running backend was adopted rather than spawned by us.
    pub fn is_adopted(&self) -> bool {
        self.adopted_pid.lock().unwrap().is_some()
    }

    /// Take over a healthy backend that is already listening on our port.
    ///
    /// A force-quit or a crash of the desktop app leaves the backend child
    /// behind (it is reparented, not killed). Without this, the next launch
    /// fails with "端口 N 已被占用" even though it is *our own* runtime sitting
    /// there healthy. We adopt it instead of spawning a second one.
    ///
    /// Only a listener whose command line mentions `pixivflow` is adopted, so an
    /// unrelated service that happens to hold the port still reports an error.
    pub fn adopt_existing_backend(&mut self) -> Option<u32> {
        if self.pid().is_some() {
            return self.pid();
        }
        let port = self.config.port();
        if !port_is_open(port) {
            return None;
        }
        let (status, _) = http_status(port, "/api/health")?;
        if status != 200 {
            return None;
        }
        let pid = listener_pid(port)?;
        let cmd = process_command(pid);
        if !cmd.to_lowercase().contains("pixivflow") {
            return None;
        }
        *self.adopted_pid.lock().unwrap() = Some(pid);
        *self.healthy.lock().unwrap() = Some(true);
        *self.last_error.lock().unwrap() = None;
        Some(pid)
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
    pub fn set_command_override(&mut self, spec: Option<LaunchSpec>) {
        self.command_override = spec;
    }

    pub fn command_override(&self) -> Option<LaunchSpec> {
        self.command_override.clone()
    }

    pub fn report_err(&self, e: &str) {
        *self.last_error.lock().unwrap() = Some(e.to_string());
    }

    pub fn last_error(&self) -> Option<String> {
        self.last_error.lock().unwrap().clone()
    }

    /// What the last `start()` did about the system proxy, so the caller can log
    /// one credential-free line (host:port only).
    pub fn last_proxy_injection(&self) -> ProxyInjection {
        self.last_proxy.lock().unwrap().clone()
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
        if let Some(pid) = self.adopt_existing_backend() {
            return Ok(pid);
        }
        let port = self.config.port();
        if port_is_open(port) {
            let err = format!(
                "端口 {port} 已被占用，无法启动 backend（改 small 配置中的 port 或确认无旧进程）"
            );
            *self.last_error.lock().unwrap() = Some(err.clone());
            return Err(err);
        }
        let (argv, extra_env, cwd) = self.resolve_command()?;
        // A Finder/dock launch inherits no proxy env, so the backend would never
        // reach Pixiv even though the login webview can. Forward the system
        // proxy — never overriding a variable the user (or the LaunchSpec)
        // already set. Planned before `extra_env` is consumed below.
        let plan = proxy::plan_proxy(proxy::system_proxy(), |key| {
            extra_env.contains_key(key) || std::env::var_os(key).is_some()
        });
        let mut c = Command::new(&argv[0]);
        c.args(&argv[1..])
            .env("PORT", port.to_string())
            .env("HOST", "127.0.0.1");
        if let Some(dir) = cwd {
            c.current_dir(dir);
        }
        for (k, v) in extra_env {
            c.env(k, v);
        }
        for (k, v) in &plan.env {
            c.env(k, v);
        }
        *self.last_proxy.lock().unwrap() = plan.outcome;
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
    ///
    /// Covers both a backend we spawned (own `Child` handle) and an adopted
    /// orphan, which has to be signalled by pid.
    pub fn stop(&mut self) -> Result<(), String> {
        if self.child.lock().unwrap().is_none() {
            let adopted = *self.adopted_pid.lock().unwrap();
            if let Some(pid) = adopted {
                #[cfg(unix)]
                if pid > 0 {
                    unsafe { libc::kill(pid as i32, libc::SIGTERM) };
                }
                #[cfg(windows)]
                {
                    let _ = Command::new("taskkill")
                        .args(["/PID", &pid.to_string()])
                        .status();
                }
                let deadline = Instant::now() + GRACEFUL_TIMEOUT;
                while port_is_open(self.config.port()) && Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(120));
                }
                *self.adopted_pid.lock().unwrap() = None;
                *self.healthy.lock().unwrap() = None;
            }
            return Ok(());
        }
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

    fn resolve_command(
        &self,
    ) -> Result<(Vec<String>, BTreeMap<String, String>, Option<std::path::PathBuf>), String> {
        // Adapter-injected spec (bundled / config / PATH via discovery) wins.
        if let Some(spec) = &self.command_override {
            if !spec.command.is_empty() {
                return Ok((spec.command.clone(), spec.env.clone(), spec.cwd.clone()));
            }
        }
        // User-configured command (direct, bypassing discovery).
        if !self.config.backend.command.trim().is_empty() {
            let mut argv = vec![self.config.backend.command.clone()];
            argv.extend(self.config.backend.args.iter().cloned());
            return Ok((argv, BTreeMap::new(), None));
        }
        // Dev/test default: the bundled mock backend (health-probe stub).
        if let Some(c) = discovery::mock_script() {
            return Ok((
                vec!["node".into(), c.display().to_string()],
                BTreeMap::new(),
                None,
            ));
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

/// Raw HTTP GET probe usable for WebUI accessibility checks.
/// Returns `(status_code, response_body_len)`, or None when unreachable.
pub fn http_status(port: u16, path: &str) -> Option<(u16, usize)> {
    let status = http_get("127.0.0.1", port, path).ok()?;
    let mut parts = status.split_whitespace();
    let code: u16 = parts.nth(1).and_then(|c| c.parse().ok())?;
    Some((code, status.len()))
}

/// Pid of the process listening on `port`, if any (Unix only).
#[cfg(unix)]
fn listener_pid(port: u16) -> Option<u32> {
    let out = Command::new("lsof")
        .args(["-nP", "-ti", &format!("tcp:{port}"), "-sTCP:LISTEN"])
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.trim().parse::<u32>().ok())
}

#[cfg(not(unix))]
fn listener_pid(_port: u16) -> Option<u32> {
    None
}

/// Full command line of `pid` (empty when unknown). Used to make sure a port
/// occupant is a PixivFlow runtime before we adopt it.
#[cfg(unix)]
fn process_command(pid: u32) -> String {
    Command::new("ps")
        .args(["-o", "command=", "-p", &pid.to_string()])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

#[cfg(not(unix))]
fn process_command(_pid: u32) -> String {
    String::new()
}
