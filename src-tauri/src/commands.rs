//! Tauri commands (F1). Rust owns system capability; the frontend only
//! invokes these and renders the resulting status.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, State};

use crate::backend::discovery::{self, DoctorReport, WebuiReport};
use crate::backend::manager;
use crate::backend::{BackendSource, LaunchSpec, StatusSnapshot};
use crate::config::AppConfig;
use crate::i18n;
use crate::logger;
use crate::login_window::{extract_auth_code, parse_url};
use crate::ManagedState;

/// How long the host waits for the user to finish Pixiv authorization before
/// giving up and resolving `None` (the WebUI can start a new attempt).
const LOGIN_TIMEOUT: Duration = Duration::from_secs(300);

/// Injected into the `webui` window only, on every navigation. It exposes the
/// host login bridge: the WebUI asks the host for the `authUrl`/`redirectUri`
/// (`POST /api/auth/login/host/start`) and passes both straight back here — the
/// desktop never builds or parses a Pixiv URL. Requires `withGlobalTauri`.
///
/// The WebUI's contract is `openLoginWindow(): Promise<{ code: string | null }>`
/// (pixivflow-webui `src/types/host-bridge.d.ts`), so the raw command result is
/// wrapped: the command resolves `Option<String>` (cancellation and timeout are
/// `null`), and the bridge always resolves an object with a `code` property.
///
/// The script also forwards the WebUI's own JS errors to `log_frontend`: the
/// remote page is served from another repository, so without this its failures
/// would never appear in the desktop log. Forwarding is capped per page load,
/// truncated, wrapped in try/catch and never throws — a rendering loop must not
/// be able to flood the log or break the page.
const HOST_BRIDGE_SCRIPT: &str = r#"window.pixivflowHost = {
  openLoginWindow: function (authUrl, redirectUri) {
    return window.__TAURI__.core
      .invoke('open_login_window', { authUrl: authUrl, redirectUri: redirectUri })
      .then(function (code) { return { code: code === undefined ? null : code }; });
  },
  revealPath: function (path) {
    return window.__TAURI__.core.invoke('reveal_path', { path: path });
  }
};
(function () {
  var MAX_EVENTS = 20;
  var MAX_MESSAGE = 2000;
  var MAX_SOURCE = 200;
  var forwarded = 0;
  function clip(value, limit) {
    var text = value === undefined || value === null ? '' : String(value);
    return text.length > limit ? text.slice(0, limit) + '...' : text;
  }
  function forward(level, message, source) {
    if (forwarded >= MAX_EVENTS) { return; }
    forwarded = forwarded + 1;
    try {
      window.__TAURI__.core.invoke('log_frontend', {
        level: level,
        message: clip(message, MAX_MESSAGE),
        source: clip(source, MAX_SOURCE)
      }).catch(function () {});
    } catch (e) { /* logging must never break the page */ }
  }
  window.addEventListener('error', function (event) {
    var where = event && event.filename ? event.filename + ':' + event.lineno : '';
    forward('error', event && event.message ? event.message : 'window error', where);
  });
  window.addEventListener('unhandledrejection', function (event) {
    var reason = event ? event.reason : null;
    forward('error', reason && reason.message ? reason.message : reason, 'unhandledrejection');
  });
})();"#;

/// Log one info line for the login bridge. Uses the same `ManagedState` handle
/// as the rest of the file, and is callable from any thread (main thread, the
/// navigation callback, the waiter thread) because it re-resolves the state from
/// the handle instead of holding a borrow.
fn login_log(app: &AppHandle, level: &str, msg: &str) {
    if let Some(state) = app.try_state::<ManagedState>() {
        state.log.log(level, &format!("login: {msg}"));
    }
}

/// A URL reduced to `scheme://host/path` — never the query, which carries the
/// OAuth `code`/`state`. Used for every log line in the login bridge.
fn redacted_url(raw: &str) -> String {
    match parse_url(raw) {
        Some(parts) => format!("{}://{}{}", parts.scheme, parts.host, parts.path),
        None => "<unparseable url>".to_string(),
    }
}

fn push_status(app: &AppHandle, state: &ManagedState) {
    let snap = snapshot(state);
    let _ = app.emit("backend-status", snap);
}

/// Publish the current snapshot as a `backend-status` event (one-arg form
/// available to background code in lib.rs).
pub fn emit_status(app: &AppHandle) {
    let state = app.state::<ManagedState>();
    let snap = snapshot(&state);
    let _ = app.emit("backend-status", snap);
}

/// Poll health for the background auto-start path in lib.rs.
pub fn poll_health_for(app: &AppHandle) -> bool {
    let state = app.state::<ManagedState>();
    let healthy = poll_health(&state, app, 15);
    emit_status(app);
    healthy
}

fn snapshot(state: &ManagedState) -> StatusSnapshot {
    let m = state.manager.lock().unwrap();
    let running = m.is_running();
    let healthy = m.healthy();
    let err = m.last_error();
    let cfg_err = state.config_error.lock().unwrap().clone();
    let port = m.port();
    let pid = m.pid();

    let state_name = if cfg_err.is_some() {
        "error"
    } else if running {
        match healthy {
            Some(true) => "running",
            _ => "starting",
        }
    } else if err.is_some() {
        "error"
    } else {
        "stopped"
    };

    StatusSnapshot {
        state: state_name.into(),
        mode: m.mode().into(),
        port,
        pid,
        healthy: healthy.unwrap_or(false),
        message: cfg_err.or(err),
    }
}

/// Poll the health endpoint until it returns 200 or the attempt budget runs out.
/// Each probe result is pushed to the frontend as a `backend-status` event.
/// Returns whether the backend reached healthy within the budget.
fn poll_health(state: &ManagedState, app: &AppHandle, attempts: u32) -> bool {
    for i in 0..attempts {
        let probe = {
            let m = state.manager.lock().unwrap();
            m.health_check()
        };
        match probe {
            Ok(true) => {
                state.manager.lock().unwrap().set_healthy(true);
                state.manager.lock().unwrap().report_err("");
                state.log.info("health OK (/api/health -> 200)");
                push_status(app, state);
                return true;
            }
            Ok(false) => {
                state.manager.lock().unwrap().set_healthy(false);
            }
            Err(e) => {
                state.manager.lock().unwrap().report_err(&e);
                state.log.warn(&format!("health probe {i}: {e}"));
            }
        }
        push_status(app, state);
        std::thread::sleep(Duration::from_millis(400));
    }
    state.log.warn("health not confirmed within attempt budget");
    push_status(app, state);
    false
}

#[tauri::command]
pub fn get_status(state: State<'_, ManagedState>) -> StatusSnapshot {
    snapshot(&state)
}

#[tauri::command]
pub fn get_config(state: State<'_, ManagedState>) -> AppConfig {
    state.manager.lock().unwrap().config_clone()
}

#[tauri::command]
pub fn config_path(state: State<'_, ManagedState>) -> String {
    state.config_path.clone()
}

#[tauri::command]
pub fn start_backend(
    state: State<'_, ManagedState>,
    app: AppHandle,
) -> Result<StatusSnapshot, String> {
    let port = state.manager.lock().unwrap().port();
    state.log.info(&format!("start_backend requested (port {port})"));
    apply_discovery(&state);
    let pid = state.manager.lock().unwrap().start()?;
    state.log.info(&format!("backend started pid={pid}"));
    state.log.info(&format!(
        "backend proxy: {}",
        state.manager.lock().unwrap().last_proxy_injection().describe()
    ));
    poll_health(&state, &app, 15);
    push_status(&app, &state);
    Ok(snapshot(&state))
}

#[tauri::command]
pub fn stop_backend(
    state: State<'_, ManagedState>,
    app: AppHandle,
) -> Result<StatusSnapshot, String> {
    state.manager.lock().unwrap().stop()?;
    state.log.info("backend stopped");
    push_status(&app, &state);
    Ok(snapshot(&state))
}

#[tauri::command]
pub fn restart_backend(
    state: State<'_, ManagedState>,
    app: AppHandle,
) -> Result<StatusSnapshot, String> {
    apply_discovery(&state);
    let pid = state.manager.lock().unwrap().restart()?;
    state.log.info(&format!("backend restarted pid={pid}"));
    poll_health(&state, &app, 15);
    push_status(&app, &state);
    Ok(snapshot(&state))
}


/// Apply the discovery adapter to the manager so `start()` runs the resolved
/// real command (bundled / config / PATH) instead of the plain mock default.
/// Returns the descriptor plus whether a REAL PixivFlow was found.
pub fn apply_discovery(state: &ManagedState) -> (discovery::BackendDescriptor, bool) {
    let cfg = state.manager.lock().unwrap().config_clone();
    let d = discovery::discover(&cfg);
    let cmd = if d.command.is_empty() {
        None
    } else {
        Some(LaunchSpec {
            command: d.command.clone(),
            env: d.extra_env(),
            cwd: d.cwd(),
        })
    };
    state.manager.lock().unwrap().set_command_override(cmd);
    let real = d.is_real();
    state.log.info(&format!(
        "backend resolved: source={} exe={:?} real={real}",
        d.source.as_str(),
        d.executable_path
    ));
    (d, real)
}

/// F2.1 diagnostic: what backend did discovery resolve, and is it healthy?
#[tauri::command]
pub fn backend_doctor(state: State<'_, ManagedState>) -> DoctorReport {
    doctor_report(&state)
}

/// The `backend_doctor` body as a plain function, so `export_diagnostics` can
/// serialize the same report into `doctor.json` without going through IPC.
pub fn doctor_report(state: &ManagedState) -> DoctorReport {
    let cfg = state.manager.lock().unwrap().config_clone();
    let d = discovery::discover_with_version(&cfg);
    let running = state.manager.lock().unwrap().is_running();
    let (healthy, message) = if running {
        match state.manager.lock().unwrap().health_check() {
            Ok(true) => (Some(true), "healthy (GET /api/health -> 200)".into()),
            Ok(false) => (Some(false), "running but not healthy (no 200 on /api/health)".into()),
            Err(e) => (None, format!("health probe failed: {e}")),
        }
    } else if matches!(d.source, BackendSource::Mock | BackendSource::NotFound) {
        (None, "not running — dev mock fallback (no real PixivFlow resolved)".into())
    } else {
        (None, "not running — backend resolved but not started".into())
    };
    // F2.3 WebUI report: STATIC_PATH presence + live root access while running.
    let webui = if d.serves_webui {
        let present = d
            .static_path
            .as_ref()
            .is_some_and(|p| std::path::Path::new(p).is_dir());
        let accessible = running && healthy == Some(true) && present
            && manager::http_status(cfg.port(), "/")
                .is_some_and(|(code, len)| code == 200 && len > 0);
        WebuiReport {
            static_path: d.static_path.clone(),
            present,
            accessible,
        }
    } else {
        WebuiReport {
            static_path: None,
            present: false,
            accessible: false,
        }
    };
    DoctorReport {
        backend_found: d.source != BackendSource::NotFound,
        executable_path: d.executable_path,
        source: d.source.as_str().into(),
        version: d.version,
        static_path: d.static_path,
        port: cfg.port(),
        running,
        healthy,
        message,
        runtime: discovery::validate_bundled_runtime(),
        webui,
    }
}


/// Open the desktop log file in the platform's default viewer.
#[tauri::command]
pub fn open_logs(state: State<'_, ManagedState>) -> Result<(), String> {
    let path = state.log.path().to_string();
    open_in_default_viewer(&path)
}

/// Show one path in the machine's file manager — the "Show in Finder" action.
///
/// The WebUI asks for this through the host bridge, because the backend can only
/// reach a file manager on *its* host, which is a different machine whenever
/// PixivFlow runs on a server. Showing a path is this layer's job precisely
/// because only the desktop knows the device in front of the user.
///
/// The backend resolves and confines the path first (`GET /api/files/location`),
/// and this command confines it *again* — against the directories this
/// installation actually downloads into — because a path handed to an OS opener
/// is a capability, not a string. `crate::reveal` owns that check, the platform
/// matrix, and the coded errors (`REVEAL_FORBIDDEN`, `REVEAL_NOT_FOUND`,
/// `REVEAL_UNAVAILABLE`, `REVEAL_FAILED`) the WebUI needs in order to degrade
/// honestly: "this path is not yours to see" is not the same event as "this
/// machine has no file manager".
#[tauri::command]
pub fn reveal_path(state: State<'_, ManagedState>, path: String) -> Result<(), String> {
    let data_root = discovery::data_root().map(|root| root.as_path());
    crate::reveal::reveal(data_root, Path::new(&state.config_path), &path)
}

/// Spawn the OS file opener for `path`. No waiting; failures surface as Err.
fn open_in_default_viewer(path: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        Command::new("open")
            .arg(path)
            .status()
            .map(|_| ())
            .map_err(|e| format!("failed to open log {path}: {e}"))
    }
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        Command::new("cmd")
            .args(["/C", "start", "", path])
            .status()
            .map(|_| ())
            .map_err(|e| format!("failed to open log {path}: {e}"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        use std::process::Command;
        Command::new("xdg-open")
            .arg(path)
            .status()
            .map(|_| ())
            .map_err(|e| format!("failed to open log {path}: {e}"))
    }
}


// --------------------------------------------------------- frontend logging

/// Forward one line from the launcher UI or the remote WebUI into the desktop
/// log, so a user report contains the JS side of the story too.
///
/// Deliberately tolerant: an unknown level degrades to `info`, the text is
/// redacted and clamped, and the call never fails the caller.
#[tauri::command]
pub fn log_frontend(
    state: State<'_, ManagedState>,
    level: Option<String>,
    message: String,
    source: Option<String>,
) -> Result<(), String> {
    let level = match level.as_deref().map(str::to_ascii_lowercase).as_deref() {
        Some("warn") => "WARN",
        Some("error") => "ERROR",
        _ => "INFO",
    };
    let text = logger::truncate_chars(
        &logger::redact_secrets(&message),
        logger::MAX_FORWARDED_CHARS,
    );
    let origin = source
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| logger::redact_secrets(&logger::truncate_chars(s, 200)));
    let line = match origin {
        Some(origin) => format!("frontend [{origin}]: {text}"),
        None => format!("frontend: {text}"),
    };
    state.log.log(level, &line);
    Ok(())
}

// ------------------------------------------------------- diagnostics export

/// Copy every regular file in `source_dir` whose name `keep` accepts into
/// `dest_dir`. Per-file best effort; returns the names that were copied.
fn copy_files_matching(
    source_dir: &Path,
    dest_dir: &Path,
    keep: impl Fn(&str) -> bool,
) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(source_dir) else {
        return Vec::new();
    };
    let mut copied = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !keep(&name) {
            continue;
        }
        if std::fs::copy(&path, dest_dir.join(&name)).is_ok() {
            copied.push(name);
        }
    }
    copied
}

fn is_secret_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    ["token", "secret", "password", "credential"]
        .iter()
        .any(|needle| key.contains(needle))
}

/// Replace the value of every key matching `(?i)token|secret|password|credential`
/// with `"***"`, at any depth. Defensive: the config carries no secret today, but
/// a user-edited `desktop-config.json` — or a field added later — might.
fn redact_json_secrets(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, val) in map.iter_mut() {
                if is_secret_key(key) {
                    *val = serde_json::Value::String("***".into());
                } else {
                    redact_json_secrets(val);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items.iter_mut() {
                redact_json_secrets(item);
            }
        }
        _ => {}
    }
}

/// Show a directory in the OS file manager. Best effort on every platform.
///
/// Used by `export_diagnostics`, where the folder matters and the selection
/// does not. The bridge's `reveal_path` uses `crate::reveal`, which also
/// selects a single file.
fn reveal_directory_in_file_manager(path: &Path) {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg("-R").arg(path).status();
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = open_in_default_viewer(&path.display().to_string());
    }
}

/// Bundle the log, the run bookkeeping, any collected crash report, the doctor
/// report, a secret-free copy of the config and an environment summary into one
/// folder, then reveal it. This is the supported way to collect evidence from a
/// user: it never needs a terminal and never carries a secret.
#[tauri::command]
pub fn export_diagnostics(
    app: AppHandle,
    state: State<'_, ManagedState>,
) -> Result<String, String> {
    let log_path = PathBuf::from(state.log.path());
    let log_dir = log_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("logs"));
    let dir = log_dir.join(format!("diagnostics-{}", logger::compact_utc_ts(logger::unix_now())));
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;

    // (a) the live log plus every rotated generation, (b) the run bookkeeping
    // and (c) any crash report a previous startup already collected.
    let logs = copy_files_matching(&log_dir, &dir, |name| {
        name == "desktop.log" || name.starts_with("desktop.log.")
    });
    let others = copy_files_matching(&log_dir, &dir, |name| {
        name == "last-run.json" || (name.starts_with("crash-") && name.ends_with(".ips"))
    });

    // (d) the doctor report, rendered exactly like `backend_doctor`.
    let doctor = serde_json::to_string_pretty(&doctor_report(&state))
        .map_err(|e| format!("serialize doctor report: {e}"))?;
    std::fs::write(dir.join("doctor.json"), doctor)
        .map_err(|e| format!("write doctor.json: {e}"))?;

    // (e) the app config with every secret-looking value masked.
    let cfg = state.manager.lock().unwrap().config_clone();
    let mut value = serde_json::to_value(&cfg).map_err(|e| format!("serialize config: {e}"))?;
    redact_json_secrets(&mut value);
    let cfg_json =
        serde_json::to_string_pretty(&value).map_err(|e| format!("serialize config: {e}"))?;
    std::fs::write(dir.join("config.json"), cfg_json)
        .map_err(|e| format!("write config.json: {e}"))?;

    // (f) the environment summary.
    let path_of = |p: Option<PathBuf>| {
        p.map(|p| p.display().to_string())
            .unwrap_or_else(|| "<unknown>".to_string())
    };
    let env_txt = format!(
        "version: {}\nos: {} arch: {}\npid: {}\nlog: {}\nconfig: {}\ndata dir: {}\nresource dir: {}\nlocale: {}\n",
        app.package_info().version,
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::process::id(),
        state.log.path(),
        state.config_path,
        path_of(app.path().app_local_data_dir().ok()),
        path_of(app.path().resource_dir().ok()),
        i18n::locale(),
    );
    std::fs::write(dir.join("env.txt"), env_txt)
        .map_err(|e| format!("write env.txt: {e}"))?;

    state.log.info(&format!(
        "export_diagnostics -> {} ({} log file(s), {} other artifact(s))",
        dir.display(),
        logs.len(),
        others.len()
    ));
    reveal_directory_in_file_manager(&dir);
    Ok(dir.display().to_string())
}

/// Show (or create) the dedicated WebUI window pointing at the backend on
/// `port`. Shared by the `open_webui` command and the auto-start path, so both
/// a manual click and a normal launch end up in the same window.
pub fn open_webui_window(app: &AppHandle, port: u16) -> Result<String, String> {
    let base = format!("http://127.0.0.1:{port}/");
    let url: tauri::Url = base.parse().map_err(|e| format!("bad url {base}: {e}"))?;
    let win = match app.get_webview_window("webui") {
        Some(win) => {
            win.navigate(url).map_err(|e| format!("navigate WebUI: {e}"))?;
            win
        }
        None => tauri::WebviewWindowBuilder::new(
            app,
            "webui",
            tauri::WebviewUrl::External(url),
        )
        .title("PixivFlow")
        .inner_size(1100.0, 780.0)
        .min_inner_size(720.0, 520.0)
        .center()
        // Host login bridge (window.pixivflowHost) — WebUI window only, never
        // the launcher or the transient login window.
        .initialization_script(HOST_BRIDGE_SCRIPT)
        .build()
        .map_err(|e| format!("open WebUI window: {e}"))?,
    };
    let _ = win.show();
    let _ = win.set_focus();
    Ok(base)
}

/// Record the backing scale factor of both windows in the desktop log. This is
/// the first thing to check when the WebUI text renders blurry: a window built
/// on a Retina display must report 2.0, not 1.0.
pub fn log_scale_factors(app: &AppHandle) {
    let Some(state) = app.try_state::<ManagedState>() else {
        return;
    };
    let of = |label: &str| {
        app.get_webview_window(label)
            .and_then(|w| w.scale_factor().ok())
    };
    state.log.info(&format!(
        "scale factors: launcher={:?} webui={:?}",
        of("main"),
        of("webui")
    ));
}

/// Open the running backend's WebUI — 方案 A: the backend serves the bundled
/// static dist (STATIC_PATH), and the desktop loads it in a dedicated webview
/// window. Kept separate from the control shell so the user keeps both.
#[tauri::command]
pub fn open_webui(
    app: AppHandle,
    state: State<'_, ManagedState>,
) -> Result<String, String> {
    let port = state.manager.lock().unwrap().port();
    if !state.manager.lock().unwrap().is_running() {
        return Err(i18n::t(
            &format!("backend 未运行(port {port})，无法打开 WebUI — 请先启动 backend"),
            &format!(
                "the backend is not running (port {port}), cannot open the WebUI — start the backend first"
            ),
        ));
    }
    let base = open_webui_window(&app, port)?;
    log_scale_factors(&app);
    state.log.info(&format!("opened WebUI at {base}"));
    Ok(base)
}

/// Outcome signalled to the login waiter: the OAuth code, or the news that the
/// sign-in view is gone (cancelled from the app menu), which ends the wait at
/// once.
enum Waiter {
    Code(String),
    Closed,
}

/// Label of the child webview that renders the Pixiv sign-in page *inside* the
/// WebUI window.
const LOGIN_WEBVIEW: &str = "login";

/// End a sign-in that is waiting on screen (the app menu's "取消登录").
///
/// The sign-in view is a child webview: it has no window chrome, so the user
/// cannot close it, and the remote page must not be able to reach the app's IPC.
/// That makes the host — here — the only side that can cancel, which is exactly
/// the control the earlier window-based design got for free from a title bar.
///
/// Returns whether a wait was actually ended.
pub fn cancel_login(app: &AppHandle) -> bool {
    let sender = app
        .try_state::<ManagedState>()
        .and_then(|state| state.login_cancel.lock().ok().and_then(|mut s| s.take()));
    match sender {
        Some(tx) => {
            let _ = tx.send(());
            login_log(app, "INFO", "cancel requested (menu)");
            true
        }
        None => {
            login_log(app, "INFO", "cancel requested with no sign-in in progress");
            false
        }
    }
}

/// Keep the embedded sign-in view exactly as large as the WebUI window: a child
/// webview is positioned by us and does not follow its parent on its own, so
/// resizing the window would otherwise leave the Pixiv page at its old size.
pub fn resize_login_overlay(app: &AppHandle, size: tauri::PhysicalSize<u32>) {
    let Some(view) = app.get_webview(LOGIN_WEBVIEW) else {
        return;
    };
    let scale = app
        .get_window("webui")
        .and_then(|w| w.scale_factor().ok())
        .unwrap_or(1.0);
    let logical = size.to_logical::<f64>(scale);
    let _ = view.set_size(tauri::LogicalSize::new(logical.width, logical.height));
}

/// Close the embedded sign-in view (when one is up) and hand the keyboard back to
/// the WebUI page that was waiting underneath it.
fn remove_login_overlay(app: &AppHandle) {
    if let Some(view) = app.get_webview(LOGIN_WEBVIEW) {
        let to_close = view.clone();
        let _ = view.run_on_main_thread(move || {
            let _ = to_close.close();
        });
    }
    if let Some(win) = app.get_window("webui") {
        let _ = win.set_focus();
    }
}

/// Host login bridge: render the Pixiv authorize page **inside the WebUI window**
/// and return the OAuth `code` it redirected with.
///
/// A separate window (an earlier revision) or the user's own browser both read as
/// "the app sent me somewhere else"; a child webview covering the WebUI keeps the
/// whole flow on the app's own surface, and — because the WebUI page is never
/// unloaded — its pending `openLoginWindow()` promise simply resolves when the
/// overlay goes away. Nothing about the WebUI/backend contract changes.
///
/// `auth_url` / `redirect_uri` come from the WebUI verbatim — the backend owns the
/// OAuth flow (`POST /api/auth/login/host/start`); the desktop only renders the
/// page, watches the navigation and hands back the raw code.
///
/// Resolves `Ok(Some(code))` when the view hits `redirect_uri` (that navigation is
/// blocked so the callback page never loads), `Ok(None)` when the sign-in is
/// cancelled from the app menu or the wait times out. The overlay is always
/// removed before returning.
#[tauri::command]
pub async fn open_login_window(
    app: AppHandle,
    auth_url: String,
    redirect_uri: String,
) -> Result<Option<String>, String> {
    let url: tauri::Url = auth_url
        .parse()
        .map_err(|e| format!("bad auth url {auth_url}: {e}"))?;
    // Validate the redirect target up front so a malformed contract fails fast
    // instead of hanging until the timeout.
    if parse_url(&redirect_uri).is_none() {
        login_log(&app, "WARN", &format!("bad redirect uri {redirect_uri}"));
        return Err(format!("bad redirect uri {redirect_uri}"));
    }
    login_log(
        &app,
        "INFO",
        &format!(
            "bridge invoked (auth {}, redirect {})",
            redacted_url(&auth_url),
            redacted_url(&redirect_uri)
        ),
    );

    let (tx, rx) = mpsc::channel::<Waiter>();
    let tx = std::sync::Mutex::new(tx);
    // Published before the view exists so the menu can end the wait even while the
    // child webview is still being built.
    let (cancel_tx, cancel_rx) = mpsc::channel::<()>();
    if let Some(state) = app.try_state::<ManagedState>() {
        if let Ok(mut slot) = state.login_cancel.lock() {
            *slot = Some(cancel_tx);
        }
    }

    // An async command runs on a worker thread, but a webview may only be created
    // on the main thread (macOS panics otherwise), so hand the build over and wait
    // for its result.
    let (built_tx, built_rx) = mpsc::channel::<Result<(), String>>();
    let main_app = app.clone();
    app.run_on_main_thread(move || {
        // Close-and-recreate: a leftover view from a previous attempt must not be
        // reused (its navigation handler would still feed a stale channel), and
        // Tauri needs the label to be free before the replacement is added.
        if let Some(existing) = main_app.get_webview(LOGIN_WEBVIEW) {
            let _ = existing.close();
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            while main_app.get_webview(LOGIN_WEBVIEW).is_some()
                && std::time::Instant::now() < deadline
            {
                std::thread::sleep(Duration::from_millis(25));
            }
        }
        let Some(parent) = main_app.get_window("webui") else {
            let _ = built_tx.send(Err(
                "the WebUI window is not open, so there is nothing to embed the sign-in page in"
                    .to_string(),
            ));
            return;
        };
        // Cover the whole window at its current size; later resizes are handled by
        // `resize_login_overlay` from the run-event loop.
        let scale = parent.scale_factor().unwrap_or(1.0);
        let size = parent
            .inner_size()
            .map(|s| s.to_logical::<f64>(scale))
            .unwrap_or(tauri::LogicalSize::new(1100.0, 780.0));
        let nav_app = main_app.clone();
        let builder = tauri::WebviewBuilder::new(LOGIN_WEBVIEW, tauri::WebviewUrl::External(url))
            .on_navigation(move |url| {
                // `extract_auth_code` owns the whole decision: it matches the
                // callback (scheme+host+path, or the callback path suffix) and
                // yields the code, or `None` for every other URL.
                match extract_auth_code(url.as_str(), &redirect_uri) {
                    // The callback itself must never load: the code is delivered to
                    // the waiting command and the WebUI comes back to the front.
                    Some(code) => {
                        // The code itself is never logged — only that one arrived.
                        login_log(
                            &nav_app,
                            "INFO",
                            &format!(
                                "callback observed with a code at {} (navigation blocked, code length {})",
                                redacted_url(url.as_str()),
                                code.len()
                            ),
                        );
                        if let Ok(sender) = tx.lock() {
                            let _ = sender.send(Waiter::Code(code));
                        }
                        false
                    }
                    // Every other URL (the authorize page, Pixiv's own pages,
                    // redirects inside the flow) is allowed through.
                    None => true,
                }
            });
        let outcome = parent
            .add_child(
                builder,
                tauri::LogicalPosition::new(0.0, 0.0),
                tauri::LogicalSize::new(size.width, size.height),
            )
            .map(|view| {
                let _ = view.set_focus();
            })
            .map_err(|e| format!("embed sign-in view: {e}"));
        match &outcome {
            Ok(()) => login_log(
                &main_app,
                "INFO",
                &format!(
                    "sign-in view embedded (label login, {}x{} inside window webui)",
                    size.width.round() as i64,
                    size.height.round() as i64
                ),
            ),
            Err(e) => login_log(&main_app, "WARN", &format!("sign-in view failed: {e}")),
        }
        let _ = built_tx.send(outcome);
    })
    .map_err(|e| format!("schedule sign-in view: {e}"))?;
    built_rx
        .recv()
        .map_err(|_| "sign-in view build was not reported".to_string())??;

    // Wait for the navigation, the timeout or a menu cancel, then remove the
    // overlay ourselves before resolving (the host owns its lifetime) and hand the
    // result back to the waiting command.
    let (done_tx, done_rx) = mpsc::channel::<Option<String>>();
    let waiter = app.clone();
    std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + LOGIN_TIMEOUT;
        let mut result: Option<String> = None;
        let mut timed_out = false;
        loop {
            // The cancel signal is polled on the same cadence as the timeout so a
            // menu click breaks out at once.
            let signal = match rx.recv_timeout(Duration::from_millis(150)) {
                Ok(value) => Some(value),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if cancel_rx.try_recv().is_ok() {
                        Some(Waiter::Closed)
                    } else {
                        None
                    }
                }
                // Every sender is gone: the command was cancelled.
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            };
            match signal {
                Some(Waiter::Code(code)) => {
                    result = Some(code);
                    break;
                }
                // Cancelled from the menu: no code will ever arrive, so stop
                // waiting right now.
                Some(Waiter::Closed) => {
                    login_log(&waiter, "INFO", "cancelled from the menu -> None");
                    break;
                }
                None => {
                    if std::time::Instant::now() >= deadline {
                        timed_out = true;
                        break;
                    }
                }
            }
        }
        if timed_out {
            login_log(
                &waiter,
                "WARN",
                &format!(
                    "timeout after {}s with no callback -> None",
                    LOGIN_TIMEOUT.as_secs()
                ),
            );
        }
        // Clear the cancel handle: it belongs to this wait only, and a stale
        // sender would make a later menu click look like a live sign-in.
        if let Some(state) = waiter.try_state::<ManagedState>() {
            if let Ok(mut slot) = state.login_cancel.lock() {
                *slot = None;
            }
        }
        remove_login_overlay(&waiter);
        let _ = done_tx.send(result);
    });

    // The waiter is bounded by LOGIN_TIMEOUT; this outer wait only guards a
    // stalled runtime, so a generous margin is enough.
    let outcome = done_rx
        .recv_timeout(LOGIN_TIMEOUT + Duration::from_secs(15))
        .map_err(|e| {
            login_log(&app, "WARN", &format!("waiter did not report: {e}"));
            format!("sign-in wait failed: {e}")
        })?;
    // Final line for the whole bridge call, so one grep shows how it ended. The
    // code itself is never logged — only its length.
    login_log(
        &app,
        "INFO",
        &match &outcome {
            Some(code) => format!("finished: code received (length {})", code.len()),
            None => "finished: cancelled, no code".to_string(),
        },
    );
    Ok(outcome)
}
