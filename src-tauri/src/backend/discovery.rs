//! Real-backend discovery (F2.1) — resolves WHICH backend executable to run.
//!
//! Priority (highest first), per the F2.1 spec:
//!   1. bundled `runtime/` (shipped inside the app bundle / repo runtime dir)
//!   2. user-configured path (`backend.command` / `backend.args`)
//!   3. `pixivflow` found on `PATH`
//!   4. fallback: the bundled **mock** backend (dev/test stand-in)
//!
//! This module only *discovers and describes* a command — it does NOT touch the
//! process. The actual spawn/stop/health lifecycle stays in `BackendManager`.

use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::config::AppConfig;

/// Where an executable was found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BackendSource {
    /// Shipped inside the app/bundle `resources/runtime/`.
    Bundled,
    /// Explicitly configured via `backend.command` (+`args`).
    Config,
    /// Found on `PATH` as `pixivflow`.
    Path,
    /// The dev/test mock backend (not a real PixivFlow).
    Mock,
    /// Rescue value — nothing resolvable at all.
    NotFound,
}

impl BackendSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Bundled => "bundled",
            Self::Config => "config",
            Self::Path => "path",
            Self::Mock => "mock",
            Self::NotFound => "not-found",
        }
    }
    pub fn is_real(&self) -> bool {
        !matches!(self, Self::Mock | Self::NotFound)
    }
}

/// A resolved backend command: the full argv to spawn plus provenance metadata.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendDescriptor {
    pub source: BackendSource,
    /// Absolute/relative executable path when a REAL backend was found.
    pub executable_path: Option<String>,
    /// Full argv (`argv[0]` = executable). Always non-empty.
    pub command: Vec<String>,
    /// Probe result of `argv --version` (real backends only).
    pub version: Option<String>,
}

impl BackendDescriptor {
    pub fn is_real(&self) -> bool {
        self.source.is_real()
    }
}

const PROBE_TIMEOUT: Duration = Duration::from_secs(4);

/// Resolve the backend command to run, applying the spec's priority order.
pub fn discover(config: &AppConfig) -> BackendDescriptor {
    // 1) bundled `resources/runtime/`
    if let Some(exe) = bundled_runtime() {
        return BackendDescriptor {
            source: BackendSource::Bundled,
            executable_path: Some(exe.display().to_string()),
            version: Some("bundled".into()), // filled by probe where possible
            command: vec![exe.display().to_string()],
        };
    }
    // 2) user-configured command
    if !config.backend.command.trim().is_empty() {
        let mut command = vec![config.backend.command.clone()];
        command.extend(config.backend.args.iter().cloned());
        return BackendDescriptor {
            source: BackendSource::Config,
            executable_path: Some(config.backend.command.clone()),
            version: None,
            command,
        };
    }
    // 3) PATH
    if let Some(exe) = find_in_path("pixivflow") {
        return BackendDescriptor {
            source: BackendSource::Path,
            executable_path: Some(exe.to_string()),
            version: None,
            command: vec![exe],
        };
    }
    // 4) fallback: bundled mock backend (dev/test)
    if let Some(mock) = mock_script() {
        return BackendDescriptor {
            source: BackendSource::Mock,
            executable_path: Some(mock.display().to_string()),
            version: None,
            command: vec!["node".into(), mock.display().to_string()],
        };
    }
    BackendDescriptor {
        source: BackendSource::NotFound,
        executable_path: None,
        version: None,
        command: Vec::new(),
    }
}

/// Look for the real bundled backend under `resources/runtime/`.
fn bundled_runtime() -> Option<PathBuf> {
    let exe = if cfg!(windows) { "pixivflow.exe" } else { "pixivflow" };
    let candidates = [
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/runtime").join(exe),
        PathBuf::from("src-tauri/resources/runtime").join(exe),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

/// Search PATH for a command name (case-insensitive on Windows).
fn find_in_path(name: &str) -> Option<String> {
    let path_var = std::env::var("PATH").unwrap_or_default();
    let exe_names = if cfg!(windows) {
        vec![name.to_owned(), format!("{name}.exe")]
    } else {
        vec![name.to_owned()]
    };
    for dir in std::env::split_paths(&path_var) {
        for n in &exe_names {
            let p = dir.join(n);
            if p.is_file() {
                return Some(p.display().to_string());
            }
        }
    }
    None
}

/// Locate the dev mock backend script.
fn mock_script() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/mock-backend.mjs"),
        PathBuf::from("src-tauri/resources/mock-backend.mjs"),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

/// Run `argv --version` and return the first non-empty stdout line.
///
/// Uses `argv[0]` as the executable; prepends `args + "--version"`. Bounded by
/// PROBE_TIMEOUT via `try_wait` polling. Returns None on failure/timeout.
pub fn probe_version(argv: &[String]) -> Option<String> {
    if argv.is_empty() {
        return None;
    }
    let exe = &argv[0];
    let ver_flag = "--version".to_string();
    let mut args: Vec<&String> = argv[1..].iter().collect();
    args.push(&ver_flag);

    let mut child = match Command::new(exe)
        .args(&args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return None,
    };

    let deadline = Instant::now() + PROBE_TIMEOUT;
    let mut out = String::new();
    loop {
        if let Some(status) = child.try_wait().unwrap_or(None) {
            let _ = status;
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            return None;
        }
        std::thread::sleep(Duration::from_millis(40));
    }
    use std::io::Read;
    if let Some(mut so) = child.stdout.take() {
        let mut buf = Vec::new();
        let _ = so.read_to_end(&mut buf);
        out = String::from_utf8_lossy(&buf).into_owned();
    }
    out.lines()
        .map(|l| l.trim())
        .find(|l| !l.is_empty())
        .map(|s| s.to_string())
}


/// The `backend_doctor` command report: what was discovered + current runtime.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorReport {
    /// A backend (real source or the mock fallback) could be resolved.
    pub backend_found: bool,
    /// Absolute/relative executable path when a REAL backend was found.
    pub executable_path: Option<String>,
    /// `bundled` | `config` | `path` | `mock` | `not-found`.
    pub source: String,
    /// `argv --version` probe result (real backends only).
    pub version: Option<String>,
    pub port: u16,
    pub running: bool,
    /// Live health probe while running: Some(true)=200, Some(false)=no-200,
    /// None=not probing (not running).
    pub healthy: Option<bool>,
    pub message: String,
}

/// Convenience: discover + probe version for real backends.
pub fn discover_with_version(config: &AppConfig) -> BackendDescriptor {
    let mut d = discover(config);
    if d.is_real() {
        d.version = probe_version(&d.command);
    }
    d
}
