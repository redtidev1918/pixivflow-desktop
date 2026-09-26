//! Real-backend discovery — resolves WHICH backend executable to run.
//!
//! Priority (highest first), per the F2.2 spec (bundled first so a user install
//! is never perturbed by a stray `pixivflow` on PATH / in config):
//!   1. bundled `resources/runtime/pixivflow/` (described by `runtime-manifest.json`)
//!   2. user-configured path (`backend.command` / `backend.args`)   [user override]
//!   3. `pixivflow` found on `PATH`
//!   4. fallback: the bundled **mock** backend (dev/test stand-in only)
//!
//! This module only *discovers and describes* a command — it does NOT touch the
//! process. The actual spawn/stop/health lifecycle stays in `BackendManager`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::config::AppConfig;

/// Where an executable was found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BackendSource {
    /// Shipped inside the app/bundle `resources/runtime/pixivflow/`.
    Bundled,
    /// Explicitly configured via `backend.command` (+`args`).
    Config,
    /// Found on `PATH` as `pixivflow` (can shadow nothing when bundled exists).
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

/// The F2.2.1 runtime-manifest contract for a bundled backend.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeManifest {
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub platform: String,
    /// argv (relative paths resolved against the runtime dir).
    pub command: Vec<String>,
    #[serde(default = "default_health")]
    pub health: String,
    /// Whether this backend can serve the WebUI over STATIC_PATH (方案 A).
    #[serde(default)]
    pub serves_webui: bool,
}

fn default_health() -> String {
    "/api/health".into()
}

/// A resolved bundled backend (manifest parsed + path-resolved + ready to run).
#[derive(Debug, Clone)]
pub struct BundledRuntime {
    pub manifest: RuntimeManifest,
    /// Absolute path of `resources/runtime/pixivflow/`.
    pub dir: PathBuf,
    /// argv with any relative path absolutized against `dir`.
    pub command: Vec<String>,
    /// Absolute path of the actual entry file (for `exe` display / `--version`).
    pub executable_path: Option<String>,
    /// Absolute path of the bundled WebUI dist ("" when the backend can't serve it).
    pub static_path: Option<String>,
}

/// A resolved backend command: the full argv to spawn plus provenance metadata.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendDescriptor {
    pub source: BackendSource,
    /// Absolute/relative executable path when a REAL backend was found.
    pub executable_path: Option<String>,
    /// Full argv (`argv[0]` = executable). Always non-empty for real sources.
    pub command: Vec<String>,
    /// Probe result of `argv --version` (real backends only).
    pub version: Option<String>,
    /// If the backend serves the bundled WebUI (方案 A), the STATIC_PATH to pass.
    pub static_path: Option<String>,
    /// True when the resolved backend serves the WebUI over STATIC_PATH.
    pub serves_webui: bool,
}

impl BackendDescriptor {
    pub fn is_real(&self) -> bool {
        self.source.is_real()
    }
    /// The extra env to inject when spawning (STATIC_PATH for 方案 A, else empty).
    pub fn extra_env(&self) -> BTreeMap<String, String> {
        let mut env = BTreeMap::new();
        if let Some(p) = self.static_path.as_ref() {
            env.insert("STATIC_PATH".into(), p.clone());
        }
        env
    }
}

const PROBE_TIMEOUT: Duration = Duration::from_secs(4);

/// Resolve the backend command to run, applying the spec's priority order.
pub fn discover(config: &AppConfig) -> BackendDescriptor {
    // 1) bundled `resources/runtime/pixivflow/` (manifest-described)
    if let Some(br) = bundled_runtime() {
        return BackendDescriptor {
            source: BackendSource::Bundled,
            executable_path: br.executable_path,
            command: br.command,
            version: Some(br.manifest.version),
            static_path: br.static_path,
            serves_webui: br.manifest.serves_webui,
        };
    }
    // 2) user-configured command (user override)
    let cfg_cmd = command_from_config(config);
    if let Some(argv) = cfg_cmd {
        return BackendDescriptor {
            source: BackendSource::Config,
            executable_path: Some(argv[0].clone()),
            command: argv,
            version: None,
            static_path: None,
            serves_webui: false,
        };
    }
    // 3) PATH
    if let Some(exe) = find_in_path("pixivflow") {
        return BackendDescriptor {
            source: BackendSource::Path,
            executable_path: Some(exe.clone()),
            command: vec![exe],
            version: None,
            static_path: None,
            serves_webui: false,
        };
    }
    // 4) fallback: bundled mock backend (dev/test stand-in ONLY)
    if let Some(mock) = mock_script() {
        return BackendDescriptor {
            source: BackendSource::Mock,
            executable_path: Some(mock.display().to_string()),
            command: vec!["node".into(), mock.display().to_string()],
            version: None,
            static_path: None,
            serves_webui: false,
        };
    }
    BackendDescriptor {
        source: BackendSource::NotFound,
        executable_path: None,
        command: Vec::new(),
        version: None,
        static_path: None,
        serves_webui: false,
    }
}

/// Build the config-provided argv, or None when `backend.command` is empty.
fn command_from_config(config: &AppConfig) -> Option<Vec<String>> {
    if config.backend.command.trim().is_empty() {
        return None;
    }
    let mut command = vec![config.backend.command.clone()];
    command.extend(config.backend.args.iter().cloned());
    Some(command)
}

/// Resolve the bundled backend under `resources/runtime/pixivflow/`.
///
/// New-style: reads `runtime-manifest.json` inside the dir. Legacy fallback:
/// a bare `pixivflow`/`pixivflow.exe` executable directly under `runtime/`.
fn bundled_runtime() -> Option<BundledRuntime> {
    let dir = bundled_dir()?; // runtime dir → runtime/pixivflow dir
    if !dir.is_dir() {
        return None;
    }
    // (a) manifest-described backend
    let manifest_path = dir.join("runtime-manifest.json");
    if manifest_path.is_file() {
        if let Ok(text) = std::fs::read_to_string(&manifest_path) {
            if let Ok(manifest) = serde_json::from_str::<RuntimeManifest>(&text) {
                if !manifest.command.is_empty() {
                    let (command, exe) = resolve_argv(&dir, &manifest.command);
                    let static_path = if manifest.serves_webui {
                        static_webui_path()
                    } else {
                        None
                    };
                    return Some(BundledRuntime {
                        dir,
                        manifest,
                        command,
                        executable_path: exe,
                        static_path,
                    });
                }
            }
        }
    }
    // (b) legacy bare executable under `runtime/`
    let exe_name = if cfg!(windows) { "pixivflow.exe" } else { "pixivflow" };
    let bare = dir.join(exe_name);
    if bare.is_file() {
        return Some(BundledRuntime {
            manifest: RuntimeManifest {
                name: "pixivflow".into(),
                version: "unknown".into(),
                platform: String::new(),
                command: vec![bare.display().to_string()],
                health: "/api/health".into(),
                serves_webui: false,
            },
            dir,
            command: vec![bare.display().to_string()],
            executable_path: Some(bare.display().to_string()),
            static_path: None,
        });
    }
    None
}

/// `resources/runtime/pixivflow/` absolute path (from cargo manifest or CWD).
fn bundled_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/runtime/pixivflow");
    if dir.is_dir() {
        return Some(dir);
    }
    let alt = PathBuf::from("src-tauri/resources/runtime/pixivflow");
    if alt.is_dir() {
        return Some(alt);
    }
    None
}

/// Resolve relative argv entries against `dir`. Returns (argv, entry-file).
fn resolve_argv(dir: &Path, raw: &[String]) -> (Vec<String>, Option<String>) {
    let mut argv: Vec<String> = Vec::with_capacity(raw.len());
    let mut exe: Option<String> = None;
    for elem in raw {
        let has_sep = elem.contains('/') || elem.contains('\\');
        let joined = dir.join(elem);
        if has_sep && joined.is_file() {
            argv.push(joined.display().to_string());
            if exe.is_none() {
                exe = Some(joined.display().to_string());
            }
        } else {
            argv.push(elem.clone());
        }
    }
    (argv, exe)
}

/// Absolute path of the bundled WebUI dist, if present (方案 A STATIC_PATH).
pub fn static_webui_path() -> Option<String> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/webui/dist");
    if dir.is_dir() {
        return Some(dir.display().to_string());
    }
    let alt = PathBuf::from("src-tauri/resources/webui/dist");
    if alt.is_dir() {
        return Some(alt.display().to_string());
    }
    None
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
    /// WebUI dist the backend will serve (方案 A), when applicable.
    pub static_path: Option<String>,
    pub port: u16,
    pub running: bool,
    /// Live health probe while running: Some(true)=200, Some(false)=no-200,
    /// None=not probing (not running).
    pub healthy: Option<bool>,
    pub message: String,
}

/// Convenience: discover + probe version for real backends + set serves_webui.
///
/// When the descriptor already carries a version (a bundled runtime's manifest),
/// it is trusted — `--version` is NOT probed, because a real release binary may
/// not support a `--version` flag (it would boot a server instead of printing).
/// Only version-less sources (PATH / config binaries) get the `--version` probe.
pub fn discover_with_version(config: &AppConfig) -> BackendDescriptor {
    let mut d = discover(config);
    if d.is_real() && d.version.is_none() {
        d.version = probe_version(&d.command);
    }
    d
}
