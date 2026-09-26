//! Tauri commands (F1). Rust owns system capability; the frontend only
//! invokes these and renders the resulting status.

use std::sync::mpsc;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, State};

use crate::backend::discovery::{self, DoctorReport, WebuiReport};
use crate::backend::manager;
use crate::backend::{BackendSource, LaunchSpec, StatusSnapshot};
use crate::config::AppConfig;
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
const HOST_BRIDGE_SCRIPT: &str = r#"window.pixivflowHost = {
  openLoginWindow: function (authUrl, redirectUri) {
    return window.__TAURI__.core
      .invoke('open_login_window', { authUrl: authUrl, redirectUri: redirectUri })
      .then(function (code) { return { code: code === undefined ? null : code }; });
  }
};"#;

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
        return Err(format!("backend 未运行(port {port})，无法打开 WebUI — 请先启动 backend"));
    }
    let base = open_webui_window(&app, port)?;
    log_scale_factors(&app);
    state.log.info(&format!("opened WebUI at {base}"));
    Ok(base)
}

/// Outcome signalled to the login waiter: the OAuth code, or the news that the
/// login window is gone (the user closed it), which ends the wait at once.
enum Waiter {
    Code(String),
    Closed,
}

/// Host login bridge: open the Pixiv authorize page in a Tauri app window and
/// return the OAuth `code` it redirected with.
///
/// `auth_url` / `redirect_uri` come from the WebUI verbatim — the backend owns
/// the OAuth flow (`POST /api/auth/login/host/start`); the desktop only opens
/// the window, watches the navigation and hands back the raw code.
///
/// Resolves `Ok(Some(code))` when the login window hits `redirect_uri` (that
/// navigation is blocked so the callback page never loads), `Ok(None)` when the
/// user closes the window or the wait times out. The login window is always
/// closed before returning.
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

    // An async command runs on a worker thread, but a window may only be created
    // on the main thread (macOS panics otherwise), so hand the build over and
    // wait for its result. The builder itself borrows `app`, so it is built
    // inside the closure.
    let (built_tx, built_rx) = mpsc::channel::<Result<(), String>>();
    let (closed_tx, closed_rx) = mpsc::channel::<Waiter>();
    let main_app = app.clone();
    app.run_on_main_thread(move || {
        let nav_app = main_app.clone();
        let builder =
            tauri::WebviewWindowBuilder::new(&main_app, "login", tauri::WebviewUrl::External(url))
                .title("Pixiv 登录")
                .inner_size(560.0, 780.0)
                .resizable(true)
                .center()
                .on_navigation(move |url| {
                    // `extract_auth_code` owns the whole decision: it matches the
                    // callback (scheme+host+path, or the callback path suffix) and
                    // yields the code, or `None` for every other URL.
                    match extract_auth_code(url.as_str(), &redirect_uri) {
                        // The callback itself must never load: the code is
                        // delivered to the waiting command and the URL is
                        // restored on screen.
                        Some(code) => {
                            // The code itself is never logged — only that one
                            // arrived.
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
                        // Every other URL (the authorize page, Pixiv's own
                        // pages, redirects inside the flow) is allowed through.
                        None => true,
                    }
                });
        // Close-and-recreate: a leftover `login` window from a previous attempt
        // must not be reused (its navigation handler would still feed a stale
        // channel), and Tauri needs the label to be free before the replacement
        // is built. Destruction is asynchronous on the event loop.
        if let Some(existing) = main_app.get_webview_window("login") {
            let _ = existing.close();
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            while main_app.get_webview_window("login").is_some()
                && std::time::Instant::now() < deadline
            {
                std::thread::sleep(Duration::from_millis(25));
            }
        }
        let outcome = builder
            .build()
            .map(|_| ())
            .map_err(|e| format!("open login window: {e}"));
        match &outcome {
            Ok(()) => login_log(&main_app, "INFO", "window opened (label login, 560x780)"),
            Err(e) => login_log(&main_app, "WARN", &format!("window build failed: {e}")),
        }
        // The user closing the window must end the wait immediately — without
        // this the caller would sit here for the full timeout. `on_window_event`
        // is scoped to this one window (the runtime registers the listener
        // against its window id), and the handler is dropped with it.
        if outcome.is_ok() {
            if let Some(win) = main_app.get_webview_window("login") {
                let closed_tx = closed_tx.clone();
                win.on_window_event(move |event| {
                    if matches!(event, tauri::WindowEvent::Destroyed) {
                        let _ = closed_tx.send(Waiter::Closed);
                    }
                });
            }
        }
        let _ = built_tx.send(outcome);
    })
    .map_err(|e| format!("schedule login window: {e}"))?;
    built_rx
        .recv()
        .map_err(|_| "login window build was not reported".to_string())??;

    // Wait for the navigation, the timeout or the user closing the window, then
    // close the login window ourselves before resolving (the host owns its
    // lifetime) and hand the result back to the waiting command.
    let (done_tx, done_rx) = mpsc::channel::<Option<String>>();
    let waiter = app.clone();
    std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + LOGIN_TIMEOUT;
        let mut result: Option<String> = None;
        let mut timed_out = false;
        loop {
            // The window-closed signal is polled on the same cadence as the
            // timeout so a user close breaks out at once.
            let signal = match rx.recv_timeout(Duration::from_millis(150)) {
                Ok(value) => Some(value),
                Err(mpsc::RecvTimeoutError::Timeout) => closed_rx.try_recv().ok(),
                // Every sender is gone: the command was cancelled.
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            };
            match signal {
                Some(Waiter::Code(code)) => {
                    result = Some(code);
                    break;
                }
                // The user closed the login window (or it was destroyed): no
                // code will ever arrive, so stop waiting right now.
                Some(Waiter::Closed) => {
                    login_log(&waiter, "INFO", "window closed by the user -> None");
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
        // `close()` also fires for the user's own close, so a single call covers
        // both paths and is idempotent when the window is already gone. It must
        // run on the main thread.
        if let Some(win) = waiter.get_webview_window("login") {
            let to_close = win.clone();
            let _ = win.run_on_main_thread(move || {
                let _ = to_close.close();
            });
        }
        let _ = done_tx.send(result);
    });

    // The waiter is bounded by LOGIN_TIMEOUT; this outer wait only guards a
    // stalled runtime, so a generous margin is enough.
    let outcome = done_rx
        .recv_timeout(LOGIN_TIMEOUT + Duration::from_secs(15))
        .map_err(|e| {
            login_log(&app, "WARN", &format!("waiter did not report: {e}"));
            format!("login window wait failed: {e}")
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
