//! BackendManager — owns the lifecycle of the PixivFlow backend subprocess.
//!
//! Responsibilities:
//!  - resolve program / entry (`node + dist/webui/index.js`, or a `command` override)
//!  - start the subprocess with desktop-derived env (port, host, data/db/download dirs, STATIC_PATH)
//!  - adopt an already-running backend on the same port instead of double-spawning
//!  - stop() does GRACEFUL shutdown (SIGTERM / taskkill) then, only as a fallback,
//!    force-kills after a timeout — never starts with `kill -9`.
//!  - restart() = stop + start.

use crate::config::AppConfig;
use crate::health;
use serde::Serialize;
use std::fs::File;
use std::net::ToSocketAddrs;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

const HEALTH_TIMEOUT: Duration = Duration::from_secs(2);

/// Resolved app dirs. Built once from the Tauri app handle.
#[derive(Clone)]
pub struct Paths {
    pub data_dir: PathBuf,
    pub download_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub resource_dir: PathBuf,
}

/// Public, serializable snapshot of the backend state.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BackendStatus {
    pub mode: String,
    /// stopped | starting | running | stopping | error
    pub state: String,
    /// pid of the process WE spawned (None when we adopted an existing one).
    pub pid: Option<u32>,
    pub host: String,
    pub port: u16,
    pub url: String,
    pub data_dir: String,
    pub download_dir: String,
    pub last_error: Option<String>,
}

pub struct BackendManager {
    pub cfg: AppConfig,
    pub paths: Paths,
    pub webui_static: Option<PathBuf>,
    backend_root: Option<PathBuf>,
    state: String,
    child: Option<Child>,
    pid: Option<u32>,
    last_error: Option<String>,
}

impl BackendManager {
    pub fn new(cfg: AppConfig, paths: Paths) -> Self {
        let backend_root = BackendManager::detect_backend_root(&cfg, &paths);
        let webui_static = BackendManager::detect_webui_static(&paths);
        let state = if cfg.mode == "remote" { "remote".to_string() } else { "stopped".to_string() };
        BackendManager {
            cfg,
            paths,
            webui_static,
            backend_root,
            state,
            child: None,
            pid: None,
            last_error: None,
        }
    }

    fn detect_backend_root(cfg: &AppConfig, paths: &Paths) -> Option<PathBuf> {
        if !cfg.backend.dist_path.trim().is_empty() {
            let p = PathBuf::from(cfg.backend.dist_path.trim());
            if p.join("dist/webui/index.js").exists() {
                return Some(p);
            }
            // The user might point straight at a dist dir.
            if p.join("webui/index.js").exists() {
                return Some(p.parent().map(|x| x.to_path_buf()).unwrap_or(p));
            }
        }
        if let Some(p) = std::env::var_os("PIXIVFLOW_DIST_PATH") {
            let p = PathBuf::from(p);
            if p.join("dist/webui/index.js").exists() {
                return Some(p);
            }
        }
        for cand in ["resources/backend", "backend", "pixivflow"] {
            let p = paths.resource_dir.join(cand);
            if p.join("dist/webui/index.js").exists() {
                return Some(p);
            }
        }
        // Dev fallback: `npm run bundle` writes into <repo>/src-tauri/resources;
        // under `tauri dev` the runtime resource_dir does not contain them.
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        for cand in ["resources/backend", "backend"] {
            let p = manifest.join(cand);
            if p.join("dist/webui/index.js").exists() {
                return Some(p);
            }
        }
        None
    }

    fn detect_webui_static(paths: &Paths) -> Option<PathBuf> {
        for cand in ["resources/webui", "webui", "webui-dist"] {
            let p = paths.resource_dir.join(cand);
            if p.join("index.html").exists() {
                return Some(p);
            }
        }
        // Dev fallback (see detect_backend_root).
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        for cand in ["resources/webui", "webui"] {
            let p = manifest.join(cand);
            if p.join("index.html").exists() {
                return Some(p);
            }
        }
        None
    }

    /// Resolved backend base URL (`http://<host>:<port>`).
    pub fn backend_url(&self) -> String {
        format!("http://{}:{}", self.cfg.backend.host, self.cfg.backend.port)
    }

    pub fn status(&self) -> BackendStatus {
        BackendStatus {
            mode: self.cfg.mode.clone(),
            state: self.state.clone(),
            pid: self.pid,
            host: self.cfg.backend.host.clone(),
            port: self.cfg.backend.port,
            url: self.backend_url(),
            data_dir: self.paths.data_dir.to_string_lossy().into_owned(),
            download_dir: self.paths.download_dir.to_string_lossy().into_owned(),
            last_error: self.last_error.clone(),
        }
    }

    /// Returns true if a backend is currently healthy on the configured port.
    pub fn health_check(&self) -> bool {
        health::health_ok(&self.cfg.backend.host, self.cfg.backend.port, HEALTH_TIMEOUT)
    }

    /// Starts the backend. Idempotent:
    ///  - if already tracked/starting, returns current status
    ///  - if a backend is already healthy on the port, ADOPTS it (no double spawn)
    ///  - if the port is open but not a healthy PixivFlow → error (occupied)
    ///  - otherwise spawn `node <root>/dist/webui/index.js` (or `command` override)
    ///
    /// In remote mode there is nothing to start; state stays "remote".
    pub fn start(&mut self) -> Result<BackendStatus, String> {
        if self.cfg.mode == "remote" {
            return Ok(self.status());
        }
        if self.state == "running" || self.state == "starting" {
            return Ok(self.status());
        }

        // Already healthy somewhere → adopt, don't duplicate.
        if self.health_check() {
            self.state = "running".to_string();
            self.pid = None; // not ours
            self.last_error = None;
            return Ok(self.status());
        }
        // Port reachable but not healthy → occupied by another service.
        if port_open(&self.cfg.backend.host, self.cfg.backend.port) {
            let err = format!(
                "port {} is occupied by another process but is not a PixivFlow health endpoint",
                self.cfg.backend.port
            );
            self.state = "error".to_string();
            self.last_error = Some(err.clone());
            return Err(err);
        }

        self.state = "starting".to_string();
        self.last_error = None;

        // Build the launch command.
        let launch = self.launch_command()?;

        // Redirect child stdout/stderr into our logs dir for debugging.
        let out_path = self.paths.logs_dir.join("backend.out.log");
        let err_path = self.paths.logs_dir.join("backend.err.log");
        let _ = std::fs::create_dir_all(&self.paths.logs_dir);
        let out_file = File::options().create(true).append(true).open(&out_path);
        let err_file = File::options().create(true).append(true).open(&err_path);

        let mut cmd = Command::new(&launch.program);
        cmd.args(&launch.args);
        // Set the working dir only when a backend root is known; a `command`
        // override (e.g. a self-contained backend exe) works without one.
        if let Some(dir) = self.backend_root.as_ref() {
            cmd.current_dir(dir);
        }

        // Desktop-derived environment for PixivFlow.
        cmd.env("PORT", self.cfg.backend.port.to_string());
        cmd.env("HOST", &self.cfg.backend.host);
        cmd.env("PIXIV_DATABASE_PATH", self.paths.data_dir.join("pixiv-downloader.db"));
        cmd.env("PIXIV_DOWNLOAD_DIR", &self.paths.download_dir);
        cmd.env("PIXIV_LOG_LEVEL", "info");
        if let Some(static_dir) = &self.webui_static {
            cmd.env("STATIC_PATH", static_dir);
        }

        if let Ok(f) = out_file {
            cmd.stdout(Stdio::from(f));
        }
        if let Ok(f) = err_file {
            cmd.stderr(Stdio::from(f));
        }

        match cmd.spawn() {
            Ok(child) => {
                self.pid = Some(child.id());
                self.child = Some(child);
                self.state = "starting".to_string();
                Ok(self.status())
            }
            Err(e) => {
                let err = format!("failed to launch backend: {e}");
                self.state = "error".to_string();
                self.last_error = Some(err.clone());
                Err(err)
            }
        }
    }

    /// Blocks until `/api/health` is OK, up to `cfg.backend.launch_timeout_ms`.
    pub fn wait_healthy(&mut self) -> Result<(), String> {
        let timeout = Duration::from_millis(self.cfg.backend.launch_timeout_ms);
        let ok = health::wait_healthy(&self.cfg.backend.host, self.cfg.backend.port, timeout);
        if ok {
            self.state = "running".to_string();
            self.last_error = None;
            return Ok(());
        }
        let err = if self.pid.is_some() && !self.alive() {
            "backend exited before becoming healthy".to_string()
        } else {
            format!(
                "backend did not become healthy within {} ms (check {} log)",
                self.cfg.backend.launch_timeout_ms,
                self.paths.logs_dir.join("backend.err.log").display()
            )
        };
        self.state = "error".to_string();
        self.last_error = Some(err.clone());
        Err(err)
    }

    /// Gracious shutdown. Never `kill -9` first.
    pub fn stop(&mut self) -> Result<BackendStatus, String> {
        match self.state.as_str() {
            "stopped" | "remote" => return Ok(self.status()),
            "stopping" => return Ok(self.status()),
            _ => {}
        }

        // If we adopted a backend (pid == None), we don't own it — leave it alone.
        if self.pid.is_none() && self.state == "running" {
            self.last_error =
                Some("backend was already running and is not owned by this app".to_string());
            self.state = "running".to_string();
            return Ok(self.status());
        }

        self.state = "stopping".to_string();
        let pid = self.pid;

        // Reap a child if we spawned one.
        if let Ok(exit) = self.try_reap() {
            if exit {
                self.finish_stop(None);
                return Ok(self.status());
            }
        }

        let Some(pid) = pid else {
            self.finish_stop(None);
            return Ok(self.status());
        };

        let term_grace = Duration::from_secs(8);
        send_terminate(pid);
        // Wait for graceful exit.
        let deadline = std::time::Instant::now() + term_grace;
        while std::time::Instant::now() < deadline {
            if let Ok(exit) = self.try_reap() {
                if exit {
                    self.finish_stop(Some(pid));
                    return Ok(self.status());
                }
            }
            std::thread::sleep(Duration::from_millis(150));
        }
        // The backend is still alive — escalate to a forced kill, which is the
        // CORRECT last resort (not the first step).
        safety_kill(pid);
        std::thread::sleep(Duration::from_millis(300));
        let _ = self.try_reap();
        self.finish_stop(Some(pid));
        Ok(self.status())
    }

    pub fn restart(&mut self) -> Result<BackendStatus, String> {
        let _ = self.stop();
        self.child = None;
        self.pid = None;
        self.state = "stopped".to_string();
        let s = self.start()?;
        if self.wait_healthy().is_err() {
            return Err(self.last_error.clone().unwrap_or_default());
        }
        Ok(s)
    }

    /// Checks whether a spawned child has exited. Returns true if it did (and
    /// updates state to reflect a crash).
    pub fn poll_exit(&mut self) -> bool {
        self.state == "running" && match self.try_reap() {
            Ok(true) => {
                self.state = "error".to_string();
                self.last_error = Some("backend process exited unexpectedly".to_string());
                self.child = None;
                self.pid = None;
                true
            }
            _ => false,
        }
    }

    fn alive(&mut self) -> bool {
        match self.try_reap() {
            Ok(true) => false,
            _ => true,
        }
    }

    /// Non-blocking reap. Ok(true) when the child has exited.
    fn try_reap(&mut self) -> std::io::Result<bool> {
        if let Some(child) = self.child.as_mut() {
            match child.try_wait()? {
                Some(_) => {
                    self.child = None;
                    Ok(true)
                }
                None => Ok(false),
            }
        } else {
            Ok(true)
        }
    }

    fn finish_stop(&mut self, _pid: Option<u32>) {
        self.child = None;
        self.pid = None;
        self.state = "stopped".to_string();
    }

    fn launch_command(&self) -> Result<Launch, String> {
        let bc = &self.cfg.backend;
        if !bc.command.trim().is_empty() {
            let (program, args) = split_command(&bc.command);
            if program.is_empty() {
                return Err("backend.command is empty".to_string());
            }
            return Ok(Launch { program, args });
        }
        let node = if bc.node_path.trim().is_empty() {
            "node".to_string()
        } else {
            bc.node_path.trim().to_string()
        };
        let entry = match self.backend_root.as_ref() {
            Some(root) => root.join("dist").join("webui").join("index.js"),
            None => return Err("could not locate PixivFlow backend (set backend.dist_path)".to_string()),
        };
        if !entry.exists() {
            return Err(format!(
                "backend entry not found: {} (build the backend with `npm run bundle:backend`)",
                entry.display()
            ));
        }
        Ok(Launch {
            program: node,
            args: vec![entry.to_string_lossy().into_owned()],
        })
    }
}

struct Launch {
    program: String,
    args: Vec<String>,
}

/// Splits a shell-ish command string into program + args on whitespace
/// (quoted segments preserved minimally). Good enough for paths/commands.
fn split_command(cmd: &str) -> (String, Vec<String>) {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() {
        (String::new(), vec![])
    } else {
        (parts[0].to_string(), parts[1..].iter().map(|s| s.to_string()).collect())
    }
}

/// True if a TCP connect to `host:port` succeeds at all.
fn port_open(host: &str, port: u16) -> bool {
    let host = if host == "localhost" { "127.0.0.1" } else { host };
    if let Ok(addrs) = format!("{}:{}", host, port).to_socket_addrs() {
        for a in addrs {
            if std::net::TcpStream::connect_timeout(&a, Duration::from_millis(800)).is_ok() {
                return true;
            }
        }
    }
    false
}

/// Sends a terminate signal. Unix: SIGTERM. Windows: `taskkill /PID` (best-effort
/// graceful for console apps; WM_CLOSE gesture). We never start with force.
fn send_terminate(pid: u32) {
    if cfg!(windows) {
        let _ = Command::new("taskkill").args(["/PID", &pid.to_string()]).status();
    } else {
        let _ = Command::new("kill").args(["-TERM", &pid.to_string()]).status();
    }
}

/// Last-resort forced kill after the graceful timeout elapsed.
fn safety_kill(pid: u32) {
    if cfg!(windows) {
        let _ = Command::new("taskkill")
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .status();
    } else {
        let _ = Command::new("kill")
            .args(["-KILL", &pid.to_string()])
            .status();
    }
}